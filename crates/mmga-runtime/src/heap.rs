//! Object Arena Heap & Write-Barrier Generational Garbage Collector (`Rt`).
//!
//! Performance characteristics:
//! - Slot `0` is permanently `null` (`None`), so `V::R(0)` never dereferences.
//! - Interned string table (`FxHashMap<String, u32>`) avoids duplicate string allocations.
//! - Write-barrier generational collection (`enter_young_epoch` / `collect_young`):
//!   During a server tick or event dispatch, newly allocated temporary objects (iterators,
//!   `BlockPos`, event instances, boxed numbers, lambdas) get `born >= watermark`.
//!   Any mutation of an older object (`id < watermark`) records `id` in `dirty_old`.
//!   At the end of the tick, `collect_young` traces *only* from `statics`, `pinned`,
//!   `dirty_old`, and explicit stack roots, promoting surviving young objects (`born = 0`)
//!   and reclaiming all ephemeral garbage in microseconds (truncating the arena tail when
//!   possible) with zero stop-the-world full-heap scan!

use crate::fxhash::{FxHashMap, FxHashSet};
use crate::value::V;
use mmga_classfile::MemberRef;

/// Target of a Java lambda / method reference created by `LambdaMetafactory`.
#[derive(Debug, Clone)]
pub enum ClosureTarget {
    /// Method handle to a bytecode or host method (`handle_kind`: JVMS `REF_*` 5=virtual, 6=static, 7=special, 8=newInvokeSpecial, 9=interface).
    MethodHandle { handle_kind: u8, member: MemberRef },
    /// Pre-compiled AOT native lambda function ID registered in `LinkedSpace`.
    NativeLambda(u32),
}

/// Captured state and functional interface metadata for a Java lambda object.
#[derive(Debug, Clone)]
pub struct ClosureData {
    /// Functional interface internal name, e.g. `"java/util/function/Consumer"`, `"java/util/function/Supplier"`.
    pub interface_class: String,
    /// SAM method name, e.g. `"accept"`, `"get"`, `"apply"`, `"run"`, `"test"`.
    pub sam_name: String,
    /// SAM erased descriptor, e.g. `"(Ljava/lang/Object;)V"`.
    pub sam_desc: String,
    /// Instantiated generic method type from `LambdaMetafactory` arg #2, e.g. `"(Lnet/neoforged/fml/event/lifecycle/FMLCommonSetupEvent;)V"`.
    pub instantiated_desc: String,
    pub captures: Vec<V>,
    pub target: ClosureTarget,
}

/// Specialized storage variants inside a heap `Object`.
#[derive(Debug, Clone)]
pub enum Data {
    /// Java class instance fields (`field_name -> V`).
    Fields(FxHashMap<String, V>),
    /// Immutable `java.lang.String`.
    Str(String),
    /// Mutable `java.lang.StringBuilder`.
    StringBuilder(String),
    /// Fixed-length Java array (`T[]` or primitive array).
    Array(Vec<V>),
    /// `java.util.ArrayList` / `ArrayDeque`.
    List(Vec<V>),
    /// Insertion-ordered `java.util.HashMap` / `LinkedHashMap` / `ConcurrentHashMap`.
    Map(Vec<(V, V)>),
    /// Insertion-ordered `java.util.HashSet` / `LinkedHashSet`.
    Set(Vec<V>),
    /// `java.util.Optional<T>`.
    Optional(Option<V>),
    /// `java.util.stream.Stream<T>`.
    Stream(Vec<V>),
    /// `java.util.Iterator<T>`.
    Iter { items: Vec<V>, pos: usize },
    /// Netty `ByteBuf` / Minecraft `FriendlyByteBuf`.
    ByteBuf { bytes: Vec<u8>, reader_idx: usize },
    /// `invokedynamic` lambda or method reference.
    Closure(Box<ClosureData>),
    /// SpongePowered Mixin `CallbackInfo` / `CallbackInfoReturnable`.
    CallbackInfo {
        name: String,
        cancellable: bool,
        cancelled: bool,
        return_value: Option<V>,
    },
    /// Zero-overhead handle to a native Rust engine/NeoForge resource.
    NativeHandle {
        kind: &'static str,
        id: u64,
        tag: String,
        payload: [i64; 2],
    },
}

/// Heap-allocated object slot.
#[derive(Debug, Clone)]
pub struct Object {
    pub class: String,
    /// Optional compact class ID in `LinkedSpace` (`0` for unlinked/API classes).
    pub cid: u32,
    /// Allocation generation (`0` = tenured/old, `> 0` = young allocation index).
    pub born: u32,
    pub data: Data,
}

/// Call-site and execution telemetry recorded by `Rt`.
#[derive(Debug, Clone, Default)]
pub struct RuntimeTelemetry {
    pub total_calls: u64,
    pub host_calls: u64,
    pub unmapped_calls: FxHashMap<String, u64>,
    pub young_gc_runs: u64,
    pub young_objects_reclaimed: u64,
    pub full_gc_runs: u64,
    pub full_objects_reclaimed: u64,
    pub logs: Vec<String>,
}

impl RuntimeTelemetry {
    pub fn unknown_call_count(&self) -> u64 {
        self.unmapped_calls.values().sum()
    }
}

/// Core runtime state owning the object heap, interned strings, static fields,
/// initialized-class set, active exception state, and write-barrier generational GC.
pub struct Rt {
    slots: Vec<Option<Object>>,
    free_list: Vec<u32>,
    alloc_counter: u32,
    dirty_old: Vec<u32>,
    dirty_old_set: FxHashSet<u32>,
    interned_strings: FxHashMap<String, u32>,
    /// Static fields `"owner.fieldName" -> V`.
    pub statics: FxHashMap<String, V>,
    /// Classes whose `<clinit>` has begun or finished.
    pub initialized_classes: FxHashSet<String>,
    /// Long-lived pinned heap handles (registered objects, event listeners, mod instances, enum constants).
    pub pinned: FxHashSet<u32>,
    /// Active Java call frames (`"ownerClass.methodName"`), outermost first.
    pub call_stack: Vec<(String, String)>,
    /// Pending uncaught/in-flight Java exception reference (`V::R(id)`), cleared when caught.
    pub pending_exception: Option<u32>,
    pub telemetry: RuntimeTelemetry,
}

impl Default for Rt {
    fn default() -> Self {
        Self::new()
    }
}

impl Rt {
    pub fn new() -> Self {
        Self {
            slots: vec![None], // slot 0 is permanently null
            free_list: Vec::new(),
            alloc_counter: 1,
            dirty_old: Vec::new(),
            dirty_old_set: FxHashSet::default(),
            interned_strings: FxHashMap::default(),
            statics: FxHashMap::default(),
            initialized_classes: FxHashSet::default(),
            pinned: FxHashSet::default(),
            call_stack: Vec::new(),
            pending_exception: None,
            telemetry: RuntimeTelemetry::default(),
        }
    }

    /// Number of live objects currently on the heap.
    pub fn live_object_count(&self) -> usize {
        self.slots.iter().skip(1).filter(|s| s.is_some()).count()
    }

    /// Allocates a heap object and returns its `V::R(id)`.
    pub fn alloc(&mut self, class: impl Into<String>, cid: u32, data: Data) -> V {
        let born = self.alloc_counter;
        self.alloc_counter = self.alloc_counter.saturating_add(1).max(1);
        let obj = Object {
            class: class.into(),
            cid,
            born,
            data,
        };
        let id = if let Some(reused) = self.free_list.pop() {
            self.slots[reused as usize] = Some(obj);
            reused
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(Some(obj));
            idx
        };
        V::R(id)
    }

    /// Allocates a tenured (`born = 0`) pinned object that is never collected until unpinned.
    pub fn alloc_pinned(&mut self, class: impl Into<String>, cid: u32, data: Data) -> V {
        let v = self.alloc(class, cid, data);
        let id = v.r();
        if let Some(Some(obj)) = self.slots.get_mut(id as usize) {
            obj.born = 0;
        }
        self.pinned.insert(id);
        v
    }

    /// Pins an existing reference so GC treats it as a root.
    #[inline]
    pub fn pin(&mut self, v: V) {
        let id = v.r();
        if id != 0 {
            self.pinned.insert(id);
        }
    }

    /// Allocates a field-backed instance of `class`.
    pub fn new_instance(&mut self, class: impl Into<String>, cid: u32) -> V {
        self.alloc(class, cid, Data::Fields(FxHashMap::default()))
    }

    /// Allocates or returns an interned `java.lang.String` (pinned so string literals are O(1) forever).
    pub fn intern_str(&mut self, s: &str) -> V {
        if let Some(&id) = self.interned_strings.get(s) {
            if self.slots.get(id as usize).and_then(Option::as_ref).is_some() {
                return V::R(id);
            }
        }
        let v = self.alloc_pinned("java/lang/String", 0, Data::Str(s.to_owned()));
        self.interned_strings.insert(s.to_owned(), v.r());
        v
    }

    /// Allocates a non-interned transient `java.lang.String` (collectible by young GC!).
    pub fn new_str(&mut self, s: impl Into<String>) -> V {
        self.alloc("java/lang/String", 0, Data::Str(s.into()))
    }

    /// Allocates a native engine handle object.
    pub fn new_handle(
        &mut self,
        class: impl Into<String>,
        kind: &'static str,
        id: u64,
        tag: impl Into<String>,
    ) -> V {
        self.alloc(
            class,
            0,
            Data::NativeHandle {
                kind,
                id,
                tag: tag.into(),
                payload: [0, 0],
            },
        )
    }

    /// Allocates and pins a singleton handle (e.g., `NeoForge.EVENT_BUS`, `Registries.BLOCK`).
    pub fn new_pinned_handle(
        &mut self,
        class: impl Into<String>,
        kind: &'static str,
        id: u64,
        tag: impl Into<String>,
    ) -> V {
        self.alloc_pinned(
            class,
            0,
            Data::NativeHandle {
                kind,
                id,
                tag: tag.into(),
                payload: [0, 0],
            },
        )
    }

    #[inline]
    pub fn get_obj(&self, v: V) -> Option<&Object> {
        let id = v.r() as usize;
        if id == 0 {
            None
        } else {
            self.slots.get(id).and_then(Option::as_ref)
        }
    }

    #[inline]
    pub fn get_obj_mut(&mut self, v: V) -> Option<&mut Object> {
        let id = v.r();
        if id == 0 {
            return None;
        }
        self.record_write_barrier(id);
        self.slots.get_mut(id as usize).and_then(Option::as_mut)
    }

    #[inline]
    fn record_write_barrier(&mut self, obj_id: u32) {
        if let Some(Some(obj)) = self.slots.get(obj_id as usize) {
            if obj.born == 0 && self.dirty_old_set.insert(obj_id) {
                self.dirty_old.push(obj_id);
            }
        }
    }

    pub fn class_of(&self, v: V) -> Option<&str> {
        self.get_obj(v).map(|o| o.class.as_str())
    }

    pub fn str_of(&self, v: V) -> Option<&str> {
        match &self.get_obj(v)?.data {
            Data::Str(s) | Data::StringBuilder(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn handle_of(&self, v: V) -> Option<(&'static str, u64, &str, [i64; 2])> {
        match &self.get_obj(v)?.data {
            Data::NativeHandle {
                kind,
                id,
                tag,
                payload,
            } => Some((kind, *id, tag.as_str(), *payload)),
            _ => None,
        }
    }

    pub fn get_field(&self, receiver: V, field: &str) -> V {
        let Some(obj) = self.get_obj(receiver) else {
            return V::NULL;
        };
        match &obj.data {
            Data::Fields(map) => map.get(field).copied().unwrap_or(V::NULL),
            _ => V::NULL,
        }
    }

    pub fn put_field(&mut self, receiver: V, field: impl Into<String>, val: V) {
        let id = receiver.r();
        if id == 0 {
            return;
        }
        self.record_write_barrier(id);
        if let Some(Some(obj)) = self.slots.get_mut(id as usize) {
            if let Data::Fields(map) = &mut obj.data {
                map.insert(field.into(), val);
            }
        }
    }

    pub fn get_static(&self, owner: &str, name: &str) -> Option<V> {
        self.statics.get(&format!("{owner}.{name}")).copied()
    }

    pub fn put_static(&mut self, owner: &str, name: &str, val: V) {
        self.statics.insert(format!("{owner}.{name}"), val);
    }

    /// Raises a Java exception object on this runtime thread.
    pub fn throw_exception(&mut self, exception_class: &str, message: &str) -> V {
        let trace_suffix = self
            .call_stack
            .last()
            .map(|(cls, m)| format!(" (at {}.{})", cls.replace('/', "."), m))
            .unwrap_or_default();
        let full_msg = if message.contains(" (at ") {
            message.to_owned()
        } else {
            format!("{message}{trace_suffix}")
        };
        let msg_v = self.new_str(full_msg);
        let exc = self.new_instance(exception_class, 0);
        self.put_field(exc, "detailMessage", msg_v);
        self.pending_exception = Some(exc.r());
        V::NULL
    }

    pub fn take_exception(&mut self) -> Option<V> {
        self.pending_exception.take().map(V::R)
    }

    pub fn exception_summary(&self, exc: V) -> String {
        let cls = self.class_of(exc).unwrap_or("java/lang/Throwable");
        let msg = self
            .str_of(self.get_field(exc, "detailMessage"))
            .unwrap_or("");
        if msg.is_empty() {
            cls.replace('/', ".")
        } else {
            format!("{}: {msg}", cls.replace('/', "."))
        }
    }

    /// Value equality matching Java `Objects.equals` for primitives, strings, handles, and boxed values.
    pub fn val_eq(&self, a: V, b: V) -> bool {
        if a == b {
            return true;
        }
        let (Some(oa), Some(ob)) = (self.get_obj(a), self.get_obj(b)) else {
            return false;
        };
        match (&oa.data, &ob.data) {
            (Data::Str(sa), Data::Str(sb)) => sa == sb,
            (
                Data::NativeHandle {
                    kind: ka,
                    id: ia,
                    tag: ta,
                    ..
                },
                Data::NativeHandle {
                    kind: kb,
                    id: ib,
                    tag: tb,
                    ..
                },
            ) => ka == kb && ia == ib && ta == tb,
            (Data::Optional(xa), Data::Optional(xb)) => match (xa, xb) {
                (None, None) => true,
                (Some(va), Some(vb)) => self.val_eq(*va, *vb),
                _ => false,
            },
            (Data::Fields(fa), Data::Fields(fb)) if oa.class == ob.class => {
                // Boxed primitives (`java/lang/Integer`, etc.) store `$value`
                if let (Some(&va), Some(&vb)) = (fa.get("$value"), fb.get("$value")) {
                    return va == vb;
                }
                false
            }
            _ => false,
        }
    }

    /// Formats a runtime value as Java `String.valueOf(v)`.
    pub fn value_to_string(&self, v: V) -> String {
        match v {
            V::Void => "void".into(),
            V::I(i) => i.to_string(),
            V::J(j) => j.to_string(),
            V::F(f) => format_java_float(f),
            V::D(d) => format_java_double(d),
            V::R(0) => "null".into(),
            V::R(_) => {
                let Some(obj) = self.get_obj(v) else {
                    return "null".into();
                };
                match &obj.data {
                    Data::Str(s) | Data::StringBuilder(s) => s.clone(),
                    Data::Optional(None) => "Optional.empty".into(),
                    Data::Optional(Some(inner)) => {
                        format!("Optional[{}]", self.value_to_string(*inner))
                    }
                    Data::List(items) | Data::Array(items) => {
                        let parts: Vec<String> =
                            items.iter().map(|x| self.value_to_string(*x)).collect();
                        format!("[{}]", parts.join(", "))
                    }
                    Data::Set(items) => {
                        let parts: Vec<String> =
                            items.iter().map(|x| self.value_to_string(*x)).collect();
                        format!("[{}]", parts.join(", "))
                    }
                    Data::Map(entries) => {
                        let parts: Vec<String> = entries
                            .iter()
                            .map(|(k, val)| {
                                format!(
                                    "{}={}",
                                    self.value_to_string(*k),
                                    self.value_to_string(*val)
                                )
                            })
                            .collect();
                        format!("{{{}}}", parts.join(", "))
                    }
                    Data::NativeHandle { kind, tag, id, .. } => {
                        if !tag.is_empty() {
                            tag.clone()
                        } else {
                            format!("{kind}#{id}")
                        }
                    }
                    Data::Fields(f) => {
                        if let Some(&boxed) = f.get("$value") {
                            if obj.class == "java/lang/Boolean" {
                                return if boxed.i() != 0 {
                                    "true".into()
                                } else {
                                    "false".into()
                                };
                            }
                            return self.value_to_string(boxed);
                        }
                        if let Some(name) = f.get("$name").and_then(|n| self.str_of(*n)) {
                            return name.to_owned();
                        }
                        format!("{}@{:x}", obj.class.replace('/', "."), v.r())
                    }
                    _ => format!("{}@{:x}", obj.class.replace('/', "."), v.r()),
                }
            }
        }
    }

    // =========================================================================
    // Write-Barrier Generational Garbage Collector
    // =========================================================================

    /// Captures the current allocation watermark before a tick or event dispatch.
    #[inline]
    pub fn enter_young_epoch(&self) -> u32 {
        self.alloc_counter
    }

    /// Collects unreachable young objects (`born >= watermark`) in O(young_live),
    /// promoting any reachable young objects to tenured (`born = 0`).
    pub fn collect_young(&mut self, watermark: u32, extra_roots: &[V]) -> usize {
        self.telemetry.young_gc_runs += 1;
        let mut worklist: Vec<u32> = Vec::new();

        for &v in self.statics.values() {
            if v.r() >= 1 {
                if let Some(Some(o)) = self.slots.get(v.r() as usize) {
                    if o.born >= watermark {
                        worklist.push(v.r());
                    }
                }
            }
        }
        for &id in &self.pinned {
            if let Some(Some(o)) = self.slots.get(id as usize) {
                if o.born >= watermark {
                    worklist.push(id);
                }
            }
        }
        if let Some(exc) = self.pending_exception {
            worklist.push(exc);
        }
        for &v in extra_roots {
            if v.r() != 0 {
                worklist.push(v.r());
            }
        }

        // Scan dirty tenured objects recorded by the write barrier
        let dirty = std::mem::take(&mut self.dirty_old);
        self.dirty_old_set.clear();
        for old_id in dirty {
            if let Some(Some(old_obj)) = self.slots.get(old_id as usize) {
                for_each_child_ref(&old_obj.data, |child_id| {
                    if let Some(Some(co)) = self.slots.get(child_id as usize) {
                        if co.born >= watermark {
                            worklist.push(child_id);
                        }
                    }
                });
            }
        }

        // Transitively promote reachable young objects (`born = 0`)
        while let Some(id) = worklist.pop() {
            let mut children = Vec::new();
            if let Some(Some(obj)) = self.slots.get_mut(id as usize) {
                if obj.born < watermark {
                    continue;
                }
                obj.born = 0;
                for_each_child_ref(&obj.data, |c| children.push(c));
            }
            for c in children {
                if let Some(Some(co)) = self.slots.get(c as usize) {
                    if co.born >= watermark {
                        worklist.push(c);
                    }
                }
            }
        }

        // Sweep unpromoted young objects
        let mut reclaimed = 0usize;
        for idx in 1..self.slots.len() {
            let is_dead_young = self
                .slots
                .get(idx)
                .and_then(Option::as_ref)
                .is_some_and(|o| o.born >= watermark);
            if is_dead_young {
                self.slots[idx] = None;
                reclaimed += 1;
            }
        }

        // Rebuild compact free_list & truncate trailing `None`s so arena shrinks cleanly!
        while self.slots.len() > 1 && self.slots.last().unwrap().is_none() {
            self.slots.pop();
        }
        self.free_list.clear();
        for idx in (1..self.slots.len()).rev() {
            if self.slots[idx].is_none() {
                self.free_list.push(idx as u32);
            }
        }

        self.telemetry.young_objects_reclaimed += reclaimed as u64;
        reclaimed
    }

    /// Performs a full mark-and-sweep collection over the entire heap.
    pub fn collect_full(&mut self, extra_roots: &[V]) -> usize {
        self.telemetry.full_gc_runs += 1;
        let mut marked = vec![false; self.slots.len()];
        let mut worklist = Vec::new();

        for &v in self.statics.values() {
            if v.r() != 0 {
                worklist.push(v.r());
            }
        }
        for &id in &self.pinned {
            worklist.push(id);
        }
        if let Some(exc) = self.pending_exception {
            worklist.push(exc);
        }
        for &v in extra_roots {
            if v.r() != 0 {
                worklist.push(v.r());
            }
        }

        while let Some(id) = worklist.pop() {
            let idx = id as usize;
            if idx == 0 || idx >= marked.len() || marked[idx] {
                continue;
            }
            marked[idx] = true;
            if let Some(Some(obj)) = self.slots.get_mut(idx) {
                obj.born = 0;
                for_each_child_ref(&obj.data, |c| worklist.push(c));
            }
        }

        self.dirty_old.clear();
        self.dirty_old_set.clear();
        self.interned_strings
            .retain(|_, &mut id| (id as usize) < marked.len() && marked[id as usize]);

        let mut reclaimed = 0usize;
        for idx in 1..self.slots.len() {
            if !marked[idx] && self.slots[idx].is_some() {
                self.slots[idx] = None;
                reclaimed += 1;
            }
        }
        while self.slots.len() > 1 && self.slots.last().unwrap().is_none() {
            self.slots.pop();
        }
        self.free_list.clear();
        for idx in (1..self.slots.len()).rev() {
            if self.slots[idx].is_none() {
                self.free_list.push(idx as u32);
            }
        }
        self.telemetry.full_objects_reclaimed += reclaimed as u64;
        reclaimed
    }
}

fn for_each_child_ref(data: &Data, mut f: impl FnMut(u32)) {
    let mut visit = |v: V| {
        if v.r() != 0 {
            f(v.r());
        }
    };
    match data {
        Data::Fields(m) => {
            for &v in m.values() {
                visit(v);
            }
        }
        Data::Array(xs) | Data::List(xs) | Data::Set(xs) | Data::Stream(xs) => {
            for &v in xs {
                visit(v);
            }
        }
        Data::Iter { items, .. } => {
            for &v in items {
                visit(v);
            }
        }
        Data::Map(entries) => {
            for &(k, v) in entries {
                visit(k);
                visit(v);
            }
        }
        Data::Optional(Some(v)) => visit(*v),
        Data::Closure(c) => {
            for &v in &c.captures {
                visit(v);
            }
        }
        Data::CallbackInfo {
            return_value: Some(v),
            ..
        } => visit(*v),
        Data::Str(_)
        | Data::StringBuilder(_)
        | Data::Optional(None)
        | Data::ByteBuf { .. }
        | Data::CallbackInfo {
            return_value: None, ..
        }
        | Data::NativeHandle { .. } => {}
    }
}

fn format_java_float(f: f32) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f == f32::INFINITY {
        "Infinity".into()
    } else if f == f32::NEG_INFINITY {
        "-Infinity".into()
    } else if f.fract() == 0.0 {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

fn format_java_double(d: f64) -> String {
    if d.is_nan() {
        "NaN".into()
    } else if d == f64::INFINITY {
        "Infinity".into()
    } else if d == f64::NEG_INFINITY {
        "-Infinity".into()
    } else if d.fract() == 0.0 {
        format!("{d:.1}")
    } else {
        d.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_barrier_generational_gc_promotes_escaped_and_frees_ephemeral() {
        let mut rt = Rt::new();
        let tenured_holder = rt.alloc_pinned("pkg/Holder", 1, Data::Fields(FxHashMap::default()));

        let mark = rt.enter_young_epoch();
        let ephemeral_1 = rt.new_str("ephemeral_1");
        let ephemeral_2 = rt.new_str("ephemeral_2");
        let escaped = rt.new_str("escaped_into_tenured");
        // Mutating `tenured_holder` triggers the write barrier!
        rt.put_field(tenured_holder, "kept", escaped);
        let _ = (ephemeral_1, ephemeral_2);

        assert_eq!(rt.live_object_count(), 4);
        let reclaimed = rt.collect_young(mark, &[]);
        assert_eq!(reclaimed, 2);
        assert_eq!(rt.live_object_count(), 2);
        assert_eq!(
            rt.str_of(rt.get_field(tenured_holder, "kept")),
            Some("escaped_into_tenured")
        );
    }
}

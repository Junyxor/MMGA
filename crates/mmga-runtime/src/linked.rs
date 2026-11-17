//! Runtime-linked Class Space (`LinkedSpace`), inline-cached virtual/interface/Mixin
//! method resolution, and the `HostApi` boundary trait.

use crate::fxhash::{fnv1a_64, FxHashMap};
use crate::heap::Rt;
use crate::value::V;
use mmga_classfile::{
    ClassFile, MemberRef, MethodInfo, MixinWeaveReport, SyntheticBridge, TargetWeavePlan,
};
use std::collections::{BTreeMap, VecDeque};

/// Compiled native Rust method signature emitted by `mmga-aot`.
/// For instance methods & constructors, `args[0]` is `this` (`V::R(id)`).
pub type NativeMethodFn = fn(&mut ExecCtx<'_>, &[V]) -> V;

/// Compiled native Rust lambda body emitted by `mmga-aot`.
/// `args` contains captured values followed by SAM call arguments.
pub type NativeLambdaFn = fn(&mut ExecCtx<'_>, &[V]) -> V;

/// Static registration descriptor for one AOT-compiled Java class.
#[derive(Clone, Copy)]
pub struct NativeClassDef {
    pub name: &'static str,
    pub superclass: Option<&'static str>,
    pub interfaces: &'static [&'static str],
    pub is_interface: bool,
    /// `(member_key "name.desc", fn_ptr, is_static, is_private)`
    pub methods: &'static [(&'static str, NativeMethodFn, bool, bool)],
}

/// Static registration bundle emitted for an AOT-translated crate/module.
#[derive(Clone, Copy)]
pub struct NativeModuleRegistry {
    pub classes: &'static [NativeClassDef],
    pub lambdas: &'static [(u32, NativeLambdaFn)],
}

/// External host boundary (implemented by `mmga-neoforge` / `mmga-horizon`).
pub trait HostApi {
    /// Resolves a static field read (`GETSTATIC`) on an external/API class.
    fn get_static_field(&mut self, rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V>;

    /// Resolves a static field write (`PUTSTATIC`) on an external/API class.
    fn put_static_field(
        &mut self,
        rt: &mut Rt,
        owner: &str,
        name: &str,
        desc: &str,
        val: V,
    ) -> bool {
        let _ = (rt, owner, name, desc, val);
        false
    }

    /// Resolves an instance field read (`GETFIELD`) on a host-owned object/handle.
    fn get_instance_field(
        &mut self,
        rt: &mut Rt,
        receiver: V,
        owner: &str,
        name: &str,
        desc: &str,
    ) -> Option<V> {
        let _ = (rt, receiver, owner, name, desc);
        None
    }

    /// Invokes a method on an external/API class (NeoForge, Minecraft, etc.).
    /// `args` includes `receiver` at `args[0]` when `is_static == false`.
    /// If the host wants to re-enter user Java code (e.g. firing an `IEventBus` listener or
    /// `DeferredRegister` supplier), it can use `Invoker` callbacks via `call_host_method`.
    fn call_method(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        member: &MemberRef,
        is_static: bool,
        args: &[V],
    ) -> Option<V>;

    /// Checks whether `sub_class` is assignable to `super_class` according to the host's
    /// API hierarchy (e.g. `ServerPlayer` -> `Player` -> `LivingEntity` -> `Entity`).
    fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
        let _ = (sub_class, super_class);
        false
    }
}

/// No-op host for pure JDK / standalone bytecode tests.
#[derive(Default)]
pub struct NullHost;

impl HostApi for NullHost {
    fn get_static_field(
        &mut self,
        _rt: &mut Rt,
        _owner: &str,
        _name: &str,
        _desc: &str,
    ) -> Option<V> {
        None
    }

    fn call_method(
        &mut self,
        _rt: &mut Rt,
        _linked: &LinkedSpace,
        _member: &MemberRef,
        _is_static: bool,
        _args: &[V],
    ) -> Option<V> {
        None
    }
}

/// Resolved callable target in `LinkedSpace`.
#[derive(Clone)]
pub enum ResolvedMethod {
    Native {
        owner: String,
        func: NativeMethodFn,
    },
    Bytecode {
        /// Class whose constant pool owns `method.code` (for Mixin merged methods, this is `mixin_class`!).
        pool_owner: String,
        /// Target class on which the method is declared/woven.
        declaring_class: String,
        method: MethodInfo,
    },
    Bridge {
        declaring_class: String,
        bridge: SyntheticBridge,
    },
}

/// Unified linked class space holding bytecode classes, woven Mixin plans, and AOT native classes.
#[derive(Default, Clone)]
pub struct LinkedSpace {
    pub bytecode_classes: BTreeMap<String, ClassFile>,
    pub mixin_report: MixinWeaveReport,
    /// `META-INF/services/<spi_internal_name>` -> implementation internal class names.
    pub services: BTreeMap<String, Vec<String>>,
    class_ids: FxHashMap<String, u32>,
    id_to_class: Vec<String>,
    superclasses: FxHashMap<String, String>,
    interfaces: FxHashMap<String, Vec<String>>,
    native_methods: FxHashMap<String, FxHashMap<u64, Vec<(&'static str, NativeMethodFn, bool, bool)>>>,
    native_lambdas: FxHashMap<u32, NativeLambdaFn>,
}

impl LinkedSpace {
    pub fn new() -> Self {
        Self {
            id_to_class: vec![String::new()], // cid 0 = unlinked
            ..Default::default()
        }
    }

    fn intern_cid(&mut self, class_name: &str) -> u32 {
        if let Some(&cid) = self.class_ids.get(class_name) {
            return cid;
        }
        let cid = self.id_to_class.len() as u32;
        self.id_to_class.push(class_name.to_owned());
        self.class_ids.insert(class_name.to_owned(), cid);
        cid
    }

    pub fn cid_of(&self, class_name: &str) -> u32 {
        self.class_ids.get(class_name).copied().unwrap_or(0)
    }

    pub fn class_name_of_cid(&self, cid: u32) -> Option<&str> {
        if cid == 0 {
            None
        } else {
            self.id_to_class.get(cid as usize).map(String::as_str)
        }
    }

    /// Loads bytecode classes and runs the Mixin planner + structural weaver over them.
    pub fn add_bytecode_classes(&mut self, mut classes: BTreeMap<String, ClassFile>) {
        let mixin_refs: Vec<&ClassFile> = classes.values().filter(|c| c.is_mixin()).collect();
        let report = mmga_classfile::plan_mixins(&mixin_refs, &classes);
        mmga_classfile::apply_mixin_structural_weave(&mut classes, &report);

        for (name, cf) in &classes {
            self.intern_cid(name);
            if let Some(sup) = &cf.superclass {
                self.superclasses.insert(name.clone(), sup.clone());
            }
            self.interfaces.insert(name.clone(), cf.interfaces.clone());
        }

        for (t, plan) in report.targets {
            self.mixin_report.targets.insert(t, plan);
        }
        for ext in report.external_targets {
            if !self.mixin_report.external_targets.contains(&ext) {
                self.mixin_report.external_targets.push(ext);
            }
        }
        self.mixin_report.unsupported.extend(report.unsupported);

        for (name, cf) in classes {
            self.bytecode_classes.insert(name, cf);
        }
    }

    /// Registers an AOT-compiled native Rust module into this class space.
    pub fn register_native_module(&mut self, reg: NativeModuleRegistry) {
        for &(lid, lfn) in reg.lambdas {
            self.native_lambdas.insert(lid, lfn);
        }
        for cdef in reg.classes {
            self.intern_cid(cdef.name);
            if let Some(sup) = cdef.superclass {
                self.superclasses
                    .insert(cdef.name.to_owned(), sup.to_owned());
            }
            self.interfaces.insert(
                cdef.name.to_owned(),
                cdef.interfaces.iter().map(|s| (*s).to_owned()).collect(),
            );
            let table = self
                .native_methods
                .entry(cdef.name.to_owned())
                .or_default();
            for &(mkey, func, is_static, is_private) in cdef.methods {
                let h = fnv1a_64(mkey);
                table
                    .entry(h)
                    .or_default()
                    .push((mkey, func, is_static, is_private));
            }
        }
    }

    pub fn native_lambda(&self, id: u32) -> Option<NativeLambdaFn> {
        self.native_lambdas.get(&id).copied()
    }

    pub fn weave_plan(&self, class_name: &str) -> Option<&TargetWeavePlan> {
        self.mixin_report.targets.get(class_name)
    }

    pub fn has_class(&self, class_name: &str) -> bool {
        self.class_ids.contains_key(class_name)
    }

    pub fn superclass_of<'a>(&'a self, class_name: &str) -> Option<&'a str> {
        self.superclasses.get(class_name).map(String::as_str)
    }

    /// Resolves the declaring bytecode class of a field per JVMS §5.4.3.2
    /// (searching `start_class`, direct/transitive superinterfaces, and superclasses).
    pub fn resolve_field_owner(
        &self,
        start_class: &str,
        field_name: &str,
        is_static: bool,
    ) -> Option<String> {
        let mut visited = Vec::new();
        self.resolve_field_owner_inner(start_class, field_name, is_static, &mut visited)
    }

    fn resolve_field_owner_inner(
        &self,
        cls: &str,
        field_name: &str,
        is_static: bool,
        visited: &mut Vec<String>,
    ) -> Option<String> {
        if visited.iter().any(|v| v == cls) {
            return None;
        }
        visited.push(cls.to_owned());

        if let Some(cf) = self.bytecode_classes.get(cls) {
            if cf
                .fields
                .iter()
                .any(|f| f.name == field_name && f.is_static() == is_static)
            {
                return Some(cls.to_owned());
            }
        }
        if let Some(ifaces) = self.interfaces.get(cls) {
            for iface in ifaces {
                if let Some(found) =
                    self.resolve_field_owner_inner(iface, field_name, is_static, visited)
                {
                    return Some(found);
                }
            }
        }
        if let Some(sup) = self.superclasses.get(cls) {
            if let Some(found) = self.resolve_field_owner_inner(sup, field_name, is_static, visited)
            {
                return Some(found);
            }
        }
        None
    }

    /// Checks whether `sub` is equal to or a subtype of `sup` across loaded classes, interfaces, and host APIs.
    pub fn is_subtype(&self, sub: &str, sup: &str, host: &dyn HostApi) -> bool {
        if sub == sup || sup == "java/lang/Object" {
            return true;
        }
        if host.is_api_subtype(sub, sup) {
            return true;
        }
        let mut queue = VecDeque::new();
        let mut visited = Vec::new();
        queue.push_back(sub.to_owned());
        while let Some(cur) = queue.pop_front() {
            if visited.contains(&cur) {
                continue;
            }
            visited.push(cur.clone());
            if cur == sup || host.is_api_subtype(&cur, sup) {
                return true;
            }
            if let Some(parent) = self.superclasses.get(&cur) {
                queue.push_back(parent.clone());
            }
            if let Some(ifaces) = self.interfaces.get(&cur) {
                for iface in ifaces {
                    queue.push_back(iface.clone());
                }
            }
            for &jdk_sup in jdk_supertypes(&cur) {
                queue.push_back(jdk_sup.to_owned());
            }
        }
        false
    }

    /// Resolves a direct member on `class_name` (without walking superclasses).
    pub fn find_declared_method(
        &self,
        class_name: &str,
        name: &str,
        desc: &str,
    ) -> Option<ResolvedMethod> {
        let mkey = format!("{name}.{desc}");
        let hash = fnv1a_64(&mkey);

        // 1. AOT Native method table (already includes woven Mixin methods when AOT-compiled!)
        if let Some(table) = self.native_methods.get(class_name) {
            if let Some(bucket) = table.get(&hash) {
                for &(k, func, _, _) in bucket {
                    if k == mkey {
                        return Some(ResolvedMethod::Native {
                            owner: class_name.to_owned(),
                            func,
                        });
                    }
                }
            }
        }

        // 2. Mixin synthetic bridges (`@Accessor` / `@Invoker`) and merged/overwritten methods
        if let Some(plan) = self.mixin_report.targets.get(class_name) {
            for bridge in &plan.bridges {
                let (b_name, b_desc) = match bridge {
                    SyntheticBridge::FieldGetter {
                        interface_method,
                        interface_desc,
                        ..
                    }
                    | SyntheticBridge::FieldSetter {
                        interface_method,
                        interface_desc,
                        ..
                    }
                    | SyntheticBridge::MethodInvoker {
                        interface_method,
                        interface_desc,
                        ..
                    } => (interface_method.as_str(), interface_desc.as_str()),
                };
                if b_name == name && b_desc == desc {
                    return Some(ResolvedMethod::Bridge {
                        declaring_class: class_name.to_owned(),
                        bridge: bridge.clone(),
                    });
                }
            }
            for (mixin_cls, m) in plan.merged_methods.iter().rev() {
                if m.name == name && m.desc == desc && m.code.is_some() {
                    return Some(ResolvedMethod::Bytecode {
                        pool_owner: mixin_cls.clone(),
                        declaring_class: class_name.to_owned(),
                        method: m.clone(),
                    });
                }
            }
        }

        // 3. ClassFile's own methods
        if let Some(cf) = self.bytecode_classes.get(class_name) {
            if let Some(m) = cf.method(name, desc) {
                if m.code.is_some() {
                    return Some(ResolvedMethod::Bytecode {
                        pool_owner: class_name.to_owned(),
                        declaring_class: class_name.to_owned(),
                        method: m.clone(),
                    });
                }
            }
        }
        None
    }

    /// Resolves a virtual, interface, or static call per JVMS §5.4.3.3 / §5.4.3.4:
    /// walks `start_class` and its superclasses first, then searches superinterfaces
    /// for non-private, non-abstract default methods.
    pub fn resolve_method(
        &self,
        start_class: &str,
        name: &str,
        desc: &str,
    ) -> Option<ResolvedMethod> {
        let mut cur = Some(start_class);
        let mut guard = 0usize;
        while let Some(cls) = cur {
            if guard > 64 {
                break;
            }
            guard += 1;
            if let Some(found) = self.find_declared_method(cls, name, desc) {
                return Some(found);
            }
            cur = self.superclasses.get(cls).map(String::as_str);
        }

        // Search superinterfaces breadth-first for default methods
        let mut queue = VecDeque::new();
        let mut seen = Vec::new();
        let mut cur_cls = Some(start_class);
        while let Some(cls) = cur_cls {
            if let Some(ifaces) = self.interfaces.get(cls) {
                for iface in ifaces {
                    queue.push_back(iface.as_str());
                }
            }
            cur_cls = self.superclasses.get(cls).map(String::as_str);
        }

        while let Some(iface) = queue.pop_front() {
            if seen.contains(&iface) {
                continue;
            }
            seen.push(iface);
            if let Some(found) = self.find_declared_method(iface, name, desc) {
                let is_private = match &found {
                    ResolvedMethod::Bytecode { method, .. } => method.is_private(),
                    ResolvedMethod::Native { owner, .. } => self
                        .native_methods
                        .get(owner)
                        .and_then(|t| t.get(&fnv1a_64(&format!("{name}.{desc}"))))
                        .and_then(|b| b.iter().find(|x| x.0 == format!("{name}.{desc}")))
                        .is_some_and(|x| x.3),
                    ResolvedMethod::Bridge { .. } => false,
                };
                if !is_private {
                    return Some(found);
                }
            }
            if let Some(sub_ifaces) = self.interfaces.get(iface) {
                for sub in sub_ifaces {
                    queue.push_back(sub.as_str());
                }
            }
        }

        None
    }
}

/// Execution context bundling `&mut Rt`, `&LinkedSpace`, and `&mut dyn HostApi`.
/// Passed to both the Tier-0 bytecode interpreter and Tier-1 AOT-compiled Rust methods.
pub struct ExecCtx<'a> {
    pub rt: &'a mut Rt,
    pub linked: &'a LinkedSpace,
    pub host: &'a mut dyn HostApi,
    pub depth: usize,
}

fn jdk_supertypes(cls: &str) -> &'static [&'static str] {
    match cls {
        "mmga/internal/IdentityFunction" => &["java/util/function/Function"],
        "java/lang/ThreadLocal$SuppliedThreadLocal" => &["java/lang/ThreadLocal"],
        "java/lang/Throwable" => &["java/io/Serializable"],
        "java/lang/Error" | "java/lang/Exception" => &["java/lang/Throwable"],
        "java/lang/LinkageError" | "java/lang/AssertionError" | "java/lang/VirtualMachineError" => {
            &["java/lang/Error"]
        }
        "java/lang/UnsatisfiedLinkError"
        | "java/lang/NoClassDefFoundError"
        | "java/lang/ExceptionInInitializerError"
        | "java/lang/IncompatibleClassChangeError" => &["java/lang/LinkageError"],
        "java/lang/StackOverflowError" | "java/lang/OutOfMemoryError" => {
            &["java/lang/VirtualMachineError"]
        }
        "java/lang/RuntimeException"
        | "java/io/IOException"
        | "java/lang/ReflectiveOperationException" => &["java/lang/Exception"],
        "java/io/EOFException" | "java/io/FileNotFoundException" => &["java/io/IOException"],
        "java/io/UncheckedIOException"
        | "java/lang/NullPointerException"
        | "java/lang/IllegalArgumentException"
        | "java/lang/IllegalStateException"
        | "java/lang/UnsupportedOperationException"
        | "java/lang/IndexOutOfBoundsException"
        | "java/lang/ClassCastException"
        | "java/lang/ArithmeticException"
        | "java/lang/IllegalCallerException"
        | "java/util/NoSuchElementException"
        | "java/util/ConcurrentModificationException" => &["java/lang/RuntimeException"],
        "java/lang/NumberFormatException" => &["java/lang/IllegalArgumentException"],
        "java/lang/ArrayIndexOutOfBoundsException" | "java/lang/StringIndexOutOfBoundsException" => {
            &["java/lang/IndexOutOfBoundsException"]
        }
        "java/lang/String" | "java/lang/StringBuilder" | "java/lang/StringBuffer" => &[
            "java/lang/CharSequence",
            "java/lang/Comparable",
            "java/io/Serializable",
        ],
        "java/lang/Integer"
        | "java/lang/Long"
        | "java/lang/Float"
        | "java/lang/Double"
        | "java/lang/Byte"
        | "java/lang/Short" => &["java/lang/Number", "java/lang/Comparable"],
        "java/lang/Boolean" | "java/lang/Character" | "java/lang/Enum" => {
            &["java/lang/Comparable", "java/io/Serializable"]
        }
        "java/util/Collection" => &["java/lang/Iterable"],
        "java/util/SequencedCollection" => &["java/util/Collection"],
        "java/util/List" | "java/util/Set" | "java/util/Queue" => {
            &["java/util/SequencedCollection", "java/util/Collection"]
        }
        "java/util/Deque" => &["java/util/Queue"],
        "java/util/ArrayList"
        | "java/util/LinkedList"
        | "java/util/concurrent/CopyOnWriteArrayList"
        | "it/unimi/dsi/fastutil/objects/ObjectArrayList"
        | "it/unimi/dsi/fastutil/ints/IntArrayList"
        | "com/google/common/collect/ImmutableList" => &["java/util/List"],
        "java/util/ArrayDeque" => &["java/util/Deque"],
        "java/util/HashSet"
        | "java/util/LinkedHashSet"
        | "java/util/TreeSet"
        | "it/unimi/dsi/fastutil/objects/ObjectOpenHashSet"
        | "it/unimi/dsi/fastutil/objects/ObjectLinkedOpenHashSet"
        | "it/unimi/dsi/fastutil/objects/ReferenceOpenHashSet"
        | "it/unimi/dsi/fastutil/objects/ObjectArraySet"
        | "com/google/common/collect/ImmutableSet" => &["java/util/Set"],
        "java/util/HashMap"
        | "java/util/LinkedHashMap"
        | "java/util/TreeMap"
        | "java/util/EnumMap"
        | "java/util/IdentityHashMap"
        | "java/util/concurrent/ConcurrentHashMap"
        | "it/unimi/dsi/fastutil/objects/Object2ObjectOpenHashMap"
        | "it/unimi/dsi/fastutil/objects/Object2ObjectLinkedOpenHashMap"
        | "it/unimi/dsi/fastutil/objects/Object2ObjectArrayMap"
        | "it/unimi/dsi/fastutil/objects/Reference2ObjectOpenHashMap"
        | "it/unimi/dsi/fastutil/objects/Object2IntOpenHashMap"
        | "it/unimi/dsi/fastutil/objects/Object2BooleanOpenHashMap"
        | "it/unimi/dsi/fastutil/ints/Int2ObjectOpenHashMap"
        | "com/google/common/collect/ImmutableMap" => &["java/util/Map"],
        "java/util/AbstractMap$SimpleEntry" | "java/util/AbstractMap$SimpleImmutableEntry" => {
            &["java/util/Map$Entry"]
        }
        "org/apache/commons/lang3/tuple/ImmutablePair" => &["org/apache/commons/lang3/tuple/Pair"],
        "org/apache/commons/lang3/tuple/Pair" => &["java/util/Map$Entry"],
        "java/io/ByteArrayInputStream" => &["java/io/InputStream"],
        "java/io/InputStream" => &["java/io/Closeable"],
        "java/io/Closeable" => &["java/lang/AutoCloseable"],
        _ => &[],
    }
}

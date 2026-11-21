//! Fastutil 8.5.12 Object2DoubleOpenHashMap subset used by Create CStress.
//!
//! Official Minecraft library SHA-1 c24946d46824bd528054bface3231d2ecb7e95e8.
//! Primitive default-return values are separate from boxed Map missing values.
//! Storage is field-backed so the generic Map dispatcher cannot consume a
//! primitive `(Object,double)double` call and return a reference.
use mmga_classfile::MemberRef;
use mmga_runtime::{box_primitive, invoke_member, Data, ExecCtx, Rt, V};
const CLASS: &str = "it/unimi/dsi/fastutil/objects/Object2DoubleOpenHashMap";
const MAP: &str = "it/unimi/dsi/fastutil/objects/Object2DoubleMap";
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if member.owner == CLASS && name == "<init>" && matches!(desc, "()V" | "(I)V" | "(IF)V") {
        let expected = if desc == "()V" { 16 } else { args.get(1)?.i() };
        let load = if desc == "(IF)V" {
            args.get(2)?.f()
        } else {
            0.75
        };
        if expected < 0 || !(load > 0.0 && load < 1.0) {
            return Some(ctx.rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "invalid expected size or load factor",
            ));
        }
        let Some(size) = table_size(expected as usize, load) else {
            return Some(ctx.rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "expected map size exceeds Fastutil limit",
            ));
        };
        let keys = ctx.rt.alloc(
            "[Ljava/lang/Object;",
            0,
            Data::Array(vec![V::NULL; size + 1]),
        );
        let values = ctx
            .rt
            .alloc("[D", 0, Data::Array(vec![V::D(0.0); size + 1]));
        for (field, value) in [
            ("key", keys),
            ("value", values),
            ("n", V::I(size as i32)),
            ("f", V::F(load)),
            ("size", V::I(0)),
            ("containsNullKey", V::FALSE),
            ("defRetValue", V::D(0.0)),
        ] {
            ctx.rt.put_field(receiver, field, value);
        }
        return Some(V::Void);
    }
    if ctx.rt.class_of(receiver) != Some(CLASS) || ctx.rt.get_field(receiver, "key").is_null() {
        return None;
    }
    let default = ctx.rt.get_field(receiver, "defRetValue");
    match (name, desc) {
        ("defaultReturnValue", "()D") => Some(default),
        ("defaultReturnValue", "(D)V") => {
            ctx.rt.put_field(receiver, "defRetValue", args[1]);
            Some(V::Void)
        }
        ("size", "()I") => Some(ctx.rt.get_field(receiver, "size")),
        ("isEmpty", "()Z") => Some(V::bool(ctx.rt.get_field(receiver, "size").i() == 0)),
        ("clear", "()V") => {
            let keys = ctx.rt.get_field(receiver, "key");
            if let Some(Data::Array(v)) = ctx.rt.get_obj_mut(keys).map(|o| &mut o.data) {
                v.fill(V::NULL);
            }
            ctx.rt.put_field(receiver, "size", V::I(0));
            ctx.rt.put_field(receiver, "containsNullKey", V::FALSE);
            Some(V::Void)
        }
        ("getDouble" | "applyAsDouble", "(Ljava/lang/Object;)D")
        | ("containsKey", "(Ljava/lang/Object;)Z")
        | (
            "get",
            "(Ljava/lang/Object;)Ljava/lang/Double;" | "(Ljava/lang/Object;)Ljava/lang/Object;",
        )
        | ("getOrDefault", "(Ljava/lang/Object;D)D") => {
            let (slot, present) = match find(ctx, receiver, args[1]) {
                Ok(v) => v,
                Err(e) => return Some(e),
            };
            if name == "containsKey" {
                return Some(V::bool(present));
            }
            let value = if present {
                at(ctx.rt, ctx.rt.get_field(receiver, "value"), slot)
            } else {
                default
            };
            if name == "get" {
                return Some(if present || value.d() != default.d() {
                    box_primitive(ctx.rt, "java/lang/Double", value)
                } else {
                    V::NULL
                });
            }
            if name == "getOrDefault" && !present && value.d() == default.d() {
                return Some(args[2]);
            }
            Some(value)
        }
        ("put", "(Ljava/lang/Object;D)D")
        | (
            "put",
            "(Ljava/lang/Object;Ljava/lang/Double;)Ljava/lang/Double;"
            | "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        ) => {
            let boxed = desc != "(Ljava/lang/Object;D)D";
            let (slot, present) = match find(ctx, receiver, args[1]) {
                Ok(v) => v,
                Err(e) => return Some(e),
            };
            let value = if boxed {
                if args[2].is_null() {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/NullPointerException", "null Double value"),
                    );
                }
                if ctx.rt.class_of(args[2]) != Some("java/lang/Double") {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/ClassCastException", "value is not Double"),
                    );
                }
                ctx.rt.get_field(args[2], "$value")
            } else {
                args[2]
            };
            let old = if present {
                at(ctx.rt, ctx.rt.get_field(receiver, "value"), slot)
            } else {
                default
            };
            let keys = ctx.rt.get_field(receiver, "key");
            let values = ctx.rt.get_field(receiver, "value");
            set(ctx.rt, values, slot, value);
            if !present {
                set(ctx.rt, keys, slot, args[1]);
                if args[1].is_null() {
                    ctx.rt.put_field(receiver, "containsNullKey", V::TRUE);
                }
                let old_size = ctx.rt.get_field(receiver, "size").i();
                ctx.rt.put_field(receiver, "size", V::I(old_size + 1));
                let n = ctx.rt.get_field(receiver, "n").i() as usize;
                let load = ctx.rt.get_field(receiver, "f").f();
                let max = ((n as f64 * load as f64).ceil() as usize).min(n - 1);
                if old_size as usize >= max {
                    let Some(new_size) = table_size(old_size as usize + 2, load) else {
                        return Some(ctx.rt.throw_exception(
                            "java/lang/IllegalArgumentException",
                            "map exceeds Fastutil size limit",
                        ));
                    };
                    if let Err(error) = rehash(ctx, receiver, new_size) {
                        return Some(error);
                    }
                }
            }
            Some(if boxed {
                if present {
                    box_primitive(ctx.rt, "java/lang/Double", old)
                } else {
                    V::NULL
                }
            } else {
                old
            })
        }
        ("forEach", "(Ljava/util/function/BiConsumer;)V") => {
            let consumer = *args.get(1)?;
            let count = ctx.rt.get_field(receiver, "n").i() as usize;
            if ctx.rt.get_field(receiver, "containsNullKey") == V::TRUE {
                let raw = at(ctx.rt, ctx.rt.get_field(receiver, "value"), count);
                let boxed = box_primitive(ctx.rt, "java/lang/Double", raw);
                invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/BiConsumer",
                        "accept",
                        "(Ljava/lang/Object;Ljava/lang/Object;)V",
                    ),
                    &[consumer, V::NULL, boxed],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
            }
            // FastEntrySet traverses null first then current backing slots descending.
            let mut index = ctx.rt.get_field(receiver, "n").i() as usize;
            while index > 0 {
                index -= 1;
                let key = at(ctx.rt, ctx.rt.get_field(receiver, "key"), index);
                if key.is_null() {
                    continue;
                }
                let raw = at(ctx.rt, ctx.rt.get_field(receiver, "value"), index);
                let boxed = box_primitive(ctx.rt, "java/lang/Double", raw);
                invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/BiConsumer",
                        "accept",
                        "(Ljava/lang/Object;Ljava/lang/Object;)V",
                    ),
                    &[consumer, key, boxed],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
            }
            Some(V::Void)
        }
        _ => None,
    }
}
fn table_size(expected: usize, load: f32) -> Option<usize> {
    // HashCommon.arraySize performs its division in float before Math.ceil.
    let needed = (expected as f32 / load).ceil();
    if !needed.is_finite() || needed > (1u32 << 30) as f32 {
        return None;
    }
    Some((needed as usize).next_power_of_two().max(2))
}
fn at(rt: &Rt, array: V, index: usize) -> V {
    match rt.get_obj(array).map(|o| &o.data) {
        Some(Data::Array(v)) => v[index],
        _ => V::NULL,
    }
}
fn set(rt: &mut Rt, array: V, index: usize, value: V) {
    if let Some(Data::Array(v)) = rt.get_obj_mut(array).map(|o| &mut o.data) {
        v[index] = value;
    }
}
fn hash(ctx: &mut ExecCtx<'_>, key: V) -> Result<usize, V> {
    let value = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", "hashCode", "()I"),
        &[key],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err(V::NULL);
    }
    let mixed = (value.i() as u32).wrapping_mul(0x9e3779b9);
    Ok((mixed ^ (mixed >> 16)) as usize)
}
fn find(ctx: &mut ExecCtx<'_>, map: V, key: V) -> Result<(usize, bool), V> {
    let n = ctx.rt.get_field(map, "n").i() as usize;
    if key.is_null() {
        return Ok((n, ctx.rt.get_field(map, "containsNullKey") == V::TRUE));
    }
    let mut slot = hash(ctx, key)? & (n - 1);
    loop {
        let current = at(ctx.rt, ctx.rt.get_field(map, "key"), slot);
        if current.is_null() {
            return Ok((slot, false));
        }
        let equal = invoke_member(
            ctx,
            mmga_classfile::op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
            &[key, current],
        );
        if ctx.rt.pending_exception.is_some() {
            return Err(V::NULL);
        }
        if equal.i() != 0 {
            return Ok((slot, true));
        }
        slot = (slot + 1) & (n - 1);
    }
}
fn rehash(ctx: &mut ExecCtx<'_>, map: V, new_n: usize) -> Result<(), V> {
    let old_n = ctx.rt.get_field(map, "n").i() as usize;
    let old_keys = ctx.rt.get_field(map, "key");
    let old_values = ctx.rt.get_field(map, "value");
    let keys = ctx.rt.alloc(
        "[Ljava/lang/Object;",
        0,
        Data::Array(vec![V::NULL; new_n + 1]),
    );
    let values = ctx
        .rt
        .alloc("[D", 0, Data::Array(vec![V::D(0.0); new_n + 1]));
    for i in (0..old_n).rev() {
        let key = at(ctx.rt, old_keys, i);
        if key.is_null() {
            continue;
        }
        let mut slot = hash(ctx, key)? & (new_n - 1);
        while !at(ctx.rt, keys, slot).is_null() {
            slot = (slot + 1) & (new_n - 1);
        }
        set(ctx.rt, keys, slot, key);
        let value = at(ctx.rt, old_values, i);
        set(ctx.rt, values, slot, value);
    }
    let null_value = at(ctx.rt, old_values, old_n);
    set(ctx.rt, values, new_n, null_value);
    ctx.rt.put_field(map, "key", keys);
    ctx.rt.put_field(map, "value", values);
    ctx.rt.put_field(map, "n", V::I(new_n as i32));
    Ok(())
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == CLASS
        && matches!(
            sup,
            MAP | "it/unimi/dsi/fastutil/objects/Object2DoubleFunction"
                | "java/util/Map"
                | "java/io/Serializable"
                | "java/lang/Cloneable"
                | "java/util/function/ToDoubleFunction"
        ))
        || (sub == MAP
            && matches!(
                sup,
                "it/unimi/dsi/fastutil/objects/Object2DoubleFunction"
                    | "java/util/Map"
                    | "java/util/function/ToDoubleFunction"
            ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, HostApi, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt,
    };
    #[derive(Default)]
    struct TestHost {
        seen: Vec<(V, f64)>,
    }
    impl HostApi for TestHost {
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
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            if member.owner == "java/util/function/BiConsumer" && member.name == "accept" {
                self.seen
                    .push((args[1], rt.get_field(args[2], "$value").d()));
                return Some(V::Void);
            }
            super::call(
                &mut ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                },
                member,
                is_static,
                args,
            )
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(MAP, name, desc),
            args,
        )
    }
    fn initialize(ctx: &mut ExecCtx<'_>) -> V {
        let map = ctx.rt.new_instance(CLASS, 0);
        invoke_member(
            ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(CLASS, "<init>", "()V"),
            &[map],
        );
        map
    }
    #[test]
    fn primitive_defaults_and_boxed_missing_values_stay_distinct() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = initialize(&mut ctx);
        assert!(ctx.rt.pending_exception.is_none());
        let key = ctx.rt.intern_str("create:shaft");
        assert_eq!(
            invoke(&mut ctx, "getDouble", "(Ljava/lang/Object;)D", &[map, key]),
            V::D(0.0)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[map, key]
            ),
            V::NULL
        );
        invoke(&mut ctx, "defaultReturnValue", "(D)V", &[map, V::D(-1.0)]);
        assert_eq!(
            invoke(
                &mut ctx,
                "put",
                "(Ljava/lang/Object;D)D",
                &[map, key, V::D(8.5)]
            ),
            V::D(-1.0)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "put",
                "(Ljava/lang/Object;D)D",
                &[map, key, V::D(-1.0)]
            ),
            V::D(8.5)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "containsKey",
                "(Ljava/lang/Object;)Z",
                &[map, key]
            ),
            V::TRUE
        );
        let boxed = invoke(
            &mut ctx,
            "get",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[map, key],
        );
        assert_eq!(ctx.rt.get_field(boxed, "$value"), V::D(-1.0));
        assert_eq!(
            invoke(
                &mut ctx,
                "put",
                "(Ljava/lang/Object;D)D",
                &[map, V::NULL, V::D(4.0)]
            ),
            V::D(-1.0)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "getDouble",
                "(Ljava/lang/Object;)D",
                &[map, V::NULL]
            ),
            V::D(4.0)
        );
        assert_eq!(invoke(&mut ctx, "size", "()I", &[map]), V::I(2));
    }
    #[test]
    fn object_keys_execute_real_equality_and_for_each_boxes_double_values() {
        fn equals(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            let count = ctx.rt.get_field(args[0], "equalsCalls").i() + 1;
            ctx.rt.put_field(args[0], "equalsCalls", V::I(count));
            V::bool(ctx.rt.get_field(args[0], "id") == ctx.rt.get_field(args[1], "id"))
        }
        fn hash(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            ctx.rt.get_field(args[0], "id")
        }
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/Key",
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[
                    ("equals.(Ljava/lang/Object;)Z", equals, false, false),
                    ("hashCode.()I", hash, false, false),
                ],
            }],
            lambdas: &[],
        });
        let mut rt = Rt::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = initialize(&mut ctx);
        let a = ctx.rt.new_instance("test/Key", 0);
        let b = ctx.rt.new_instance("test/Key", 0);
        ctx.rt.put_field(a, "id", V::I(5));
        ctx.rt.put_field(b, "id", V::I(5));
        invoke(
            &mut ctx,
            "put",
            "(Ljava/lang/Object;D)D",
            &[map, a, V::D(32.0)],
        );
        assert_eq!(
            invoke(&mut ctx, "getDouble", "(Ljava/lang/Object;)D", &[map, b]),
            V::D(32.0)
        );
        assert!(ctx.rt.get_field(b, "equalsCalls").i() > 0);
        let callback = ctx.rt.new_instance("test/BiConsumer", 0);
        invoke(
            &mut ctx,
            "forEach",
            "(Ljava/util/function/BiConsumer;)V",
            &[map, callback],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(host.seen, vec![(a, 32.0)]);
    }

    #[test]
    fn map_growth_retains_entries_and_key_errors_abort_insertion() {
        fn broken(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
            ctx.rt.throw_exception("test/HashFailure", "hash failed")
        }
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/BrokenKey",
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[("hashCode.()I", broken, false, false)],
            }],
            lambdas: &[],
        });
        let mut rt = Rt::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = initialize(&mut ctx);
        for n in 0..100 {
            let key = ctx.rt.intern_str(&format!("key-{n}"));
            assert_eq!(
                invoke(
                    &mut ctx,
                    "put",
                    "(Ljava/lang/Object;D)D",
                    &[map, key, V::D(n as f64)]
                ),
                V::D(0.0)
            );
        }
        for n in 0..100 {
            let key = ctx.rt.intern_str(&format!("key-{n}"));
            assert_eq!(
                invoke(&mut ctx, "getDouble", "(Ljava/lang/Object;)D", &[map, key]),
                V::D(n as f64)
            );
        }
        assert_eq!(invoke(&mut ctx, "size", "()I", &[map]), V::I(100));
        let bad = ctx.rt.new_instance("test/BrokenKey", 0);
        invoke(
            &mut ctx,
            "put",
            "(Ljava/lang/Object;D)D",
            &[map, bad, V::D(5.0)],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("HashFailure"));
        assert_eq!(invoke(&mut ctx, "size", "()I", &[map]), V::I(100));
        invoke(
            &mut ctx,
            "forEach",
            "(Ljava/util/function/BiConsumer;)V",
            &[map, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        let empty = initialize(&mut ctx);
        invoke(
            &mut ctx,
            "forEach",
            "(Ljava/util/function/BiConsumer;)V",
            &[empty, V::NULL],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "Fastutil empty forEach never dereferences the consumer"
        );
    }
}

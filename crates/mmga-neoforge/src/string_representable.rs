//! Minecraft 1.21.1 StringRepresentable enum codecs and name resolvers.
//!
//! Based on official client classes `azk`, `azk$a`, and `azk$b`. The resolver
//! retains callbacks and values; small enums are looked up lazily as in vanilla.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, HostApi, LinkedSpace, Rt, V};

const STRING_REPRESENTABLE: &str = "net/minecraft/util/StringRepresentable";
const ENUM_CODEC: &str = "net/minecraft/util/StringRepresentable$EnumCodec";
const VALUE_CODEC: &str = "net/minecraft/util/StringRepresentable$StringRepresentableCodec";
const NAME_LOOKUP: &str = "mmga/internal/StringRepresentableNameLookup";
const ORDINAL_GETTER: &str = "mmga/internal/StringRepresentableOrdinal";

pub fn get_static_field(_rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    (owner == STRING_REPRESENTABLE && name == "PRE_BUILT_MAP_THRESHOLD" && desc == "I")
        .then_some(V::I(16))
}

pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    if owner == STRING_REPRESENTABLE && is_static {
        return match (name, desc) {
            ("fromEnum", "(Ljava/util/function/Supplier;)Lnet/minecraft/util/StringRepresentable$EnumCodec;") => {
                Some(from_supplier(&mut ctx, *args.first()?, None))
            }
            ("fromEnumWithMapping", "(Ljava/util/function/Supplier;Ljava/util/function/Function;)Lnet/minecraft/util/StringRepresentable$EnumCodec;") => {
                Some(from_supplier(&mut ctx, *args.first()?, Some(*args.get(1)?)))
            }
            ("createNameLookup", "([Lnet/minecraft/util/StringRepresentable;Ljava/util/function/Function;)Ljava/util/function/Function;") => {
                Some(new_lookup(&mut ctx, *args.first()?, Some(*args.get(1)?)))
            }
            _ => None,
        };
    }
    if owner == ENUM_CODEC && !is_static {
        let recv = *args.first()?;
        return match (name, desc) {
            ("<init>", "([Ljava/lang/Enum;Ljava/util/function/Function;)V") => {
                let id_getter = ctx.rt.new_instance(ORDINAL_GETTER, 0);
                let initialized =
                    crate::dfu::enum_codec(ctx.rt, *args.get(1)?, *args.get(2)?, id_getter);
                for field in ["$dfu_kind", "$dfu_args"] {
                    ctx.rt
                        .put_field(recv, field, ctx.rt.get_field(initialized, field));
                }
                ctx.rt.put_field(recv, "$resolver", *args.get(2)?);
                Some(V::Void)
            }
            ("byName", "(Ljava/lang/String;)Ljava/lang/Enum;")
            | ("byName", "(Ljava/lang/String;Ljava/lang/Enum;)Ljava/lang/Enum;") => {
                let resolver = ctx.rt.get_field(recv, "$resolver");
                let resolved = function(&mut ctx, resolver, *args.get(1)?);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if !resolved.is_null()
                    && !ctx.linked.is_subtype(
                        ctx.rt.class_of(resolved).unwrap_or(""),
                        "java/lang/Enum",
                        ctx.host,
                    )
                {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "EnumCodec resolver did not return an enum",
                    ));
                }
                if desc == "(Ljava/lang/String;Ljava/lang/Enum;)Ljava/lang/Enum;"
                    && resolved.is_null()
                {
                    let default = *args.get(2)?;
                    return Some(if default.is_null() {
                        ctx.rt
                            .throw_exception("java/lang/NullPointerException", "defaultObj")
                    } else {
                        default
                    });
                }
                Some(resolved)
            }
            _ => None,
        };
    }
    if !is_static
        && matches!(owner, NAME_LOOKUP | "java/util/function/Function")
        && name == "apply"
        && desc == "(Ljava/lang/Object;)Ljava/lang/Object;"
        && ctx.rt.class_of(*args.first()?) == Some(NAME_LOOKUP)
    {
        return Some(lookup(&mut ctx, args[0], *args.get(1)?));
    }
    if !is_static
        && matches!(owner, ORDINAL_GETTER | "java/util/function/ToIntFunction")
        && name == "applyAsInt"
        && desc == "(Ljava/lang/Object;)I"
    {
        match ctx.rt.class_of(*args.first()?) {
            Some(ORDINAL_GETTER) => {
                let value = *args.get(1)?;
                if !value.is_null()
                    && !ctx.linked.is_subtype(
                        ctx.rt.class_of(value).unwrap_or(""),
                        "java/lang/Enum",
                        ctx.host,
                    )
                {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "enum ordinal requires an enum",
                    ));
                }
                return Some(invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Enum", "ordinal", "()I"),
                    &[value],
                ));
            }
            _ => {}
        }
    }
    None
}

fn from_supplier(ctx: &mut ExecCtx<'_>, supplier: V, mapper: Option<V>) -> V {
    let values = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
        &[supplier],
    );
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    if !values.is_null() {
        let class = ctx.rt.class_of(values).unwrap_or("");
        let element_class = class.strip_prefix("[L").and_then(|s| s.strip_suffix(';'));
        if !element_class
            .is_some_and(|class| ctx.linked.is_subtype(class, "java/lang/Enum", ctx.host))
        {
            return ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "StringRepresentable.fromEnum supplier must return Enum[]",
            );
        }
    }
    let resolver = new_lookup(ctx, values, mapper);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let getter = ctx.rt.new_instance(ORDINAL_GETTER, 0);
    let codec = crate::dfu::enum_codec(ctx.rt, values, resolver, getter);
    ctx.rt.put_field(codec, "$resolver", resolver);
    codec
}

fn array(ctx: &mut ExecCtx<'_>, value: V) -> Option<Vec<V>> {
    if value.is_null() {
        ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "StringRepresentable values array",
        );
        return None;
    }
    if let Some(Data::Array(values)) = ctx.rt.get_obj(value).map(|o| &o.data) {
        return Some(values.clone());
    }
    ctx.rt.throw_exception(
        "java/lang/ClassCastException",
        "StringRepresentable requires an array",
    );
    None
}

fn function(ctx: &mut ExecCtx<'_>, callback: V, value: V) -> V {
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            "java/util/function/Function",
            "apply",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
        ),
        &[callback, value],
    )
}

fn serialized_name(ctx: &mut ExecCtx<'_>, value: V) -> V {
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            STRING_REPRESENTABLE,
            "getSerializedName",
            "()Ljava/lang/String;",
        ),
        &[value],
    )
}

fn mapped_name(ctx: &mut ExecCtx<'_>, resolver: V, value: V) -> V {
    let name = serialized_name(ctx, value);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let mapped = if ctx.rt.get_field(resolver, "$identity") == V::TRUE {
        name
    } else {
        function(ctx, ctx.rt.get_field(resolver, "$mapping"), name)
    };
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    if !mapped.is_null() && ctx.rt.class_of(mapped) != Some("java/lang/String") {
        return ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "StringRepresentable name mapping must return String",
        );
    }
    mapped
}

fn new_lookup(ctx: &mut ExecCtx<'_>, values: V, mapping: Option<V>) -> V {
    let Some(items) = array(ctx, values) else {
        return V::NULL;
    };
    let resolver = ctx.rt.new_instance(NAME_LOOKUP, 0);
    ctx.rt.put_field(resolver, "$values", values);
    ctx.rt
        .put_field(resolver, "$identity", V::bool(mapping.is_none()));
    ctx.rt
        .put_field(resolver, "$mapping", mapping.unwrap_or(V::NULL));
    if items.len() > 16 {
        let mut entries = Vec::with_capacity(items.len());
        for value in items {
            let name = mapped_name(ctx, resolver, value);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            if entries.iter().any(|(key, _)| ctx.rt.val_eq(*key, name)) {
                return ctx.rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "Duplicate StringRepresentable name",
                );
            }
            entries.push((name, value));
        }
        let map = ctx.rt.alloc("java/util/HashMap", 0, Data::Map(entries));
        ctx.rt.put_field(resolver, "$name_map", map);
    }
    resolver
}

fn lookup(ctx: &mut ExecCtx<'_>, resolver: V, target: V) -> V {
    if !target.is_null() && ctx.rt.class_of(target) != Some("java/lang/String") {
        return ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "StringRepresentable lookup key must be String",
        );
    }
    let map = ctx.rt.get_field(resolver, "$name_map");
    if let Some(Data::Map(entries)) = ctx.rt.get_obj(map).map(|o| &o.data) {
        if target.is_null() {
            return V::NULL;
        }
        return entries
            .iter()
            .find(|(key, _)| ctx.rt.val_eq(*key, target))
            .map_or(V::NULL, |(_, value)| *value);
    }
    let values = ctx.rt.get_field(resolver, "$values");
    let Some(items) = array(ctx, values) else {
        return V::NULL;
    };
    for value in items {
        let name = mapped_name(ctx, resolver, value);
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        if name.is_null() {
            return ctx.rt.throw_exception(
                "java/lang/NullPointerException",
                "StringRepresentable name is null",
            );
        }
        if ctx.rt.val_eq(name, target) {
            return value;
        }
    }
    V::NULL
}

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (sub_class == ENUM_CODEC && super_class == VALUE_CODEC)
        || (matches!(sub_class, ENUM_CODEC | VALUE_CODEC)
            && matches!(
                super_class,
                "com/mojang/serialization/Codec"
                    | "com/mojang/serialization/Encoder"
                    | "com/mojang/serialization/Decoder"
            ))
        || (sub_class == NAME_LOOKUP && super_class == "java/util/function/Function")
        || (sub_class == ORDINAL_GETTER && super_class == "java/util/function/ToIntFunction")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, NativeClassDef, NativeLambdaFn,
        NativeMethodFn, NativeModuleRegistry,
    };

    struct Host;
    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, owner, name, desc)
                .or_else(|| crate::dfu::get_static_field(rt, owner, name, desc))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, linked, self, member, is_static, args)
                .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
                || crate::dfu::is_subtype(sub_class, super_class)
        }
    }

    fn supplier(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "calls").i();
        ctx.rt.put_field(args[0], "calls", V::I(calls + 1));
        args[1]
    }
    fn mapped_name(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let prefix = ctx.rt.str_of(ctx.rt.get_field(args[0], "prefix")).unwrap();
        let text = ctx.rt.str_of(args[1]).unwrap();
        ctx.rt.new_str(format!("{prefix}{text}"))
    }
    fn serialized_name(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "name_calls").i();
        ctx.rt.put_field(args[0], "name_calls", V::I(count + 1));
        ctx.rt.get_field(args[0], "serialized")
    }
    fn failure(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("example/SourceFailure", "enum supplier failed")
    }
    static METHODS: &[(&str, NativeMethodFn, bool, bool)] = &[(
        "getSerializedName.()Ljava/lang/String;",
        serialized_name,
        false,
        false,
    )];
    static CLASSES: &[NativeClassDef] = &[NativeClassDef {
        name: "example/Side",
        superclass: Some("java/lang/Enum"),
        interfaces: &[STRING_REPRESENTABLE],
        is_interface: false,
        methods: METHODS,
    }];
    static LAMBDAS: &[(u32, NativeLambdaFn)] = &[(1, supplier), (2, mapped_name), (3, failure)];

    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: CLASSES,
            lambdas: LAMBDAS,
        });
        linked
    }
    fn closure(rt: &mut Rt, id: u32, captures: Vec<V>) -> V {
        let (class, name, desc) = if id == 2 {
            (
                "java/util/function/Function",
                "apply",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
            )
        } else {
            ("java/util/function/Supplier", "get", "()Ljava/lang/Object;")
        };
        rt.alloc(
            class,
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: class.into(),
                sam_name: name.into(),
                sam_desc: desc.into(),
                instantiated_desc: desc.into(),
                captures,
                target: ClosureTarget::NativeLambda(id),
            })),
        )
    }
    fn values(rt: &mut Rt, count: usize) -> (V, Vec<V>) {
        let values: Vec<_> = (0..count)
            .map(|i| {
                let value = rt.new_instance("example/Side", 0);
                let name = rt.intern_str(&format!("side_{i}"));
                rt.put_field(value, "serialized", name);
                rt.put_field(value, "$name", name);
                rt.put_field(value, "$ordinal", V::I(i as i32));
                value
            })
            .collect();
        (
            rt.alloc("[Lexample/Side;", 0, Data::Array(values.clone())),
            values,
        )
    }
    fn make(ctx: &mut ExecCtx<'_>, supplier: V, mapper: Option<V>) -> V {
        if let Some(mapper) = mapper {
            invoke_member(ctx, op::INVOKESTATIC, &MemberRef::new(STRING_REPRESENTABLE, "fromEnumWithMapping", "(Ljava/util/function/Supplier;Ljava/util/function/Function;)Lnet/minecraft/util/StringRepresentable$EnumCodec;"), &[supplier, mapper])
        } else {
            invoke_member(ctx, op::INVOKESTATIC, &MemberRef::new(STRING_REPRESENTABLE, "fromEnum", "(Ljava/util/function/Supplier;)Lnet/minecraft/util/StringRepresentable$EnumCodec;"), &[supplier])
        }
    }
    fn by_name(ctx: &mut ExecCtx<'_>, codec: V, name: V) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(ENUM_CODEC, "byName", "(Ljava/lang/String;)Ljava/lang/Enum;"),
            &[codec, name],
        )
    }

    #[test]
    fn supplier_runs_once_and_small_enum_looks_up_current_serialized_names() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let counter = rt.new_instance("example/Counter", 0);
        let (array, values) = values(&mut rt, 3);
        let supplier = closure(&mut rt, 1, vec![counter, array]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = make(&mut ctx, supplier, None);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(counter, "calls"), V::I(1));
        assert_eq!(ctx.rt.get_field(values[0], "name_calls").i(), 0);
        let name = ctx.rt.intern_str("side_1");
        assert_eq!(by_name(&mut ctx, codec, name), values[1]);
        let renamed = ctx.rt.intern_str("renamed");
        ctx.rt.put_field(values[1], "serialized", renamed);
        assert_eq!(by_name(&mut ctx, codec, name), V::NULL);
        assert_eq!(by_name(&mut ctx, codec, renamed), values[1]);
        let unknown = ctx.rt.intern_str("missing");
        let fallback = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                ENUM_CODEC,
                "byName",
                "(Ljava/lang/String;Ljava/lang/Enum;)Ljava/lang/Enum;",
            ),
            &[codec, unknown, values[2]],
        );
        assert_eq!(fallback, values[2]);
        invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                ENUM_CODEC,
                "byName",
                "(Ljava/lang/String;Ljava/lang/Enum;)Ljava/lang/Enum;",
            ),
            &[codec, unknown, V::NULL],
        );
        let error = ctx.rt.take_exception().expect("null fallback is rejected");
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
    }

    #[test]
    fn mappings_are_lazy_for_small_enums_and_eager_for_large_enums() {
        for count in [3, 17] {
            let mut rt = Rt::new();
            let linked = linked();
            let mut host = Host;
            let state = rt.new_instance("example/MapperState", 0);
            let prefix = rt.intern_str("old:");
            rt.put_field(state, "prefix", prefix);
            let (array, values) = values(&mut rt, count);
            let supplier = closure(&mut rt, 1, vec![state, array]);
            let mapper = closure(&mut rt, 2, vec![state]);
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            let codec = make(&mut ctx, supplier, Some(mapper));
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(
                ctx.rt.get_field(values[0], "name_calls").i(),
                if count > 16 { 1 } else { 0 }
            );
            let prefix = ctx.rt.intern_str("new:");
            ctx.rt.put_field(state, "prefix", prefix);
            let name = ctx.rt.intern_str(if count > 16 {
                "old:side_0"
            } else {
                "new:side_0"
            });
            assert_eq!(by_name(&mut ctx, codec, name), values[0]);
            assert!(ctx.rt.pending_exception.is_none());
        }
    }

    #[test]
    fn duplicate_large_enum_names_and_supplier_failures_are_not_swallowed() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let counter = rt.new_instance("example/Counter", 0);
        let (array, values) = values(&mut rt, 17);
        let duplicate = rt.get_field(values[0], "serialized");
        rt.put_field(values[1], "serialized", duplicate);
        let supplier = closure(&mut rt, 1, vec![counter, array]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        make(&mut ctx, supplier, None);
        let error = ctx.rt.take_exception().expect("duplicate eager map key");
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalStateException")
        );
        let failed = closure(ctx.rt, 3, vec![]);
        make(&mut ctx, failed, None);
        let error = ctx
            .rt
            .take_exception()
            .expect("original supplier exception");
        assert_eq!(ctx.rt.class_of(error), Some("example/SourceFailure"));
    }

    fn result_value(ctx: &mut ExecCtx<'_>, result: V) -> V {
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "com/mojang/serialization/DataResult",
                "getOrThrow",
                "()Ljava/lang/Object;",
            ),
            &[result],
        )
    }

    fn parse_json(ctx: &mut ExecCtx<'_>, source: &str) -> V {
        let source = ctx.rt.new_str(source);
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "com/google/gson/JsonParser",
                "parseString",
                "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ),
            &[source],
        )
    }

    fn parse(ctx: &mut ExecCtx<'_>, codec: V, ops: V, source: &str) -> V {
        let json = parse_json(ctx, source);
        invoke_member(ctx, op::INVOKEINTERFACE,
            &MemberRef::new("com/mojang/serialization/Codec", "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"), &[codec, ops, json])
    }

    #[test]
    fn enum_codec_executes_json_names_and_compressed_ordinals_with_real_dfu_results() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let counter = rt.new_instance("example/Counter", 0);
        let prefix = rt.intern_str("mapped:");
        rt.put_field(counter, "prefix", prefix);
        let (array, values) = values(&mut rt, 3);
        let supplier = closure(&mut rt, 1, vec![counter, array]);
        let mapper = closure(&mut rt, 2, vec![counter]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = make(&mut ctx, supplier, Some(mapper));
        assert!(ctx.rt.pending_exception.is_none());
        let ops = crate::dfu::get_static_field(
            ctx.rt,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let compressed = crate::dfu::get_static_field(
            ctx.rt,
            "com/mojang/serialization/JsonOps",
            "COMPRESSED",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let decoded = parse(&mut ctx, codec, ops, "\"mapped:side_1\"");
        assert_eq!(result_value(&mut ctx, decoded), values[1]);
        let encoded = invoke_member(&mut ctx, op::INVOKEINTERFACE,
            &MemberRef::new("com/mojang/serialization/Codec", "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"), &[codec, ops, values[1]]);
        let json = result_value(&mut ctx, encoded);
        let raw_name = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "com/google/gson/JsonElement",
                "getAsString",
                "()Ljava/lang/String;",
            ),
            &[json],
        );
        assert_eq!(ctx.rt.str_of(raw_name), Some("side_1"));

        // Vanilla encodes Enum.ordinal but decodes by array index; it does not
        // silently normalize supplied arrays into declaration order.
        if let Data::Array(values) = &mut ctx.rt.get_obj_mut(array).unwrap().data {
            values.swap(0, 2);
        }
        let decoded = parse(&mut ctx, codec, compressed, "0");
        assert_eq!(result_value(&mut ctx, decoded), values[2]);
        let encoded = invoke_member(&mut ctx, op::INVOKEINTERFACE,
            &MemberRef::new("com/mojang/serialization/Codec", "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"), &[codec, compressed, values[2]]);
        let json = result_value(&mut ctx, encoded);
        let ordinal = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("com/google/gson/JsonElement", "getAsInt", "()I"),
            &[json],
        );
        assert_eq!(ordinal, V::I(2));
        for (ops, source) in [
            (ops, "\"not-a-side\""),
            (compressed, "-1"),
            (compressed, "3"),
        ] {
            let error = parse(&mut ctx, codec, ops, source);
            assert_eq!(
                invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("com/mojang/serialization/DataResult", "isError", "()Z"),
                    &[error]
                ),
                V::TRUE
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
    }
}

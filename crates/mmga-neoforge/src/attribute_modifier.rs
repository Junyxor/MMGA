//! Minecraft 1.21.1 AttributeModifier record, operation enum, and executable codecs.
//!
//! Verified against official client `buu` and `buu$a`. NBT save/load and applying
//! modifiers to a living entity are separate APIs and remain unsupported.

use mmga_classfile::MemberRef;
use mmga_runtime::{ClosureData, ClosureTarget, Data, HostApi, LinkedSpace, NullHost, Rt, V};

const MODIFIER: &str = "net/minecraft/world/entity/ai/attributes/AttributeModifier";
const OPERATION: &str = "net/minecraft/world/entity/ai/attributes/AttributeModifier$Operation";
const OPERATION_DESC: &str =
    "Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;";
const STREAM: &str = "net/minecraft/network/codec/StreamCodec";
const CODEC: &str = "com/mojang/serialization/Codec";
const MAP_CODEC: &str = "com/mojang/serialization/MapCodec";
const INTERNAL: &str = "mmga/internal/AttributeModifierCodecs";
const RL: &str = "net/minecraft/resources/ResourceLocation";
const NAMES: [&str; 3] = ["ADD_VALUE", "ADD_MULTIPLIED_BASE", "ADD_MULTIPLIED_TOTAL"];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == OPERATION && desc == OPERATION_DESC {
        return operation(rt, name);
    }
    let expected = match (owner, name) {
        (MODIFIER, "MAP_CODEC") => "Lcom/mojang/serialization/MapCodec;",
        (MODIFIER | OPERATION, "CODEC") => "Lcom/mojang/serialization/Codec;",
        (MODIFIER | OPERATION, "STREAM_CODEC") => "Lnet/minecraft/network/codec/StreamCodec;",
        (OPERATION, "BY_ID") => "Ljava/util/function/IntFunction;",
        _ => return None,
    };
    if desc != expected {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = match (owner, name) {
        (OPERATION, "BY_ID") => callback(
            rt,
            "java/util/function/IntFunction",
            "apply",
            "(I)Ljava/lang/Object;",
            6,
            MemberRef::new(
                INTERNAL,
                "operationById",
                "(I)Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
            ),
        ),
        (OPERATION, "CODEC") => {
            let values = operation_values(rt);
            let resolver = callback(rt, "java/util/function/Function", "apply", "(Ljava/lang/Object;)Ljava/lang/Object;", 6,
                MemberRef::new(INTERNAL, "operationByName", "(Ljava/lang/String;)Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;"));
            let id = operation_id_getter(rt);
            let codec = crate::dfu::enum_codec(rt, values, resolver, id);
            rt.put_field(codec, "$resolver", resolver);
            codec
        }
        (OPERATION, "STREAM_CODEC") => {
            let resolver =
                get_static_field(rt, OPERATION, "BY_ID", "Ljava/util/function/IntFunction;")?;
            let id = operation_id_getter(rt);
            stream_call(rt, "net/minecraft/network/codec/ByteBufCodecs", "idMapper", "(Ljava/util/function/IntFunction;Ljava/util/function/ToIntFunction;)Lnet/minecraft/network/codec/StreamCodec;", true, &[resolver, id])?
        }
        (MODIFIER, "MAP_CODEC") => {
            let location =
                crate::dfu::get_static_field(rt, RL, "CODEC", "Lcom/mojang/serialization/Codec;")?;
            let amount = crate::dfu::get_static_field(
                rt,
                CODEC,
                "DOUBLE",
                "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
            )?;
            let operation =
                get_static_field(rt, OPERATION, "CODEC", "Lcom/mojang/serialization/Codec;")?;
            let mut fields = Vec::new();
            for (name, codec, getter_desc) in [
                (
                    "id",
                    location,
                    "()Lnet/minecraft/resources/ResourceLocation;",
                ),
                ("amount", amount, "()D"),
                (
                    "operation",
                    operation,
                    "()Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
                ),
            ] {
                let field = crate::dfu::field_codec(rt, codec, name);
                let getter = getter(rt, name, getter_desc);
                fields.push((field, getter));
            }
            let constructor = constructor(rt);
            crate::dfu::record_map_codec(rt, &fields, constructor)
        }
        (MODIFIER, "CODEC") => {
            let map = get_static_field(
                rt,
                MODIFIER,
                "MAP_CODEC",
                "Lcom/mojang/serialization/MapCodec;",
            )?;
            crate::dfu::codec_from_map(rt, map)
        }
        (MODIFIER, "STREAM_CODEC") => {
            let text = crate::codecs::get_static_field(
                rt,
                "net/minecraft/network/codec/ByteBufCodecs",
                "STRING_UTF8",
                "Lnet/minecraft/network/codec/StreamCodec;",
            )?;
            let from_text = callback(
                rt,
                "java/util/function/Function",
                "apply",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                6,
                MemberRef::new(
                    INTERNAL,
                    "parseResourceId",
                    "(Ljava/lang/String;)Lnet/minecraft/resources/ResourceLocation;",
                ),
            );
            let to_text = callback(
                rt,
                "java/util/function/Function",
                "apply",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                6,
                MemberRef::new(
                    INTERNAL,
                    "resourceIdString",
                    "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/String;",
                ),
            );
            let location = stream_call(rt, STREAM, "map", "(Ljava/util/function/Function;Ljava/util/function/Function;)Lnet/minecraft/network/codec/StreamCodec;", false, &[text, from_text, to_text])?;
            let amount = crate::codecs::get_static_field(
                rt,
                "net/minecraft/network/codec/ByteBufCodecs",
                "DOUBLE",
                "Lnet/minecraft/network/codec/StreamCodec;",
            )?;
            let operation = get_static_field(
                rt,
                OPERATION,
                "STREAM_CODEC",
                "Lnet/minecraft/network/codec/StreamCodec;",
            )?;
            let location_getter = getter(rt, "id", "()Lnet/minecraft/resources/ResourceLocation;");
            let amount_getter = getter(rt, "amount", "()D");
            let operation_getter = getter(
                rt,
                "operation",
                "()Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
            );
            let constructor = constructor(rt);
            stream_call(rt, STREAM, "composite", "(Lnet/minecraft/network/codec/StreamCodec;Ljava/util/function/Function;Lnet/minecraft/network/codec/StreamCodec;Ljava/util/function/Function;Lnet/minecraft/network/codec/StreamCodec;Ljava/util/function/Function;Lcom/mojang/datafixers/util/Function3;)Lnet/minecraft/network/codec/StreamCodec;", true,
                &[location, location_getter, amount, amount_getter, operation, operation_getter, constructor])?
        }
        _ => return None,
    };
    rt.put_static(owner, name, value);
    Some(value)
}

pub fn call(
    rt: &mut Rt,
    _linked: &LinkedSpace,
    _host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == INTERNAL && is_static {
        return match (name, desc) {
            ("operationById", "(I)Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;") => {
                let index = args.first()?.i();
                operation(rt, NAMES[if (0..3).contains(&index) { index as usize } else { 0 }])
            }
            ("operationByName", "(Ljava/lang/String;)Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;") => {
                let name = rt.str_of(*args.first()?).map(str::to_owned);
                let found = name.as_ref().and_then(|name| NAMES.iter().find(|candidate| candidate.to_ascii_lowercase() == *name)).copied();
                Some(found.and_then(|name| operation(rt, name)).unwrap_or(V::NULL))
            }
            ("parseResourceId", "(Ljava/lang/String;)Lnet/minecraft/resources/ResourceLocation;") => {
                let text = *args.first()?;
                if text.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "ResourceLocation input")); }
                let text = rt.str_of(text)?.to_owned();
                let (namespace, path) = text.split_once(':').unwrap_or(("minecraft", &text));
                let namespace = if namespace.is_empty() { "minecraft" } else { namespace };
                if !namespace.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
                    || !path.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"/_.-".contains(&c)) {
                    return Some(rt.throw_exception("net/minecraft/ResourceLocationException", &format!("Invalid resource location: {text}")));
                }
                Some(rt.new_pinned_handle(RL, "resource_location", 0, format!("{namespace}:{path}")))
            }
            ("resourceIdString", "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/String;") => {
                let value = *args.first()?;
                if value.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "ResourceLocation is null")); }
                let location = resource_id(rt, value)?;
                Some(rt.new_str(location))
            }
            _ => None,
        };
    }
    if owner == OPERATION {
        return match (name, desc, is_static) {
            ("values", "()[Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;", true) => Some(operation_values(rt)),
            ("valueOf", "(Ljava/lang/String;)Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;", true) => {
                let text = *args.first()?;
                if text.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null")); }
                let text = rt.str_of(text)?.to_owned();
                Some(operation(rt, &text).unwrap_or_else(|| rt.throw_exception("java/lang/IllegalArgumentException", &format!("No enum constant AttributeModifier.Operation.{text}"))))
            }
            ("id", "()I", false) => Some(rt.get_field(*args.first()?, "id")),
            ("getSerializedName", "()Ljava/lang/String;", false) => Some(rt.get_field(*args.first()?, "name")),
            _ => None,
        };
    }
    if owner == "net/minecraft/util/StringRepresentable"
        && !is_static
        && name == "getSerializedName"
        && desc == "()Ljava/lang/String;"
        && rt.class_of(*args.first()?) == Some(OPERATION)
    {
        return Some(rt.get_field(args[0], "name"));
    }
    if owner != MODIFIER || is_static {
        return None;
    }
    let recv = *args.first()?;
    match (name, desc) {
        ("<init>", "(Lnet/minecraft/resources/ResourceLocation;DLnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;)V") => {
            rt.put_field(recv, "id", *args.get(1)?);
            rt.put_field(recv, "amount", *args.get(2)?);
            rt.put_field(recv, "operation", *args.get(3)?);
            Some(V::Void)
        }
        ("id", "()Lnet/minecraft/resources/ResourceLocation;") | ("amount", "()D")
        | ("operation", "()Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;") => Some(rt.get_field(recv, name)),
        ("is", "(Lnet/minecraft/resources/ResourceLocation;)Z") => {
            let target = *args.get(1)?;
            if target.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "AttributeModifier.is(null)")); }
            Some(V::bool(rt.val_eq(target, rt.get_field(recv, "id"))))
        }
        ("equals", "(Ljava/lang/Object;)Z") => {
            let other = *args.get(1)?;
            Some(V::bool(rt.class_of(other) == Some(MODIFIER)
                && rt.val_eq(rt.get_field(recv, "id"), rt.get_field(other, "id"))
                && double_bits(rt.get_field(recv, "amount").d()) == double_bits(rt.get_field(other, "amount").d())
                && rt.get_field(recv, "operation") == rt.get_field(other, "operation")))
        }
        ("hashCode", "()I") => {
            let id = rt.get_field(recv, "id");
            let id_hash = resource_id(rt, id).map_or(0i32, |id| {
                let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", &id));
                java_string_hash(namespace).wrapping_mul(31).wrapping_add(java_string_hash(path))
            });
            let amount = double_bits(rt.get_field(recv, "amount").d());
            let amount_hash = (amount ^ (amount >> 32)) as i32;
            let operation = rt.get_field(recv, "operation").r() as i32;
            Some(V::I(id_hash.wrapping_mul(31).wrapping_add(amount_hash).wrapping_mul(31).wrapping_add(operation)))
        }
        _ => None,
    }
}

fn operation(rt: &mut Rt, name: &str) -> Option<V> {
    let index = NAMES.iter().position(|&candidate| candidate == name)?;
    if let Some(value) = rt.get_static(OPERATION, name) {
        return Some(value);
    }
    let value = rt.new_instance(OPERATION, 0);
    let label = rt.intern_str(name);
    let serialized = rt.intern_str(&name.to_ascii_lowercase());
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(index as i32));
    rt.put_field(value, "name", serialized);
    rt.put_field(value, "id", V::I(index as i32));
    rt.put_static(OPERATION, name, value);
    Some(value)
}
fn operation_values(rt: &mut Rt) -> V {
    let values = NAMES
        .iter()
        .map(|name| operation(rt, name).unwrap())
        .collect();
    rt.alloc(
        "[Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
        0,
        Data::Array(values),
    )
}
fn operation_id_getter(rt: &mut Rt) -> V {
    callback(
        rt,
        "java/util/function/ToIntFunction",
        "applyAsInt",
        "(Ljava/lang/Object;)I",
        5,
        MemberRef::new(OPERATION, "id", "()I"),
    )
}
fn getter(rt: &mut Rt, name: &str, desc: &str) -> V {
    callback(
        rt,
        "java/util/function/Function",
        "apply",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        5,
        MemberRef::new(MODIFIER, name, desc),
    )
}
fn constructor(rt: &mut Rt) -> V {
    callback(rt, "com/mojang/datafixers/util/Function3", "apply", "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", 8,
        MemberRef::new(MODIFIER, "<init>", "(Lnet/minecraft/resources/ResourceLocation;DLnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;)V"))
}
fn callback(
    rt: &mut Rt,
    interface: &str,
    sam: &str,
    sam_desc: &str,
    kind: u8,
    member: MemberRef,
) -> V {
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: sam.into(),
            sam_desc: sam_desc.into(),
            instantiated_desc: sam_desc.into(),
            captures: vec![],
            target: ClosureTarget::MethodHandle {
                handle_kind: kind,
                member,
            },
        })),
    )
}
fn stream_call(
    rt: &mut Rt,
    owner: &str,
    name: &str,
    desc: &str,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    crate::codecs::call(
        rt,
        &LinkedSpace::new(),
        &mut NullHost,
        &MemberRef::new(owner, name, desc),
        is_static,
        args,
    )
}
fn resource_id(rt: &Rt, value: V) -> Option<String> {
    if let Some(("resource_location", _, id, _)) = rt.handle_of(value) {
        return Some(id.to_owned());
    }
    rt.str_of(rt.get_field(value, "id")).map(str::to_owned)
}
fn double_bits(value: f64) -> u64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        value.to_bits()
    }
}

fn java_string_hash(value: &str) -> i32 {
    value.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(unit as i32)
    })
}
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (sub_class == MODIFIER && super_class == "java/lang/Record")
        || (sub_class == OPERATION
            && matches!(
                super_class,
                "java/lang/Enum"
                    | "net/minecraft/util/StringRepresentable"
                    | "java/lang/Comparable"
            ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, Data, ExecCtx};

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
                .or_else(|| crate::codecs::get_static_field(rt, owner, name, desc))
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
                .or_else(|| {
                    crate::string_representable::call(rt, linked, self, member, is_static, args)
                })
                .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
                .or_else(|| crate::codecs::call(rt, linked, self, member, is_static, args))
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
                || crate::dfu::is_subtype(sub_class, super_class)
                || crate::string_representable::is_subtype(sub_class, super_class)
        }
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        is_static: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if is_static {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn result_value(ctx: &mut ExecCtx<'_>, result: V) -> V {
        invoke(
            ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[result],
        )
    }
    fn json(ctx: &mut ExecCtx<'_>, source: &str) -> V {
        let source = ctx.rt.new_str(source);
        invoke(
            ctx,
            "com/google/gson/JsonParser",
            "parseString",
            "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            true,
            &[source],
        )
    }
    fn new_modifier(ctx: &mut ExecCtx<'_>, amount: f64, operation: &str) -> V {
        let operation = read_static_field(ctx, OPERATION, operation, OPERATION_DESC);
        let id = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "example:test",
        );
        let value = ctx.rt.new_instance(MODIFIER, 0);
        invoke(ctx, MODIFIER, "<init>", "(Lnet/minecraft/resources/ResourceLocation;DLnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;)V", false, &[value, id, V::D(amount), operation]);
        assert!(ctx.rt.pending_exception.is_none());
        value
    }

    #[test]
    fn modifier_record_json_round_trip_has_exact_field_names_and_operation() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = read_static_field(
            &mut ctx,
            MODIFIER,
            "MAP_CODEC",
            "Lcom/mojang/serialization/MapCodec;",
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "real MAP_CODEC must exist"
        );
        let codec = invoke(
            &mut ctx,
            MAP_CODEC,
            "codec",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[map],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let value = new_modifier(&mut ctx, 1.5, "ADD_MULTIPLIED_BASE");
        let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, value]);
        let encoded = result_value(&mut ctx, encoded);
        let text = invoke(
            &mut ctx,
            "com/google/gson/JsonElement",
            "toString",
            "()Ljava/lang/String;",
            false,
            &[encoded],
        );
        let actual: serde_json::Value = serde_json::from_str(ctx.rt.str_of(text).unwrap()).unwrap();
        assert_eq!(
            actual,
            serde_json::json!({"id":"example:test","amount":1.5,"operation":"add_multiplied_base"})
        );
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, encoded]);
        let decoded = result_value(&mut ctx, decoded);
        assert_eq!(
            invoke(&mut ctx, MODIFIER, "amount", "()D", false, &[decoded]),
            V::D(1.5)
        );
        let decoded_id = invoke(
            &mut ctx,
            MODIFIER,
            "id",
            "()Lnet/minecraft/resources/ResourceLocation;",
            false,
            &[decoded],
        );
        assert_eq!(ctx.rt.handle_of(decoded_id).unwrap().2, "example:test");
        for source in [
            r#"{"id":"example:test","amount":1.5,"operation":"invalid"}"#,
            r#"{"id":"example:test","operation":"add_value"}"#,
        ] {
            let invalid = json(&mut ctx, source);
            let result = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, invalid]);
            assert_eq!(
                invoke(
                    &mut ctx,
                    "com/mojang/serialization/DataResult",
                    "isError",
                    "()Z",
                    false,
                    &[result]
                ),
                V::TRUE
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn modifier_stream_matches_resource_id_double_and_operation_wire_bytes() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = read_static_field(
            &mut ctx,
            MODIFIER,
            "STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "real stream codec must exist"
        );
        let value = new_modifier(&mut ctx, 1.5, "ADD_MULTIPLIED_BASE");
        let buffer = ctx.rt.alloc(
            "io/netty/buffer/ByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![],
                reader_idx: 0,
            },
        );
        invoke(
            &mut ctx,
            STREAM,
            "encode",
            "(Ljava/lang/Object;Ljava/lang/Object;)V",
            false,
            &[codec, buffer, value],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let mut expected = vec![12];
        expected.extend_from_slice(b"example:test");
        expected.extend_from_slice(&1.5f64.to_be_bytes());
        expected.push(1);
        assert!(
            matches!(&ctx.rt.get_obj(buffer).unwrap().data, Data::ByteBuf { bytes, .. } if bytes == &expected)
        );
        let decoded = invoke(
            &mut ctx,
            STREAM,
            "decode",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
            &[codec, buffer],
        );
        assert_eq!(
            invoke(&mut ctx, MODIFIER, "amount", "()D", false, &[decoded]),
            V::D(1.5)
        );
        let operation = invoke(
            &mut ctx,
            MODIFIER,
            "operation",
            "()Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
            false,
            &[decoded],
        );
        assert_eq!(
            operation,
            read_static_field(&mut ctx, OPERATION, "ADD_MULTIPLIED_BASE", OPERATION_DESC)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn operation_enum_invalid_ids_and_names_follow_distinct_wire_and_data_rules() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = invoke(
            &mut ctx,
            OPERATION,
            "values",
            "()[Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
            true,
            &[],
        );
        let second = invoke(
            &mut ctx,
            OPERATION,
            "values",
            "()[Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;",
            true,
            &[],
        );
        assert_ne!(first, second);
        let values = match &ctx.rt.get_obj(first).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("enum array"),
        };
        for (index, name) in NAMES.iter().enumerate() {
            let constant = read_static_field(&mut ctx, OPERATION, name, OPERATION_DESC);
            assert_eq!(constant, values[index]);
            assert_eq!(
                invoke(&mut ctx, OPERATION, "id", "()I", false, &[constant]),
                V::I(index as i32)
            );
        }
        let by_id = read_static_field(
            &mut ctx,
            OPERATION,
            "BY_ID",
            "Ljava/util/function/IntFunction;",
        );
        for id in [-99, 3, i32::MAX] {
            assert_eq!(
                invoke(
                    &mut ctx,
                    "java/util/function/IntFunction",
                    "apply",
                    "(I)Ljava/lang/Object;",
                    false,
                    &[by_id, V::I(id)]
                ),
                values[0]
            );
        }
        let bad_name = ctx.rt.new_str("add_value");
        invoke(&mut ctx, OPERATION, "valueOf", "(Ljava/lang/String;)Lnet/minecraft/world/entity/ai/attributes/AttributeModifier$Operation;", true, &[bad_name]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalArgumentException")
        );

        let a = new_modifier(&mut ctx, 1.5, "ADD_VALUE");
        let b = new_modifier(&mut ctx, 1.5, "ADD_VALUE");
        assert_eq!(
            invoke(
                &mut ctx,
                MODIFIER,
                "equals",
                "(Ljava/lang/Object;)Z",
                false,
                &[a, b]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(&mut ctx, MODIFIER, "hashCode", "()I", false, &[a]),
            invoke(&mut ctx, MODIFIER, "hashCode", "()I", false, &[b])
        );
        assert!(get_static_field(ctx.rt, OPERATION, "NOT_AN_OPERATION", OPERATION_DESC).is_none());
        assert!(get_static_field(ctx.rt, MODIFIER, "MAP_CODEC", "I").is_none());
        assert!(ctx.rt.pending_exception.is_none());
    }
}

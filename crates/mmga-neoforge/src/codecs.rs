//! Minecraft stream codecs are immutable descriptions with retained child codecs and callbacks.
//!
//! Construction never drops a mapper, constructor, or limit. The executable subset uses the
//! runtime's byte buffer storage; codecs requiring a registry, NBT, or DFU fail explicitly when
//! executed. Building a codec graph does not imply that the network protocol is implemented.
//! Semantics were checked against Mojang's 1.21.1 client and official mappings (classes `yv`,
//! `yx`, and their inner classes), including the collection initial-capacity cap of 65536.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{box_primitive, invoke_member, Data, ExecCtx, HostApi, LinkedSpace, Rt, V};

const STREAM: &str = "net/minecraft/network/codec/StreamCodec";
const BYTE_CODECS: &str = "net/minecraft/network/codec/ByteBufCodecs";
const OPERATION: &str = "net/minecraft/network/codec/StreamCodec$CodecOperation";
const ENCODER: &str = "io/netty/handler/codec/EncoderException";
const DECODER: &str = "io/netty/handler/codec/DecoderException";

fn graph(rt: &mut Rt, kind: &str, args: &[V]) -> V {
    let codec = rt.new_instance(STREAM, 0);
    let kind = rt.intern_str(kind);
    let args = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(args.to_vec()));
    rt.put_field(codec, "$codec_kind", kind);
    rt.put_field(codec, "$codec_args", args);
    codec
}

fn graph_args(rt: &Rt, codec: V) -> Option<(String, Vec<V>)> {
    let kind = rt.str_of(rt.get_field(codec, "$codec_kind"))?.to_owned();
    match rt
        .get_obj(rt.get_field(codec, "$codec_args"))
        .map(|o| &o.data)
    {
        Some(Data::Array(args)) => Some((kind, args.clone())),
        _ => None,
    }
}

/// Static codec objects, including named protocol codecs whose execution is not implemented yet.
/// Unlike the former opaque handles, these objects compose into graphs without losing operands.
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    let expected = if owner == BYTE_CODECS && name == "MAX_INITIAL_COLLECTION_SIZE" {
        "I"
    } else {
        "Lnet/minecraft/network/codec/StreamCodec;"
    };
    if desc != expected {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = create_static_field(rt, owner, name)?;
    rt.put_static(owner, name, value);
    Some(value)
}

fn create_static_field(rt: &mut Rt, owner: &str, name: &str) -> Option<V> {
    if owner == BYTE_CODECS {
        let kind = match name {
            "BOOL" | "BYTE" | "SHORT" | "UNSIGNED_SHORT" | "INT" | "VAR_INT" | "VAR_LONG"
            | "FLOAT" | "DOUBLE" => name,
            "MAX_INITIAL_COLLECTION_SIZE" => return Some(V::I(65536)),
            "STRING_UTF8" => return Some(graph(rt, "stringUtf8", &[V::I(32767)])),
            "BYTE_ARRAY" => return Some(graph(rt, "byteArrayUnbounded", &[])),
            "TAG"
            | "TRUSTED_TAG"
            | "COMPOUND_TAG"
            | "TRUSTED_COMPOUND_TAG"
            | "OPTIONAL_COMPOUND_TAG"
            | "VECTOR3F"
            | "QUATERNIONF"
            | "GAME_PROFILE"
            | "GAME_PROFILE_PROPERTIES" => {
                return Some(graph(rt, &format!("{owner}.{name}"), &[]));
            }
            _ => return None,
        };
        return Some(graph(rt, kind, &[]));
    }
    if matches!(
        (owner, name),
        ("net/minecraft/resources/ResourceLocation", "STREAM_CODEC")
            | ("net/minecraft/core/UUIDUtil", "STREAM_CODEC")
            | ("net/minecraft/core/BlockPos", "STREAM_CODEC")
            | ("net/minecraft/core/GlobalPos", "STREAM_CODEC")
            | (
                "net/minecraft/network/chat/ComponentSerialization",
                "STREAM_CODEC"
            )
            | (
                "net/minecraft/network/chat/ComponentSerialization",
                "TRUSTED_STREAM_CODEC"
            )
            | (
                "net/minecraft/network/chat/ComponentSerialization",
                "OPTIONAL_STREAM_CODEC"
            )
            | ("net/minecraft/world/item/ItemStack", "STREAM_CODEC")
            | (
                "net/minecraft/world/item/ItemStack",
                "OPTIONAL_STREAM_CODEC"
            )
            | ("net/minecraft/world/item/ItemStack", "LIST_STREAM_CODEC")
            | (
                "net/minecraft/world/item/ItemStack",
                "OPTIONAL_LIST_STREAM_CODEC"
            )
            | ("net/neoforged/neoforge/fluids/FluidStack", "STREAM_CODEC")
            | (
                "net/neoforged/neoforge/fluids/FluidStack",
                "OPTIONAL_STREAM_CODEC"
            )
            | ("net/minecraft/core/Direction", "STREAM_CODEC")
    ) {
        return Some(graph(rt, &format!("{owner}.{name}"), &[]));
    }
    None
}

/// Dispatches real graph constructors/combinators and the StreamEncoder/StreamDecoder methods.
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
    if !known_signature(member, is_static) {
        return None;
    }
    if owner == BYTE_CODECS && is_static {
        return match name {
            "list" if args.len() <= 1 => {
                let operation = rt.new_instance(OPERATION, 0);
                let max = args.first().copied().unwrap_or(V::I(i32::MAX));
                let kind = rt.intern_str("list");
                rt.put_field(operation, "$codec_operation", kind);
                rt.put_field(operation, "$codec_max", max);
                Some(operation)
            }
            "idMapper" if args.len() == 2 => Some(graph(rt, "idMapper", args)),
            "idMapper" if args.len() == 1 => Some(if args[0].is_null() {
                rt.throw_exception("java/lang/NullPointerException", "idMapper(null)")
            } else {
                graph(rt, "idMap", args)
            }),
            "stringUtf8" | "byteArray" if args.len() == 1 => Some(graph(rt, name, args)),
            "optional" if args.len() == 1 => Some(graph(rt, "optional", args)),
            "collection" if args.len() == 1 => {
                let operation = rt.new_instance(OPERATION, 0);
                let kind = rt.intern_str("collection");
                rt.put_field(operation, "$codec_operation", kind);
                rt.put_field(operation, "$codec_factory", args[0]);
                Some(operation)
            }
            "collection" if matches!(args.len(), 2 | 3) => Some(graph(rt, "collection", args)),
            "map" if matches!(args.len(), 3 | 4) => Some(graph(rt, "mapEntries", args)),
            // Registry/DFU factories describe their operands faithfully. Their wire semantics
            // require additional host services and fail at encode/decode, never with fake bytes.
            "holderRegistry"
            | "registry"
            | "holder"
            | "holderSet"
            | "fromCodec"
            | "fromCodecTrusted"
            | "fromCodecWithRegistries"
            | "fromCodecWithRegistriesTrusted"
            | "either"
            | "tagCodec"
            | "compoundTagCodec" => {
                Some(graph(rt, &format!("{owner}.{name}{}", member.desc), args))
            }
            _ => None,
        };
    }
    if owner == STREAM && is_static {
        return match name {
            "unit" if args.len() == 1 => Some(graph(rt, "unit", args)),
            "of" | "ofMember" if args.len() == 2 => Some(graph(rt, name, args)),
            "composite" if args.len() >= 3 && args.len() % 2 == 1 => {
                Some(graph(rt, "composite", args))
            }
            "recursive" if args.len() == 1 => Some(graph(rt, "recursive", args)),
            _ => None,
        };
    }
    if owner == OPERATION && !is_static && name == "apply" && args.len() == 2 {
        if rt.str_of(rt.get_field(args[0], "$codec_operation")) == Some("list") {
            let max = rt.get_field(args[0], "$codec_max");
            return Some(graph(rt, "list", &[args[1], max]));
        }
        if rt.str_of(rt.get_field(args[0], "$codec_operation")) == Some("collection") {
            let factory = rt.get_field(args[0], "$codec_factory");
            return Some(graph(rt, "collection", &[factory, args[1]]));
        }
        return None;
    }
    if !is_static && owner == STREAM {
        match name {
            "cast" if args.len() == 1 => return Some(args[0]), // generic buffer cast, no transform
            "map" | "dispatch" if args.len() == 3 => return Some(graph(rt, name, args)),
            "mapStream" if args.len() == 2 => return Some(graph(rt, "mapStream", args)),
            "apply" if args.len() == 2 => {
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host,
                    depth: 0,
                };
                return Some(sam(
                    &mut ctx,
                    args[1],
                    OPERATION,
                    "apply",
                    &format!("(L{STREAM};)L{STREAM};"),
                    &[args[0]],
                ));
            }
            _ => {}
        }
    }
    if !is_static
        && matches!(
            owner,
            STREAM
                | "net/minecraft/network/codec/StreamEncoder"
                | "net/minecraft/network/codec/StreamDecoder"
        )
    {
        let mut ctx = ExecCtx {
            rt,
            linked,
            host,
            depth: 0,
        };
        return match name {
            "encode" if args.len() == 3 => Some(encode(&mut ctx, args[0], args[1], args[2], 0)),
            "decode" if args.len() == 2 => Some(decode(&mut ctx, args[0], args[1], 0)),
            _ => None,
        };
    }
    None
}

fn known_signature(member: &MemberRef, is_static: bool) -> bool {
    let function = "Ljava/util/function/Function;";
    let int_function = "Ljava/util/function/IntFunction;";
    let stream = format!("L{STREAM};");
    let codec = "Lcom/mojang/serialization/Codec;";
    let supplier = "Ljava/util/function/Supplier;";
    let key = "Lnet/minecraft/resources/ResourceKey;";
    let desc = member.desc.as_str();
    let returns = |params: &str, result: &str| desc == format!("({params})L{result};");
    match (member.owner.as_str(), member.name.as_str(), is_static) {
        (BYTE_CODECS, "list", true) => returns("", OPERATION) || returns("I", OPERATION),
        (BYTE_CODECS, "idMapper", true) => returns(&format!("{int_function}Ljava/util/function/ToIntFunction;"), STREAM)
            || returns("Lnet/minecraft/core/IdMap;", STREAM),
        (BYTE_CODECS, "stringUtf8" | "byteArray", true) => returns("I", STREAM),
        (BYTE_CODECS, "optional", true) => returns(&stream, STREAM),
        (BYTE_CODECS, "collection", true) => returns(int_function, OPERATION)
            || returns(&format!("{int_function}{stream}"), STREAM)
            || returns(&format!("{int_function}{stream}I"), STREAM),
        (BYTE_CODECS, "map", true) => returns(&format!("{int_function}{stream}{stream}"), STREAM)
            || returns(&format!("{int_function}{stream}{stream}I"), STREAM),
        (BYTE_CODECS, "holderRegistry" | "holderSet" | "registry", true) => returns(key, STREAM),
        (BYTE_CODECS, "holder", true) => returns(&format!("{key}{stream}"), STREAM),
        (BYTE_CODECS, "fromCodecTrusted" | "fromCodecWithRegistriesTrusted", true) => returns(codec, STREAM),
        (BYTE_CODECS, "fromCodec" | "fromCodecWithRegistries", true) => returns(codec, STREAM)
            || returns(&format!("{codec}{supplier}"), STREAM),
        (BYTE_CODECS, "either", true) => returns(&format!("{stream}{stream}"), STREAM),
        (BYTE_CODECS, "tagCodec" | "compoundTagCodec", true) => returns(supplier, STREAM),
        (STREAM, "unit", true) => returns("Ljava/lang/Object;", STREAM),
        (STREAM, "of", true) => returns("Lnet/minecraft/network/codec/StreamEncoder;Lnet/minecraft/network/codec/StreamDecoder;", STREAM),
        (STREAM, "ofMember", true) => returns("Lnet/minecraft/network/codec/StreamMemberEncoder;Lnet/minecraft/network/codec/StreamDecoder;", STREAM),
        (STREAM, "composite", true) => (1..=6).any(|n| {
            let constructor = match n {
                1 => function.to_owned(),
                2 => "Ljava/util/function/BiFunction;".to_owned(),
                _ => format!("Lcom/mojang/datafixers/util/Function{n};"),
            };
            returns(&format!("{}{constructor}", format!("{stream}{function}").repeat(n)), STREAM)
        }),
        (STREAM, "recursive", true) => returns("Ljava/util/function/UnaryOperator;", STREAM),
        (STREAM, "map" | "dispatch", false) => returns(&function.repeat(2), STREAM),
        (STREAM, "mapStream", false) => returns(function, STREAM),
        (STREAM, "cast", false) => returns("", STREAM),
        (STREAM, "apply", false) => returns(&format!("L{OPERATION};"), STREAM),
        (OPERATION, "apply", false) => returns(&stream, STREAM),
        (STREAM | "net/minecraft/network/codec/StreamEncoder", "encode", false) => desc == "(Ljava/lang/Object;Ljava/lang/Object;)V",
        (STREAM | "net/minecraft/network/codec/StreamDecoder", "decode", false) => desc == "(Ljava/lang/Object;)Ljava/lang/Object;",
        _ => false,
    }
}

fn sam(ctx: &mut ExecCtx<'_>, callback: V, owner: &str, name: &str, desc: &str, values: &[V]) -> V {
    let mut args = Vec::with_capacity(values.len() + 1);
    args.push(callback);
    args.extend_from_slice(values);
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(owner, name, desc),
        &args,
    )
}

fn function(ctx: &mut ExecCtx<'_>, callback: V, value: V) -> V {
    sam(
        ctx,
        callback,
        "java/util/function/Function",
        "apply",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        &[value],
    )
}

fn unsupported(rt: &mut Rt, kind: &str, operation: &str) -> V {
    rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("MMGA does not implement StreamCodec {operation} for `{kind}`"),
    )
}

fn depth_ok(rt: &mut Rt, depth: usize) -> bool {
    if depth < 256 {
        true
    } else {
        rt.throw_exception(
            "java/lang/StackOverflowError",
            "StreamCodec nesting exceeds 256",
        );
        false
    }
}

fn append(rt: &mut Rt, buffer: V, value: &[u8]) -> bool {
    if buffer.is_null() {
        rt.throw_exception(
            "java/lang/NullPointerException",
            "StreamCodec write to null ByteBuf",
        );
        return false;
    }
    if let Some(Data::ByteBuf { bytes, .. }) = rt.get_obj_mut(buffer).map(|o| &mut o.data) {
        bytes.extend_from_slice(value);
        true
    } else {
        unsupported(rt, "buffer without ByteBuf storage", "encode");
        false
    }
}

fn read(rt: &mut Rt, buffer: V, count: usize) -> Option<Vec<u8>> {
    if buffer.is_null() {
        rt.throw_exception(
            "java/lang/NullPointerException",
            "StreamCodec read from null ByteBuf",
        );
        return None;
    }
    if let Some(Data::ByteBuf { bytes, reader_idx }) = rt.get_obj_mut(buffer).map(|o| &mut o.data) {
        if count <= bytes.len().saturating_sub(*reader_idx) {
            let out = bytes[*reader_idx..*reader_idx + count].to_vec();
            *reader_idx += count;
            return Some(out);
        }
        rt.throw_exception(
            "java/lang/IndexOutOfBoundsException",
            "StreamCodec read beyond available bytes",
        );
    } else {
        unsupported(rt, "buffer without ByteBuf storage", "decode");
    }
    None
}

fn write_var(rt: &mut Rt, buffer: V, mut value: u64) -> bool {
    let mut bytes = Vec::with_capacity(10);
    loop {
        let byte = (value & 127) as u8;
        value >>= 7;
        bytes.push(if value == 0 { byte } else { byte | 128 });
        if value == 0 {
            break;
        }
    }
    append(rt, buffer, &bytes)
}

fn read_var(rt: &mut Rt, buffer: V, max_bytes: usize) -> Option<u64> {
    let mut value = 0u64;
    for index in 0..=max_bytes {
        let byte = read(rt, buffer, 1)?[0];
        if index == max_bytes {
            rt.throw_exception(
                "java/lang/RuntimeException",
                if max_bytes == 5 {
                    "VarInt too big"
                } else {
                    "VarLong too big"
                },
            );
            return None;
        }
        value |= u64::from(byte & 127).wrapping_shl((index * 7) as u32);
        if byte & 128 == 0 {
            return Some(value);
        }
    }
    unreachable!("loop returns after the first overlong byte")
}

fn numeric(rt: &mut Rt, value: V, expected: &str) -> Option<V> {
    if value.is_null() {
        rt.throw_exception(
            "java/lang/NullPointerException",
            "StreamCodec cannot unbox null",
        );
        return None;
    }
    let class = match expected {
        "BOOL" => "java/lang/Boolean",
        "BYTE" => "java/lang/Byte",
        "SHORT" => "java/lang/Short",
        "LONG" | "VAR_LONG" => "java/lang/Long",
        "FLOAT" => "java/lang/Float",
        "DOUBLE" => "java/lang/Double",
        _ => "java/lang/Integer",
    };
    if matches!(value, V::R(_)) && rt.class_of(value) != Some(class) {
        rt.throw_exception(
            "java/lang/ClassCastException",
            &format!("{expected} StreamCodec requires {class}"),
        );
        return None;
    }
    let raw = if matches!(value, V::R(_)) {
        rt.get_field(value, "$value")
    } else {
        value
    };
    let valid = match expected {
        "LONG" | "VAR_LONG" => matches!(raw, V::J(_)),
        "FLOAT" => matches!(raw, V::F(_)),
        "DOUBLE" => matches!(raw, V::D(_)),
        _ => matches!(raw, V::I(_)),
    };
    if valid {
        Some(raw)
    } else {
        rt.throw_exception(
            "java/lang/ClassCastException",
            &format!("invalid value for {expected} StreamCodec"),
        );
        None
    }
}

fn write_length(rt: &mut Rt, buffer: V, length: usize, max: i32) -> bool {
    if max < 0 || length > max as usize {
        rt.throw_exception(ENCODER, &format!("Size {length} exceeds maximum {max}"));
        return false;
    }
    write_var(rt, buffer, length as u64)
}

fn read_length(rt: &mut Rt, buffer: V, max: i32) -> Option<usize> {
    let length = read_var(rt, buffer, 5)? as i32;
    if length < 0 || length > max {
        rt.throw_exception(DECODER, &format!("Invalid size {length}; maximum {max}"));
        return None;
    }
    Some(length as usize)
}

fn recursive_target(ctx: &mut ExecCtx<'_>, codec: V, callback: V) -> V {
    let cached = ctx.rt.get_field(codec, "$codec_resolved");
    if ctx.rt.get_field(codec, "$codec_initialized") == V::TRUE {
        return cached;
    }
    let target = sam(
        ctx,
        callback,
        "java/util/function/UnaryOperator",
        "apply",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        &[codec],
    );
    if ctx.rt.pending_exception.is_none() {
        ctx.rt.put_field(codec, "$codec_resolved", target);
        ctx.rt.put_field(codec, "$codec_initialized", V::TRUE);
    }
    target
}

fn encode(ctx: &mut ExecCtx<'_>, codec: V, buffer: V, value: V, depth: usize) -> V {
    if codec.is_null() {
        return ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "encode on null StreamCodec",
        );
    }
    if !depth_ok(ctx.rt, depth) {
        return V::NULL;
    }
    let Some((kind, args)) = graph_args(ctx.rt, codec) else {
        // A mod may supply its own implementation, so use regular virtual dispatch.
        if ctx.rt.class_of(codec).is_some_and(|class| {
            ctx.linked
                .resolve_method(class, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V")
                .is_some()
        }) {
            return sam(
                ctx,
                codec,
                "net/minecraft/network/codec/StreamEncoder",
                "encode",
                "(Ljava/lang/Object;Ljava/lang/Object;)V",
                &[buffer, value],
            );
        }
        return unsupported(ctx.rt, "unknown graph", "encode");
    };
    match kind.as_str() {
        "BOOL" | "BYTE" | "SHORT" | "UNSIGNED_SHORT" | "INT" | "VAR_INT" | "LONG" | "VAR_LONG"
        | "FLOAT" | "DOUBLE" => {
            let Some(raw) = numeric(ctx.rt, value, &kind) else {
                return V::NULL;
            };
            match kind.as_str() {
                "BOOL" => {
                    append(ctx.rt, buffer, &[u8::from(raw.i() != 0)]);
                }
                "BYTE" => {
                    append(ctx.rt, buffer, &[raw.i() as u8]);
                }
                "SHORT" | "UNSIGNED_SHORT" => {
                    append(ctx.rt, buffer, &(raw.i() as u16).to_be_bytes());
                }
                "INT" => {
                    append(ctx.rt, buffer, &raw.i().to_be_bytes());
                }
                "VAR_INT" => {
                    write_var(ctx.rt, buffer, raw.i() as u32 as u64);
                }
                "LONG" => {
                    append(ctx.rt, buffer, &raw.j().to_be_bytes());
                }
                "VAR_LONG" => {
                    write_var(ctx.rt, buffer, raw.j() as u64);
                }
                "FLOAT" => {
                    append(ctx.rt, buffer, &raw.f().to_be_bytes());
                }
                "DOUBLE" => {
                    append(ctx.rt, buffer, &raw.d().to_be_bytes());
                }
                _ => unreachable!(),
            }
        }
        "stringUtf8" => {
            if value.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "stringUtf8 cannot encode null",
                );
            }
            if ctx.rt.class_of(value) != Some("java/lang/String") {
                return ctx
                    .rt
                    .throw_exception("java/lang/ClassCastException", "stringUtf8 requires String");
            }
            let Some(text) = ctx.rt.str_of(value).map(str::to_owned) else {
                return ctx
                    .rt
                    .throw_exception("java/lang/ClassCastException", "stringUtf8 requires String");
            };
            let max = args[0].i();
            if max < 0 || text.encode_utf16().count() > max as usize {
                return ctx
                    .rt
                    .throw_exception(ENCODER, "String exceeds maximum UTF-16 length");
            }
            if write_length(ctx.rt, buffer, text.len(), max.saturating_mul(3)) {
                append(ctx.rt, buffer, text.as_bytes());
            }
        }
        "byteArray" | "byteArrayUnbounded" => {
            if value.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "byteArray cannot encode null",
                );
            }
            if ctx.rt.class_of(value) != Some("[B") {
                return ctx
                    .rt
                    .throw_exception("java/lang/ClassCastException", "byteArray requires byte[]");
            }
            let Some(Data::Array(values)) = ctx.rt.get_obj(value).map(|o| &o.data) else {
                return ctx
                    .rt
                    .throw_exception("java/lang/ClassCastException", "byteArray requires byte[]");
            };
            let bytes: Vec<_> = values.iter().map(|v| v.i() as u8).collect();
            let max = args.first().copied().unwrap_or(V::I(i32::MAX)).i();
            if write_length(ctx.rt, buffer, bytes.len(), max) {
                append(ctx.rt, buffer, &bytes);
            }
        }
        "unit" => {
            let equals = invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                &[value, args[0]],
            );
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            if equals.i() == 0 {
                return ctx.rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "StreamCodec.unit received a different value",
                );
            }
        }
        "map" => {
            let mapped = function(ctx, args[2], value);
            if ctx.rt.pending_exception.is_none() {
                encode(ctx, args[0], buffer, mapped, depth + 1);
            }
        }
        "mapStream" => {
            let mapped = function(ctx, args[1], buffer);
            if ctx.rt.pending_exception.is_none() {
                encode(ctx, args[0], mapped, value, depth + 1);
            }
        }
        "idMapper" => {
            let id = sam(
                ctx,
                args[1],
                "java/util/function/ToIntFunction",
                "applyAsInt",
                "(Ljava/lang/Object;)I",
                &[value],
            );
            if ctx.rt.pending_exception.is_none() {
                write_var(ctx.rt, buffer, id.i() as u32 as u64);
            }
        }
        "idMap" => {
            let id = sam(
                ctx,
                args[0],
                "net/minecraft/core/IdMap",
                "getIdOrThrow",
                "(Ljava/lang/Object;)I",
                &[value],
            );
            if ctx.rt.pending_exception.is_none() {
                write_var(ctx.rt, buffer, id.i() as u32 as u64);
            }
        }
        "optional" => {
            if value.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "optional codec cannot encode null",
                );
            }
            let Some(Data::Optional(inner)) = ctx.rt.get_obj(value).map(|o| &o.data) else {
                return ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "optional codec requires Optional",
                );
            };
            let inner = *inner;
            if append(ctx.rt, buffer, &[u8::from(inner.is_some())]) {
                if let Some(inner) = inner {
                    encode(ctx, args[0], buffer, inner, depth + 1);
                }
            }
        }
        "list" | "collection" => {
            if value.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "collection codec cannot encode null",
                );
            }
            let Some(values) = ctx.rt.get_obj(value).and_then(|o| match &o.data {
                Data::List(values) | Data::Set(values) => Some(values.clone()),
                _ => None,
            }) else {
                return unsupported(ctx.rt, "collection without native storage", "encode");
            };
            let (element, max) = if kind == "list" {
                (args[0], args[1].i())
            } else {
                (args[1], args.get(2).copied().unwrap_or(V::I(i32::MAX)).i())
            };
            if write_length(ctx.rt, buffer, values.len(), max) {
                for item in values {
                    encode(ctx, element, buffer, item, depth + 1);
                    if ctx.rt.pending_exception.is_some() {
                        break;
                    }
                }
            }
        }
        "mapEntries" => {
            if value.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "map codec cannot encode null",
                );
            }
            let Some(Data::Map(entries)) = ctx.rt.get_obj(value).map(|o| &o.data) else {
                return unsupported(ctx.rt, "map without native storage", "encode");
            };
            let entries = entries.clone();
            if write_length(
                ctx.rt,
                buffer,
                entries.len(),
                args.get(3).copied().unwrap_or(V::I(i32::MAX)).i(),
            ) {
                for (key, val) in entries {
                    encode(ctx, args[1], buffer, key, depth + 1);
                    if ctx.rt.pending_exception.is_some() {
                        break;
                    }
                    encode(ctx, args[2], buffer, val, depth + 1);
                    if ctx.rt.pending_exception.is_some() {
                        break;
                    }
                }
            }
        }
        "composite" => {
            for field in args[..args.len() - 1].chunks_exact(2) {
                let part = function(ctx, field[1], value);
                if ctx.rt.pending_exception.is_some() {
                    break;
                }
                encode(ctx, field[0], buffer, part, depth + 1);
                if ctx.rt.pending_exception.is_some() {
                    break;
                }
            }
        }
        "dispatch" => {
            let key = function(ctx, args[1], value);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            let selected = function(ctx, args[2], key);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            encode(ctx, args[0], buffer, key, depth + 1);
            if ctx.rt.pending_exception.is_none() {
                encode(ctx, selected, buffer, value, depth + 1);
            }
        }
        "of" => {
            sam(
                ctx,
                args[0],
                "net/minecraft/network/codec/StreamEncoder",
                "encode",
                "(Ljava/lang/Object;Ljava/lang/Object;)V",
                &[buffer, value],
            );
        }
        "ofMember" => {
            sam(
                ctx,
                args[0],
                "net/minecraft/network/codec/StreamMemberEncoder",
                "encode",
                "(Ljava/lang/Object;Ljava/lang/Object;)V",
                &[value, buffer],
            );
        }
        "recursive" => {
            let target = recursive_target(ctx, codec, args[0]);
            if ctx.rt.pending_exception.is_none() {
                encode(ctx, target, buffer, value, depth + 1);
            }
        }
        _ => {
            return unsupported(ctx.rt, &kind, "encode");
        }
    }
    if ctx.rt.pending_exception.is_some() {
        V::NULL
    } else {
        V::Void
    }
}

fn decode(ctx: &mut ExecCtx<'_>, codec: V, buffer: V, depth: usize) -> V {
    if codec.is_null() {
        return ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "decode on null StreamCodec",
        );
    }
    if !depth_ok(ctx.rt, depth) {
        return V::NULL;
    }
    let Some((kind, args)) = graph_args(ctx.rt, codec) else {
        if ctx.rt.class_of(codec).is_some_and(|class| {
            ctx.linked
                .resolve_method(class, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;")
                .is_some()
        }) {
            return sam(
                ctx,
                codec,
                "net/minecraft/network/codec/StreamDecoder",
                "decode",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[buffer],
            );
        }
        return unsupported(ctx.rt, "unknown graph", "decode");
    };
    let result = match kind.as_str() {
        "BOOL" | "BYTE" | "SHORT" | "UNSIGNED_SHORT" | "INT" | "LONG" | "FLOAT" | "DOUBLE" => {
            let length = match kind.as_str() {
                "BOOL" | "BYTE" => 1,
                "SHORT" | "UNSIGNED_SHORT" => 2,
                "INT" | "FLOAT" => 4,
                _ => 8,
            };
            let Some(bytes) = read(ctx.rt, buffer, length) else {
                return V::NULL;
            };
            let (class, raw) = match kind.as_str() {
                "BOOL" => ("java/lang/Boolean", V::bool(bytes[0] != 0)),
                "BYTE" => ("java/lang/Byte", V::I(bytes[0] as i8 as i32)),
                "SHORT" => (
                    "java/lang/Short",
                    V::I(i16::from_be_bytes(bytes.try_into().unwrap()) as i32),
                ),
                "UNSIGNED_SHORT" => (
                    "java/lang/Integer",
                    V::I(u16::from_be_bytes(bytes.try_into().unwrap()) as i32),
                ),
                "INT" => (
                    "java/lang/Integer",
                    V::I(i32::from_be_bytes(bytes.try_into().unwrap())),
                ),
                "LONG" => (
                    "java/lang/Long",
                    V::J(i64::from_be_bytes(bytes.try_into().unwrap())),
                ),
                "FLOAT" => (
                    "java/lang/Float",
                    V::F(f32::from_be_bytes(bytes.try_into().unwrap())),
                ),
                _ => (
                    "java/lang/Double",
                    V::D(f64::from_be_bytes(bytes.try_into().unwrap())),
                ),
            };
            box_primitive(ctx.rt, class, raw)
        }
        "VAR_INT" | "VAR_LONG" => {
            let Some(raw) = read_var(ctx.rt, buffer, if kind == "VAR_INT" { 5 } else { 10 }) else {
                return V::NULL;
            };
            if kind == "VAR_INT" {
                box_primitive(ctx.rt, "java/lang/Integer", V::I(raw as i32))
            } else {
                box_primitive(ctx.rt, "java/lang/Long", V::J(raw as i64))
            }
        }
        "stringUtf8" => {
            let max = args[0].i();
            let Some(length) = read_length(ctx.rt, buffer, max.saturating_mul(3)) else {
                return V::NULL;
            };
            let Some(bytes) = read(ctx.rt, buffer, length) else {
                return V::NULL;
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            if max < 0 || text.encode_utf16().count() > max as usize {
                return ctx
                    .rt
                    .throw_exception(DECODER, "String exceeds maximum UTF-16 length");
            }
            ctx.rt.new_str(text)
        }
        "byteArray" | "byteArrayUnbounded" => {
            if buffer.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "byteArray cannot read null ByteBuf",
                );
            }
            let max = if kind == "byteArrayUnbounded" {
                match ctx.rt.get_obj(buffer).map(|o| &o.data) {
                    Some(Data::ByteBuf { bytes, reader_idx }) => bytes
                        .len()
                        .saturating_sub(*reader_idx)
                        .min(i32::MAX as usize)
                        as i32,
                    _ => return unsupported(ctx.rt, "buffer without ByteBuf storage", "decode"),
                }
            } else {
                args[0].i()
            };
            let Some(length) = read_length(ctx.rt, buffer, max) else {
                return V::NULL;
            };
            let Some(bytes) = read(ctx.rt, buffer, length) else {
                return V::NULL;
            };
            ctx.rt.alloc(
                "[B",
                0,
                Data::Array(bytes.into_iter().map(|b| V::I(b as i8 as i32)).collect()),
            )
        }
        "unit" => args[0],
        "map" => {
            let value = decode(ctx, args[0], buffer, depth + 1);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            function(ctx, args[1], value)
        }
        "mapStream" => {
            let mapped = function(ctx, args[1], buffer);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            decode(ctx, args[0], mapped, depth + 1)
        }
        "idMapper" => {
            let Some(id) = read_var(ctx.rt, buffer, 5) else {
                return V::NULL;
            };
            sam(
                ctx,
                args[0],
                "java/util/function/IntFunction",
                "apply",
                "(I)Ljava/lang/Object;",
                &[V::I(id as i32)],
            )
        }
        "idMap" => {
            let Some(id) = read_var(ctx.rt, buffer, 5) else {
                return V::NULL;
            };
            sam(
                ctx,
                args[0],
                "net/minecraft/core/IdMap",
                "byIdOrThrow",
                "(I)Ljava/lang/Object;",
                &[V::I(id as i32)],
            )
        }
        "optional" => {
            let Some(flag) = read(ctx.rt, buffer, 1) else {
                return V::NULL;
            };
            let inner = if flag[0] != 0 {
                let value = decode(ctx, args[0], buffer, depth + 1);
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
                if value.is_null() {
                    return ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "optional codec decoded null",
                    );
                }
                Some(value)
            } else {
                None
            };
            ctx.rt.alloc("java/util/Optional", 0, Data::Optional(inner))
        }
        "list" | "collection" => {
            let (element, max) = if kind == "list" {
                (args[0], args[1].i())
            } else {
                (args[1], args.get(2).copied().unwrap_or(V::I(i32::MAX)).i())
            };
            let Some(length) = read_length(ctx.rt, buffer, max) else {
                return V::NULL;
            };
            let container = if kind == "list" {
                ctx.rt
                    .alloc("java/util/ArrayList", 0, Data::List(Vec::new()))
            } else {
                sam(
                    ctx,
                    args[0],
                    "java/util/function/IntFunction",
                    "apply",
                    "(I)Ljava/lang/Object;",
                    &[V::I(length.min(65536) as i32)],
                )
            };
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            for _ in 0..length {
                let item = decode(ctx, element, buffer, depth + 1);
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
                sam(
                    ctx,
                    container,
                    "java/util/Collection",
                    "add",
                    "(Ljava/lang/Object;)Z",
                    &[item],
                );
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
            }
            container
        }
        "mapEntries" => {
            let Some(length) = read_length(
                ctx.rt,
                buffer,
                args.get(3).copied().unwrap_or(V::I(i32::MAX)).i(),
            ) else {
                return V::NULL;
            };
            let map = sam(
                ctx,
                args[0],
                "java/util/function/IntFunction",
                "apply",
                "(I)Ljava/lang/Object;",
                &[V::I(length.min(65536) as i32)],
            );
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            for _ in 0..length {
                let key = decode(ctx, args[1], buffer, depth + 1);
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
                let val = decode(ctx, args[2], buffer, depth + 1);
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
                sam(
                    ctx,
                    map,
                    "java/util/Map",
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                    &[key, val],
                );
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
            }
            map
        }
        "composite" => {
            let mut values = Vec::new();
            for field in args[..args.len() - 1].chunks_exact(2) {
                let value = decode(ctx, field[0], buffer, depth + 1);
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
                values.push(value);
            }
            let owner = match values.len() {
                1 => "java/util/function/Function".to_owned(),
                2 => "java/util/function/BiFunction".to_owned(),
                n => format!("com/mojang/datafixers/util/Function{n}"),
            };
            let desc = format!(
                "({})Ljava/lang/Object;",
                "Ljava/lang/Object;".repeat(values.len())
            );
            sam(ctx, *args.last().unwrap(), &owner, "apply", &desc, &values)
        }
        "dispatch" => {
            let key = decode(ctx, args[0], buffer, depth + 1);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            let selected = function(ctx, args[2], key);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            decode(ctx, selected, buffer, depth + 1)
        }
        "of" | "ofMember" => sam(
            ctx,
            args[1],
            "net/minecraft/network/codec/StreamDecoder",
            "decode",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[buffer],
        ),
        "recursive" => {
            let target = recursive_target(ctx, codec, args[0]);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            decode(ctx, target, buffer, depth + 1)
        }
        _ => unsupported(ctx.rt, &kind, "decode"),
    };
    if ctx.rt.pending_exception.is_some() {
        V::NULL
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{
        unbox_primitive, ClosureData, ClosureTarget, NativeLambdaFn, NativeModuleRegistry, NullHost,
    };

    fn closure(rt: &mut Rt, id: u32, captures: Vec<V>) -> V {
        rt.alloc(
            "java/util/function/Function",
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: "java/util/function/Function".into(),
                sam_name: "apply".into(),
                sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                instantiated_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                captures,
                target: ClosureTarget::NativeLambda(id),
            })),
        )
    }

    fn plus_ten(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let n = unbox_primitive(ctx.rt, args[0]).i() + 10;
        box_primitive(ctx.rt, "java/lang/Integer", V::I(n))
    }
    fn minus_ten(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let n = unbox_primitive(ctx.rt, args[0]).i() - 10;
        box_primitive(ctx.rt, "java/lang/Integer", V::I(n))
    }
    fn first(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.get_field(args[0], "first")
    }
    fn second(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.get_field(args[0], "second")
    }
    fn pair(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let pair = ctx.rt.new_instance("example/Pair", 0);
        ctx.rt.put_field(pair, "first", args[0]);
        ctx.rt.put_field(pair, "second", args[1]);
        pair
    }
    fn fail(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("example/MapperFailure", "mapper failure")
    }
    fn null_factory(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "calls").i();
        ctx.rt.put_field(args[0], "calls", V::I(calls + 1));
        V::NULL
    }

    static LAMBDAS: &[(u32, NativeLambdaFn)] = &[
        (1, plus_ten),
        (2, minus_ten),
        (3, first),
        (4, second),
        (5, pair),
        (6, fail),
        (7, null_factory),
    ];

    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[],
            lambdas: LAMBDAS,
        });
        linked
    }

    fn buffer(rt: &mut Rt, bytes: Vec<u8>) -> V {
        rt.alloc(
            "io/netty/buffer/ByteBuf",
            0,
            Data::ByteBuf {
                bytes,
                reader_idx: 0,
            },
        )
    }

    fn bytes(rt: &Rt, buf: V) -> Vec<u8> {
        match &rt.get_obj(buf).unwrap().data {
            Data::ByteBuf { bytes, .. } => bytes.clone(),
            _ => panic!("not a buffer"),
        }
    }

    #[test]
    fn mapped_composite_encodes_fields_and_runs_decode_constructor() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let linked = linked();
        let mut rt = Rt::new();
        let varint = get_static_field(
            &mut rt,
            BYTE_CODECS,
            "VAR_INT",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let string = get_static_field(
            &mut rt,
            BYTE_CODECS,
            "STRING_UTF8",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let forward = closure(&mut rt, 1, vec![]);
        let backward = closure(&mut rt, 2, vec![]);
        let mapped = call(&mut rt, &linked, &mut prepared.loader, &MemberRef::new(STREAM, "map", "(Ljava/util/function/Function;Ljava/util/function/Function;)Lnet/minecraft/network/codec/StreamCodec;"), false, &[varint, forward, backward]).unwrap();
        assert_ne!(mapped, varint);
        let first = closure(&mut rt, 3, vec![]);
        let second = closure(&mut rt, 4, vec![]);
        let constructor = closure(&mut rt, 5, vec![]);
        let composite = graph(
            &mut rt,
            "composite",
            &[mapped, first, string, second, constructor],
        );
        let value = rt.new_instance("example/Pair", 0);
        let integer = box_primitive(&mut rt, "java/lang/Integer", V::I(310));
        let text = rt.new_str("齿轮");
        rt.put_field(value, "first", integer);
        rt.put_field(value, "second", text);
        let buf = buffer(&mut rt, vec![]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert_eq!(encode(&mut ctx, composite, buf, value, 0), V::Void);
        assert_eq!(
            bytes(ctx.rt, buf),
            vec![0xac, 0x02, 6, 0xe9, 0xbd, 0xbf, 0xe8, 0xbd, 0xae]
        );
        let decoded = decode(&mut ctx, composite, buf, 0);
        assert_ne!(decoded, value);
        assert_eq!(
            unbox_primitive(ctx.rt, ctx.rt.get_field(decoded, "first")),
            V::I(310)
        );
        assert_eq!(
            ctx.rt.str_of(ctx.rt.get_field(decoded, "second")),
            Some("齿轮")
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn list_operation_preserves_element_codec_and_enforces_limit() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let linked = linked();
        let mut rt = Rt::new();
        let element = get_static_field(
            &mut rt,
            BYTE_CODECS,
            "VAR_INT",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let operation = call(
            &mut rt,
            &linked,
            &mut prepared.loader,
            &MemberRef::new(
                BYTE_CODECS,
                "list",
                "(I)Lnet/minecraft/network/codec/StreamCodec$CodecOperation;",
            ),
            true,
            &[V::I(2)],
        )
        .unwrap();
        let codec = call(&mut rt, &linked, &mut prepared.loader, &MemberRef::new(OPERATION, "apply", "(Lnet/minecraft/network/codec/StreamCodec;)Lnet/minecraft/network/codec/StreamCodec;"), false, &[operation, element]).unwrap();
        let one = box_primitive(&mut rt, "java/lang/Integer", V::I(1));
        let minus_one = box_primitive(&mut rt, "java/lang/Integer", V::I(-1));
        let value = rt.alloc("java/util/ArrayList", 0, Data::List(vec![one, minus_one]));
        let buf = buffer(&mut rt, vec![]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        encode(&mut ctx, codec, buf, value, 0);
        assert_eq!(bytes(ctx.rt, buf), vec![2, 1, 255, 255, 255, 255, 15]);
        let decoded = decode(&mut ctx, codec, buf, 0);
        let Data::List(values) = &ctx.rt.get_obj(decoded).unwrap().data else {
            panic!("not list")
        };
        assert_eq!(
            values
                .iter()
                .map(|v| unbox_primitive(ctx.rt, *v))
                .collect::<Vec<_>>(),
            vec![V::I(1), V::I(-1)]
        );
        let oversized = buffer(ctx.rt, vec![3]);
        assert_eq!(decode(&mut ctx, codec, oversized, 0), V::NULL);
        assert!(ctx
            .rt
            .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
            .contains("maximum 2"));
    }

    #[test]
    fn incomplete_graph_and_failed_mapper_throw_without_fabricated_payload() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let nbt = get_static_field(
            &mut rt,
            BYTE_CODECS,
            "COMPOUND_TAG",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let original = rt.new_instance("example/DfuCodec", 0);
        let from_dfu = call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(
                BYTE_CODECS,
                "fromCodec",
                "(Lcom/mojang/serialization/Codec;)Lnet/minecraft/network/codec/StreamCodec;",
            ),
            true,
            &[original],
        )
        .unwrap();
        assert_eq!(graph_args(&rt, from_dfu).unwrap().1, vec![original]);
        let buf = buffer(&mut rt, vec![]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(encode(&mut ctx, nbt, buf, original, 0), V::NULL);
        assert!(ctx
            .rt
            .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
            .contains("does not implement StreamCodec encode"));
        assert!(bytes(ctx.rt, buf).is_empty());
        ctx.rt.take_exception();
        let base = graph(ctx.rt, "INT", &[]);
        let bad = closure(ctx.rt, 6, vec![]);
        let mapped = graph(ctx.rt, "map", &[base, bad, bad]);
        assert_eq!(encode(&mut ctx, mapped, buf, V::I(1), 0), V::NULL);
        assert!(ctx
            .rt
            .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
            .contains("MapperFailure"));
        assert!(bytes(ctx.rt, buf).is_empty());
    }

    #[test]
    fn malformed_varints_and_truncated_payloads_fail_instead_of_zero() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let varint = graph(&mut rt, "VAR_INT", &[]);
        let invalid = buffer(&mut rt, vec![128; 6]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(decode(&mut ctx, varint, invalid, 0), V::NULL);
        assert!(ctx
            .rt
            .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
            .contains("java.lang.RuntimeException: VarInt too big"));
        let Data::ByteBuf { reader_idx, .. } = &ctx.rt.get_obj(invalid).unwrap().data else {
            unreachable!()
        };
        assert_eq!(*reader_idx, 6);
        ctx.rt.take_exception();
        let int = graph(ctx.rt, "INT", &[]);
        let truncated = buffer(ctx.rt, vec![1, 2, 3]);
        assert_eq!(decode(&mut ctx, int, truncated, 0), V::NULL);
        assert!(ctx
            .rt
            .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
            .contains("IndexOutOfBoundsException"));
    }

    #[test]
    fn codec_constants_have_static_identity_and_wrong_signatures_stay_unhandled() {
        let mut rt = Rt::new();
        let desc = "Lnet/minecraft/network/codec/StreamCodec;";
        let first = get_static_field(&mut rt, BYTE_CODECS, "INT", desc).unwrap();
        assert_eq!(
            get_static_field(&mut rt, BYTE_CODECS, "INT", desc),
            Some(first)
        );
        assert_eq!(get_static_field(&mut rt, BYTE_CODECS, "INT", "I"), None);
        assert_eq!(get_static_field(&mut rt, BYTE_CODECS, "LONG", desc), None);
        assert_eq!(
            get_static_field(&mut rt, BYTE_CODECS, "MADE_UP", desc),
            None
        );
        let mut host = NullHost;
        assert_eq!(
            call(
                &mut rt,
                &linked(),
                &mut host,
                &MemberRef::new(STREAM, "map", "()Lnet/minecraft/network/codec/StreamCodec;"),
                false,
                &[first]
            ),
            None
        );
    }

    #[test]
    fn nulls_and_erased_type_mismatches_follow_java_casts_and_recursive_null_is_cached() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let buf = buffer(&mut rt, vec![]);
        let bool_codec = graph(&mut rt, "BOOL", &[]);
        let byte_array = graph(&mut rt, "byteArray", &[V::I(4)]);
        let wrong_integer = box_primitive(&mut rt, "java/lang/Integer", V::I(1));
        let wrong_array = rt.alloc("[I", 0, Data::Array(vec![V::I(1000)]));
        let calls = rt.new_instance("example/Counter", 0);
        let factory = closure(&mut rt, 7, vec![calls]);
        let recursive = graph(&mut rt, "recursive", &[factory]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (codec, value, expected) in [
            (bool_codec, V::NULL, "NullPointerException"),
            (bool_codec, wrong_integer, "ClassCastException"),
            (byte_array, wrong_array, "ClassCastException"),
        ] {
            assert_eq!(encode(&mut ctx, codec, buf, value, 0), V::NULL);
            assert!(ctx
                .rt
                .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
                .contains(expected));
            ctx.rt.take_exception();
            assert!(bytes(ctx.rt, buf).is_empty());
        }
        for _ in 0..2 {
            assert_eq!(decode(&mut ctx, recursive, buf, 0), V::NULL);
            assert!(ctx
                .rt
                .exception_summary(V::R(ctx.rt.pending_exception.unwrap()))
                .contains("NullPointerException"));
            ctx.rt.take_exception();
        }
        assert_eq!(ctx.rt.get_field(calls, "calls"), V::I(1));
    }
}

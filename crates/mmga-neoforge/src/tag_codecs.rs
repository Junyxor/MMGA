//! Vanilla TagKey codec composition (`awu` in the official 1.21.1 client).
//! Plain tag codecs use ResourceLocation strings; hashed codecs require '#'.
//! These codecs describe tag keys only and do not pretend tag membership exists.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, read_static_field, ClosureData, ClosureTarget, Data, ExecCtx, Rt, V,
};

const TAG: &str = "net/minecraft/tags/TagKey";
const CODEC: &str = "com/mojang/serialization/Codec";
const ADAPTER: &str = "mmga/internal/TagKeyCodec";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static {
        return None;
    }
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if member.owner == TAG {
        if name == "create" && desc == "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;" {
            return create(ctx, *args.first()?, *args.get(1)?);
        }
        if matches!(name, "codec" | "hashedCodec")
            && desc == "(Lnet/minecraft/resources/ResourceKey;)Lcom/mojang/serialization/Codec;"
        {
            let registry = *args.first()?;
            let hashed = name == "hashedCodec";
            let base = if hashed {
                read_static_field(
                    ctx,
                    CODEC,
                    "STRING",
                    "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
                )
            } else {
                read_static_field(
                    ctx,
                    "net/minecraft/resources/ResourceLocation",
                    "CODEC",
                    "Lcom/mojang/serialization/Codec;",
                )
            };
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let decoder = callback(
                ctx.rt,
                if hashed {
                    "decodeHashed"
                } else {
                    "decodePlain"
                },
                vec![registry],
            );
            let encoder = callback(
                ctx.rt,
                if hashed {
                    "encodeHashed"
                } else {
                    "encodePlain"
                },
                vec![],
            );
            return Some(invoke_member(ctx, op::INVOKEINTERFACE, &MemberRef::new(CODEC, if hashed { "comapFlatMap" } else { "xmap" }, "(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;"), &[base, decoder, encoder]));
        }
        return None;
    }
    if member.owner != ADAPTER {
        return None;
    }
    match (name, desc) {
        (
            "decodePlain",
            "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)Ljava/lang/Object;",
        ) => create(ctx, *args.first()?, *args.get(1)?),
        (
            "decodeHashed",
            "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)Ljava/lang/Object;",
        ) => {
            let text = *args.get(1)?;
            let Some(text) = ctx.rt.str_of(text) else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "hashed tag requires String",
                ));
            };
            let Some(location) = text.strip_prefix('#') else {
                return Some(error(ctx, "Not a tag id"));
            };
            let location = location.to_owned();
            let location = ctx.rt.new_str(location);
            // ResourceLocation.CODEC validates the stripped name. Its result is
            // transformed only on success; invalid ids remain DataResult errors.
            let input = ctx.rt.new_instance("com/google/gson/JsonPrimitive", 0);
            invoke_member(
                ctx,
                op::INVOKESPECIAL,
                &MemberRef::new(
                    "com/google/gson/JsonPrimitive",
                    "<init>",
                    "(Ljava/lang/String;)V",
                ),
                &[input, location],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let codec = read_static_field(
                ctx,
                "net/minecraft/resources/ResourceLocation",
                "CODEC",
                "Lcom/mojang/serialization/Codec;",
            );
            let ops = read_static_field(
                ctx,
                "com/mojang/serialization/JsonOps",
                "INSTANCE",
                "Lcom/mojang/serialization/JsonOps;",
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let result = invoke_member(ctx, op::INVOKEINTERFACE, &MemberRef::new(CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"), &[codec, ops, input]);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let mapper = callback(ctx.rt, "decodePlain", vec![args[0]]);
            Some(invoke_member(
                ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(
                    "com/mojang/serialization/DataResult",
                    "map",
                    "(Ljava/util/function/Function;)Lcom/mojang/serialization/DataResult;",
                ),
                &[result, mapper],
            ))
        }
        ("encodePlain" | "encodeHashed", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
            let tag = *args.first()?;
            if tag.is_null() {
                return Some(
                    ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "TagKey location getter",
                    ),
                );
            }
            let Some((_, location)) = tag_parts(ctx.rt, tag) else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "TagKey codec requires TagKey",
                ));
            };
            Some(if name == "encodeHashed" {
                ctx.rt.new_str(format!("#{location}"))
            } else {
                ctx.rt.new_pinned_handle(
                    "net/minecraft/resources/ResourceLocation",
                    "resource_location",
                    0,
                    location,
                )
            })
        }
        ("errorMessage", "(Ljava/lang/String;)Ljava/lang/Object;") => Some(*args.first()?),
        _ => None,
    }
}
fn create(ctx: &mut ExecCtx<'_>, registry: V, location: V) -> Option<V> {
    if registry.is_null() || location.is_null() {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Null-field TagKey records are not implemented",
        ));
    }
    let (owner, registry) = crate::minecraft::resource_key_parts(ctx.rt, registry)?;
    if owner != "minecraft:root" {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "TagKeys with a non-registry ResourceKey are not implemented",
        ));
    }
    let ("resource_location", _, location, _) = ctx.rt.handle_of(location)? else {
        return None;
    };
    Some(intern_key(ctx.rt, &registry, &location.to_owned()))
}

/// Shared construction path for direct calls and known static tag constants.
/// Handles remain pinned in this host, so their canonical cache has the same lifetime.
pub fn intern_key(rt: &mut Rt, registry: &str, location: &str) -> V {
    let key = format!("{registry}|{location}");
    if let Some(tag) = rt.get_static("mmga/tag_keys", &key) {
        return tag;
    }
    let tag = rt.new_pinned_handle(TAG, "tag_key", 0, &key);
    rt.put_static("mmga/tag_keys", &key, tag);
    tag
}
fn callback(rt: &mut Rt, name: &str, captures: Vec<V>) -> V {
    let desc = if captures.is_empty() {
        "(Ljava/lang/Object;)Ljava/lang/Object;"
    } else {
        "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;)Ljava/lang/Object;"
    };
    rt.alloc(
        "java/util/function/Function",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Function".into(),
            sam_name: "apply".into(),
            sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            instantiated_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            captures,
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(ADAPTER, name, desc),
            },
        })),
    )
}
fn error(ctx: &mut ExecCtx<'_>, text: &str) -> V {
    let text = ctx.rt.new_str(text);
    let supplier = ctx.rt.alloc(
        "java/util/function/Supplier",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Supplier".into(),
            sam_name: "get".into(),
            sam_desc: "()Ljava/lang/Object;".into(),
            instantiated_desc: "()Ljava/lang/Object;".into(),
            captures: vec![text],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(
                    ADAPTER,
                    "errorMessage",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                ),
            },
        })),
    );
    invoke_member(
        ctx,
        op::INVOKESTATIC,
        &MemberRef::new(
            "com/mojang/serialization/DataResult",
            "error",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/DataResult;",
        ),
        &[supplier],
    )
}
fn tag_parts(rt: &Rt, tag: V) -> Option<(String, String)> {
    let ("tag_key", _, text, _) = rt.handle_of(tag)? else {
        return None;
    };
    let (registry, location) = text.split_once('|')?;
    Some((registry.into(), location.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryStore;
    use mmga_runtime::{HostApi, LinkedSpace};
    #[derive(Default)]
    struct TagHost(RegistryStore);
    impl HostApi for TagHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            crate::dfu::get_static_field(rt, owner, name, desc)
                .or_else(|| crate::tags::get_static_field(rt, owner, name, desc))
                .or_else(|| crate::vanilla_block_tags::get_static_field(rt, owner, name, desc))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
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
            .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
            .or_else(|| {
                crate::minecraft::call_keys_authlib_and_libs(
                    rt,
                    &mut self.0,
                    member,
                    is_static,
                    args,
                )
            })
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn static_call(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn parse_json(ctx: &mut ExecCtx<'_>, text: &str) -> V {
        let text = ctx.rt.new_str(text);
        static_call(
            ctx,
            "com/google/gson/JsonParser",
            "parseString",
            "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            &[text],
        )
    }
    fn value(ctx: &mut ExecCtx<'_>, result: V) -> V {
        invoke(
            ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            &[result],
        )
    }
    #[test]
    fn plain_tag_codec_round_trips_real_registry_and_uses_the_create_interner() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TagHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:item");
        let codec = static_call(
            &mut ctx,
            TAG,
            "codec",
            "(Lnet/minecraft/resources/ResourceKey;)Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let input = parse_json(&mut ctx, "\"c:ingots/iron\"");
        let first = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
        let tag = value(&mut ctx, first);
        assert_eq!(
            tag_parts(ctx.rt, tag),
            Some(("minecraft:item".into(), "c:ingots/iron".into()))
        );
        let second = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
        assert_eq!(value(&mut ctx, second), tag);
        let location = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "c:ingots/iron",
        );
        assert_eq!(static_call(&mut ctx, TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;", &[registry, location]), tag);
        let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, tag]);
        let encoded = value(&mut ctx, encoded);
        let text = invoke(
            &mut ctx,
            "com/google/gson/JsonElement",
            "toString",
            "()Ljava/lang/String;",
            &[encoded],
        );
        assert_eq!(ctx.rt.str_of(text), Some("\"c:ingots/iron\""));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn hashed_codec_requires_hash_and_plain_codec_rejects_it_without_losing_key_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TagHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:block");
        let plain = static_call(
            &mut ctx,
            TAG,
            "codec",
            "(Lnet/minecraft/resources/ResourceKey;)Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        let hashed = static_call(
            &mut ctx,
            TAG,
            "hashedCodec",
            "(Lnet/minecraft/resources/ResourceKey;)Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        for (codec, text) in [
            (hashed, "\"stone\""),
            (hashed, "\"#UPPERCASE\""),
            (plain, "\"#stone\""),
        ] {
            let input = parse_json(&mut ctx, text);
            let result = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(
                invoke(
                    &mut ctx,
                    "com/mojang/serialization/DataResult",
                    "isError",
                    "()Z",
                    &[result]
                ),
                V::TRUE
            );
        }
        let input = parse_json(&mut ctx, "\"#stone\"");
        let result = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[hashed, ops, input]);
        let tag = value(&mut ctx, result);
        assert_eq!(
            tag_parts(ctx.rt, tag),
            Some(("minecraft:block".into(), "minecraft:stone".into()))
        );
        let result = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[hashed, ops, tag]);
        let encoded = value(&mut ctx, result);
        let text = invoke(
            &mut ctx,
            "com/google/gson/JsonElement",
            "toString",
            "()Ljava/lang/String;",
            &[encoded],
        );
        assert_eq!(ctx.rt.str_of(text), Some("\"#minecraft:stone\""));
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn tag_codec_encoder_extracts_location_without_registry_validation_and_rejects_fake_tags() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TagHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:item");
        let block_registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:block");
        let location = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "test:shared",
        );
        let item_tag = static_call(&mut ctx, TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;", &[registry, location]);
        let block_tag = static_call(&mut ctx, TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;", &[block_registry, location]);
        assert_ne!(item_tag, block_tag);
        let codec = static_call(
            &mut ctx,
            TAG,
            "codec",
            "(Lnet/minecraft/resources/ResourceKey;)Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, block_tag]);
        let encoded = value(&mut ctx, encoded);
        let text = invoke(
            &mut ctx,
            "com/google/gson/JsonElement",
            "toString",
            "()Ljava/lang/String;",
            &[encoded],
        );
        assert_eq!(ctx.rt.str_of(text), Some("\"test:shared\""));
        let fake = ctx.rt.new_str("test:shared");
        invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, fake]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("ClassCastException"));
        invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, V::NULL]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        let wrong = crate::minecraft::new_resource_key(ctx.rt, "minecraft:item", "minecraft:item");
        static_call(&mut ctx, TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;", &[wrong, location]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }

    #[test]
    fn static_tag_constants_and_direct_or_codec_creation_share_canonical_identity_in_both_orders() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TagHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let item_registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:item");
        let block_registry =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:block");
        let iron = read_static_field(
            &mut ctx,
            "net/neoforged/neoforge/common/Tags$Items",
            "INGOTS_IRON",
            "Lnet/minecraft/tags/TagKey;",
        );
        let iron_location = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "c:ingots/iron",
        );
        let direct = static_call(&mut ctx, TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;", &[item_registry, iron_location]);
        assert_eq!(
            iron, direct,
            "static-then-create must use the same interner"
        );
        let pickaxe_location = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "minecraft:mineable/pickaxe",
        );
        let direct = static_call(&mut ctx, TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;", &[block_registry, pickaxe_location]);
        let pickaxe = read_static_field(
            &mut ctx,
            "net/minecraft/tags/BlockTags",
            "MINEABLE_WITH_PICKAXE",
            "Lnet/minecraft/tags/TagKey;",
        );
        assert_eq!(
            direct, pickaxe,
            "create-then-static must use the same interner"
        );
        let codec = static_call(
            &mut ctx,
            TAG,
            "codec",
            "(Lnet/minecraft/resources/ResourceKey;)Lcom/mojang/serialization/Codec;",
            &[item_registry],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let input = parse_json(&mut ctx, "\"c:ingots/iron\"");
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
        assert_eq!(value(&mut ctx, decoded), iron);
        assert!(ctx.rt.pending_exception.is_none());
        let generic = MemberRef::new(TAG, "create", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;");
        let via_existing_host = crate::minecraft::call_keys_authlib_and_libs(
            ctx.rt,
            &mut RegistryStore::new(),
            &generic,
            true,
            &[item_registry, iron_location],
        )
        .unwrap();
        assert_eq!(
            via_existing_host, iron,
            "existing host create must use the shared interner"
        );
    }
}

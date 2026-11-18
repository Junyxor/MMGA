//! Minecraft 1.21.1 SimpleCraftingRecipeSerializer and its category codecs.
//!
//! Verified with Mojang's client mappings (SHA1 2244b6f072256667bcd9a73df124d6c58de77992)
//! and client jar (SHA1 30c73b1c5da787909b2f73340419fdf13b9def88), classes czn/cyo.
//! JSON uses category.fieldOf("category").orElse(MISC); the stream uses the category
//! VarInt ID. Both codec graphs retain factory::create and CraftingRecipe::category.
//! Constructing a serializer never constructs a recipe or evaluates crafting logic.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, read_static_field, ClosureData, ClosureTarget, Data, ExecCtx, Rt, V,
};

const SIMPLE: &str = "net/minecraft/world/item/crafting/SimpleCraftingRecipeSerializer";
const SERIALIZER: &str = "net/minecraft/world/item/crafting/RecipeSerializer";
const FACTORY: &str = "net/minecraft/world/item/crafting/SimpleCraftingRecipeSerializer$Factory";
const CRAFTING: &str = "net/minecraft/world/item/crafting/CraftingRecipe";
const CATEGORY: &str = "net/minecraft/world/item/crafting/CraftingBookCategory";
const CATEGORY_DESC: &str = "Lnet/minecraft/world/item/crafting/CraftingBookCategory;";
#[cfg(test)]
const CODEC: &str = "com/mojang/serialization/Codec";
const MAP: &str = "com/mojang/serialization/MapCodec";
const STREAM: &str = "net/minecraft/network/codec/StreamCodec";
const LOOKUP: &str = "mmga/internal/CraftingBookCategoryLookup";
const ID_GETTER: &str = "mmga/internal/CraftingBookCategoryId";
const BY_ID: &str = "mmga/internal/CraftingBookCategoryById";
const NAMES: [&str; 4] = ["BUILDING", "REDSTONE", "EQUIPMENT", "MISC"];

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == SIMPLE && sup == SERIALIZER)
        || (sub == CATEGORY
            && matches!(
                sup,
                "java/lang/Enum"
                    | "java/lang/Comparable"
                    | "java/io/Serializable"
                    | "net/minecraft/util/StringRepresentable"
            ))
        || (sub == LOOKUP && sup == "java/util/function/Function")
        || (sub == ID_GETTER && sup == "java/util/function/ToIntFunction")
        || (sub == BY_ID && sup == "java/util/function/IntFunction")
}

fn category_values(rt: &mut Rt) -> Vec<V> {
    NAMES
        .iter()
        .enumerate()
        .map(|(id, name)| {
            if let Some(value) = rt.get_static(CATEGORY, name) {
                return value;
            }
            let value = rt.new_instance(CATEGORY, 0);
            let enum_name = rt.intern_str(name);
            let serialized = rt.intern_str(&name.to_ascii_lowercase());
            rt.put_field(value, "$name", enum_name);
            rt.put_field(value, "$ordinal", V::I(id as i32));
            rt.put_field(value, "name", serialized);
            rt.put_field(value, "id", V::I(id as i32));
            rt.put_static(CATEGORY, name, value);
            value
        })
        .collect()
}

/// Exact vanilla constants and executable category codecs, shared by all serializers.
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != CATEGORY {
        return None;
    }
    let expected = if NAMES.contains(&name) {
        CATEGORY_DESC
    } else {
        match name {
            "CODEC" => "Lcom/mojang/serialization/Codec;",
            "STREAM_CODEC" => "Lnet/minecraft/network/codec/StreamCodec;",
            "BY_ID" => "Ljava/util/function/IntFunction;",
            _ => return None,
        }
    };
    if desc != expected {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let values = category_values(rt);
    if let Some(id) = NAMES.iter().position(|n| *n == name) {
        return Some(values[id]);
    }
    let value = match name {
        "CODEC" => {
            let values = rt.alloc(format!("[L{CATEGORY};"), 0, Data::Array(values));
            let lookup = rt.new_instance(LOOKUP, 0);
            let getter = rt.new_instance(ID_GETTER, 0);
            crate::dfu::enum_codec(rt, values, lookup, getter)
        }
        "BY_ID" => rt.new_instance(BY_ID, 0),
        "STREAM_CODEC" => {
            let resolver =
                get_static_field(rt, CATEGORY, "BY_ID", "Ljava/util/function/IntFunction;")?;
            let getter = rt.new_instance(ID_GETTER, 0);
            // ByteBufCodecs.idMapper graph representation, preserving both callbacks.
            let value = rt.new_instance(STREAM, 0);
            let kind = rt.intern_str("idMapper");
            let args = rt.alloc(
                "[Ljava/lang/Object;",
                0,
                Data::Array(vec![resolver, getter]),
            );
            rt.put_field(value, "$codec_kind", kind);
            rt.put_field(value, "$codec_args", args);
            value
        }
        _ => return None,
    };
    rt.put_static(owner, name, value);
    Some(value)
}

fn method_reference(rt: &mut Rt, member: MemberRef, captures: Vec<V>) -> V {
    let instantiated_desc = member.desc.clone();
    rt.alloc(
        "java/util/function/Function",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Function".into(),
            sam_name: "apply".into(),
            sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            instantiated_desc,
            captures,
            target: ClosureTarget::MethodHandle {
                handle_kind: 9,
                member,
            },
        })),
    )
}

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if !is_static
        && owner == SIMPLE
        && name == "<init>"
        && desc == "(Lnet/minecraft/world/item/crafting/SimpleCraftingRecipeSerializer$Factory;)V"
    {
        let recv = *args.first()?;
        let factory = *args.get(1)?;
        let category_codec =
            read_static_field(ctx, CATEGORY, "CODEC", "Lcom/mojang/serialization/Codec;");
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let field = crate::dfu::field_codec(ctx.rt, category_codec, "category");
        let misc = read_static_field(ctx, CATEGORY, "MISC", CATEGORY_DESC);
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let field = invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                MAP,
                "orElse",
                "(Ljava/lang/Object;)Lcom/mojang/serialization/MapCodec;",
            ),
            &[field, misc],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        // The bound method reference requiresNonNull in vanilla's mapCodec builder.
        if factory.is_null() {
            return Some(ctx.rt.throw_exception(
                "java/lang/NullPointerException",
                "SimpleCraftingRecipeSerializer factory",
            ));
        }
        let getter = method_reference(
            ctx.rt,
            MemberRef::new(
                CRAFTING,
                "category",
                "()Lnet/minecraft/world/item/crafting/CraftingBookCategory;",
            ),
            vec![],
        );
        let constructor = method_reference(ctx.rt, MemberRef::new(FACTORY, "create",
            "(Lnet/minecraft/world/item/crafting/CraftingBookCategory;)Lnet/minecraft/world/item/crafting/CraftingRecipe;"), vec![factory]);
        let codec = crate::dfu::record_map_codec(ctx.rt, &[(field, getter)], constructor);
        ctx.rt.put_field(recv, "codec", codec);
        let category_stream = read_static_field(
            ctx,
            CATEGORY,
            "STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let stream = invoke_member(ctx, op::INVOKESTATIC,
            &MemberRef::new(STREAM, "composite", "(Lnet/minecraft/network/codec/StreamCodec;Ljava/util/function/Function;Ljava/util/function/Function;)Lnet/minecraft/network/codec/StreamCodec;"),
            &[category_stream, getter, constructor]);
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        ctx.rt.put_field(recv, "streamCodec", stream);
        return Some(V::Void);
    }
    if !is_static
        && (owner == SERIALIZER || ctx.linked.is_subtype(owner, SIMPLE, ctx.host))
        && ctx
            .rt
            .class_of(*args.first()?)
            .is_some_and(|class| ctx.linked.is_subtype(class, SIMPLE, ctx.host))
    {
        return match (name, desc) {
            ("codec", "()Lcom/mojang/serialization/MapCodec;") => {
                Some(ctx.rt.get_field(args[0], "codec"))
            }
            ("streamCodec", "()Lnet/minecraft/network/codec/StreamCodec;") => {
                Some(ctx.rt.get_field(args[0], "streamCodec"))
            }
            _ => None,
        };
    }
    if owner == CATEGORY && is_static {
        return match (name, desc) {
            ("values", "()[Lnet/minecraft/world/item/crafting/CraftingBookCategory;") => {
                let values = category_values(ctx.rt);
                Some(
                    ctx.rt
                        .alloc(format!("[L{CATEGORY};"), 0, Data::Array(values)),
                )
            }
            (
                "valueOf",
                "(Ljava/lang/String;)Lnet/minecraft/world/item/crafting/CraftingBookCategory;",
            ) => {
                let text = *args.first()?;
                if text.is_null() {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/NullPointerException", "Name is null"),
                    );
                }
                let index = NAMES.iter().position(|n| Some(*n) == ctx.rt.str_of(text));
                Some(match index {
                    Some(id) => category_values(ctx.rt)[id],
                    None => ctx.rt.throw_exception(
                        "java/lang/IllegalArgumentException",
                        "No enum constant CraftingBookCategory",
                    ),
                })
            }
            _ => None,
        };
    }
    if !is_static
        && name == "getSerializedName"
        && desc == "()Ljava/lang/String;"
        && ctx.rt.class_of(*args.first()?) == Some(CATEGORY)
    {
        return Some(ctx.rt.get_field(args[0], "name"));
    }
    if !is_static {
        match (ctx.rt.class_of(*args.first()?), name, desc) {
            (Some(LOOKUP), "apply", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
                let text = ctx.rt.str_of(*args.get(1)?);
                let index = NAMES
                    .iter()
                    .position(|n| Some(n.to_ascii_lowercase().as_str()) == text);
                return Some(
                    index
                        .map(|id| category_values(ctx.rt)[id])
                        .unwrap_or(V::NULL),
                );
            }
            (Some(BY_ID), "apply", "(I)Ljava/lang/Object;") => {
                let id = args.get(1)?.i();
                // ByIdMap.continuous(..., ZERO) maps every out-of-range ID to BUILDING.
                let index = if (0..4).contains(&id) { id as usize } else { 0 };
                return Some(category_values(ctx.rt)[index]);
            }
            (Some(ID_GETTER), "applyAsInt", "(Ljava/lang/Object;)I") => {
                let value = *args.get(1)?;
                if value.is_null() {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "CraftingBookCategory id",
                    ));
                }
                if ctx.rt.class_of(value) != Some(CATEGORY) {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "Category ID requires CraftingBookCategory",
                    ));
                }
                return Some(ctx.rt.get_field(value, "id"));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{HostApi, LinkedSpace, NativeClassDef, NativeModuleRegistry};

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
            let value = super::call(
                &mut ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                },
                member,
                is_static,
                args,
            );
            if value.is_some() {
                return value;
            }
            crate::dfu::call(rt, linked, self, member, is_static, args)
                .or_else(|| crate::codecs::call(rt, linked, self, member, is_static, args))
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup) || crate::dfu::is_subtype(sub, sup)
        }
    }
    fn create(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let factory = args[0];
        let calls = ctx.rt.get_field(factory, "calls").i() + 1;
        ctx.rt.put_field(factory, "calls", V::I(calls));
        if ctx.rt.get_field(factory, "fail") == V::TRUE {
            return ctx
                .rt
                .throw_exception("test/FactoryFailure", "actual factory failed");
        }
        let recipe = ctx.rt.new_instance("test/Recipe", 0);
        ctx.rt.put_field(recipe, "category", args[1]);
        ctx.rt.put_field(recipe, "factory", factory);
        recipe
    }
    fn category(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.get_field(args[0], "category")
    }
    #[test]
    fn serializer_subclass_inherits_both_accessors_with_subclass_or_supertype_owner() {
        let mut rt = Rt::new();
        let mut linked = linked();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/SerializerSubclass",
                superclass: Some(SIMPLE),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            }],
            lambdas: &[],
        });
        let mut host = Host;
        let factory = rt.new_instance("test/Factory", 0);
        let serializer = rt.new_instance("test/SerializerSubclass", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                SIMPLE,
                "<init>",
                "(Lnet/minecraft/world/item/crafting/SimpleCraftingRecipeSerializer$Factory;)V",
            ),
            &[serializer, factory],
        );
        assert!(ctx.rt.pending_exception.is_none());
        for owner in ["test/SerializerSubclass", SIMPLE, SERIALIZER] {
            for (method, desc, field) in [
                ("codec", "()Lcom/mojang/serialization/MapCodec;", "codec"),
                (
                    "streamCodec",
                    "()Lnet/minecraft/network/codec/StreamCodec;",
                    "streamCodec",
                ),
            ] {
                let expected = ctx.rt.get_field(serializer, field);
                assert!(!expected.is_null());
                assert_eq!(
                    invoke(&mut ctx, owner, method, desc, &[serializer]),
                    expected
                );
            }
        }
        assert_eq!(ctx.rt.get_field(factory, "calls").i(), 0);
    }
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry { classes: &[
            NativeClassDef { name: "test/Factory", superclass: Some("java/lang/Object"), interfaces: &[FACTORY], is_interface: false,
                methods: &[("create.(Lnet/minecraft/world/item/crafting/CraftingBookCategory;)Lnet/minecraft/world/item/crafting/CraftingRecipe;", create, false, false)] },
            NativeClassDef { name: "test/Recipe", superclass: Some("java/lang/Object"), interfaces: &[CRAFTING], is_interface: false,
                methods: &[("category.()Lnet/minecraft/world/item/crafting/CraftingBookCategory;", category, false, false)] },
        ], lambdas: &[] });
        linked
    }
    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        let value = invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{}",
            ctx.rt
                .pending_exception
                .map(|id| ctx.rt.exception_summary(V::R(id)))
                .unwrap_or_default()
        );
        value
    }
    fn serializer(ctx: &mut ExecCtx<'_>, factory: V) -> V {
        let serializer = ctx.rt.new_instance(SIMPLE, 0);
        invoke(
            ctx,
            SIMPLE,
            "<init>",
            "(Lnet/minecraft/world/item/crafting/SimpleCraftingRecipeSerializer$Factory;)V",
            &[serializer, factory],
        );
        serializer
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
    fn json(ctx: &mut ExecCtx<'_>, text: &str) -> V {
        let text = ctx.rt.new_str(text);
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "com/google/gson/JsonParser",
                "parseString",
                "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ),
            &[text],
        )
    }
    fn value(ctx: &mut ExecCtx<'_>, result: V) -> V {
        let option = invoke(
            ctx,
            "com/mojang/serialization/DataResult",
            "result",
            "()Ljava/util/Optional;",
            &[result],
        );
        invoke(
            ctx,
            "java/util/Optional",
            "get",
            "()Ljava/lang/Object;",
            &[option],
        )
    }
    fn recipe_codec(ctx: &mut ExecCtx<'_>, serializer: V) -> V {
        let map = invoke(
            ctx,
            SERIALIZER,
            "codec",
            "()Lcom/mojang/serialization/MapCodec;",
            &[serializer],
        );
        invoke(
            ctx,
            MAP,
            "codec",
            "()Lcom/mojang/serialization/Codec;",
            &[map],
        )
    }
    #[test]
    fn preserves_factory_identity_and_json_category_defaults_without_eager_creation() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let factory = rt.new_instance("test/Factory", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let serializer = serializer(&mut ctx, factory);
        assert_eq!(ctx.rt.get_field(factory, "calls"), V::NULL);
        assert!(linked.is_subtype(SIMPLE, SERIALIZER, ctx.host));
        let map = invoke(
            &mut ctx,
            SERIALIZER,
            "codec",
            "()Lcom/mojang/serialization/MapCodec;",
            &[serializer],
        );
        assert_eq!(
            map,
            invoke(
                &mut ctx,
                SIMPLE,
                "codec",
                "()Lcom/mojang/serialization/MapCodec;",
                &[serializer]
            )
        );
        let codec = recipe_codec(&mut ctx, serializer);
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        for (input, expected) in [
            ("{}", "MISC"),
            ("{\"category\":\"redstone\"}", "REDSTONE"),
            ("{\"category\":\"invalid\"}", "MISC"),
            ("{\"category\":42}", "MISC"),
        ] {
            let input = json(&mut ctx, input);
            let parsed = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
            let recipe = value(&mut ctx, parsed);
            let expected = read_static_field(&mut ctx, CATEGORY, expected, CATEGORY_DESC);
            assert_eq!(ctx.rt.get_field(recipe, "category"), expected);
            assert_eq!(ctx.rt.get_field(recipe, "factory"), factory);
            let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, recipe]);
            let encoded = value(&mut ctx, encoded);
            let text = invoke(
                &mut ctx,
                "java/lang/Object",
                "toString",
                "()Ljava/lang/String;",
                &[encoded],
            );
            let expected = ctx.rt.str_of(ctx.rt.get_field(expected, "name")).unwrap();
            assert_eq!(
                ctx.rt.str_of(text),
                Some(format!("{{\"category\":\"{expected}\"}}").as_str())
            );
        }
        assert_eq!(ctx.rt.get_field(factory, "calls"), V::I(4));
    }
    #[test]
    fn stream_uses_real_category_ids_and_factory_and_maps_invalid_ids_to_building() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let factory = rt.new_instance("test/Factory", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let serializer = serializer(&mut ctx, factory);
        let stream = invoke(
            &mut ctx,
            SERIALIZER,
            "streamCodec",
            "()Lnet/minecraft/network/codec/StreamCodec;",
            &[serializer],
        );
        assert_eq!(
            stream,
            invoke(
                &mut ctx,
                SIMPLE,
                "streamCodec",
                "()Lnet/minecraft/network/codec/StreamCodec;",
                &[serializer]
            )
        );
        for id in 0..4 {
            let data = buffer(ctx.rt, vec![id]);
            let recipe = invoke(
                &mut ctx,
                STREAM,
                "decode",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[stream, data],
            );
            let expected = read_static_field(&mut ctx, CATEGORY, NAMES[id as usize], CATEGORY_DESC);
            assert_eq!(ctx.rt.get_field(recipe, "category"), expected);
            assert_eq!(ctx.rt.get_field(recipe, "factory"), factory);
            let encoded = buffer(ctx.rt, vec![]);
            invoke(
                &mut ctx,
                STREAM,
                "encode",
                "(Ljava/lang/Object;Ljava/lang/Object;)V",
                &[stream, encoded, recipe],
            );
            assert!(
                matches!(&ctx.rt.get_obj(encoded).unwrap().data, Data::ByteBuf { bytes, .. } if bytes == &[id])
            );
        }
        for bytes in [vec![4], vec![0xff, 0xff, 0xff, 0xff, 0x0f]] {
            let data = buffer(ctx.rt, bytes);
            let recipe = invoke(
                &mut ctx,
                STREAM,
                "decode",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[stream, data],
            );
            let building = read_static_field(&mut ctx, CATEGORY, "BUILDING", CATEGORY_DESC);
            assert_eq!(ctx.rt.get_field(recipe, "category"), building);
        }
        assert_eq!(ctx.rt.get_field(factory, "calls"), V::I(6));
    }
    #[test]
    fn null_factory_and_real_factory_exceptions_are_preserved() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let factory = rt.new_instance("test/Factory", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let uninitialized = ctx.rt.new_instance(SIMPLE, 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                SIMPLE,
                "<init>",
                "(Lnet/minecraft/world/item/crafting/SimpleCraftingRecipeSerializer$Factory;)V",
            ),
            &[uninitialized, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert_eq!(ctx.rt.get_field(uninitialized, "codec"), V::NULL);
        let serializer = serializer(&mut ctx, factory);
        ctx.rt.put_field(factory, "fail", V::TRUE);
        let stream = invoke(
            &mut ctx,
            SERIALIZER,
            "streamCodec",
            "()Lnet/minecraft/network/codec/StreamCodec;",
            &[serializer],
        );
        let data = buffer(ctx.rt, vec![3]);
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[stream, data],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("test/FactoryFailure"));
        let codec = recipe_codec(&mut ctx, serializer);
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let input = json(&mut ctx, "{}");
        invoke_member(&mut ctx, op::INVOKEINTERFACE, &MemberRef::new(CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"), &[codec, ops, input]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("test/FactoryFailure"));
    }
    #[test]
    fn category_values_are_identity_stable_but_arrays_are_fresh_and_signatures_are_exact() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let method = MemberRef::new(
            CATEGORY,
            "values",
            "()[Lnet/minecraft/world/item/crafting/CraftingBookCategory;",
        );
        let first = invoke_member(&mut ctx, op::INVOKESTATIC, &method, &[]);
        let second = invoke_member(&mut ctx, op::INVOKESTATIC, &method, &[]);
        assert_ne!(first, second);
        for (id, name) in NAMES.iter().enumerate() {
            let constant = read_static_field(&mut ctx, CATEGORY, name, CATEGORY_DESC);
            assert!(
                matches!(&ctx.rt.get_obj(first).unwrap().data, Data::Array(values) if values[id] == constant)
            );
            assert!(
                matches!(&ctx.rt.get_obj(second).unwrap().data, Data::Array(values) if values[id] == constant)
            );
            assert!(linked.is_subtype(CATEGORY, "java/lang/Enum", ctx.host));
        }
        assert!(get_static_field(ctx.rt, CATEGORY, "MISC", "Ljava/lang/Object;").is_none());
        assert!(call(
            &mut ctx,
            &MemberRef::new(SIMPLE, "<init>", "()V"),
            false,
            &[first]
        )
        .is_none());
    }
}

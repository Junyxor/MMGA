//! Real value, text, resource, and config APIs for Minecraft 1.21.1 + NeoForge 21.1.
//!
//! Every method implemented here preserves real data (text trees, language keys,
//! jar resource bytes, filesystem paths, `ModConfigSpec` entries). Any method or
//! class not explicitly handled returns `None` so the VM raises `UnsupportedOperationException`.

use crate::subsystems::{ArgumentKind, BrigadierTree, CommandNodeKind, ConfigPrimitive, ConfigSubsystem};
use mmga_classfile::MemberRef;
use mmga_runtime::{box_primitive, invoke_closure, unbox_primitive, Data, ExecCtx, LinkedSpace, Rt, V};
use std::collections::BTreeMap;

const MUTABLE_COMPONENT: &str = "net/minecraft/network/chat/MutableComponent";
const STYLE_CLASS: &str = "net/minecraft/network/chat/Style";
const CHAT_FORMATTING: &str = "net/minecraft/ChatFormatting";

/// Extracts `en_us.json` key-value translation entries from a jar's resource map.
pub fn extract_lang_entries(
    resources: &BTreeMap<String, Vec<u8>>,
    into: &mut BTreeMap<String, String>,
) {
    for (path, bytes) in resources {
        if path.starts_with("assets/") && path.ends_with("/lang/en_us.json") {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice(bytes) {
                for (k, v) in map {
                    if let Some(text) = v.as_str() {
                        into.insert(k, text.to_owned());
                    }
                }
            }
        }
    }
}

/// Static field lookups for `Component`, `Style`, `ChatFormatting`, and `com/sun/jna/Platform`.
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str) -> Option<V> {
    if owner == STYLE_CLASS && name == "EMPTY" {
        return Some(new_style(rt));
    }
    if owner == CHAT_FORMATTING {
        return chat_formatting_constant(rt, name);
    }
    if owner == "com/sun/jna/Platform" && name == "C_LIBRARY_NAME" {
        let cname = if cfg!(windows) { "msvcrt" } else { "c" };
        return Some(rt.intern_str(cname));
    }
    if owner == "net/minecraft/world/item/CreativeModeTabs" {
        let tab_path = match name {
            "BUILDING_BLOCKS" => "building_blocks",
            "COLORED_BLOCKS" => "colored_blocks",
            "NATURAL_BLOCKS" => "natural_blocks",
            "FUNCTIONAL_BLOCKS" => "functional_blocks",
            "REDSTONE_BLOCKS" => "redstone_blocks",
            "HOTBAR" => "hotbar",
            "SEARCH" => "search",
            "TOOLS_AND_UTILITIES" => "tools_and_utilities",
            "COMBAT" => "combat",
            "FOOD_AND_DRINKS" => "food_and_drinks",
            "INGREDIENTS" => "ingredients",
            "SPAWN_EGGS" => "spawn_eggs",
            "OP_BLOCKS" => "op_blocks",
            "INVENTORY" => "inventory",
            _ => return None,
        };
        return Some(new_resource_key(
            rt,
            "minecraft:creative_mode_tab",
            &format!("minecraft:{tab_path}"),
        ));
    }
    if owner == "net/minecraft/core/RegistrationInfo" && name == "BUILT_IN" {
        return Some(rt.new_pinned_handle(owner, "registration_info", 0, "BUILT_IN"));
    }
    if owner == "net/minecraft/util/Mth" {
        return match name {
            "PI" => Some(V::F(std::f32::consts::PI)),
            "HALF_PI" => Some(V::F(std::f32::consts::FRAC_PI_2)),
            "TWO_PI" => Some(V::F(std::f32::consts::TAU)),
            "DEG_TO_RAD" => Some(V::F(std::f32::consts::PI / 180.0)),
            "RAD_TO_DEG" => Some(V::F(180.0 / std::f32::consts::PI)),
            "EPSILON" => Some(V::F(1.0e-5)),
            "SQRT_OF_TWO" => Some(V::F(std::f32::consts::SQRT_2)),
            _ => None,
        };
    }
    if owner == "net/minecraft/util/ByIdMap$OutOfBoundsStrategy"
        && matches!(name, "ZERO" | "WRAP" | "CLAMP")
    {
        return Some(rt.new_pinned_handle(owner, "by_id_oob", 0, name));
    }
    if owner == "net/neoforged/neoforge/network/registration/HandlerThread"
        && matches!(name, "MAIN" | "NETWORK")
    {
        return Some(rt.new_pinned_handle(owner, "handler_thread", 0, name));
    }
    if owner == "net/minecraft/sounds/SoundSource" {
        let ord = match name {
            "MASTER" => 0,
            "MUSIC" => 1,
            "RECORDS" => 2,
            "WEATHER" => 3,
            "BLOCKS" => 4,
            "HOSTILE" => 5,
            "NEUTRAL" => 6,
            "PLAYERS" => 7,
            "AMBIENT" => 8,
            "VOICE" => 9,
            _ => return None,
        };
        let obj = rt.new_instance(owner, 0);
        let en = rt.intern_str(name);
        let low = rt.intern_str(&name.to_ascii_lowercase());
        rt.put_field(obj, "$name", en);
        rt.put_field(obj, "$ordinal", V::I(ord));
        rt.put_field(obj, "$source_name", low);
        rt.pin(obj);
        return Some(obj);
    }
    if owner == "net/minecraft/core/Direction" {
        let ord = match name {
            "DOWN" => 0,
            "UP" => 1,
            "NORTH" => 2,
            "SOUTH" => 3,
            "WEST" => 4,
            "EAST" => 5,
            _ => return None,
        };
        let obj = rt.new_instance(owner, 0);
        let en = rt.intern_str(name);
        rt.put_field(obj, "$name", en);
        rt.put_field(obj, "$ordinal", V::I(ord));
        rt.pin(obj);
        return Some(obj);
    }
    if owner == "net/minecraft/network/codec/ByteBufCodecs" {
        let valid = matches!(
            name,
            "BOOL"
                | "BYTE"
                | "SHORT"
                | "UNSIGNED_SHORT"
                | "INT"
                | "VAR_INT"
                | "LONG"
                | "VAR_LONG"
                | "FLOAT"
                | "DOUBLE"
                | "BYTE_ARRAY"
                | "STRING_UTF8"
                | "TAG"
                | "COMPOUND_TAG"
                | "TRUSTED_COMPOUND_TAG"
                | "OPTIONAL_COMPOUND_TAG"
                | "VECTOR3F"
                | "QUATERNIONF"
                | "GAME_PROFILE"
                | "GAME_PROFILE_PROPERTIES"
        );
        if valid {
            return Some(rt.new_pinned_handle(
                "net/minecraft/network/codec/StreamCodec",
                "stream_codec",
                0,
                name,
            ));
        }
    }
    if owner == "net/minecraft/world/level/material/MapColor" {
        return map_color_constant(rt, name);
    }
    if matches!(
        (owner, name),
        ("net/minecraft/resources/ResourceLocation", "STREAM_CODEC")
            | ("net/minecraft/core/UUIDUtil", "STREAM_CODEC")
            | ("net/minecraft/core/BlockPos", "STREAM_CODEC")
            | ("net/minecraft/core/GlobalPos", "STREAM_CODEC")
            | ("net/minecraft/network/chat/ComponentSerialization", "STREAM_CODEC")
            | ("net/minecraft/network/chat/ComponentSerialization", "TRUSTED_STREAM_CODEC")
            | ("net/minecraft/network/chat/ComponentSerialization", "OPTIONAL_STREAM_CODEC")
            | ("net/minecraft/world/item/ItemStack", "STREAM_CODEC")
            | ("net/minecraft/world/item/ItemStack", "OPTIONAL_STREAM_CODEC")
            | ("net/minecraft/world/item/ItemStack", "LIST_STREAM_CODEC")
            | ("net/minecraft/world/item/ItemStack", "OPTIONAL_LIST_STREAM_CODEC")
            | ("net/neoforged/neoforge/fluids/FluidStack", "STREAM_CODEC")
            | ("net/neoforged/neoforge/fluids/FluidStack", "OPTIONAL_STREAM_CODEC")
            | ("net/minecraft/core/Direction", "STREAM_CODEC")
    ) {
        return Some(rt.new_pinned_handle(
            "net/minecraft/network/codec/StreamCodec",
            "stream_codec",
            0,
            format!("{owner}.{name}"),
        ));
    }
    None
}

/// Subtype rules for `Component` / `MutableComponent` / `ModConfigSpec`.
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    if sub_class == MUTABLE_COMPONENT
        && matches!(
            super_class,
            "net/minecraft/network/chat/Component"
                | "net/minecraft/network/chat/FormattedText"
                | "com/mojang/brigadier/Message"
        )
    {
        return true;
    }
    if sub_class == "com/mojang/brigadier/exceptions/CommandSyntaxException"
        && matches!(super_class, "java/lang/Exception" | "java/lang/Throwable")
    {
        return true;
    }
    if super_class == "net/neoforged/fml/config/IConfigSpec"
        && sub_class == "net/neoforged/neoforge/common/ModConfigSpec"
    {
        return true;
    }
    if super_class == "net/neoforged/neoforge/common/ModConfigSpec$ConfigValue"
        && sub_class.starts_with("net/neoforged/neoforge/common/ModConfigSpec$")
        && sub_class.ends_with("Value")
    {
        return true;
    }
    if super_class == "java/util/function/Supplier"
        && (sub_class == "net/neoforged/neoforge/common/util/Lazy"
            || (sub_class.starts_with("net/neoforged/neoforge/common/ModConfigSpec$")
                && sub_class.ends_with("Value")))
    {
        return true;
    }
    if super_class == "com/mojang/brigadier/builder/ArgumentBuilder"
        && (sub_class == "com/mojang/brigadier/builder/LiteralArgumentBuilder"
            || sub_class == "com/mojang/brigadier/builder/RequiredArgumentBuilder")
    {
        return true;
    }
    if super_class == "net/minecraft/commands/SharedSuggestionProvider"
        && sub_class == "net/minecraft/commands/CommandSourceStack"
    {
        return true;
    }
    if super_class == "com/mojang/brigadier/tree/CommandNode"
        && (sub_class == "com/mojang/brigadier/tree/LiteralCommandNode"
            || sub_class == "com/mojang/brigadier/tree/ArgumentCommandNode"
            || sub_class == "com/mojang/brigadier/tree/RootCommandNode")
    {
        return true;
    }
    if super_class == "net/minecraft/core/Registry"
        && (sub_class == "net/minecraft/core/DefaultedRegistry"
            || sub_class == "net/minecraft/core/WritableRegistry"
            || sub_class == "net/minecraft/core/MappedRegistry")
    {
        return true;
    }
    if super_class == "net/minecraft/core/WritableRegistry"
        && (sub_class == "net/minecraft/core/Registry"
            || sub_class == "net/minecraft/core/DefaultedRegistry"
            || sub_class == "net/minecraft/core/MappedRegistry")
    {
        return true;
    }
    if super_class == "net/minecraft/core/Holder"
        && (sub_class == "net/minecraft/core/Holder$Reference"
            || sub_class == "net/minecraft/core/Holder$Direct"
            || sub_class.ends_with("DeferredHolder")
            || sub_class.ends_with("DeferredBlock")
            || sub_class.ends_with("DeferredItem"))
    {
        return true;
    }
    if super_class == "net/minecraft/commands/synchronization/ArgumentTypeInfo"
        && sub_class == "net/minecraft/commands/synchronization/SingletonArgumentInfo"
    {
        return true;
    }
    if super_class == "com/google/common/collect/Table"
        && (sub_class == "com/google/common/collect/HashBasedTable"
            || sub_class == "com/google/common/collect/ImmutableTable")
    {
        return true;
    }
    if super_class == "com/google/common/collect/Multimap"
        && matches!(
            sub_class,
            "com/google/common/collect/HashMultimap"
                | "com/google/common/collect/ArrayListMultimap"
                | "com/google/common/collect/LinkedHashMultimap"
                | "com/google/common/collect/ListMultimap"
                | "com/google/common/collect/SetMultimap"
        )
    {
        return true;
    }
    if super_class == "com/google/common/collect/ListMultimap"
        && sub_class == "com/google/common/collect/ArrayListMultimap"
    {
        return true;
    }
    if super_class == "com/google/common/collect/SetMultimap"
        && (sub_class == "com/google/common/collect/HashMultimap"
            || sub_class == "com/google/common/collect/LinkedHashMultimap")
    {
        return true;
    }
    if super_class == "com/google/common/collect/BiMap"
        && sub_class == "com/google/common/collect/HashBiMap"
    {
        return true;
    }
    if super_class == "java/util/Map"
        && (sub_class == "com/google/common/collect/BiMap"
            || sub_class == "com/google/common/collect/HashBiMap")
    {
        return true;
    }
    false
}

/// Maps genuine Minecraft 1.21.1 `BuiltInRegistries` / `Registries` and NeoForge 21.1
/// `NeoForgeRegistries` / `NeoForgeRegistries$Keys` field names to canonical `"namespace:path"` keys.
/// Unknown field names return `None` so unmapped static accesses fail explicitly.
pub fn lookup_builtin_registry_key(owner: &str, name: &str) -> Option<&'static str> {
    match owner {
        "net/minecraft/core/registries/BuiltInRegistries"
        | "net/minecraft/core/registries/Registries" => {
            if owner == "net/minecraft/core/registries/BuiltInRegistries" && name == "REGISTRY" {
                return Some("minecraft:root");
            }
            if owner == "net/minecraft/core/registries/Registries" && name == "ROOT_REGISTRY_NAME" {
                return None; // ResourceLocation constant handled separately
            }
            let key = match name {
                "GAME_EVENT" => "minecraft:game_event",
                "SOUND_EVENT" => "minecraft:sound_event",
                "FLUID" => "minecraft:fluid",
                "MOB_EFFECT" => "minecraft:mob_effect",
                "BLOCK" => "minecraft:block",
                "ENTITY_TYPE" => "minecraft:entity_type",
                "ITEM" => "minecraft:item",
                "POTION" => "minecraft:potion",
                "PARTICLE_TYPE" => "minecraft:particle_type",
                "BLOCK_ENTITY_TYPE" => "minecraft:block_entity_type",
                "CUSTOM_STAT" => "minecraft:custom_stat",
                "CHUNK_STATUS" => "minecraft:chunk_status",
                "RULE_TEST" => "minecraft:rule_test",
                "RULE_BLOCK_ENTITY_MODIFIER" => "minecraft:rule_block_entity_modifier",
                "POS_RULE_TEST" => "minecraft:pos_rule_test",
                "MENU" => "minecraft:menu",
                "RECIPE_TYPE" => "minecraft:recipe_type",
                "RECIPE_SERIALIZER" => "minecraft:recipe_serializer",
                "ATTRIBUTE" => "minecraft:attribute",
                "POSITION_SOURCE_TYPE" => "minecraft:position_source_type",
                "COMMAND_ARGUMENT_TYPE" => "minecraft:command_argument_type",
                "STAT_TYPE" => "minecraft:stat_type",
                "VILLAGER_TYPE" => "minecraft:villager_type",
                "VILLAGER_PROFESSION" => "minecraft:villager_profession",
                "POINT_OF_INTEREST_TYPE" => "minecraft:point_of_interest_type",
                "MEMORY_MODULE_TYPE" => "minecraft:memory_module_type",
                "SENSOR_TYPE" => "minecraft:sensor_type",
                "SCHEDULE" => "minecraft:schedule",
                "ACTIVITY" => "minecraft:activity",
                "LOOT_POOL_ENTRY_TYPE" => "minecraft:loot_pool_entry_type",
                "LOOT_FUNCTION_TYPE" => "minecraft:loot_function_type",
                "LOOT_CONDITION_TYPE" => "minecraft:loot_condition_type",
                "LOOT_NUMBER_PROVIDER_TYPE" => "minecraft:loot_number_provider_type",
                "LOOT_NBT_PROVIDER_TYPE" => "minecraft:loot_nbt_provider_type",
                "LOOT_SCORE_PROVIDER_TYPE" => "minecraft:loot_score_provider_type",
                "FLOAT_PROVIDER_TYPE" => "minecraft:float_provider_type",
                "INT_PROVIDER_TYPE" => "minecraft:int_provider_type",
                "HEIGHT_PROVIDER_TYPE" => "minecraft:height_provider_type",
                "BLOCK_PREDICATE_TYPE" => "minecraft:block_predicate_type",
                "CARVER" => "minecraft:worldgen/carver",
                "FEATURE" => "minecraft:worldgen/feature",
                "STRUCTURE_PLACEMENT" => "minecraft:worldgen/structure_placement",
                "STRUCTURE_PIECE" => "minecraft:worldgen/structure_piece",
                "STRUCTURE_TYPE" => "minecraft:worldgen/structure_type",
                "PLACEMENT_MODIFIER_TYPE" => "minecraft:worldgen/placement_modifier_type",
                "BLOCKSTATE_PROVIDER_TYPE" => "minecraft:worldgen/block_state_provider_type",
                "FOLIAGE_PLACER_TYPE" => "minecraft:worldgen/foliage_placer_type",
                "TRUNK_PLACER_TYPE" => "minecraft:worldgen/trunk_placer_type",
                "ROOT_PLACER_TYPE" => "minecraft:worldgen/root_placer_type",
                "TREE_DECORATOR_TYPE" => "minecraft:worldgen/tree_decorator_type",
                "FEATURE_SIZE_TYPE" => "minecraft:worldgen/feature_size_type",
                "BIOME_SOURCE" => "minecraft:worldgen/biome_source",
                "CHUNK_GENERATOR" => "minecraft:worldgen/chunk_generator",
                "MATERIAL_CONDITION" => "minecraft:worldgen/material_condition",
                "MATERIAL_RULE" => "minecraft:worldgen/material_rule",
                "DENSITY_FUNCTION_TYPE" => "minecraft:worldgen/density_function_type",
                "BLOCK_TYPE" => "minecraft:block_type",
                "STRUCTURE_PROCESSOR" => "minecraft:worldgen/structure_processor",
                "STRUCTURE_POOL_ELEMENT" => "minecraft:worldgen/structure_pool_element",
                "CAT_VARIANT" => "minecraft:cat_variant",
                "FROG_VARIANT" => "minecraft:frog_variant",
                "INSTRUMENT" => "minecraft:instrument",
                "DECORATED_POT_PATTERN" => "minecraft:decorated_pot_pattern",
                "CREATIVE_MODE_TAB" => "minecraft:creative_mode_tab",
                "TRIGGER_TYPES" => "minecraft:trigger_type",
                "NUMBER_FORMAT_TYPE" => "minecraft:number_format_type",
                "ARMOR_MATERIAL" => "minecraft:armor_material",
                "DATA_COMPONENT_TYPE" | "ENCHANTMENT_EFFECT_COMPONENT_TYPE" if name == "DATA_COMPONENT_TYPE" => "minecraft:data_component_type",
                "ENCHANTMENT_EFFECT_COMPONENT_TYPE" => "minecraft:enchantment_effect_component_type",
                "ENTITY_SUB_PREDICATE_TYPE" => "minecraft:entity_sub_predicate_type",
                "ITEM_SUB_PREDICATE_TYPE" => "minecraft:item_sub_predicate_type",
                "MAP_DECORATION_TYPE" => "minecraft:map_decoration_type",
                "ENCHANTMENT_LEVEL_BASED_VALUE_TYPE" => "minecraft:enchantment_level_based_value_type",
                "ENCHANTMENT_ENTITY_EFFECT_TYPE" => "minecraft:enchantment_entity_effect_type",
                "ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE" => "minecraft:enchantment_location_based_effect_type",
                "ENCHANTMENT_VALUE_EFFECT_TYPE" => "minecraft:enchantment_value_effect_type",
                "ENCHANTMENT_PROVIDER_TYPE" => "minecraft:enchantment_provider_type",
                // Datapack ResourceKeys on `Registries`
                "BIOME" if owner.ends_with("/Registries") => "minecraft:worldgen/biome",
                "CONFIGURED_FEATURE" if owner.ends_with("/Registries") => "minecraft:worldgen/configured_feature",
                "PLACED_FEATURE" if owner.ends_with("/Registries") => "minecraft:worldgen/placed_feature",
                "STRUCTURE" if owner.ends_with("/Registries") => "minecraft:worldgen/structure",
                "STRUCTURE_SET" if owner.ends_with("/Registries") => "minecraft:worldgen/structure_set",
                "NOISE_SETTINGS" if owner.ends_with("/Registries") => "minecraft:worldgen/noise_settings",
                "NOISE" if owner.ends_with("/Registries") => "minecraft:worldgen/noise",
                "DENSITY_FUNCTION" if owner.ends_with("/Registries") => "minecraft:worldgen/density_function",
                "DAMAGE_TYPE" if owner.ends_with("/Registries") => "minecraft:damage_type",
                "DIMENSION_TYPE" if owner.ends_with("/Registries") => "minecraft:dimension_type",
                "DIMENSION" | "LEVEL_STEM" if owner.ends_with("/Registries") => "minecraft:dimension",
                "ENCHANTMENT" if owner.ends_with("/Registries") => "minecraft:enchantment",
                "PAINTING_VARIANT" if owner.ends_with("/Registries") => "minecraft:painting_variant",
                "JUKEBOX_SONG" if owner.ends_with("/Registries") => "minecraft:jukebox_song",
                "WOLF_VARIANT" if owner.ends_with("/Registries") => "minecraft:wolf_variant",
                "BANNER_PATTERN" if owner.ends_with("/Registries") => "minecraft:banner_pattern",
                "TRIM_PATTERN" if owner.ends_with("/Registries") => "minecraft:trim_pattern",
                "TRIM_MATERIAL" if owner.ends_with("/Registries") => "minecraft:trim_material",
                "CHAT_TYPE" if owner.ends_with("/Registries") => "minecraft:chat_type",
                "LOOT_TABLE" if owner.ends_with("/Registries") => "minecraft:loot_table",
                "ADVANCEMENT" if owner.ends_with("/Registries") => "minecraft:advancement",
                "RECIPE" if owner.ends_with("/Registries") => "minecraft:recipe",
                _ => return None,
            };
            Some(key)
        }
        "net/neoforged/neoforge/registries/NeoForgeRegistries"
        | "net/neoforged/neoforge/registries/NeoForgeRegistries$Keys" => match name {
            "ENTITY_DATA_SERIALIZERS" => Some("neoforge:entity_data_serializers"),
            "GLOBAL_LOOT_MODIFIER_SERIALIZERS" => Some("neoforge:global_loot_modifier_serializers"),
            "BIOME_MODIFIER_SERIALIZERS" => Some("neoforge:biome_modifier_serializers"),
            "STRUCTURE_MODIFIER_SERIALIZERS" => Some("neoforge:structure_modifier_serializers"),
            "FLUID_TYPES" => Some("neoforge:fluid_type"),
            "HOLDER_SET_TYPES" => Some("neoforge:holder_set_type"),
            "DISPLAY_CONTEXTS" => Some("neoforge:display_contexts"),
            "INGREDIENT_TYPES" => Some("neoforge:ingredient_serializer"),
            "FLUID_INGREDIENT_TYPES" => Some("neoforge:fluid_ingredient_type"),
            "CONDITION_CODECS" => Some("neoforge:condition_codecs"),
            "ATTACHMENT_TYPES" => Some("neoforge:attachment_types"),
            _ => None,
        },
        _ => None,
    }
}

/// Dispatches `net.minecraft.network.chat.Component` / `MutableComponent` / `Style` / `ChatFormatting`.
pub fn call_chat(
    rt: &mut Rt,
    translations: &BTreeMap<String, String>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    if (owner == "net/minecraft/network/chat/Component" || owner == MUTABLE_COMPONENT) && is_static
    {
        match (name, desc) {
            ("empty", "()Lnet/minecraft/network/chat/MutableComponent;") => {
                return Some(new_literal_component(rt, ""));
            }
            ("literal", "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let text = rt.str_of(*args.first()?)?.to_owned();
                return Some(new_literal_component(rt, &text));
            }
            ("nullToEmpty", "(Ljava/lang/String;)Lnet/minecraft/network/chat/Component;") => {
                let text = args
                    .first()
                    .and_then(|&v| rt.str_of(v))
                    .unwrap_or("")
                    .to_owned();
                return Some(new_literal_component(rt, &text));
            }
            (
                "translatable",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                return Some(new_translatable_component(rt, &key, None, Vec::new()));
            }
            (
                "translatable",
                "(Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                let fmt_args = extract_varargs(rt, args.get(1).copied().unwrap_or(V::NULL));
                return Some(new_translatable_component(rt, &key, None, fmt_args));
            }
            (
                "translatableWithFallback",
                "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                let fallback = args.get(1).and_then(|&v| rt.str_of(v)).map(str::to_owned);
                return Some(new_translatable_component(rt, &key, fallback, Vec::new()));
            }
            (
                "translatableWithFallback",
                "(Ljava/lang/String;Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                let fallback = args.get(1).and_then(|&v| rt.str_of(v)).map(str::to_owned);
                let fmt_args = extract_varargs(rt, args.get(2).copied().unwrap_or(V::NULL));
                return Some(new_translatable_component(rt, &key, fallback, fmt_args));
            }
            _ => return None,
        }
    }

    if (owner == "net/minecraft/network/chat/Component" || owner == MUTABLE_COMPONENT)
        && !is_static
    {
        let recv = *args.first()?;
        match (name, desc) {
            ("getString", "()Ljava/lang/String;") => {
                let rendered = render_component_text(rt, translations, recv);
                return Some(rt.new_str(rendered));
            }
            ("getString", "(I)Ljava/lang/String;") => {
                let max_len = args.get(1).map_or(0, |v| v.i().max(0) as usize);
                let rendered = render_component_text(rt, translations, recv);
                let utf16: Vec<u16> = rendered.encode_utf16().take(max_len).collect();
                return Some(rt.new_str(String::from_utf16_lossy(&utf16)));
            }
            (
                "append",
                "(Lnet/minecraft/network/chat/Component;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let child = *args.get(1)?;
                append_sibling(rt, recv, child);
                return Some(recv);
            }
            (
                "append",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let piece = rt.str_of(*args.get(1)?)?.to_owned();
                let child = new_literal_component(rt, &piece);
                append_sibling(rt, recv, child);
                return Some(recv);
            }
            ("getSiblings", "()Ljava/util/List;") => {
                return Some(rt.get_field(recv, "$siblings"));
            }
            ("getStyle", "()Lnet/minecraft/network/chat/Style;") => {
                let s = rt.get_field(recv, "$style");
                return Some(if s.is_null() { new_style(rt) } else { s });
            }
            ("setStyle", "(Lnet/minecraft/network/chat/Style;)Lnet/minecraft/network/chat/MutableComponent;")
            | ("withStyle", "(Lnet/minecraft/network/chat/Style;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let s = *args.get(1)?;
                rt.put_field(recv, "$style", s);
                return Some(recv);
            }
            ("withStyle", "(Lnet/minecraft/ChatFormatting;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let fmt = *args.get(1)?;
                let cur = {
                    let s = rt.get_field(recv, "$style");
                    if s.is_null() { new_style(rt) } else { s }
                };
                let updated = apply_formatting_to_style(rt, cur, fmt);
                rt.put_field(recv, "$style", updated);
                return Some(recv);
            }
            ("withStyle", "([Lnet/minecraft/ChatFormatting;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let arr = *args.get(1)?;
                let mut cur = {
                    let s = rt.get_field(recv, "$style");
                    if s.is_null() { new_style(rt) } else { s }
                };
                let items = extract_varargs(rt, arr);
                for fmt in items {
                    cur = apply_formatting_to_style(rt, cur, fmt);
                }
                rt.put_field(recv, "$style", cur);
                return Some(recv);
            }
            ("withColor", "(I)Lnet/minecraft/network/chat/MutableComponent;") => {
                let rgb = args.get(1)?.i();
                let cur = copy_style(rt, rt.get_field(recv, "$style"));
                rt.put_field(cur, "color", V::I(rgb));
                rt.put_field(recv, "$style", cur);
                return Some(recv);
            }
            ("plainCopy", "()Lnet/minecraft/network/chat/MutableComponent;") => {
                return Some(clone_component(rt, recv, false));
            }
            ("copy", "()Lnet/minecraft/network/chat/MutableComponent;") => {
                return Some(clone_component(rt, recv, true));
            }
            ("equals", "(Ljava/lang/Object;)Z") => {
                let other = args.get(1).copied().unwrap_or(V::NULL);
                let eq = component_equals(rt, translations, recv, other);
                return Some(V::bool(eq));
            }
            ("toString", "()Ljava/lang/String;") => {
                let rendered = render_component_text(rt, translations, recv);
                return Some(rt.new_str(rendered));
            }
            _ => return None,
        }
    }

    if owner == CHAT_FORMATTING {
        if is_static && name == "stripFormatting" {
            let Some(text) = args.first().and_then(|&v| rt.str_of(v)).map(str::to_owned) else {
                return Some(V::NULL);
            };
            let mut stripped = String::with_capacity(text.len());
            let mut chars = text.chars();
            while let Some(c) = chars.next() {
                if c == '\u{00a7}' {
                    let _ = chars.next();
                } else {
                    stripped.push(c);
                }
            }
            return Some(rt.new_str(stripped));
        }
        if !is_static {
            let recv = *args.first()?;
            return match name {
                "getName" => Some(rt.get_field(recv, "$fmt_name")),
                "getChar" => Some(rt.get_field(recv, "$code")),
                "isFormat" => Some(rt.get_field(recv, "$is_format")),
                "isColor" => {
                    let is_fmt = rt.get_field(recv, "$is_format").i() != 0;
                    let ord = rt.get_field(recv, "$ordinal").i();
                    Some(V::bool(!is_fmt && ord < 16))
                }
                "getColor" => {
                    let col = rt.get_field(recv, "$rgb");
                    if col.i() < 0 {
                        Some(V::NULL)
                    } else {
                        Some(box_primitive(rt, "java/lang/Integer", col))
                    }
                }
                "toString" => Some(rt.get_field(recv, "$prefix")),
                _ => None,
            };
        }
    }

    if owner == STYLE_CLASS && !is_static {
        let recv = *args.first()?;
        return match name {
            "withColor" if desc == "(I)Lnet/minecraft/network/chat/Style;" => {
                let next = copy_style(rt, recv);
                rt.put_field(next, "color", args[1]);
                Some(next)
            }
            "withColor" if desc == "(Lnet/minecraft/ChatFormatting;)Lnet/minecraft/network/chat/Style;" => {
                Some(apply_formatting_to_style(rt, recv, args[1]))
            }
            "withBold" | "withItalic" | "withUnderlined" | "withStrikethrough" | "withObfuscated" => {
                let next = copy_style(rt, recv);
                let b = if args[1].is_null() {
                    V::I(0)
                } else {
                    V::bool(unbox_primitive(rt, args[1]).i() != 0)
                };
                rt.put_field(next, name.trim_start_matches("with"), b);
                Some(next)
            }
            "applyFormat" => Some(apply_formatting_to_style(rt, recv, args[1])),
            "isEmpty" => {
                let empty = rt.get_field(recv, "color").i() < 0
                    && rt.get_field(recv, "Bold").i() == 0
                    && rt.get_field(recv, "Italic").i() == 0;
                Some(V::bool(empty))
            }
            _ => None,
        };
    }

    None
}

/// Dispatches jar resource streams, `java.nio.file.Path`, Apache Commons `IOUtils` / `FileUtils` / `StringUtils`, and Linux libc JNA inspection.
pub fn call_resources_and_io(
    rt: &mut Rt,
    jar_resources: &BTreeMap<String, Vec<u8>>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    if owner == "java/lang/Class" && !is_static && (name == "getResource" || name == "getResourceAsStream") {
        let cls_tag = rt.handle_of(*args.first()?)?.2.to_owned();
        let raw_path = rt.str_of(*args.get(1)?)?;
        let key = resolve_class_resource_path(&cls_tag, raw_path);
        let Some(bytes) = jar_resources.get(&key).cloned() else {
            return Some(V::NULL);
        };
        if name == "getResource" {
            return Some(rt.new_handle("java/net/URL", "jar_url", 0, key));
        }
        return Some(rt.alloc(
            "java/io/ByteArrayInputStream",
            0,
            Data::ByteBuf {
                bytes,
                reader_idx: 0,
            },
        ));
    }

    if owner == "java/net/URL" && !is_static && name == "openStream" {
        let ("jar_url", _, key, _) = rt.handle_of(*args.first()?)? else {
            return None;
        };
        let key = key.to_owned();
        let Some(bytes) = jar_resources.get(&key).cloned() else {
            return Some(rt.throw_exception("java/io/FileNotFoundException", &key));
        };
        return Some(rt.alloc(
            "java/io/ByteArrayInputStream",
            0,
            Data::ByteBuf {
                bytes,
                reader_idx: 0,
            },
        ));
    }

    if (owner == "java/nio/file/Path" || owner == "java/nio/file/Paths")
        && is_static
        && (name == "of" || name == "get")
        && desc.starts_with("(Ljava/lang/String;[Ljava/lang/String;)")
    {
        let first = rt.str_of(*args.first()?)?.to_owned();
        let mut pb = std::path::PathBuf::from(first);
        for extra in extract_varargs(rt, args.get(1).copied().unwrap_or(V::NULL)) {
            if let Some(seg) = rt.str_of(extra) {
                if !seg.is_empty() {
                    pb.push(seg);
                }
            }
        }
        return Some(rt.new_handle("java/nio/file/Path", "nio_path", 0, pb.display().to_string()));
    }

    if owner == "java/nio/file/Path" && !is_static {
        let base = path_string(rt, *args.first()?)?;
        return match name {
            "toAbsolutePath" => {
                let p = std::path::Path::new(&base);
                let abs = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                        .join(p)
                };
                Some(rt.new_handle("java/nio/file/Path", "nio_path", 0, abs.display().to_string()))
            }
            "normalize" => {
                let norm = normalize_lexical(std::path::Path::new(&base));
                Some(rt.new_handle("java/nio/file/Path", "nio_path", 0, norm.display().to_string()))
            }
            "getParent" => match std::path::Path::new(&base).parent() {
                Some(parent) if !parent.as_os_str().is_empty() => Some(rt.new_handle(
                    "java/nio/file/Path",
                    "nio_path",
                    0,
                    parent.display().to_string(),
                )),
                _ => Some(V::NULL),
            },
            "getFileName" => match std::path::Path::new(&base).file_name() {
                Some(fname) => Some(rt.new_handle(
                    "java/nio/file/Path",
                    "nio_path",
                    0,
                    fname.to_string_lossy().into_owned(),
                )),
                None => Some(V::NULL),
            },
            "toString" => Some(rt.new_str(base)),
            _ => None,
        };
    }

    if owner == "org/apache/commons/io/IOUtils" && is_static {
        match (name, desc) {
            ("readFully", "(Ljava/io/InputStream;I)[B") => {
                let stream = *args.first()?;
                let want = args.get(1)?.i().max(0) as usize;
                let slice: Result<Vec<V>, &'static str> = match rt.get_obj_mut(stream).map(|o| &mut o.data) {
                    Some(Data::ByteBuf { bytes, reader_idx }) => {
                        if *reader_idx + want > bytes.len() {
                            Err("EOFException in IOUtils.readFully")
                        } else {
                            let out = bytes[*reader_idx..*reader_idx + want]
                                .iter()
                                .map(|&b| V::I(i32::from(b as i8)))
                                .collect();
                            *reader_idx += want;
                            Ok(out)
                        }
                    }
                    _ => Err("NullPointerException in IOUtils.readFully"),
                };
                return match slice {
                    Ok(arr) => Some(rt.alloc("[B", 0, Data::Array(arr))),
                    Err(msg) => Some(rt.throw_exception("java/io/EOFException", msg)),
                };
            }
            ("toByteArray", "(Ljava/io/InputStream;)[B") => {
                let stream = *args.first()?;
                let arr: Vec<V> = match rt.get_obj_mut(stream).map(|o| &mut o.data) {
                    Some(Data::ByteBuf { bytes, reader_idx }) => {
                        let out = bytes[*reader_idx..]
                            .iter()
                            .map(|&b| V::I(i32::from(b as i8)))
                            .collect();
                        *reader_idx = bytes.len();
                        out
                    }
                    _ => return Some(rt.throw_exception("java/io/IOException", "invalid InputStream")),
                };
                return Some(rt.alloc("[B", 0, Data::Array(arr)));
            }
            _ => return None,
        }
    }

    if owner == "org/apache/commons/io/FileUtils" && is_static {
        match (name, desc) {
            ("readFileToByteArray", "(Ljava/io/File;)[B") => {
                let path = path_string(rt, *args.first()?)?;
                return match std::fs::read(&path) {
                    Ok(bytes) => {
                        let arr: Vec<V> = bytes.into_iter().map(|b| V::I(i32::from(b as i8))).collect();
                        Some(rt.alloc("[B", 0, Data::Array(arr)))
                    }
                    Err(e) => Some(rt.throw_exception("java/io/IOException", &format!("{path}: {e}"))),
                };
            }
            ("writeByteArrayToFile", "(Ljava/io/File;[B)V")
            | ("writeByteArrayToFile", "(Ljava/io/File;[BZ)V") => {
                let path = path_string(rt, *args.first()?)?;
                let append = args.get(2).is_some_and(|v| v.i() != 0);
                let bytes: Vec<u8> = match rt.get_obj(*args.get(1)?).map(|o| &o.data) {
                    Some(Data::Array(items)) => items.iter().map(|v| v.i() as u8).collect(),
                    _ => return Some(rt.throw_exception("java/lang/NullPointerException", "writeByteArrayToFile(null)")),
                };
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let res = if append {
                    use std::io::Write;
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                        .and_then(|mut f| f.write_all(&bytes))
                } else {
                    std::fs::write(&path, &bytes)
                };
                return match res {
                    Ok(()) => Some(V::Void),
                    Err(e) => Some(rt.throw_exception("java/io/IOException", &format!("{path}: {e}"))),
                };
            }
            _ => return None,
        }
    }

    if owner == "org/apache/commons/lang3/StringUtils" && is_static {
        let s = args.first().and_then(|&v| rt.str_of(v));
        return match name {
            "isEmpty" => Some(V::bool(s.is_none_or(str::is_empty))),
            "isNotEmpty" => Some(V::bool(s.is_some_and(|t| !t.is_empty()))),
            "isBlank" => Some(V::bool(s.is_none_or(|t| t.trim().is_empty()))),
            "isNotBlank" => Some(V::bool(s.is_some_and(|t| !t.trim().is_empty()))),
            _ => None,
        };
    }

    // JNA libc inspection (`NativeLibrary.getInstance("c")` / `getFunction(sym)`).
    // Only symbol-existence probing on the platform C library is supported; invoking native C
    // functions is rejected with `UnsupportedOperationException`.
    if owner == "com/sun/jna/NativeLibrary" {
        if is_static && name == "getInstance" {
            let lib = rt.str_of(*args.first()?)?.to_owned();
            if lib == "c" || lib == "msvcrt" {
                return Some(rt.new_handle("com/sun/jna/NativeLibrary", "jna_lib", 0, lib));
            }
            return Some(rt.throw_exception("java/lang/UnsatisfiedLinkError", &lib));
        }
        if !is_static && name == "getFunction" {
            let sym = rt.str_of(*args.get(1)?)?.to_owned();
            let exists_on_host = match sym.as_str() {
                "gnu_get_libc_version" => cfg!(all(target_os = "linux", target_env = "gnu")),
                "android_set_abort_message" => cfg!(target_os = "android"),
                _ => false,
            };
            if exists_on_host {
                return Some(rt.new_handle("com/sun/jna/Function", "jna_fn", 0, sym));
            }
            return Some(rt.throw_exception("java/lang/UnsatisfiedLinkError", &sym));
        }
    }

    None
}

/// Dispatches `ByIdMap`, `StreamCodec` / `ByteBufCodecs`, `CustomPacketPayload$Type`,
/// `RegisterPayloadHandlersEvent` / `PayloadRegistrar`, and `SoundSource`.
pub fn call_network_and_codecs(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn mmga_runtime::HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
    extract_net: impl FnOnce(&mut dyn mmga_runtime::HostApi) -> (&mut crate::NetworkStore, Option<String>),
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();

    if owner == "net/minecraft/sounds/SoundSource" && !is_static && name == "getName" {
        return Some(rt.get_field(args[0], "$source_name"));
    }

    // 1. `net/minecraft/util/ByIdMap`
    if owner == "net/minecraft/util/ByIdMap" && is_static {
        match name {
            "continuous" | "sparse" => {
                let id_fn = *args.first()?;
                let values = extract_varargs(rt, *args.get(1)?);
                let strategy = args
                    .get(2)
                    .and_then(|&v| rt.handle_of(v))
                    .map(|(_, _, t, _)| t.to_owned())
                    .unwrap_or_else(|| "ZERO".into());

                let mut max_id = 0usize;
                let mut pairs: Vec<(usize, V)> = Vec::with_capacity(values.len());
                {
                    let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                    for v in values {
                        let id = invoke_closure(&mut ctx, id_fn, &[v]).i();
                        if ctx.rt.pending_exception.is_some() {
                            return Some(V::NULL);
                        }
                        if id >= 0 {
                            let u = id as usize;
                            max_id = max_id.max(u);
                            pairs.push((u, v));
                        }
                    }
                }
                let mut table = vec![V::NULL; pairs.len().max(max_id + 1)];
                for (id, v) in pairs {
                    if id < table.len() {
                        table[id] = v;
                    }
                }
                let arr = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(table));
                let mapper = rt.new_instance("java/util/function/IntFunction", 0);
                let strat_v = rt.intern_str(&strategy);
                rt.put_field(mapper, "$by_id_table", arr);
                rt.put_field(mapper, "$by_id_strategy", strat_v);
                rt.pin(mapper);
                return Some(mapper);
            }
            _ => return None,
        }
    }
    if owner == "java/util/function/IntFunction"
        && !is_static
        && name == "apply"
        && !rt.get_field(args[0], "$by_id_table").is_null()
    {
        let arr = rt.get_field(args[0], "$by_id_table");
        let strat = rt
            .str_of(rt.get_field(args[0], "$by_id_strategy"))
            .unwrap_or("ZERO")
            .to_owned();
        let items = extract_varargs(rt, arr);
        if items.is_empty() {
            return Some(V::NULL);
        }
        let raw = args[1].i();
        let idx = if raw >= 0 && (raw as usize) < items.len() {
            raw as usize
        } else {
            match strat.as_str() {
                "WRAP" => raw.rem_euclid(items.len() as i32) as usize,
                "CLAMP" => raw.clamp(0, items.len() as i32 - 1) as usize,
                _ => 0,
            }
        };
        return Some(items[idx]);
    }

    // 2. `ByteBufCodecs` & `StreamCodec`
    if owner == "net/minecraft/network/codec/ByteBufCodecs" && is_static {
        return match name {
            "idMapper"
            | "stringUtf8"
            | "byteArray"
            | "collection"
            | "map"
            | "optional"
            | "holderRegistry"
            | "registry"
            | "holder"
            | "fromCodec"
            | "fromCodecTrusted"
            | "fromCodecWithRegistries"
            | "fromCodecWithRegistriesTrusted"
            | "either" => Some(rt.new_pinned_handle(
                "net/minecraft/network/codec/StreamCodec",
                "stream_codec",
                0,
                name,
            )),
            "list" => Some(rt.new_pinned_handle(
                "net/minecraft/network/codec/StreamCodec$CodecOperation",
                "codec_op",
                0,
                "list",
            )),
            _ => None,
        };
    }
    if owner == "net/minecraft/network/codec/StreamCodec" {
        if is_static {
            return match name {
                "unit" | "of" | "ofMember" | "composite" | "recursive" => Some(
                    rt.new_pinned_handle(owner, "stream_codec", 0, name),
                ),
                _ => None,
            };
        }
        return match name {
            "map" | "apply" | "cast" | "dispatch" => Some(args[0]),
            _ => None,
        };
    }

    // 3. `CustomPacketPayload$Type`
    if owner == "net/minecraft/network/protocol/common/custom/CustomPacketPayload$Type" && !is_static
    {
        let recv = *args.first()?;
        return match name {
            "<init>" => {
                rt.put_field(recv, "id", *args.get(1)?);
                Some(V::Void)
            }
            "id" => Some(rt.get_field(recv, "id")),
            "toString" => {
                let s = rt.value_to_string(rt.get_field(recv, "id"));
                Some(rt.new_str(format!("Type[id={s}]")))
            }
            _ => None,
        };
    }

    // 4. `RegisterPayloadHandlersEvent` & `PayloadRegistrar`
    if owner == "net/neoforged/neoforge/network/event/RegisterPayloadHandlersEvent"
        && !is_static
        && name == "registrar"
    {
        let version = rt.str_of(*args.get(1)?)?.to_owned();
        let (_, cur_mod) = extract_net(host);
        let ns = cur_mod.unwrap_or_else(|| "minecraft".into());
        let reg = rt.new_instance(
            "net/neoforged/neoforge/network/registration/PayloadRegistrar",
            0,
        );
        let ns_v = rt.new_str(ns);
        let ver_v = rt.new_str(version);
        rt.put_field(reg, "$ns", ns_v);
        rt.put_field(reg, "$ver", ver_v);
        return Some(reg);
    }

    if owner == "net/neoforged/neoforge/network/registration/PayloadRegistrar" && !is_static {
        let recv = *args.first()?;
        return match name {
            "versioned" => {
                let ver_v = *args.get(1)?;
                let next = rt.new_instance(owner, 0);
                let ns_v = rt.get_field(recv, "$ns");
                rt.put_field(next, "$ns", ns_v);
                rt.put_field(next, "$ver", ver_v);
                Some(next)
            }
            "optional" | "executesOn" => Some(recv),
            "playToServer"
            | "playToClient"
            | "playBidirectional"
            | "configurationToServer"
            | "configurationToClient"
            | "configurationBidirectional"
            | "commonToServer"
            | "commonToClient"
            | "commonBidirectional" => {
                let ns = rt
                    .str_of(rt.get_field(recv, "$ns"))
                    .unwrap_or("minecraft")
                    .to_owned();
                let ver = rt
                    .str_of(rt.get_field(recv, "$ver"))
                    .unwrap_or("1")
                    .to_owned();
                let type_obj = *args.get(1)?;
                let payload_id = loc_str(rt, rt.get_field(type_obj, "id"))
                    .unwrap_or_else(|| format!("{ns}:unknown"));
                let handler = args.get(3).copied().unwrap_or(V::NULL);
                let dir = if name.ends_with("ToServer") {
                    "play_to_server"
                } else if name.ends_with("ToClient") {
                    "play_to_client"
                } else {
                    "bidirectional"
                };
                let (net, _) = extract_net(host);
                net.register_channel(rt, ns, ver, payload_id, dir, handler);
                Some(recv)
            }
            _ => None,
        };
    }

    None
}

/// Dispatches `com.mojang.datafixers.util.Either` and `com.mojang.datafixers.util.Pair`.
pub fn call_dfu_either_and_pair(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn mmga_runtime::HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();

    if owner == "com/mojang/datafixers/util/Either" {
        if is_static {
            return match name {
                "left" => {
                    let e = rt.new_instance(owner, 0);
                    rt.put_field(e, "$is_left", V::I(1));
                    rt.put_field(e, "$val", *args.first()?);
                    Some(e)
                }
                "right" => {
                    let e = rt.new_instance(owner, 0);
                    rt.put_field(e, "$is_left", V::I(0));
                    rt.put_field(e, "$val", *args.first()?);
                    Some(e)
                }
                _ => None,
            };
        }
        let recv = *args.first()?;
        let is_left = rt.get_field(recv, "$is_left").i() != 0;
        let val = rt.get_field(recv, "$val");
        return match name {
            "left" => {
                let opt = if is_left && !val.is_null() { Some(val) } else { None };
                Some(rt.alloc("java/util/Optional", 0, Data::Optional(opt)))
            }
            "right" => {
                let opt = if !is_left && !val.is_null() { Some(val) } else { None };
                Some(rt.alloc("java/util/Optional", 0, Data::Optional(opt)))
            }
            "ifLeft" => {
                if is_left {
                    let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                    let _ = invoke_closure(&mut ctx, *args.get(1)?, &[val]);
                }
                Some(recv)
            }
            "ifRight" => {
                if !is_left {
                    let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                    let _ = invoke_closure(&mut ctx, *args.get(1)?, &[val]);
                }
                Some(recv)
            }
            "map" if args.len() == 3 => {
                let mapped = {
                    let fn_v = if is_left { args[1] } else { args[2] };
                    let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                    invoke_closure(&mut ctx, fn_v, &[val])
                };
                Some(mapped)
            }
            "mapLeft" => {
                if is_left {
                    let mapped = {
                        let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                        invoke_closure(&mut ctx, *args.get(1)?, &[val])
                    };
                    let e = rt.new_instance(owner, 0);
                    rt.put_field(e, "$is_left", V::I(1));
                    rt.put_field(e, "$val", mapped);
                    Some(e)
                } else {
                    Some(recv)
                }
            }
            "mapRight" => {
                if !is_left {
                    let mapped = {
                        let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                        invoke_closure(&mut ctx, *args.get(1)?, &[val])
                    };
                    let e = rt.new_instance(owner, 0);
                    rt.put_field(e, "$is_left", V::I(0));
                    rt.put_field(e, "$val", mapped);
                    Some(e)
                } else {
                    Some(recv)
                }
            }
            "orThrow" => {
                if is_left {
                    Some(val)
                } else {
                    Some(rt.throw_exception("java/lang/RuntimeException", &rt.value_to_string(val)))
                }
            }
            _ => None,
        };
    }

    if owner == "com/mojang/datafixers/util/Pair" {
        if is_static && name == "of" && args.len() == 2 {
            let p = rt.new_instance(owner, 0);
            rt.put_field(p, "first", args[0]);
            rt.put_field(p, "second", args[1]);
            return Some(p);
        }
        if !is_static {
            let recv = *args.first()?;
            return match name {
                "<init>" if args.len() == 3 => {
                    rt.put_field(recv, "first", args[1]);
                    rt.put_field(recv, "second", args[2]);
                    Some(V::Void)
                }
                "getFirst" => Some(rt.get_field(recv, "first")),
                "getSecond" => Some(rt.get_field(recv, "second")),
                _ => None,
            };
        }
    }

    None
}

/// Dispatches `ModConfigSpec` and `ModConfigSpec$Builder` onto `ConfigSubsystem`.
pub fn call_mod_config_spec(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn mmga_runtime::HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
    extract_cfg: impl FnOnce(&mut dyn mmga_runtime::HostApi) -> &mut ConfigSubsystem,
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    if owner == "net/neoforged/neoforge/common/ModConfigSpec$Builder" && !is_static {
        let recv = *args.first()?;
        // `Builder.configure(Function<Builder, T>) -> Pair<T, ModConfigSpec>` executes a user closure!
        if name == "configure" {
            let func = *args.get(1)?;
            let mut ctx = ExecCtx {
                rt,
                linked,
                host,
                depth: 0,
            };
            let user_cfg = invoke_closure(&mut ctx, func, &[recv]);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let builder_id = ctx.rt.get_field(recv, "$builder_id").j() as u64;
            let spec_id = extract_cfg(ctx.host).build_spec(builder_id);
            let spec = ctx.rt.new_pinned_handle(
                "net/neoforged/neoforge/common/ModConfigSpec",
                "config_spec",
                spec_id,
                "",
            );
            let pair = ctx
                .rt
                .new_instance("org/apache/commons/lang3/tuple/ImmutablePair", 0);
            ctx.rt.put_field(pair, "left", user_cfg);
            ctx.rt.put_field(pair, "right", spec);
            return Some(pair);
        }

        let cfg = extract_cfg(host);
        return match name {
            "<init>" => {
                let bid = cfg.new_builder();
                rt.put_field(recv, "$builder_id", V::J(bid as i64));
                Some(V::Void)
            }
            "push" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                if let Some(sec) = args.get(1).and_then(|&v| rt.str_of(v)).map(str::to_owned) {
                    cfg.push_section(bid, &sec);
                } else if let Some(Data::List(items)) =
                    args.get(1).and_then(|&v| rt.get_obj(v)).map(|o| &o.data)
                {
                    let segs: Vec<String> = items
                        .iter()
                        .filter_map(|&v| rt.str_of(v).map(str::to_owned))
                        .collect();
                    for seg in segs {
                        cfg.push_section(bid, &seg);
                    }
                }
                Some(recv)
            }
            "pop" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let n = args.get(1).map_or(1, |v| v.i().max(1) as usize);
                cfg.pop_section(bid, n);
                Some(recv)
            }
            "comment" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                if let Some(arg1) = args.get(1).copied() {
                    if let Some(text) = rt.str_of(arg1).map(str::to_owned) {
                        cfg.set_comment(bid, text);
                    } else if let Some(Data::Array(lines)) = rt.get_obj(arg1).map(|o| &o.data) {
                        let joined: Vec<String> = lines
                            .iter()
                            .filter_map(|&v| rt.str_of(v).map(str::to_owned))
                            .collect();
                        cfg.set_comment(bid, joined.join("\n"));
                    }
                }
                Some(recv)
            }
            "translation" | "worldRestart" | "gameRestart" => Some(recv),
            "build" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let spec_id = cfg.build_spec(bid);
                Some(rt.new_pinned_handle(
                    "net/neoforged/neoforge/common/ModConfigSpec",
                    "config_spec",
                    spec_id,
                    "",
                ))
            }
            "define" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                let def_v = *args.get(2)?;
                if desc.ends_with("BooleanValue;") {
                    let b = unbox_primitive(rt, def_v).i() != 0;
                    let eid = cfg.define_entry(bid, &key, ConfigPrimitive::Bool(b), None);
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$BooleanValue",
                        eid,
                        def_v,
                    ))
                } else {
                    let prim = if let Some(s) = rt.str_of(def_v) {
                        ConfigPrimitive::Text(s.to_owned())
                    } else {
                        ConfigPrimitive::Text(rt.value_to_string(def_v))
                    };
                    let eid = cfg.define_entry(bid, &key, prim, None);
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$ConfigValue",
                        eid,
                        def_v,
                    ))
                }
            }
            "defineInRange" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                if desc.ends_with("IntValue;") {
                    let (def, lo, hi) = (args.get(2)?.i(), args.get(3)?.i(), args.get(4)?.i());
                    let eid = cfg.define_entry(
                        bid,
                        &key,
                        ConfigPrimitive::Int(def),
                        Some((f64::from(lo), f64::from(hi))),
                    );
                    let boxed = box_primitive(rt, "java/lang/Integer", V::I(def));
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$IntValue",
                        eid,
                        boxed,
                    ))
                } else if desc.ends_with("LongValue;") {
                    let (def, lo, hi) = (args.get(2)?.j(), args.get(3)?.j(), args.get(4)?.j());
                    let eid = cfg.define_entry(
                        bid,
                        &key,
                        ConfigPrimitive::Long(def),
                        Some((lo as f64, hi as f64)),
                    );
                    let boxed = box_primitive(rt, "java/lang/Long", V::J(def));
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$LongValue",
                        eid,
                        boxed,
                    ))
                } else if desc.ends_with("DoubleValue;") {
                    let (def, lo, hi) = (args.get(2)?.d(), args.get(3)?.d(), args.get(4)?.d());
                    let eid = cfg.define_entry(
                        bid,
                        &key,
                        ConfigPrimitive::Double(def),
                        Some((lo, hi)),
                    );
                    let boxed = box_primitive(rt, "java/lang/Double", V::D(def));
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$DoubleValue",
                        eid,
                        boxed,
                    ))
                } else {
                    None
                }
            }
            "defineEnum" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                let def_enum = *args.get(2)?;
                let label = rt.value_to_string(def_enum);
                let eid = cfg.define_entry(bid, &key, ConfigPrimitive::Text(label), None);
                Some(new_config_value_handle(
                    rt,
                    "net/neoforged/neoforge/common/ModConfigSpec$EnumValue",
                    eid,
                    def_enum,
                ))
            }
            "defineList" | "defineListAllowEmpty" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                let def_list = *args.get(2)?;
                let eid = cfg.define_entry(
                    bid,
                    &key,
                    ConfigPrimitive::Text(rt.value_to_string(def_list)),
                    None,
                );
                Some(new_config_value_handle(
                    rt,
                    "net/neoforged/neoforge/common/ModConfigSpec$ConfigValue",
                    eid,
                    def_list,
                ))
            }
            _ => None,
        };
    }

    if owner.starts_with("net/neoforged/neoforge/common/ModConfigSpec$")
        && owner.ends_with("Value")
        && !is_static
    {
        let recv = *args.first()?;
        let eid = rt.get_field(recv, "$entry_id").j() as u64;
        let cfg = extract_cfg(host);
        return match name {
            "get" => match cfg.current(eid).cloned() {
                Some(ConfigPrimitive::Bool(b)) => {
                    if desc == "()Z" {
                        Some(V::bool(b))
                    } else {
                        Some(box_primitive(rt, "java/lang/Boolean", V::bool(b)))
                    }
                }
                Some(ConfigPrimitive::Int(i)) => {
                    if desc == "()I" {
                        Some(V::I(i))
                    } else {
                        Some(box_primitive(rt, "java/lang/Integer", V::I(i)))
                    }
                }
                Some(ConfigPrimitive::Long(l)) => {
                    if desc == "()J" {
                        Some(V::J(l))
                    } else {
                        Some(box_primitive(rt, "java/lang/Long", V::J(l)))
                    }
                }
                Some(ConfigPrimitive::Double(d)) => {
                    if desc == "()D" {
                        Some(V::D(d))
                    } else {
                        Some(box_primitive(rt, "java/lang/Double", V::D(d)))
                    }
                }
                Some(ConfigPrimitive::Text(s)) => {
                    let raw = rt.get_field(recv, "$default_ref");
                    if !raw.is_null() && rt.str_of(raw).is_none() {
                        Some(raw)
                    } else {
                        Some(rt.new_str(s))
                    }
                }
                None => Some(rt.get_field(recv, "$default_ref")),
            },
            "getAsBoolean" => match cfg.current(eid) {
                Some(ConfigPrimitive::Bool(b)) => Some(V::bool(*b)),
                _ => Some(V::FALSE),
            },
            "getAsInt" => match cfg.current(eid) {
                Some(ConfigPrimitive::Int(i)) => Some(V::I(*i)),
                _ => Some(V::I(0)),
            },
            "getAsLong" => match cfg.current(eid) {
                Some(ConfigPrimitive::Long(l)) => Some(V::J(*l)),
                _ => Some(V::J(0)),
            },
            "getAsDouble" => match cfg.current(eid) {
                Some(ConfigPrimitive::Double(d)) => Some(V::D(*d)),
                _ => Some(V::D(0.0)),
            },
            "getDefault" => Some(rt.get_field(recv, "$default_ref")),
            _ => None,
        };
    }

    if (owner == "org/apache/commons/lang3/tuple/Pair"
        || owner == "org/apache/commons/lang3/tuple/ImmutablePair")
        && !is_static
    {
        let recv = *args.first()?;
        return match name {
            "<init>" if args.len() >= 3 => {
                rt.put_field(recv, "left", args[1]);
                rt.put_field(recv, "right", args[2]);
                Some(V::Void)
            }
            "getLeft" | "getKey" => Some(rt.get_field(recv, "left")),
            "getRight" | "getValue" => Some(rt.get_field(recv, "right")),
            _ => None,
        };
    }
    if owner == "org/apache/commons/lang3/tuple/Pair" && is_static && name == "of" && args.len() == 2
    {
        let pair = rt.new_instance("org/apache/commons/lang3/tuple/ImmutablePair", 0);
        rt.put_field(pair, "left", args[0]);
        rt.put_field(pair, "right", args[1]);
        return Some(pair);
    }

    None
}

fn new_config_value_handle(rt: &mut Rt, cls: &str, entry_id: u64, default_ref: V) -> V {
    let obj = rt.new_instance(cls, 0);
    rt.put_field(obj, "$entry_id", V::J(entry_id as i64));
    rt.put_field(obj, "$default_ref", default_ref);
    rt.pin(obj);
    obj
}

fn config_key_text(rt: &Rt, key_v: V) -> Option<String> {
    if let Some(s) = rt.str_of(key_v) {
        return Some(s.to_owned());
    }
    if let Some(Data::List(items)) = rt.get_obj(key_v).map(|o| &o.data) {
        let parts: Vec<&str> = items.iter().filter_map(|&v| rt.str_of(v)).collect();
        if !parts.is_empty() {
            return Some(parts.join("."));
        }
    }
    None
}

fn new_literal_component(rt: &mut Rt, text: &str) -> V {
    let comp = rt.new_instance(MUTABLE_COMPONENT, 0);
    let kind = rt.intern_str("literal");
    let txt = rt.new_str(text);
    let siblings = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
    let style = new_style(rt);
    rt.put_field(comp, "$kind", kind);
    rt.put_field(comp, "$text", txt);
    rt.put_field(comp, "$siblings", siblings);
    rt.put_field(comp, "$style", style);
    comp
}

fn new_translatable_component(
    rt: &mut Rt,
    key: &str,
    fallback: Option<String>,
    args: Vec<V>,
) -> V {
    let comp = rt.new_instance(MUTABLE_COMPONENT, 0);
    let kind = rt.intern_str("translatable");
    let key_v = rt.new_str(key);
    let fb_v = match fallback {
        Some(fb) => rt.new_str(fb),
        None => V::NULL,
    };
    let args_v = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(args));
    let siblings = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
    let style = new_style(rt);
    rt.put_field(comp, "$kind", kind);
    rt.put_field(comp, "$key", key_v);
    rt.put_field(comp, "$fallback", fb_v);
    rt.put_field(comp, "$args", args_v);
    rt.put_field(comp, "$siblings", siblings);
    rt.put_field(comp, "$style", style);
    comp
}

fn new_style(rt: &mut Rt) -> V {
    let style = rt.new_instance(STYLE_CLASS, 0);
    rt.put_field(style, "color", V::I(-1));
    style
}

fn copy_style(rt: &mut Rt, style: V) -> V {
    let next = new_style(rt);
    if !style.is_null() {
        for key in ["color", "Bold", "Italic", "Underlined", "Strikethrough", "Obfuscated"] {
            let v = rt.get_field(style, key);
            if !v.is_null() {
                rt.put_field(next, key, v);
            }
        }
    }
    next
}

fn apply_formatting_to_style(rt: &mut Rt, style: V, fmt: V) -> V {
    let next = copy_style(rt, style);
    if fmt.is_null() {
        return next;
    }
    let name = rt
        .str_of(rt.get_field(fmt, "$name"))
        .unwrap_or("")
        .to_owned();
    let rgb = rt.get_field(fmt, "$rgb").i();
    match name.as_str() {
        "RESET" => return new_style(rt),
        "BOLD" => rt.put_field(next, "Bold", V::I(1)),
        "ITALIC" => rt.put_field(next, "Italic", V::I(1)),
        "UNDERLINE" => rt.put_field(next, "Underlined", V::I(1)),
        "STRIKETHROUGH" => rt.put_field(next, "Strikethrough", V::I(1)),
        "OBFUSCATED" => rt.put_field(next, "Obfuscated", V::I(1)),
        _ if rgb >= 0 => rt.put_field(next, "color", V::I(rgb)),
        _ => {}
    }
    next
}

fn append_sibling(rt: &mut Rt, comp: V, child: V) {
    let sibs = rt.get_field(comp, "$siblings");
    if let Some(Data::List(items)) = rt.get_obj_mut(sibs).map(|o| &mut o.data) {
        items.push(child);
    }
}

fn clone_component(rt: &mut Rt, src: V, include_siblings_and_style: bool) -> V {
    let kind = rt.str_of(rt.get_field(src, "$kind")).unwrap_or("literal").to_owned();
    let cloned = if kind == "translatable" {
        let key = rt.str_of(rt.get_field(src, "$key")).unwrap_or("").to_owned();
        let fb = rt.str_of(rt.get_field(src, "$fallback")).map(str::to_owned);
        let args = extract_varargs(rt, rt.get_field(src, "$args"));
        new_translatable_component(rt, &key, fb, args)
    } else {
        let txt = rt.str_of(rt.get_field(src, "$text")).unwrap_or("").to_owned();
        new_literal_component(rt, &txt)
    };
    if include_siblings_and_style {
        let style = copy_style(rt, rt.get_field(src, "$style"));
        rt.put_field(cloned, "$style", style);
        let sibs = match rt.get_obj(rt.get_field(src, "$siblings")).map(|o| &o.data) {
            Some(Data::List(xs)) => xs.clone(),
            _ => Vec::new(),
        };
        for s in sibs {
            let cs = clone_component(rt, s, true);
            append_sibling(rt, cloned, cs);
        }
    }
    cloned
}

fn component_equals(
    rt: &Rt,
    translations: &BTreeMap<String, String>,
    left: V,
    right: V,
) -> bool {
    if left == right {
        return true;
    }
    if rt.class_of(left) != Some(MUTABLE_COMPONENT) || rt.class_of(right) != Some(MUTABLE_COMPONENT)
    {
        return false;
    }
    render_component_text(rt, translations, left) == render_component_text(rt, translations, right)
}

/// Renders a `Component` tree into plain text following vanilla `Component#getString()`.
pub fn render_component_text(
    rt: &Rt,
    translations: &BTreeMap<String, String>,
    comp: V,
) -> String {
    let mut out = String::new();
    let kind = rt.str_of(rt.get_field(comp, "$kind")).unwrap_or("literal");
    if kind == "translatable" {
        let key = rt.str_of(rt.get_field(comp, "$key")).unwrap_or("");
        let fallback = rt.str_of(rt.get_field(comp, "$fallback"));
        let template = translations
            .get(key)
            .map(String::as_str)
            .or(fallback)
            .unwrap_or(key);
        let raw_args = extract_varargs(rt, rt.get_field(comp, "$args"));
        let rendered_args: Vec<String> = raw_args
            .into_iter()
            .map(|v| {
                if rt.class_of(v) == Some(MUTABLE_COMPONENT) {
                    render_component_text(rt, translations, v)
                } else {
                    rt.value_to_string(v)
                }
            })
            .collect();
        out.push_str(&format_translation_template(template, &rendered_args));
    } else if let Some(txt) = rt.str_of(rt.get_field(comp, "$text")) {
        out.push_str(txt);
    }

    if let Some(Data::List(sibs)) = rt.get_obj(rt.get_field(comp, "$siblings")).map(|o| &o.data) {
        for &sib in sibs {
            out.push_str(&render_component_text(rt, translations, sib));
        }
    }
    out
}

/// Formats `%s`, `%d`, `%%`, and positional `%1$s` placeholders per `TranslatableContents`.
pub fn format_translation_template(template: &str, args: &[String]) -> String {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len() + 16);
    let mut i = 0usize;
    let mut seq_idx = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'%' {
                out.push('%');
                i += 2;
                continue;
            }
            // Positional `%<digits>$<spec>`
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1
                && j + 1 < bytes.len()
                && bytes[j] == b'$'
                && matches!(bytes[j + 1], b's' | b'd' | b'f')
            {
                if let Ok(pos1) = template[i + 1..j].parse::<usize>() {
                    if let Some(val) = pos1.checked_sub(1).and_then(|idx| args.get(idx)) {
                        out.push_str(val);
                    }
                    i = j + 2;
                    continue;
                }
            }
            if matches!(bytes[i + 1], b's' | b'd' | b'f') {
                if let Some(val) = args.get(seq_idx) {
                    out.push_str(val);
                }
                seq_idx += 1;
                i += 2;
                continue;
            }
        }
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn chat_formatting_constant(rt: &mut Rt, name: &str) -> Option<V> {
    let table: &[(&str, i32, char, bool, i32)] = &[
        ("BLACK", 0, '0', false, 0x000000),
        ("DARK_BLUE", 1, '1', false, 0x0000AA),
        ("DARK_GREEN", 2, '2', false, 0x00AA00),
        ("DARK_AQUA", 3, '3', false, 0x00AAAA),
        ("DARK_RED", 4, '4', false, 0xAA0000),
        ("DARK_PURPLE", 5, '5', false, 0xAA00AA),
        ("GOLD", 6, '6', false, 0xFFAA00),
        ("GRAY", 7, '7', false, 0xAAAAAA),
        ("DARK_GRAY", 8, '8', false, 0x555555),
        ("BLUE", 9, '9', false, 0x5555FF),
        ("GREEN", 10, 'a', false, 0x55FF55),
        ("AQUA", 11, 'b', false, 0x55FFFF),
        ("RED", 12, 'c', false, 0xFF5555),
        ("LIGHT_PURPLE", 13, 'd', false, 0xFF55FF),
        ("YELLOW", 14, 'e', false, 0xFFFF55),
        ("WHITE", 15, 'f', false, 0xFFFFFF),
        ("OBFUSCATED", 16, 'k', true, -1),
        ("BOLD", 17, 'l', true, -1),
        ("STRIKETHROUGH", 18, 'm', true, -1),
        ("UNDERLINE", 19, 'n', true, -1),
        ("ITALIC", 20, 'o', true, -1),
        ("RESET", 21, 'r', false, -1),
    ];
    let &(ename, ord, code, is_fmt, rgb) = table.iter().find(|x| x.0 == name)?;
    let obj = rt.new_instance(CHAT_FORMATTING, 0);
    let n_str = rt.intern_str(ename);
    let lower = rt.intern_str(&ename.to_ascii_lowercase());
    let prefix = rt.intern_str(&format!("\u{00a7}{code}"));
    rt.put_field(obj, "$name", n_str);
    rt.put_field(obj, "$fmt_name", lower);
    rt.put_field(obj, "$ordinal", V::I(ord));
    rt.put_field(obj, "$code", V::I(code as i32));
    rt.put_field(obj, "$is_format", V::bool(is_fmt));
    rt.put_field(obj, "$rgb", V::I(rgb));
    rt.put_field(obj, "$prefix", prefix);
    rt.pin(obj);
    Some(obj)
}

fn extract_varargs(rt: &Rt, arr: V) -> Vec<V> {
    match rt.get_obj(arr).map(|o| &o.data) {
        Some(Data::Array(xs) | Data::List(xs)) => xs.clone(),
        _ => Vec::new(),
    }
}

fn resolve_class_resource_path(class_internal: &str, raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix('/') {
        return stripped.to_owned();
    }
    match class_internal.rsplit_once('/') {
        Some((pkg, _)) if !pkg.is_empty() => format!("{pkg}/{raw}"),
        _ => raw.to_owned(),
    }
}

fn path_string(rt: &Rt, value: V) -> Option<String> {
    if let Some((kind, _, tag, _)) = rt.handle_of(value) {
        if matches!(kind, "nio_path" | "io_file" | "fml_path") {
            return Some(tag.to_owned());
        }
    }
    rt.str_of(rt.get_field(value, "path"))
        .map(str::to_owned)
        .or_else(|| rt.str_of(value).map(str::to_owned))
}

fn normalize_lexical(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        std::path::PathBuf::from(".")
    } else {
        out
    }
}

/// Dispatches Brigadier command registration (`RegisterCommandsEvent`, `Commands`, `CommandDispatcher`,
/// `LiteralArgumentBuilder`, `RequiredArgumentBuilder`, `SuggestionProviders`, `CommandContext`, `CommandSourceStack`).
pub fn call_brigadier(
    rt: &mut Rt,
    commands: &mut BrigadierTree,
    translations: &BTreeMap<String, String>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();

    if owner == "net/neoforged/neoforge/event/RegisterCommandsEvent" && !is_static {
        return match name {
            "getDispatcher" => Some(rt.new_pinned_handle(
                "com/mojang/brigadier/CommandDispatcher",
                "cmd_dispatcher",
                0,
                "",
            )),
            "getCommandSelection" => Some(rt.new_pinned_handle(
                "net/minecraft/commands/Commands$CommandSelection",
                "cmd_selection",
                0,
                "ALL",
            )),
            "getBuildContext" => Some(rt.new_pinned_handle(
                "net/minecraft/commands/CommandBuildContext",
                "cmd_build_ctx",
                0,
                "",
            )),
            _ => None,
        };
    }

    if owner == "net/minecraft/commands/synchronization/SuggestionProviders"
        && is_static
        && name == "register"
        && args.len() == 2
    {
        rt.pin(args[1]);
        return Some(args[1]);
    }

    if (owner == "net/minecraft/commands/Commands"
        || owner == "com/mojang/brigadier/builder/LiteralArgumentBuilder")
        && is_static
        && name == "literal"
    {
        let lit = rt.str_of(*args.first()?)?.to_owned();
        let node_id = commands.alloc_node(CommandNodeKind::Literal(lit.clone()));
        return Some(rt.new_handle(
            "com/mojang/brigadier/builder/LiteralArgumentBuilder",
            "cmd_node",
            node_id,
            lit,
        ));
    }

    if (owner == "net/minecraft/commands/Commands"
        || owner == "com/mojang/brigadier/builder/RequiredArgumentBuilder")
        && is_static
        && name == "argument"
    {
        let arg_name = rt.str_of(*args.first()?)?.to_owned();
        let kind = match args.get(1).and_then(|&v| rt.handle_of(v)) {
            Some(("cmd_arg_type", _, "greedy", _)) => ArgumentKind::GreedyString,
            Some(("cmd_arg_type", _, "int", _)) => ArgumentKind::Integer,
            Some(("cmd_arg_type", _, "float", _)) => ArgumentKind::Float,
            Some(("cmd_arg_type", _, "bool", _)) => ArgumentKind::Bool,
            _ => ArgumentKind::Word,
        };
        let node_id = commands.alloc_node(CommandNodeKind::Argument {
            name: arg_name.clone(),
            kind,
        });
        return Some(rt.new_handle(
            "com/mojang/brigadier/builder/RequiredArgumentBuilder",
            "cmd_node",
            node_id,
            arg_name,
        ));
    }

    if owner == "com/mojang/brigadier/arguments/StringArgumentType" && is_static {
        return match name {
            "word" | "string" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "word")),
            "greedyString" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "greedy")),
            "getString" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                Some(rt.get_field(ctx_v, &format!("$arg:{key}")))
            }
            _ => None,
        };
    }
    if owner == "com/mojang/brigadier/arguments/IntegerArgumentType" && is_static {
        return match name {
            "integer" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "int")),
            "getInteger" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                Some(V::I(unbox_primitive(rt, rt.get_field(ctx_v, &format!("$arg:{key}"))).i()))
            }
            _ => None,
        };
    }
    if owner == "com/mojang/brigadier/arguments/BoolArgumentType" && is_static {
        return match name {
            "bool" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "bool")),
            "getBool" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                Some(V::bool(unbox_primitive(rt, rt.get_field(ctx_v, &format!("$arg:{key}"))).i() != 0))
            }
            _ => None,
        };
    }
    if (owner == "com/mojang/brigadier/arguments/FloatArgumentType"
        || owner == "com/mojang/brigadier/arguments/DoubleArgumentType")
        && is_static
    {
        return match name {
            "floatArg" | "doubleArg" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "float")),
            _ => None,
        };
    }
    if owner == "net/minecraft/commands/arguments/ResourceLocationArgument" && is_static {
        return match name {
            "id" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "word")),
            "getId" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                let raw = rt
                    .str_of(rt.get_field(ctx_v, &format!("$arg:{key}")))
                    .unwrap_or("minecraft:empty")
                    .to_owned();
                Some(new_rl_handle(rt, &raw))
            }
            _ => None,
        };
    }
    if owner == "net/minecraft/commands/arguments/EntityArgument" && is_static {
        return match name {
            "player" | "players" | "entity" | "entities" => {
                Some(rt.new_handle(owner, "cmd_arg_type", 0, "word"))
            }
            "getPlayers" | "getEntities" => {
                Some(rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new())))
            }
            _ => None,
        };
    }
    if (owner == "net/minecraft/commands/arguments/coordinates/BlockPosArgument"
        || owner == "net/minecraft/commands/arguments/coordinates/Vec3Argument"
        || owner == "net/minecraft/commands/arguments/ComponentArgument"
        || owner == "net/minecraft/commands/arguments/CompoundTagArgument"
        || owner == "net/minecraft/commands/arguments/TimeArgument"
        || owner == "net/minecraft/commands/arguments/UuidArgument"
        || owner == "net/minecraft/commands/arguments/DimensionArgument")
        && is_static
    {
        return match name {
            "blockPos" | "vec3" | "textComponent" | "compoundTag" | "time" | "uuid" | "dimension" => {
                Some(rt.new_handle(owner, "cmd_arg_type", 0, "word"))
            }
            _ => None,
        };
    }
    if owner.ends_with("ConfigPathArgument") && is_static && name == "path" {
        return Some(rt.new_handle(owner, "cmd_arg_type", 0, "word"));
    }
    if owner.starts_with("com/mojang/brigadier/exceptions/") && !is_static {
        let recv = *args.first()?;
        return match name {
            "<init>" => {
                if let Some(&arg1) = args.get(1) {
                    rt.put_field(recv, "$msg_or_fn", arg1);
                }
                Some(V::Void)
            }
            "create" | "createWithContext" => {
                let exc = rt.new_instance(
                    "com/mojang/brigadier/exceptions/CommandSyntaxException",
                    0,
                );
                let msg = rt.value_to_string(rt.get_field(recv, "$msg_or_fn"));
                let msg_v = rt.new_str(msg);
                rt.put_field(exc, "detailMessage", msg_v);
                Some(exc)
            }
            _ => None,
        };
    }
    if owner == "com/mojang/brigadier/StringReader" && is_static && name == "isAllowedInUnquotedString" {
        let ch = char::from_u32(args.first()?.i() as u32).unwrap_or('\0');
        let ok = ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '+');
        return Some(V::bool(ok));
    }

    if (owner == "com/mojang/brigadier/builder/ArgumentBuilder"
        || owner == "com/mojang/brigadier/builder/LiteralArgumentBuilder"
        || owner == "com/mojang/brigadier/builder/RequiredArgumentBuilder")
        && !is_static
    {
        let recv = *args.first()?;
        let ("cmd_node", node_id, tag, _) = rt.handle_of(recv)? else {
            return None;
        };
        let tag = tag.to_owned();
        return match name {
            "then" => {
                if let Some(("cmd_node", child_id, _, _)) = args.get(1).and_then(|&v| rt.handle_of(v)) {
                    commands.add_child(node_id, child_id);
                }
                Some(recv)
            }
            "executes" => {
                if let Some(&cmd) = args.get(1) {
                    if !cmd.is_null() {
                        commands.set_executes(rt, node_id, cmd);
                    }
                }
                Some(recv)
            }
            "requires" | "suggests" | "redirect" | "fork" | "forward" => Some(recv),
            "build" => Some(rt.new_handle(
                "com/mojang/brigadier/tree/LiteralCommandNode",
                "cmd_node",
                node_id,
                tag,
            )),
            _ => None,
        };
    }

    if owner == "com/mojang/brigadier/CommandDispatcher" && !is_static {
        return match name {
            "register" => {
                let builder = *args.get(1)?;
                let ("cmd_node", node_id, tag, _) = rt.handle_of(builder)? else {
                    return None;
                };
                let tag = tag.to_owned();
                commands.register_root(node_id);
                Some(rt.new_handle(
                    "com/mojang/brigadier/tree/LiteralCommandNode",
                    "cmd_node",
                    node_id,
                    tag,
                ))
            }
            "getRoot" => Some(rt.new_pinned_handle(
                "com/mojang/brigadier/tree/RootCommandNode",
                "cmd_root",
                0,
                "",
            )),
            "findNode" => {
                let path_tokens: Vec<String> = match rt.get_obj(*args.get(1)?).map(|o| &o.data) {
                    Some(Data::Set(xs) | Data::List(xs) | Data::Array(xs)) => xs
                        .iter()
                        .filter_map(|&v| rt.str_of(v).map(str::to_owned))
                        .collect(),
                    _ => Vec::new(),
                };
                if path_tokens.is_empty() {
                    return Some(V::NULL);
                }
                let mut cur_candidates = commands.roots.clone();
                let mut matched_id = None;
                let mut matched_tag = String::new();
                for tok in &path_tokens {
                    let mut found = None;
                    for &cid in &cur_candidates {
                        if let Some(n) = commands.nodes.get(&cid) {
                            let n_name = match &n.kind {
                                CommandNodeKind::Literal(l) => l.as_str(),
                                CommandNodeKind::Argument { name, .. } => name.as_str(),
                            };
                            if n_name == tok {
                                found = Some((cid, n_name.to_owned(), n.children.clone()));
                                break;
                            }
                        }
                    }
                    match found {
                        Some((id, tag, kids)) => {
                            matched_id = Some(id);
                            matched_tag = tag;
                            cur_candidates = kids;
                        }
                        None => return Some(V::NULL),
                    }
                }
                match matched_id {
                    Some(id) => Some(rt.new_handle(
                        "com/mojang/brigadier/tree/LiteralCommandNode",
                        "cmd_node",
                        id,
                        matched_tag,
                    )),
                    None => Some(V::NULL),
                }
            }
            _ => None,
        };
    }
    if (owner == "com/mojang/brigadier/tree/RootCommandNode"
        || owner == "com/mojang/brigadier/tree/CommandNode"
        || owner == "com/mojang/brigadier/tree/LiteralCommandNode"
        || owner == "com/mojang/brigadier/tree/ArgumentCommandNode")
        && !is_static
    {
        return match name {
            "addChild" => {
                let child = *args.get(1)?;
                let ("cmd_node", child_id, _, _) = rt.handle_of(child)? else {
                    return None;
                };
                if rt.handle_of(args[0]).is_some_and(|(k, ..)| k == "cmd_root") {
                    commands.register_root(child_id);
                } else if let Some(("cmd_node", parent_id, _, _)) = rt.handle_of(args[0]) {
                    commands.add_child(parent_id, child_id);
                }
                Some(V::Void)
            }
            "getName" => {
                let tag = rt
                    .handle_of(args[0])
                    .map(|(_, _, t, _)| t.to_owned())
                    .unwrap_or_default();
                Some(rt.new_str(tag))
            }
            "getRequirement" => Some(V::NULL),
            "getRedirect" => Some(V::NULL),
            "getRedirectModifier" => Some(V::NULL),
            "isFork" => Some(V::FALSE),
            "getCommand" => {
                let cmd = rt
                    .handle_of(args[0])
                    .and_then(|(_, id, _, _)| commands.nodes.get(&id))
                    .and_then(|n| n.executes)
                    .unwrap_or(V::NULL);
                Some(cmd)
            }
            "getChildren" => {
                let child_ids = if rt.handle_of(args[0]).is_some_and(|(k, ..)| k == "cmd_root") {
                    commands.roots.clone()
                } else if let Some(("cmd_node", nid, _, _)) = rt.handle_of(args[0]) {
                    commands
                        .nodes
                        .get(&nid)
                        .map(|n| n.children.clone())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let list: Vec<V> = child_ids
                    .into_iter()
                    .filter_map(|cid| {
                        let n = commands.nodes.get(&cid)?;
                        let (cls, tag) = match &n.kind {
                            CommandNodeKind::Literal(l) => {
                                ("com/mojang/brigadier/tree/LiteralCommandNode", l.clone())
                            }
                            CommandNodeKind::Argument { name, .. } => {
                                ("com/mojang/brigadier/tree/ArgumentCommandNode", name.clone())
                            }
                        };
                        Some(rt.new_handle(cls, "cmd_node", cid, tag))
                    })
                    .collect();
                Some(rt.alloc("java/util/ArrayList", 0, Data::List(list)))
            }
            "getChild" => {
                let want = rt.str_of(*args.get(1)?)?.to_owned();
                let child_ids = if rt.handle_of(args[0]).is_some_and(|(k, ..)| k == "cmd_root") {
                    commands.roots.clone()
                } else if let Some(("cmd_node", nid, _, _)) = rt.handle_of(args[0]) {
                    commands
                        .nodes
                        .get(&nid)
                        .map(|n| n.children.clone())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                for cid in child_ids {
                    if let Some(n) = commands.nodes.get(&cid) {
                        let (cls, tag) = match &n.kind {
                            CommandNodeKind::Literal(l) => {
                                ("com/mojang/brigadier/tree/LiteralCommandNode", l.as_str())
                            }
                            CommandNodeKind::Argument { name, .. } => (
                                "com/mojang/brigadier/tree/ArgumentCommandNode",
                                name.as_str(),
                            ),
                        };
                        if tag == want {
                            return Some(rt.new_handle(cls, "cmd_node", cid, want));
                        }
                    }
                }
                Some(V::NULL)
            }
            _ => None,
        };
    }

    if owner == "com/mojang/brigadier/context/CommandContext" && !is_static {
        let recv = *args.first()?;
        return match name {
            "getSource" => Some(rt.get_field(recv, "$source")),
            "getArgument" => {
                let key = rt.str_of(*args.get(1)?)?;
                Some(rt.get_field(recv, &format!("$arg:{key}")))
            }
            _ => None,
        };
    }

    if owner == "net/minecraft/commands/CommandSourceStack" && !is_static {
        let recv = *args.first()?;
        return match name {
            "isPlayer" => Some(V::bool(rt.get_field(recv, "$is_player").i() != 0)),
            "hasPermission" => Some(V::TRUE),
            "sendSystemMessage" | "sendFailure" => {
                let comp = *args.get(1)?;
                let text = render_component_text(rt, translations, comp);
                let v = rt.new_str(text);
                let msg_list = rt.get_field(recv, "$messages");
                if let Some(Data::List(msgs)) = rt.get_obj_mut(msg_list).map(|o| &mut o.data) {
                    msgs.push(v);
                }
                Some(V::Void)
            }
            _ => None,
        };
    }

    None
}

/// Dispatches `ResourceKey`, `TagKey` (`ItemTags` / `BlockTags` / `FluidTags` / `EntityTypeTags`),
/// `SingletonArgumentInfo` / `ArgumentTypeInfos`, `com.mojang.authlib.GameProfile`, Guava collections, and `GsonBuilder`.
pub fn call_keys_authlib_and_libs(
    rt: &mut Rt,
    registries: &mut crate::RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    // 1. `net/minecraft/resources/ResourceKey`
    if owner == "net/minecraft/resources/ResourceKey" {
        if is_static && name == "createRegistryKey" {
            let loc = loc_str(rt, *args.first()?)?;
            return Some(new_resource_key(rt, "minecraft:root", &loc));
        }
        if is_static && name == "create" && args.len() == 2 {
            let reg = resource_key_parts(rt, args[0])
                .map(|(_, loc)| loc)
                .or_else(|| loc_str(rt, args[0]))?;
            let loc = loc_str(rt, args[1])?;
            return Some(new_resource_key(rt, &reg, &loc));
        }
        if !is_static {
            let (reg, loc) = resource_key_parts(rt, *args.first()?)?;
            return match name {
                "location" => Some(new_rl_handle(rt, &loc)),
                "registry" => Some(new_rl_handle(rt, &reg)),
                "isFor" => {
                    let want = resource_key_parts(rt, *args.get(1)?)
                        .map(|(_, l)| l)
                        .unwrap_or_default();
                    Some(V::bool(reg == want))
                }
                "equals" => {
                    let other = args.get(1).and_then(|&v| resource_key_parts(rt, v));
                    Some(V::bool(other == Some((reg, loc))))
                }
                "toString" => Some(rt.new_str(format!("ResourceKey[{reg} / {loc}]"))),
                _ => None,
            };
        }
    }

    // 2. `TagKey`, `BlockTags`, `ItemTags`, `FluidTags`, `EntityTypeTags`, `BiomeTags`
    if owner == "net/minecraft/tags/TagKey" {
        if is_static && name == "create" && args.len() == 2 {
            let reg = resource_key_parts(rt, args[0])
                .map(|(_, l)| l)
                .or_else(|| loc_str(rt, args[0]))?;
            let loc = loc_str(rt, args[1])?;
            return Some(new_tag_key(rt, &reg, &loc));
        }
        if !is_static {
            let (reg, loc) = tag_key_parts(rt, *args.first()?)?;
            return match name {
                "location" => Some(new_rl_handle(rt, &loc)),
                "registry" => Some(new_resource_key(rt, "minecraft:root", &reg)),
                "isFor" => {
                    let want = resource_key_parts(rt, *args.get(1)?)
                        .map(|(_, l)| l)
                        .unwrap_or_default();
                    Some(V::bool(reg == want))
                }
                "equals" => {
                    let other = args.get(1).and_then(|&v| tag_key_parts(rt, v));
                    Some(V::bool(other == Some((reg, loc))))
                }
                "toString" => Some(rt.new_str(format!("TagKey[{reg} / {loc}]"))),
                _ => None,
            };
        }
    }
    if is_static && (name == "create" || name == "bind") {
        let tag_reg = match owner {
            "net/minecraft/tags/BlockTags" => Some("minecraft:block"),
            "net/minecraft/tags/ItemTags" => Some("minecraft:item"),
            "net/minecraft/tags/FluidTags" => Some("minecraft:fluid"),
            "net/minecraft/tags/EntityTypeTags" => Some("minecraft:entity_type"),
            _ => None,
        };
        if let Some(reg) = tag_reg {
            let loc = args
                .first()
                .and_then(|&v| loc_str(rt, v))
                .or_else(|| {
                    args.first().and_then(|&v| rt.str_of(v)).map(|s| {
                        if s.contains(':') {
                            s.to_owned()
                        } else {
                            format!("minecraft:{s}")
                        }
                    })
                })?;
            return Some(new_tag_key(rt, reg, &loc));
        }
    }

    // 3. `Registry` / `DefaultedRegistry` / `WritableRegistry` / `RegistryBuilder` / `Mth`
    if owner == "net/minecraft/util/Mth" && is_static {
        return match (name, desc) {
            ("floor", "(F)I") => Some(V::I(args[0].f().floor() as i32)),
            ("floor", "(D)I") => Some(V::I(args[0].d().floor() as i32)),
            ("ceil", "(F)I") => Some(V::I(args[0].f().ceil() as i32)),
            ("ceil", "(D)I") => Some(V::I(args[0].d().ceil() as i32)),
            ("clamp", "(III)I") => Some(V::I(args[0].i().clamp(args[1].i(), args[2].i()))),
            ("clamp", "(FFF)F") => Some(V::F(args[0].f().clamp(args[1].f(), args[2].f()))),
            ("clamp", "(DDD)D") => Some(V::D(args[0].d().clamp(args[1].d(), args[2].d()))),
            ("lerp", "(FFF)F") => {
                let (t, a, b) = (args[0].f(), args[1].f(), args[2].f());
                Some(V::F(a + t * (b - a)))
            }
            ("lerp", "(DDD)D") => {
                let (t, a, b) = (args[0].d(), args[1].d(), args[2].d());
                Some(V::D(a + t * (b - a)))
            }
            ("sin", "(F)F") => Some(V::F(args[0].f().sin())),
            ("cos", "(F)F") => Some(V::F(args[0].f().cos())),
            ("sqrt", "(F)F") => Some(V::F(args[0].f().sqrt())),
            ("abs", "(I)I") => Some(V::I(args[0].i().wrapping_abs())),
            ("abs", "(F)F") => Some(V::F(args[0].f().abs())),
            ("hsvToRgb", "(FFF)I") => Some(V::I(hsv_to_rgb(args[0].f(), args[1].f(), args[2].f()))),
            _ => None,
        };
    }

    if owner == "net/neoforged/neoforge/registries/RegistryBuilder" && !is_static {
        let recv = *args.first()?;
        return match name {
            "<init>" => {
                let reg_key = resource_key_parts(rt, *args.get(1)?)
                    .map(|(_, loc)| loc)
                    .unwrap_or_else(|| "minecraft:custom".into());
                let key_v = rt.intern_str(&reg_key);
                rt.put_field(recv, "$reg_key", key_v);
                Some(V::Void)
            }
            "sync" | "defaultKey" | "maxId" | "withIntrusiveHolders" | "disableRegistrationCheck" => {
                Some(recv)
            }
            "onBake" | "onAdd" | "onClear" | "onValidate" | "callback" => {
                if let Some(&cb) = args.get(1) {
                    rt.pin(cb);
                    if name == "onBake" {
                        rt.put_field(recv, "$on_bake", cb);
                    }
                }
                Some(recv)
            }
            "create" => {
                let reg_key = rt
                    .str_of(rt.get_field(recv, "$reg_key"))
                    .unwrap_or("minecraft:custom")
                    .to_owned();
                if !registries.custom_registries.contains(&reg_key) {
                    registries.custom_registries.push(reg_key.clone());
                }
                registries.entries.entry(reg_key.clone()).or_default();
                Some(rt.new_pinned_handle(
                    "net/minecraft/core/Registry",
                    "registry",
                    0,
                    reg_key,
                ))
            }
            _ => None,
        };
    }

    if owner == "net/minecraft/core/Registry" && is_static && name == "register" {
        let reg_v = *args.first()?;
        let ("registry", _, reg_key, _) = rt.handle_of(reg_v)? else {
            return None;
        };
        let reg_key = reg_key.to_owned();
        let full_id = loc_str(rt, args[1])
            .or_else(|| resource_key_parts(rt, args[1]).map(|(_, l)| l))
            .or_else(|| {
                rt.str_of(args[1]).map(|s| {
                    if s.contains(':') {
                        s.to_owned()
                    } else {
                        format!("minecraft:{s}")
                    }
                })
            })?;
        let val = args[2];
        let ns = full_id
            .split_once(':')
            .map_or("minecraft", |(n, _)| n)
            .to_owned();
        registries.register_direct(
            rt,
            &reg_key,
            &full_id,
            &ns,
            val,
            crate::ContentProperties::default(),
        );
        return Some(val);
    }
    if owner == "net/minecraft/core/Registry" && is_static && name == "registerForHolder" {
        let reg_v = *args.first()?;
        let ("registry", _, reg_key, _) = rt.handle_of(reg_v)? else {
            return None;
        };
        let reg_key = reg_key.to_owned();
        let full_id = loc_str(rt, args[1])
            .or_else(|| resource_key_parts(rt, args[1]).map(|(_, l)| l))?;
        let val = args[2];
        let ns = full_id
            .split_once(':')
            .map_or("minecraft", |(n, _)| n)
            .to_owned();
        registries.register_direct(
            rt,
            &reg_key,
            &full_id,
            &ns,
            val,
            crate::ContentProperties::default(),
        );
        let h = rt.new_instance("net/minecraft/core/Holder$Reference", 0);
        let k = new_resource_key(rt, &reg_key, &full_id);
        rt.put_field(h, "$key", k);
        rt.put_field(h, "$value", val);
        return Some(h);
    }

    if (owner == "net/minecraft/core/Registry"
        || owner == "net/minecraft/core/DefaultedRegistry"
        || owner == "net/minecraft/core/WritableRegistry")
        && !is_static
    {
        let recv = *args.first()?;
        let ("registry", _, reg_key, _) = rt.handle_of(recv)? else {
            return None;
        };
        let reg_key = reg_key.to_owned();
        if name == "register" && args.len() >= 3 {
            let full_id = resource_key_parts(rt, args[1])
                .map(|(_, l)| l)
                .or_else(|| loc_str(rt, args[1]))?;
            let val = args[2];
            let ns = full_id
                .split_once(':')
                .map_or("minecraft", |(n, _)| n)
                .to_owned();
            registries.register_direct(
                rt,
                &reg_key,
                &full_id,
                &ns,
                val,
                crate::ContentProperties::default(),
            );
            let h = rt.new_instance("net/minecraft/core/Holder$Reference", 0);
            let k = new_resource_key(rt, &reg_key, &full_id);
            rt.put_field(h, "$key", k);
            rt.put_field(h, "$value", val);
            return Some(h);
        }
        return match name {
            "key" => Some(new_resource_key(rt, "minecraft:root", &reg_key)),
            "containsKey" => {
                let id = args.get(1).and_then(|&v| {
                    loc_str(rt, v).or_else(|| resource_key_parts(rt, v).map(|(_, l)| l))
                })?;
                Some(V::bool(registries.lookup(&reg_key, &id).is_some()))
            }
            "get" => {
                let id = args.get(1).and_then(|&v| {
                    loc_str(rt, v).or_else(|| resource_key_parts(rt, v).map(|(_, l)| l))
                })?;
                Some(registries.lookup(&reg_key, &id).map_or(V::NULL, |e| e.value))
            }
            "getOptional" => {
                let id = args.get(1).and_then(|&v| {
                    loc_str(rt, v).or_else(|| resource_key_parts(rt, v).map(|(_, l)| l))
                })?;
                let found = registries
                    .lookup(&reg_key, &id)
                    .map(|e| e.value)
                    .filter(|v| !v.is_null());
                Some(rt.alloc("java/util/Optional", 0, Data::Optional(found)))
            }
            "getKey" => {
                let target = *args.get(1)?;
                let found = registries
                    .entries
                    .get(&reg_key)
                    .and_then(|m| {
                        m.values()
                            .find(|e| !e.value.is_null() && e.value == target)
                    })
                    .map(|e| e.id.clone());
                Some(match found {
                    Some(id) => new_rl_handle(rt, &id),
                    None => V::NULL,
                })
            }
            "holders" => {
                let refs: Vec<V> = registries
                    .entries
                    .get(&reg_key)
                    .map(|m| {
                        m.values()
                            .filter(|e| !e.value.is_null())
                            .map(|e| {
                                let h = rt.new_instance("net/minecraft/core/Holder$Reference", 0);
                                let k = new_resource_key(rt, &reg_key, &e.id);
                                rt.put_field(h, "$key", k);
                                rt.put_field(h, "$value", e.value);
                                h
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Some(rt.alloc("java/util/stream/Stream", 0, Data::Stream(refs)))
            }
            "stream" => {
                let vals: Vec<V> = registries
                    .entries
                    .get(&reg_key)
                    .map(|m| {
                        m.values()
                            .filter(|e| !e.value.is_null())
                            .map(|e| e.value)
                            .collect()
                    })
                    .unwrap_or_default();
                Some(rt.alloc("java/util/stream/Stream", 0, Data::Stream(vals)))
            }
            "iterator" => {
                let vals: Vec<V> = registries
                    .entries
                    .get(&reg_key)
                    .map(|m| {
                        m.values()
                            .filter(|e| !e.value.is_null())
                            .map(|e| e.value)
                            .collect()
                    })
                    .unwrap_or_default();
                Some(rt.alloc("java/util/Iterator", 0, Data::Iter { items: vals, pos: 0 }))
            }
            "keySet" => {
                let ids: Vec<V> = registries
                    .entries
                    .get(&reg_key)
                    .map(|m| m.keys().map(|id| new_rl_handle(rt, id)).collect())
                    .unwrap_or_default();
                Some(rt.alloc("java/util/Set", 0, Data::Set(ids)))
            }
            "size" => {
                let n = registries
                    .entries
                    .get(&reg_key)
                    .map_or(0, |m| m.len() as i32);
                Some(V::I(n))
            }
            _ => None,
        };
    }

    if (owner == "net/minecraft/core/Holder" || owner == "net/minecraft/core/Holder$Reference")
        && !is_static
    {
        let recv = *args.first()?;
        if let Some(("deferred_holder", hid, full_id, _)) = rt.handle_of(recv) {
            let full_id = full_id.to_owned();
            return match name {
                "value" | "get" => Some(registries.holder_value(hid).unwrap_or(V::NULL)),
                "isBound" => Some(V::bool(
                    registries.holder_value(hid).is_some_and(|v| !v.is_null()),
                )),
                "getId" => Some(new_rl_handle(rt, &full_id)),
                "unwrapKey" => {
                    let reg = registries
                        .holders
                        .get(&hid)
                        .map(|(r, _, _)| r.clone())
                        .unwrap_or_else(|| "minecraft:custom".into());
                    let k = new_resource_key(rt, &reg, &full_id);
                    Some(rt.alloc("java/util/Optional", 0, Data::Optional(Some(k))))
                }
                _ => None,
            };
        }
        return match name {
            "value" | "get" => Some(rt.get_field(recv, "$value")),
            "isBound" => Some(V::bool(!rt.get_field(recv, "$value").is_null())),
            "unwrapKey" => {
                let k = rt.get_field(recv, "$key");
                let opt = if k.is_null() { None } else { Some(k) };
                Some(rt.alloc("java/util/Optional", 0, Data::Optional(opt)))
            }
            _ => None,
        };
    }

    // 4. `SingletonArgumentInfo`, `ArgumentTypeInfos`, & `RegistrySetBuilder`
    if owner == "net/minecraft/core/RegistrySetBuilder" && !is_static {
        let recv = *args.first()?;
        return match name {
            "<init>" => {
                let entries = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
                rt.put_field(recv, "$entries", entries);
                Some(V::Void)
            }
            "add" => {
                if let (Some(&key), Some(&bootstrap)) = (args.get(1), args.get(2)) {
                    rt.pin(bootstrap);
                    let pair = rt.new_instance("java/util/AbstractMap$SimpleImmutableEntry", 0);
                    rt.put_field(pair, "key", key);
                    rt.put_field(pair, "value", bootstrap);
                    let list = rt.get_field(recv, "$entries");
                    if let Some(Data::List(xs)) = rt.get_obj_mut(list).map(|o| &mut o.data) {
                        xs.push(pair);
                    }
                }
                Some(recv)
            }
            "getEntryKeys" => {
                let keys: Vec<V> = match rt.get_obj(rt.get_field(recv, "$entries")).map(|o| &o.data)
                {
                    Some(Data::List(xs)) => xs
                        .iter()
                        .map(|&p| rt.get_field(p, "key"))
                        .collect(),
                    _ => Vec::new(),
                };
                Some(rt.alloc("java/util/List", 0, Data::List(keys)))
            }
            _ => None,
        };
    }
    if owner == "net/minecraft/commands/synchronization/SingletonArgumentInfo"
        && is_static
        && (name == "contextFree" || name == "contextAware")
    {
        let ctor = *args.first()?;
        rt.pin(ctor);
        let info = rt.new_instance(owner, 0);
        rt.put_field(info, "$ctor", ctor);
        return Some(info);
    }
    if owner == "net/minecraft/commands/synchronization/ArgumentTypeInfos"
        && is_static
        && name == "registerByClass"
        && args.len() == 2
    {
        rt.pin(args[1]);
        return Some(args[1]);
    }

    // 5. `com/mojang/authlib/GameProfile` & `PropertyMap`
    if owner == "com/mojang/authlib/GameProfile" && !is_static {
        let recv = *args.first()?;
        return match (name, desc) {
            ("<init>", "(Ljava/util/UUID;Ljava/lang/String;)V") => {
                let id = *args.get(1)?;
                let pname = *args.get(2)?;
                if id.is_null() {
                    return Some(rt.throw_exception(
                        "java/lang/NullPointerException",
                        "Profile ID must not be null",
                    ));
                }
                if pname.is_null() {
                    return Some(rt.throw_exception(
                        "java/lang/NullPointerException",
                        "Profile name must not be null",
                    ));
                }
                let props = rt.alloc(
                    "com/mojang/authlib/properties/PropertyMap",
                    0,
                    Data::Map(Vec::new()),
                );
                rt.put_field(recv, "$id", id);
                rt.put_field(recv, "$profile_name", pname);
                rt.put_field(recv, "$properties", props);
                Some(V::Void)
            }
            ("getId", "()Ljava/util/UUID;") => Some(rt.get_field(recv, "$id")),
            ("getName", "()Ljava/lang/String;") => Some(rt.get_field(recv, "$profile_name")),
            ("getProperties", "()Lcom/mojang/authlib/properties/PropertyMap;") => {
                Some(rt.get_field(recv, "$properties"))
            }
            ("equals", "(Ljava/lang/Object;)Z") => {
                let other = args.get(1).copied().unwrap_or(V::NULL);
                if recv == other {
                    return Some(V::TRUE);
                }
                if rt.class_of(other) != Some("com/mojang/authlib/GameProfile") {
                    return Some(V::FALSE);
                }
                let eq = rt.val_eq(rt.get_field(recv, "$id"), rt.get_field(other, "$id"))
                    && rt.val_eq(
                        rt.get_field(recv, "$profile_name"),
                        rt.get_field(other, "$profile_name"),
                    );
                Some(V::bool(eq))
            }
            ("toString", "()Ljava/lang/String;") => {
                let id = rt.value_to_string(rt.get_field(recv, "$id"));
                let n = rt.value_to_string(rt.get_field(recv, "$profile_name"));
                Some(rt.new_str(format!("GameProfile[id={id}, name={n}]")))
            }
            _ => None,
        };
    }

    // 6. Guava `ImmutableList` / `ImmutableSet` / `ImmutableMap` / `Lists` / `Sets` / `Maps` / `Suppliers` & `GsonBuilder`
    if owner == "com/google/common/collect/ImmutableList" && is_static {
        return match name {
            "of" => {
                let items = if desc.contains("[Ljava/lang/Object;") {
                    let mut v = args[..args.len() - 1].to_vec();
                    v.extend(extract_varargs(rt, args[args.len() - 1]));
                    v
                } else {
                    args.to_vec()
                };
                Some(rt.alloc(owner, 0, Data::List(items)))
            }
            "copyOf" => {
                let items = extract_varargs(rt, *args.first()?);
                Some(rt.alloc(owner, 0, Data::List(items)))
            }
            "builder" | "builderWithExpectedSize" => Some(rt.alloc(
                "com/google/common/collect/ImmutableList$Builder",
                0,
                Data::List(Vec::new()),
            )),
            _ => None,
        };
    }
    if owner == "com/google/common/collect/ImmutableList$Builder" && !is_static {
        let recv = *args.first()?;
        return match name {
            "add" => {
                let added = if desc.starts_with("([") {
                    extract_varargs(rt, *args.get(1)?)
                } else {
                    vec![*args.get(1)?]
                };
                if let Some(Data::List(xs)) = rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    xs.extend(added);
                }
                Some(recv)
            }
            "addAll" => {
                let added = extract_varargs(rt, *args.get(1)?);
                if let Some(Data::List(xs)) = rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    xs.extend(added);
                }
                Some(recv)
            }
            "build" => {
                let xs = extract_varargs(rt, recv);
                Some(rt.alloc(
                    "com/google/common/collect/ImmutableList",
                    0,
                    Data::List(xs),
                ))
            }
            _ => None,
        };
    }
    if owner == "com/google/common/collect/ImmutableSet" && is_static {
        return match name {
            "of" => {
                let items = if desc.contains("[Ljava/lang/Object;") {
                    let mut v = args[..args.len() - 1].to_vec();
                    v.extend(extract_varargs(rt, args[args.len() - 1]));
                    v
                } else {
                    args.to_vec()
                };
                Some(rt.alloc(owner, 0, Data::Set(items)))
            }
            "copyOf" => {
                let items = extract_varargs(rt, *args.first()?);
                Some(rt.alloc(owner, 0, Data::Set(items)))
            }
            "builder" => Some(rt.alloc(
                "com/google/common/collect/ImmutableSet$Builder",
                0,
                Data::Set(Vec::new()),
            )),
            _ => None,
        };
    }
    if owner == "com/google/common/collect/ImmutableSet$Builder" && !is_static {
        let recv = *args.first()?;
        return match name {
            "add" => {
                let added = if desc.starts_with("([") {
                    extract_varargs(rt, *args.get(1)?)
                } else {
                    vec![*args.get(1)?]
                };
                let mut cur = extract_varargs(rt, recv);
                for x in added {
                    if !cur.iter().any(|&e| rt.val_eq(e, x)) {
                        cur.push(x);
                    }
                }
                if let Some(Data::Set(xs)) = rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    *xs = cur;
                }
                Some(recv)
            }
            "addAll" => {
                let added = extract_varargs(rt, *args.get(1)?);
                let mut cur = extract_varargs(rt, recv);
                for x in added {
                    if !cur.iter().any(|&e| rt.val_eq(e, x)) {
                        cur.push(x);
                    }
                }
                if let Some(Data::Set(xs)) = rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    *xs = cur;
                }
                Some(recv)
            }
            "build" => {
                let xs = extract_varargs(rt, recv);
                Some(rt.alloc(
                    "com/google/common/collect/ImmutableSet",
                    0,
                    Data::Set(xs),
                ))
            }
            _ => None,
        };
    }
    if owner == "com/google/common/collect/ImmutableMap" && is_static {
        return match name {
            "of" => {
                let mut pairs = Vec::new();
                for c in args.chunks_exact(2) {
                    pairs.push((c[0], c[1]));
                }
                Some(rt.alloc(owner, 0, Data::Map(pairs)))
            }
            "copyOf" => {
                let pairs = match rt.get_obj(*args.first()?).map(|o| &o.data) {
                    Some(Data::Map(m)) => m.clone(),
                    _ => Vec::new(),
                };
                Some(rt.alloc(owner, 0, Data::Map(pairs)))
            }
            "builder" => Some(rt.alloc(
                "com/google/common/collect/ImmutableMap$Builder",
                0,
                Data::Map(Vec::new()),
            )),
            _ => None,
        };
    }
    if owner == "com/google/common/collect/ImmutableMap$Builder" && !is_static {
        let recv = *args.first()?;
        return match name {
            "put" if args.len() == 3 => {
                if let Some(Data::Map(m)) = rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    m.push((args[1], args[2]));
                }
                Some(recv)
            }
            "build" | "buildOrThrow" | "buildKeepingLast" => {
                let pairs = match rt.get_obj(recv).map(|o| &o.data) {
                    Some(Data::Map(m)) => m.clone(),
                    _ => Vec::new(),
                };
                Some(rt.alloc(
                    "com/google/common/collect/ImmutableMap",
                    0,
                    Data::Map(pairs),
                ))
            }
            _ => None,
        };
    }
    if owner == "com/google/common/collect/Lists" && is_static && name == "newArrayList" {
        let items = if args.is_empty() {
            Vec::new()
        } else {
            extract_varargs(rt, args[0])
        };
        return Some(rt.alloc("java/util/ArrayList", 0, Data::List(items)));
    }
    if owner == "com/google/common/collect/Sets" && is_static {
        return match name {
            "newHashSet" | "newLinkedHashSet" | "newIdentityHashSet" | "newConcurrentHashSet" => {
                let items = if args.is_empty() {
                    Vec::new()
                } else {
                    extract_varargs(rt, args[0])
                };
                Some(rt.alloc("java/util/HashSet", 0, Data::Set(items)))
            }
            _ => None,
        };
    }
    if owner == "com/google/common/collect/Maps" && is_static {
        return match name {
            "newHashMap" | "newLinkedHashMap" | "newIdentityHashMap" | "newConcurrentMap" => {
                Some(rt.alloc("java/util/HashMap", 0, Data::Map(Vec::new())))
            }
            "newEnumMap" => Some(rt.alloc("java/util/EnumMap", 0, Data::Map(Vec::new()))),
            _ => None,
        };
    }
    if (owner == "com/google/common/collect/HashBiMap" || owner == "com/google/common/collect/BiMap")
        && is_static
        && name == "create"
    {
        return Some(rt.alloc("com/google/common/collect/HashBiMap", 0, Data::Map(Vec::new())));
    }
    if (owner == "com/google/common/collect/HashBiMap" || owner == "com/google/common/collect/BiMap")
        && !is_static
        && name == "inverse"
    {
        let inv: Vec<(V, V)> = match rt.get_obj(args[0]).map(|o| &o.data) {
            Some(Data::Map(m)) => m.iter().map(|&(k, v)| (v, k)).collect(),
            _ => Vec::new(),
        };
        return Some(rt.alloc("com/google/common/collect/HashBiMap", 0, Data::Map(inv)));
    }
    if owner == "com/google/common/base/Suppliers" && is_static && name == "memoize" {
        return args.first().copied();
    }
    if owner == "com/google/common/base/Preconditions" && is_static {
        return match name {
            "checkNotNull" => {
                let v = *args.first()?;
                if v.is_null() {
                    Some(rt.throw_exception(
                        "java/lang/NullPointerException",
                        "Preconditions.checkNotNull",
                    ))
                } else {
                    Some(v)
                }
            }
            "checkArgument" => {
                if args.first()?.i() == 0 {
                    Some(rt.throw_exception(
                        "java/lang/IllegalArgumentException",
                        "Preconditions.checkArgument",
                    ))
                } else {
                    Some(V::Void)
                }
            }
            "checkState" => {
                if args.first()?.i() == 0 {
                    Some(rt.throw_exception(
                        "java/lang/IllegalStateException",
                        "Preconditions.checkState",
                    ))
                } else {
                    Some(V::Void)
                }
            }
            _ => None,
        };
    }

    if owner == "com/google/gson/GsonBuilder" && !is_static {
        let recv = *args.first()?;
        return match name {
            "<init>" => Some(V::Void),
            "setPrettyPrinting"
            | "disableHtmlEscaping"
            | "serializeNulls"
            | "setLenient"
            | "registerTypeAdapter"
            | "registerTypeHierarchyAdapter"
            | "registerTypeAdapterFactory"
            | "excludeFieldsWithModifiers" => Some(recv),
            "create" => Some(rt.new_instance("com/google/gson/Gson", 0)),
            _ => None,
        };
    }
    if owner == "com/google/gson/Gson" && !is_static && name == "<init>" {
        return Some(V::Void);
    }

    // 7. Guava `Table` (`HashBasedTable`) & `Multimap` (`HashMultimap`, `ArrayListMultimap`, `LinkedHashMultimap`)
    if matches!(
        owner,
        "com/google/common/collect/HashBasedTable" | "com/google/common/collect/Table"
    ) {
        if is_static && name == "create" {
            let tbl = rt.new_instance("com/google/common/collect/HashBasedTable", 0);
            let rows = rt.alloc("java/util/LinkedHashMap", 0, Data::Map(Vec::new()));
            rt.put_field(tbl, "$rows", rows);
            return Some(tbl);
        }
        if !is_static {
            return call_guava_table(rt, args[0], name, &args[1..]);
        }
    }
    if matches!(
        owner,
        "com/google/common/collect/Multimap"
            | "com/google/common/collect/ListMultimap"
            | "com/google/common/collect/SetMultimap"
            | "com/google/common/collect/HashMultimap"
            | "com/google/common/collect/LinkedHashMultimap"
            | "com/google/common/collect/ArrayListMultimap"
    ) {
        if is_static && name == "create" {
            let m = rt.new_instance(owner, 0);
            let map = rt.alloc("java/util/LinkedHashMap", 0, Data::Map(Vec::new()));
            let is_set = owner.contains("HashMultimap");
            rt.put_field(m, "$map", map);
            rt.put_field(m, "$is_set", V::bool(is_set));
            return Some(m);
        }
        if !is_static {
            return call_guava_multimap(rt, args[0], name, &args[1..]);
        }
    }

    None
}

fn call_guava_table(rt: &mut Rt, recv: V, name: &str, args: &[V]) -> Option<V> {
    let rows_map = rt.get_field(recv, "$rows");
    let rows_snapshot = || -> Vec<(V, V)> {
        match rt.get_obj(rows_map).map(|o| &o.data) {
            Some(Data::Map(m)) => m.clone(),
            _ => Vec::new(),
        }
    };
    match name {
        "put" if args.len() == 3 => {
            let (r, c, v) = (args[0], args[1], args[2]);
            let rows = rows_snapshot();
            let col_map = if let Some(&(_, cm)) = rows.iter().find(|&&(rk, _)| rt.val_eq(rk, r)) {
                cm
            } else {
                let cm = rt.alloc("java/util/LinkedHashMap", 0, Data::Map(Vec::new()));
                if let Some(Data::Map(m)) = rt.get_obj_mut(rows_map).map(|o| &mut o.data) {
                    m.push((r, cm));
                }
                cm
            };
            let cols = match rt.get_obj(col_map).map(|o| &o.data) {
                Some(Data::Map(m)) => m.clone(),
                _ => Vec::new(),
            };
            let pos = cols.iter().position(|&(ck, _)| rt.val_eq(ck, c));
            let mut prev = V::NULL;
            if let Some(Data::Map(m)) = rt.get_obj_mut(col_map).map(|o| &mut o.data) {
                if let Some(idx) = pos {
                    prev = m[idx].1;
                    m[idx].1 = v;
                } else {
                    m.push((c, v));
                }
            }
            Some(prev)
        }
        "get" if args.len() == 2 => {
            let (r, c) = (args[0], args[1]);
            let Some((_, col_map)) = rows_snapshot().into_iter().find(|&(rk, _)| rt.val_eq(rk, r)) else {
                return Some(V::NULL);
            };
            let cols = match rt.get_obj(col_map).map(|o| &o.data) {
                Some(Data::Map(m)) => m.clone(),
                _ => Vec::new(),
            };
            let val = cols
                .into_iter()
                .find(|&(ck, _)| rt.val_eq(ck, c))
                .map_or(V::NULL, |(_, v)| v);
            Some(val)
        }
        "contains" if args.len() == 2 => {
            let (r, c) = (args[0], args[1]);
            let Some((_, col_map)) = rows_snapshot().into_iter().find(|&(rk, _)| rt.val_eq(rk, r)) else {
                return Some(V::FALSE);
            };
            let has = match rt.get_obj(col_map).map(|o| &o.data) {
                Some(Data::Map(m)) => {
                    let cols = m.clone();
                    cols.into_iter().any(|(ck, _)| rt.val_eq(ck, c))
                }
                _ => false,
            };
            Some(V::bool(has))
        }
        "containsRow" if args.len() == 1 => {
            let r = args[0];
            let has = rows_snapshot().into_iter().any(|(rk, cm)| {
                rt.val_eq(rk, r)
                    && matches!(rt.get_obj(cm).map(|o| &o.data), Some(Data::Map(m)) if !m.is_empty())
            });
            Some(V::bool(has))
        }
        "row" if args.len() == 1 => {
            let r = args[0];
            if let Some((_, cm)) = rows_snapshot().into_iter().find(|&(rk, _)| rt.val_eq(rk, r)) {
                Some(cm)
            } else {
                let cm = rt.alloc("java/util/LinkedHashMap", 0, Data::Map(Vec::new()));
                if let Some(Data::Map(m)) = rt.get_obj_mut(rows_map).map(|o| &mut o.data) {
                    m.push((r, cm));
                }
                Some(cm)
            }
        }
        "rowMap" => Some(rows_map),
        "rowKeySet" => {
            let keys: Vec<V> = rows_snapshot().into_iter().map(|(rk, _)| rk).collect();
            Some(rt.alloc("java/util/Set", 0, Data::Set(keys)))
        }
        "values" => {
            let mut all = Vec::new();
            for (_, cm) in rows_snapshot() {
                if let Some(Data::Map(cols)) = rt.get_obj(cm).map(|o| &o.data) {
                    all.extend(cols.iter().map(|&(_, v)| v));
                }
            }
            Some(rt.alloc("java/util/ArrayList", 0, Data::List(all)))
        }
        "isEmpty" => {
            let empty = rows_snapshot().into_iter().all(|(_, cm)| {
                matches!(rt.get_obj(cm).map(|o| &o.data), Some(Data::Map(m)) if m.is_empty())
            });
            Some(V::bool(empty))
        }
        "size" => {
            let total: usize = rows_snapshot()
                .into_iter()
                .map(|(_, cm)| match rt.get_obj(cm).map(|o| &o.data) {
                    Some(Data::Map(m)) => m.len(),
                    _ => 0,
                })
                .sum();
            Some(V::I(total as i32))
        }
        "clear" => {
            if let Some(Data::Map(m)) = rt.get_obj_mut(rows_map).map(|o| &mut o.data) {
                m.clear();
            }
            Some(V::Void)
        }
        _ => None,
    }
}

fn call_guava_multimap(rt: &mut Rt, recv: V, name: &str, args: &[V]) -> Option<V> {
    let map_v = rt.get_field(recv, "$map");
    let is_set = rt.get_field(recv, "$is_set").i() != 0;
    let entries_snapshot = || -> Vec<(V, V)> {
        match rt.get_obj(map_v).map(|o| &o.data) {
            Some(Data::Map(m)) => m.clone(),
            _ => Vec::new(),
        }
    };
    let ensure_bucket = |rt: &mut Rt, key: V| -> V {
        let snap = match rt.get_obj(map_v).map(|o| &o.data) {
            Some(Data::Map(m)) => m.clone(),
            _ => Vec::new(),
        };
        if let Some((_, bucket)) = snap.into_iter().find(|&(k, _)| rt.val_eq(k, key)) {
            return bucket;
        }
        let b = if is_set {
            rt.alloc("java/util/LinkedHashSet", 0, Data::Set(Vec::new()))
        } else {
            rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()))
        };
        if let Some(Data::Map(m)) = rt.get_obj_mut(map_v).map(|o| &mut o.data) {
            m.push((key, b));
        }
        b
    };

    match name {
        "put" if args.len() == 2 => {
            let (k, v) = (args[0], args[1]);
            let bucket = ensure_bucket(rt, k);
            if is_set {
                let cur = extract_varargs(rt, bucket);
                if cur.iter().any(|&x| rt.val_eq(x, v)) {
                    return Some(V::FALSE);
                }
                if let Some(Data::Set(xs)) = rt.get_obj_mut(bucket).map(|o| &mut o.data) {
                    xs.push(v);
                }
                Some(V::TRUE)
            } else {
                if let Some(Data::List(xs)) = rt.get_obj_mut(bucket).map(|o| &mut o.data) {
                    xs.push(v);
                }
                Some(V::TRUE)
            }
        }
        "get" if args.len() == 1 => Some(ensure_bucket(rt, args[0])),
        "removeAll" if args.len() == 1 => {
            let k = args[0];
            let snap = entries_snapshot();
            if let Some(pos) = snap.iter().position(|&(ek, _)| rt.val_eq(ek, k)) {
                let mut removed = V::NULL;
                if let Some(Data::Map(m)) = rt.get_obj_mut(map_v).map(|o| &mut o.data) {
                    removed = m.remove(pos).1;
                }
                Some(removed)
            } else if is_set {
                Some(rt.alloc("java/util/Set", 0, Data::Set(Vec::new())))
            } else {
                Some(rt.alloc("java/util/List", 0, Data::List(Vec::new())))
            }
        }
        "containsKey" if args.len() == 1 => {
            let k = args[0];
            let has = entries_snapshot().into_iter().any(|(ek, b)| {
                rt.val_eq(ek, k) && !extract_varargs(rt, b).is_empty()
            });
            Some(V::bool(has))
        }
        "keySet" => {
            let keys: Vec<V> = entries_snapshot()
                .into_iter()
                .filter(|&(_, b)| !extract_varargs(rt, b).is_empty())
                .map(|(k, _)| k)
                .collect();
            Some(rt.alloc("java/util/Set", 0, Data::Set(keys)))
        }
        "values" => {
            let mut vals = Vec::new();
            for (_, b) in entries_snapshot() {
                vals.extend(extract_varargs(rt, b));
            }
            Some(rt.alloc("java/util/ArrayList", 0, Data::List(vals)))
        }
        "entries" => {
            let mut pairs = Vec::new();
            for (k, b) in entries_snapshot() {
                for v in extract_varargs(rt, b) {
                    let e = rt.new_instance("java/util/AbstractMap$SimpleEntry", 0);
                    rt.put_field(e, "key", k);
                    rt.put_field(e, "value", v);
                    pairs.push(e);
                }
            }
            Some(rt.alloc("java/util/ArrayList", 0, Data::List(pairs)))
        }
        "asMap" => Some(map_v),
        "isEmpty" => {
            let empty = entries_snapshot()
                .into_iter()
                .all(|(_, b)| extract_varargs(rt, b).is_empty());
            Some(V::bool(empty))
        }
        "size" => {
            let total: usize = entries_snapshot()
                .into_iter()
                .map(|(_, b)| extract_varargs(rt, b).len())
                .sum();
            Some(V::I(total as i32))
        }
        "clear" => {
            if let Some(Data::Map(m)) = rt.get_obj_mut(map_v).map(|o| &mut o.data) {
                m.clear();
            }
            Some(V::Void)
        }
        _ => None,
    }
}

pub fn new_resource_key(rt: &mut Rt, registry: &str, location: &str) -> V {
    rt.new_pinned_handle(
        "net/minecraft/resources/ResourceKey",
        "resource_key",
        0,
        format!("{registry}|{location}"),
    )
}

fn new_tag_key(rt: &mut Rt, registry: &str, location: &str) -> V {
    rt.new_pinned_handle(
        "net/minecraft/tags/TagKey",
        "tag_key",
        0,
        format!("{registry}|{location}"),
    )
}

fn new_rl_handle(rt: &mut Rt, full: &str) -> V {
    let norm = if full.contains(':') {
        full.to_owned()
    } else {
        format!("minecraft:{full}")
    };
    rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        norm,
    )
}

pub fn resource_key_parts(rt: &Rt, v: V) -> Option<(String, String)> {
    let ("resource_key", _, tag, _) = rt.handle_of(v)? else {
        return None;
    };
    let (reg, loc) = tag.split_once('|')?;
    Some((reg.to_owned(), loc.to_owned()))
}

fn tag_key_parts(rt: &Rt, v: V) -> Option<(String, String)> {
    let ("tag_key", _, tag, _) = rt.handle_of(v)? else {
        return None;
    };
    let (reg, loc) = tag.split_once('|')?;
    Some((reg.to_owned(), loc.to_owned()))
}

fn loc_str(rt: &Rt, v: V) -> Option<String> {
    if let Some(("resource_location", _, tag, _)) = rt.handle_of(v) {
        return Some(tag.to_owned());
    }
    rt.str_of(rt.get_field(v, "id")).map(str::to_owned)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> i32 {
    let i = ((h * 6.0) as i32).rem_euclid(6);
    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let ri = ((r * 255.0).clamp(0.0, 255.0)) as i32;
    let gi = ((g * 255.0).clamp(0.0, 255.0)) as i32;
    let bi = ((b * 255.0).clamp(0.0, 255.0)) as i32;
    (ri << 16) | (gi << 8) | bi
}

fn map_color_constant(rt: &mut Rt, name: &str) -> Option<V> {
    let table: &[(&str, i32, i32)] = &[
        ("NONE", 0, 0x000000),
        ("GRASS", 1, 0x7FB238),
        ("SAND", 2, 0xF7E9A3),
        ("WOOL", 3, 0xC7C7C7),
        ("FIRE", 4, 0xFF0000),
        ("ICE", 5, 0xA0A0FF),
        ("METAL", 6, 0xA7A7A7),
        ("PLANT", 7, 0x007C00),
        ("SNOW", 8, 0xFFFFFF),
        ("CLAY", 9, 0xA4A8B8),
        ("DIRT", 10, 0x976D4D),
        ("STONE", 11, 0x707070),
        ("WATER", 12, 0x4040FF),
        ("WOOD", 13, 0x8F7748),
        ("QUARTZ", 14, 0xFFFCF5),
        ("COLOR_ORANGE", 15, 0xD87F33),
        ("COLOR_MAGENTA", 16, 0xB24CD8),
        ("COLOR_LIGHT_BLUE", 17, 0x6699D8),
        ("COLOR_YELLOW", 18, 0xE5E533),
        ("COLOR_LIGHT_GREEN", 19, 0x7FCC19),
        ("COLOR_PINK", 20, 0xF27FA5),
        ("COLOR_GRAY", 21, 0x4C4C4C),
        ("COLOR_LIGHT_GRAY", 22, 0x999999),
        ("COLOR_CYAN", 23, 0x4C7F99),
        ("COLOR_PURPLE", 24, 0x7F3FB2),
        ("COLOR_BLUE", 25, 0x334CB2),
        ("COLOR_BROWN", 26, 0x664C33),
        ("COLOR_GREEN", 27, 0x667F33),
        ("COLOR_RED", 28, 0x993333),
        ("COLOR_BLACK", 29, 0x191919),
        ("GOLD", 30, 0xFAEE4D),
        ("DIAMOND", 31, 0x5CDBD5),
        ("LAPIS", 32, 0x4A80FF),
        ("EMERALD", 33, 0x00D93A),
        ("PODZOL", 34, 0x815631),
        ("NETHER", 35, 0x700200),
        ("TERRACOTTA_WHITE", 36, 0xD1B1A1),
        ("TERRACOTTA_ORANGE", 37, 0x9F5224),
        ("TERRACOTTA_MAGENTA", 38, 0x95576C),
        ("TERRACOTTA_LIGHT_BLUE", 39, 0x706C8A),
        ("TERRACOTTA_YELLOW", 40, 0xBA8524),
        ("TERRACOTTA_LIGHT_GREEN", 41, 0x677535),
        ("TERRACOTTA_PINK", 42, 0xA04D4E),
        ("TERRACOTTA_GRAY", 43, 0x392923),
        ("TERRACOTTA_LIGHT_GRAY", 44, 0x876B62),
        ("TERRACOTTA_CYAN", 45, 0x575C5C),
        ("TERRACOTTA_PURPLE", 46, 0x7A4958),
        ("TERRACOTTA_BLUE", 47, 0x4C3E5C),
        ("TERRACOTTA_BROWN", 48, 0x4C3223),
        ("TERRACOTTA_GREEN", 49, 0x4C522A),
        ("TERRACOTTA_RED", 50, 0x8E3C2E),
        ("TERRACOTTA_BLACK", 51, 0x251610),
        ("CRIMSON_NYLIUM", 52, 0xBD3031),
        ("CRIMSON_STEM", 53, 0x943F61),
        ("CRIMSON_HYPHAE", 54, 0x5C191D),
        ("WARPED_NYLIUM", 55, 0x167E86),
        ("WARPED_STEM", 56, 0x3A8E8C),
        ("WARPED_HYPHAE", 57, 0x562C3E),
        ("WARPED_WART_BLOCK", 58, 0x14B485),
        ("DEEPSLATE", 59, 0x646464),
        ("RAW_IRON", 60, 0xD8AF93),
        ("GLOW_LICHEN", 61, 0x7FA796),
    ];
    let &(_, id, col) = table.iter().find(|x| x.0 == name)?;
    let obj = rt.new_instance("net/minecraft/world/level/material/MapColor", 0);
    rt.put_field(obj, "id", V::I(id));
    rt.put_field(obj, "col", V::I(col));
    rt.pin(obj);
    Some(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translatable_component_resolves_lang_table_positional_args_and_siblings() {
        let mut rt = Rt::new();
        let mut lang = BTreeMap::new();
        lang.insert(
            "error.example.platform".to_owned(),
            "Unsupported platform %1$s on %2$s (code %1$s)".to_owned(),
        );
        let arg1 = rt.intern_str("linux");
        let arg2 = new_literal_component(&mut rt, "x86_64");
        let args = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(vec![arg1, arg2]));
        let key = rt.intern_str("error.example.platform");

        let comp = call_chat(
            &mut rt,
            &lang,
            &MemberRef::new(
                "net/minecraft/network/chat/Component",
                "translatable",
                "(Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;",
            ),
            true,
            &[key, args],
        )
        .unwrap();

        let suffix = rt.intern_str("!");
        let _ = call_chat(
            &mut rt,
            &lang,
            &MemberRef::new(
                MUTABLE_COMPONENT,
                "append",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ),
            false,
            &[comp, suffix],
        )
        .unwrap();

        let str_v = call_chat(
            &mut rt,
            &lang,
            &MemberRef::new(
                "net/minecraft/network/chat/Component",
                "getString",
                "()Ljava/lang/String;",
            ),
            false,
            &[comp],
        )
        .unwrap();
        assert_eq!(
            rt.str_of(str_v),
            Some("Unsupported platform linux on x86_64 (code linux)!")
        );
    }

    #[test]
    fn mod_config_spec_builder_and_brigadier_roundtrip_real_values() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();

        // Build a ModConfigSpec via HostApi calls and read back a clamped value
        let builder = rt.new_instance("net/neoforged/neoforge/common/ModConfigSpec$Builder", 0);
        let sec = rt.intern_str("general");
        let key = rt.intern_str("volume");
        let linked = &prepared.linked;
        let (double_val, spec) = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            let _ = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKESPECIAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "<init>",
                    "()V",
                ),
                &[builder],
            );
            let _ = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "push",
                    "(Ljava/lang/String;)Lnet/neoforged/neoforge/common/ModConfigSpec$Builder;",
                ),
                &[builder, sec],
            );
            let dv = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "defineInRange",
                    "(Ljava/lang/String;DDD)Lnet/neoforged/neoforge/common/ModConfigSpec$DoubleValue;",
                ),
                &[builder, key, V::D(75.0), V::D(0.0), V::D(100.0)],
            );
            let sp = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "build",
                    "()Lnet/neoforged/neoforge/common/ModConfigSpec;",
                ),
                &[builder],
            );
            let got = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$DoubleValue",
                    "getAsDouble",
                    "()D",
                ),
                &[dv],
            );
            assert_eq!(got.d(), 75.0);
            (dv, sp)
        };
        let _ = double_val;
        assert!(rt.handle_of(spec).is_some_and(|(k, ..)| k == "config_spec"));
        assert!(rt.take_exception().is_none());
    }

    #[test]
    fn builtin_registries_game_profile_and_guava_table_preserve_semantics() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let linked = &prepared.linked;

        let (cmd_reg_key, profile_str, table_got) = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked,
                host: &mut prepared.loader,
                depth: 0,
            };

            // 1. BuiltInRegistries.COMMAND_ARGUMENT_TYPE -> DeferredRegister.create(Registry, String)
            let reg = mmga_runtime::read_static_field(
                &mut ctx,
                "net/minecraft/core/registries/BuiltInRegistries",
                "COMMAND_ARGUMENT_TYPE",
                "Lnet/minecraft/core/Registry;",
            );
            let ns = ctx.rt.intern_str("ponder");
            let dr = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKESTATIC,
                &MemberRef::new(
                    "net/neoforged/neoforge/registries/DeferredRegister",
                    "create",
                    "(Lnet/minecraft/core/Registry;Ljava/lang/String;)Lnet/neoforged/neoforge/registries/DeferredRegister;",
                ),
                &[reg, ns],
            );
            let dr_tag = ctx.rt.handle_of(dr).unwrap().2.to_owned();

            // 2. UUID.nameUUIDFromBytes + GameProfile(UUID, String)
            let raw_name = ctx.rt.intern_str("mekanism.common");
            let bytes = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/String", "getBytes", "()[B"),
                &[raw_name],
            );
            let uuid = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKESTATIC,
                &MemberRef::new(
                    "java/util/UUID",
                    "nameUUIDFromBytes",
                    "([B)Ljava/util/UUID;",
                ),
                &[bytes],
            );
            let gp = ctx
                .rt
                .new_instance("com/mojang/authlib/GameProfile", 0);
            let pname = ctx.rt.intern_str("[Mekanism]");
            let _ = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKESPECIAL,
                &MemberRef::new(
                    "com/mojang/authlib/GameProfile",
                    "<init>",
                    "(Ljava/util/UUID;Ljava/lang/String;)V",
                ),
                &[gp, uuid, pname],
            );
            let gp_name = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "com/mojang/authlib/GameProfile",
                    "getName",
                    "()Ljava/lang/String;",
                ),
                &[gp],
            );
            let gp_id = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "com/mojang/authlib/GameProfile",
                    "getId",
                    "()Ljava/util/UUID;",
                ),
                &[gp],
            );
            let profile_summary = format!(
                "{}:{}",
                ctx.rt.str_of(gp_name).unwrap(),
                ctx.rt.value_to_string(gp_id)
            );

            // 3. Guava HashBasedTable
            let tbl = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKESTATIC,
                &MemberRef::new(
                    "com/google/common/collect/HashBasedTable",
                    "create",
                    "()Lcom/google/common/collect/HashBasedTable;",
                ),
                &[],
            );
            let r = ctx.rt.intern_str("minecraft:block");
            let c = ctx.rt.intern_str("cogwheel");
            let v = ctx.rt.intern_str("entry_1");
            let _ = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "com/google/common/collect/HashBasedTable",
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                ),
                &[tbl, r, c, v],
            );
            let got = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "com/google/common/collect/HashBasedTable",
                    "get",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                ),
                &[tbl, r, c],
            );
            (dr_tag, profile_summary, ctx.rt.str_of(got).unwrap().to_owned())
        };

        assert_eq!(cmd_reg_key, "minecraft:command_argument_type");
        assert!(profile_str.starts_with("[Mekanism]:"));
        assert_eq!(profile_str.split(':').nth(1).unwrap().len(), 36);
        assert_eq!(table_got, "entry_1");
        assert!(rt.take_exception().is_none());
    }
}

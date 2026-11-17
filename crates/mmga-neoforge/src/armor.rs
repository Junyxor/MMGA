//! Minecraft 1.21.1 armor values, checked against official Mojang bytecode.
//!
//! `ArmorMaterial` = `csg`, `ArmorMaterials` = `csh`, `ArmorItem$Type` = `csf$a`,
//! `ArmorMaterial$Layer` = `csg$a`. Client SHA-1:
//! 30c73b1c5da787909b2f73340419fdf13b9def88; mappings SHA-1:
//! 2244b6f072256667bcd9a73df124d6c58de77992. This module implements values,
//! not armor rendering or ingredient matching.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{
    box_primitive, invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, Rt, V,
};

const MATERIAL: &str = "net/minecraft/world/item/ArmorMaterial";
const MATERIALS: &str = "net/minecraft/world/item/ArmorMaterials";
const TYPE: &str = "net/minecraft/world/item/ArmorItem$Type";
const TYPE_DESC: &str = "Lnet/minecraft/world/item/ArmorItem$Type;";
const LAYER: &str = "net/minecraft/world/item/ArmorMaterial$Layer";
const HOLDER_DESC: &str = "Lnet/minecraft/core/Holder;";
const TYPES: [&str; 5] = ["HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS", "BODY"];
const SLOT: &str = "net/minecraft/world/entity/EquipmentSlot";
const SLOT_KIND: &str = "net/minecraft/world/entity/EquipmentSlot$Type";
const REPAIR_FACTORY: &str = "mmga/internal/ArmorRepairIngredient";

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if owner == TYPE && desc == TYPE_DESC {
        return type_constant(rt, name);
    }
    if owner == SLOT && desc == "Lnet/minecraft/world/entity/EquipmentSlot;" {
        return slot_constant(rt, name);
    }
    if owner == SLOT_KIND && desc == "Lnet/minecraft/world/entity/EquipmentSlot$Type;" {
        let ordinal = ["HAND", "HUMANOID_ARMOR", "ANIMAL_ARMOR"]
            .iter()
            .position(|&n| n == name)?;
        return Some(enum_constant(rt, SLOT_KIND, name, ordinal));
    }
    if owner == "net/minecraft/sounds/SoundEvents" && desc == HOLDER_DESC {
        return armor_sound(rt, registries, name);
    }
    if owner != MATERIALS || desc != HOLDER_DESC {
        return None;
    }
    let (path, defense, enchantment, toughness, knockback, sound, ingredient) = match name {
        "LEATHER" => (
            "leather",
            [1, 3, 2, 1, 3],
            15,
            0.0,
            0.0,
            "LEATHER",
            "leather",
        ),
        "CHAIN" => (
            "chainmail",
            [2, 5, 4, 1, 4],
            12,
            0.0,
            0.0,
            "CHAIN",
            "iron_ingot",
        ),
        "IRON" => ("iron", [2, 6, 5, 2, 5], 9, 0.0, 0.0, "IRON", "iron_ingot"),
        "GOLD" => ("gold", [2, 5, 3, 1, 7], 25, 0.0, 0.0, "GOLD", "gold_ingot"),
        "DIAMOND" => (
            "diamond",
            [3, 8, 6, 3, 11],
            10,
            2.0,
            0.0,
            "DIAMOND",
            "diamond",
        ),
        "TURTLE" => (
            "turtle",
            [2, 6, 5, 2, 5],
            9,
            0.0,
            0.0,
            "TURTLE",
            "turtle_scute",
        ),
        "NETHERITE" => (
            "netherite",
            [3, 8, 6, 3, 11],
            15,
            3.0,
            0.1,
            "NETHERITE",
            "netherite_ingot",
        ),
        "ARMADILLO" => (
            "armadillo",
            [3, 8, 6, 3, 11],
            10,
            0.0,
            0.0,
            "WOLF",
            "armadillo_scute",
        ),
        _ => return None,
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let entries = TYPES
        .into_iter()
        .zip(defense)
        .map(|(kind, amount)| {
            let key = type_constant(rt, kind).unwrap();
            let value = box_primitive(rt, "java/lang/Integer", V::I(amount));
            (key, value)
        })
        .collect();
    let defense = rt.alloc("java/util/EnumMap", 0, Data::Map(entries));
    let sound = armor_sound(rt, registries, &format!("ARMOR_EQUIP_{sound}"))?;
    let ingredient_id = rt.intern_str(&format!("minecraft:{ingredient}"));
    let repair = rt.alloc(
        "java/util/function/Supplier",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Supplier".into(),
            sam_name: "get".into(),
            sam_desc: "()Ljava/lang/Object;".into(),
            instantiated_desc: "()Lnet/minecraft/world/item/crafting/Ingredient;".into(),
            captures: vec![ingredient_id],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(
                    REPAIR_FACTORY,
                    "create",
                    "(Ljava/lang/String;)Lnet/minecraft/world/item/crafting/Ingredient;",
                ),
            },
        })),
    );
    let asset = resource_location(rt, &format!("minecraft:{path}"));
    let empty = rt.intern_str("");
    let layer = rt.new_instance(LAYER, 0);
    initialize_layer(rt, layer, asset, empty, V::bool(name == "LEATHER"));
    let mut layers = vec![layer];
    if name == "LEATHER" {
        let overlay = rt.new_instance(LAYER, 0);
        let suffix = rt.intern_str("_overlay");
        initialize_layer(rt, overlay, asset, suffix, V::FALSE);
        layers.push(overlay);
    }
    // ArmorMaterials uses List.of (one layer, or leather's two layers).
    let layers = rt.alloc(
        "java/util/ImmutableCollections$List12",
        0,
        Data::List(layers),
    );
    let material = rt.new_instance(MATERIAL, 0);
    initialize_material(
        rt,
        material,
        &[
            defense,
            V::I(enchantment),
            sound,
            repair,
            layers,
            V::F(toughness),
            V::F(knockback),
        ],
    );
    let id = format!("minecraft:{path}");
    registries.register_direct(
        rt,
        "minecraft:armor_material",
        &id,
        "minecraft",
        material,
        ContentProperties {
            kind: "armor_material".into(),
            template: id.clone(),
            ..ContentProperties::default()
        },
    );
    let holder = reference(rt, "minecraft:armor_material", &id, material);
    rt.put_static(owner, name, holder);
    Some(holder)
}

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let rt = &mut *ctx.rt;
    if member.owner == REPAIR_FACTORY
        && is_static
        && member.name == "create"
        && member.desc == "(Ljava/lang/String;)Lnet/minecraft/world/item/crafting/Ingredient;"
    {
        let item = rt
            .str_of(*args.first()?)
            .unwrap_or("<invalid item>")
            .to_owned();
        return Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            &format!("MMGA does not implement ArmorMaterial.repairIngredient creation for {item}"),
        ));
    }
    if member.owner == TYPE {
        return match (member.name.as_str(), member.desc.as_str(), is_static) {
            ("values", "()[Lnet/minecraft/world/item/ArmorItem$Type;", true) => {
                let values = TYPES
                    .into_iter()
                    .map(|name| type_constant(rt, name).unwrap())
                    .collect();
                Some(rt.alloc(
                    "[Lnet/minecraft/world/item/ArmorItem$Type;",
                    0,
                    Data::Array(values),
                ))
            }
            ("valueOf", "(Ljava/lang/String;)Lnet/minecraft/world/item/ArmorItem$Type;", true) => {
                let value = *args.first()?;
                if value.is_null() {
                    return Some(
                        rt.throw_exception("java/lang/NullPointerException", "Name is null"),
                    );
                }
                let name = rt.str_of(value)?.to_owned();
                Some(type_constant(rt, &name).unwrap_or_else(|| {
                    rt.throw_exception(
                        "java/lang/IllegalArgumentException",
                        &format!("No enum constant net.minecraft.world.item.ArmorItem.Type.{name}"),
                    )
                }))
            }
            ("getDurability", "(I)I", false) => Some(V::I(
                rt.get_field(*args.first()?, "durability")
                    .i()
                    .wrapping_mul(args.get(1)?.i()),
            )),
            ("getName" | "getSerializedName", "()Ljava/lang/String;", false) => {
                Some(rt.get_field(*args.first()?, "name"))
            }
            ("getSlot", "()Lnet/minecraft/world/entity/EquipmentSlot;", false) => {
                Some(rt.get_field(*args.first()?, "slot"))
            }
            ("hasTrims", "()Z", false) => {
                let recv = *args.first()?;
                Some(V::bool(
                    TYPES[..4]
                        .iter()
                        .any(|name| rt.get_static(TYPE, name) == Some(recv)),
                ))
            }
            _ => None,
        };
    }
    if member.owner == SLOT && !is_static {
        return match (member.name.as_str(), member.desc.as_str()) {
            ("getName" | "getSerializedName", "()Ljava/lang/String;") => {
                Some(rt.get_field(*args.first()?, "name"))
            }
            ("getType", "()Lnet/minecraft/world/entity/EquipmentSlot$Type;") => {
                Some(rt.get_field(*args.first()?, "type"))
            }
            ("getIndex", "()I") => Some(rt.get_field(*args.first()?, "index")),
            ("getIndex", "(I)I") => Some(V::I(
                rt.get_field(*args.first()?, "index")
                    .i()
                    .wrapping_add(args.get(1)?.i()),
            )),
            ("getFilterFlag", "()I") => Some(rt.get_field(*args.first()?, "filterFlag")),
            ("isArmor", "()Z") => {
                let kind = rt.get_field(*args.first()?, "type");
                let humanoid = enum_constant(rt, SLOT_KIND, "HUMANOID_ARMOR", 1);
                let animal = enum_constant(rt, SLOT_KIND, "ANIMAL_ARMOR", 2);
                Some(V::bool(kind == humanoid || kind == animal))
            }
            _ => None,
        };
    }
    if member.owner == LAYER && !is_static {
        return match (member.name.as_str(), member.desc.as_str()) {
            ("<init>", "(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/String;Z)V") => {
                Some(initialize_layer(
                    rt,
                    *args.first()?,
                    *args.get(1)?,
                    *args.get(2)?,
                    *args.get(3)?,
                ))
            }
            ("<init>", "(Lnet/minecraft/resources/ResourceLocation;)V") => {
                let suffix = rt.intern_str("");
                Some(initialize_layer(
                    rt,
                    *args.first()?,
                    *args.get(1)?,
                    suffix,
                    V::FALSE,
                ))
            }
            ("texture", "(Z)Lnet/minecraft/resources/ResourceLocation;") => Some(rt.get_field(
                *args.first()?,
                if args.get(1)?.i() != 0 {
                    "innerTexture"
                } else {
                    "outerTexture"
                },
            )),
            ("dyeable", "()Z") => Some(rt.get_field(*args.first()?, "dyeable")),
            _ => None,
        };
    }
    if member.owner != MATERIAL || is_static {
        return None;
    }
    let recv = *args.first()?;
    match (member.name.as_str(), member.desc.as_str()) {
        ("<init>", "(Ljava/util/Map;ILnet/minecraft/core/Holder;Ljava/util/function/Supplier;Ljava/util/List;FF)V") if args.len() == 8 => {
            initialize_material(rt, recv, &args[1..]);
            Some(V::Void)
        }
        ("defense", "()Ljava/util/Map;") | ("enchantmentValue", "()I") |
        ("equipSound", "()Lnet/minecraft/core/Holder;") | ("repairIngredient", "()Ljava/util/function/Supplier;") |
        ("layers", "()Ljava/util/List;") | ("toughness", "()F") | ("knockbackResistance", "()F") => Some(rt.get_field(recv, &member.name)),
        ("getDefense", "(Lnet/minecraft/world/item/ArmorItem$Type;)I") => {
            let map = rt.get_field(recv, "defense");
            let zero = box_primitive(rt, "java/lang/Integer", V::I(0));
            let value = invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new("java/util/Map", "getOrDefault", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"),
                &[map, *args.get(1)?, zero]);
            if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
            if !value.is_null() && ctx.rt.class_of(value) != Some("java/lang/Integer") {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "ArmorMaterial.defense value is not java.lang.Integer",
                ));
            }
            Some(invoke_member(ctx, mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Integer", "intValue", "()I"), &[value]))
        }
        _ => None,
    }
}

fn enum_constant(rt: &mut Rt, class: &str, name: &str, ordinal: usize) -> V {
    if let Some(value) = rt.get_static(class, name) {
        return value;
    }
    let value = rt.new_instance(class, 0);
    let label = rt.intern_str(name);
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(ordinal as i32));
    rt.put_static(class, name, value);
    value
}

fn type_constant(rt: &mut Rt, name: &str) -> Option<V> {
    let ordinal = TYPES.iter().position(|&kind| kind == name)?;
    if let Some(value) = rt.get_static(TYPE, name) {
        return Some(value);
    }
    let value = enum_constant(rt, TYPE, name, ordinal);
    let label = rt.intern_str(&name.to_ascii_lowercase());
    let slot = slot_constant(rt, ["HEAD", "CHEST", "LEGS", "FEET", "BODY"][ordinal]).unwrap();
    rt.put_field(value, "name", label);
    rt.put_field(value, "slot", slot);
    rt.put_field(value, "durability", V::I([11, 16, 15, 13, 16][ordinal]));
    Some(value)
}

fn slot_constant(rt: &mut Rt, name: &str) -> Option<V> {
    let (ordinal, kind, kind_ordinal, index, limit, flag) = match name {
        "MAINHAND" => (0, "HAND", 0, 0, 0, 0),
        "OFFHAND" => (1, "HAND", 0, 1, 0, 5),
        "FEET" => (2, "HUMANOID_ARMOR", 1, 0, 1, 1),
        "LEGS" => (3, "HUMANOID_ARMOR", 1, 1, 1, 2),
        "CHEST" => (4, "HUMANOID_ARMOR", 1, 2, 1, 3),
        "HEAD" => (5, "HUMANOID_ARMOR", 1, 3, 1, 4),
        "BODY" => (6, "ANIMAL_ARMOR", 2, 0, 1, 6),
        _ => return None,
    };
    if let Some(value) = rt.get_static(SLOT, name) {
        return Some(value);
    }
    let value = enum_constant(rt, SLOT, name, ordinal);
    let label = rt.intern_str(&name.to_ascii_lowercase());
    let kind = enum_constant(rt, SLOT_KIND, kind, kind_ordinal);
    rt.put_field(value, "name", label);
    rt.put_field(value, "type", kind);
    rt.put_field(value, "index", V::I(index));
    rt.put_field(value, "countLimit", V::I(limit));
    rt.put_field(value, "filterFlag", V::I(flag));
    Some(value)
}

fn initialize_material(rt: &mut Rt, value: V, fields: &[V]) {
    for (name, value_field) in [
        "defense",
        "enchantmentValue",
        "equipSound",
        "repairIngredient",
        "layers",
        "toughness",
        "knockbackResistance",
    ]
    .into_iter()
    .zip(fields)
    {
        rt.put_field(value, name, *value_field);
    }
}

fn initialize_layer(rt: &mut Rt, value: V, asset: V, suffix: V, dyeable: V) -> V {
    rt.put_field(value, "assetName", asset);
    rt.put_field(value, "suffix", suffix);
    rt.put_field(value, "dyeable", dyeable);
    if asset.is_null() {
        return rt.throw_exception(
            "java/lang/NullPointerException",
            "ArmorMaterial.Layer assetName",
        );
    }
    let Some(("resource_location", _, location, _)) = rt.handle_of(asset) else {
        return rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ArmorMaterial.Layer requires a ResourceLocation",
        );
    };
    let (namespace, path) = location.split_once(':').unwrap_or(("minecraft", location));
    let suffix_text = if suffix.is_null() {
        "null"
    } else if let Some(text) = rt.str_of(suffix) {
        text
    } else {
        return rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ArmorMaterial.Layer requires a String suffix",
        );
    };
    let inner = format!("{namespace}:textures/models/armor/{path}_layer_2{suffix_text}.png");
    let outer = format!("{namespace}:textures/models/armor/{path}_layer_1{suffix_text}.png");
    if !namespace
        .bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
        || !path
            .bytes()
            .chain(suffix_text.bytes())
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"/_.-".contains(&c))
    {
        return rt.throw_exception(
            "net/minecraft/ResourceLocationException",
            &format!("Invalid armor layer texture location: {inner}"),
        );
    }
    let inner = resource_location(rt, &inner);
    let outer = resource_location(rt, &outer);
    rt.put_field(value, "innerTexture", inner);
    rt.put_field(value, "outerTexture", outer);
    V::Void
}

fn resource_location(rt: &mut Rt, id: &str) -> V {
    rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        id,
    )
}

fn reference(rt: &mut Rt, registry: &str, id: &str, value: V) -> V {
    let holder = rt.new_instance("net/minecraft/core/Holder$Reference", 0);
    let key = crate::minecraft::new_resource_key(rt, registry, id);
    rt.put_field(holder, "$key", key);
    rt.put_field(holder, "$value", value);
    holder
}

fn armor_sound(rt: &mut Rt, registries: &mut RegistryStore, name: &str) -> Option<V> {
    let suffix = match name {
        "ARMOR_EQUIP_LEATHER" => "leather",
        "ARMOR_EQUIP_CHAIN" => "chain",
        "ARMOR_EQUIP_IRON" => "iron",
        "ARMOR_EQUIP_GOLD" => "gold",
        "ARMOR_EQUIP_DIAMOND" => "diamond",
        "ARMOR_EQUIP_TURTLE" => "turtle",
        "ARMOR_EQUIP_NETHERITE" => "netherite",
        "ARMOR_EQUIP_WOLF" => "wolf",
        "ARMOR_EQUIP_GENERIC" => "generic",
        _ => return None,
    };
    let owner = "net/minecraft/sounds/SoundEvents";
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let id = format!("minecraft:item.armor.equip_{suffix}");
    let location = resource_location(rt, &id);
    let event = rt.new_instance("net/minecraft/sounds/SoundEvent", 0);
    rt.put_field(event, "location", location);
    rt.put_field(event, "range", V::F(16.0));
    rt.put_field(event, "newSystem", V::FALSE);
    registries.register_direct(
        rt,
        "minecraft:sound_event",
        &id,
        "minecraft",
        event,
        ContentProperties {
            kind: "sound_event".into(),
            template: id.clone(),
            ..ContentProperties::default()
        },
    );
    let value = reference(rt, "minecraft:sound_event", &id, event);
    rt.put_static(owner, name, value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        box_primitive, invoke_member, read_static_field, Data, HostApi, LinkedSpace,
    };

    #[derive(Default)]
    struct ArmorHost(RegistryStore);

    impl HostApi for ArmorHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, &mut self.0, owner, name, desc)
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
        }
    }

    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        static_call: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if static_call {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    fn array(rt: &Rt, value: V) -> Vec<V> {
        match &rt.get_obj(value).unwrap().data {
            Data::Array(items) => items.clone(),
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn vanilla_material_defense_maps_and_numeric_fields_match_1211() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ArmorHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, defense, enchantment, toughness, knockback) in [
            ("LEATHER", [1, 3, 2, 1, 3], 15, 0.0, 0.0),
            ("CHAIN", [2, 5, 4, 1, 4], 12, 0.0, 0.0),
            ("IRON", [2, 6, 5, 2, 5], 9, 0.0, 0.0),
            ("GOLD", [2, 5, 3, 1, 7], 25, 0.0, 0.0),
            ("DIAMOND", [3, 8, 6, 3, 11], 10, 2.0, 0.0),
            ("TURTLE", [2, 6, 5, 2, 5], 9, 0.0, 0.0),
            ("NETHERITE", [3, 8, 6, 3, 11], 15, 3.0, 0.1),
            ("ARMADILLO", [3, 8, 6, 3, 11], 10, 0.0, 0.0),
        ] {
            let holder = read_static_field(&mut ctx, MATERIALS, name, HOLDER_DESC);
            assert!(ctx.rt.pending_exception.is_none(), "missing {name}");
            assert_eq!(
                holder,
                read_static_field(&mut ctx, MATERIALS, name, HOLDER_DESC)
            );
            let material = ctx.rt.get_field(holder, "$value");
            let map = invoke(
                &mut ctx,
                MATERIAL,
                "defense",
                "()Ljava/util/Map;",
                false,
                &[material],
            );
            for (type_name, amount) in TYPES.into_iter().zip(defense) {
                let armor_type = read_static_field(&mut ctx, TYPE, type_name, TYPE_DESC);
                let value = invoke(
                    &mut ctx,
                    "java/util/Map",
                    "get",
                    "(Ljava/lang/Object;)Ljava/lang/Object;",
                    false,
                    &[map, armor_type],
                );
                let value = invoke(
                    &mut ctx,
                    "java/lang/Integer",
                    "intValue",
                    "()I",
                    false,
                    &[value],
                );
                assert_eq!(value, V::I(amount), "{name}/{type_name} map");
                assert_eq!(
                    invoke(
                        &mut ctx,
                        MATERIAL,
                        "getDefense",
                        "(Lnet/minecraft/world/item/ArmorItem$Type;)I",
                        false,
                        &[material, armor_type]
                    ),
                    V::I(amount)
                );
            }
            assert_eq!(
                invoke(
                    &mut ctx,
                    MATERIAL,
                    "enchantmentValue",
                    "()I",
                    false,
                    &[material]
                ),
                V::I(enchantment)
            );
            assert_eq!(
                invoke(&mut ctx, MATERIAL, "toughness", "()F", false, &[material]),
                V::F(toughness)
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    MATERIAL,
                    "knockbackResistance",
                    "()F",
                    false,
                    &[material]
                ),
                V::F(knockback)
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
        let holder = read_static_field(&mut ctx, MATERIALS, "IRON", HOLDER_DESC);
        let material = ctx.rt.get_field(holder, "$value");
        let supplier = invoke(
            &mut ctx,
            MATERIAL,
            "repairIngredient",
            "()Ljava/util/function/Supplier;",
            false,
            &[material],
        );
        assert!(ctx.rt.pending_exception.is_none());
        invoke(
            &mut ctx,
            "java/util/function/Supplier",
            "get",
            "()Ljava/lang/Object;",
            false,
            &[supplier],
        );
        let exception = ctx
            .rt
            .take_exception()
            .expect("vanilla ingredient creation must remain explicitly unsupported");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("repairIngredient"));
    }

    #[test]
    fn armor_type_values_have_canonical_identity_and_reject_invalid_names() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ArmorHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = invoke(
            &mut ctx,
            TYPE,
            "values",
            "()[Lnet/minecraft/world/item/ArmorItem$Type;",
            true,
            &[],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let second = invoke(
            &mut ctx,
            TYPE,
            "values",
            "()[Lnet/minecraft/world/item/ArmorItem$Type;",
            true,
            &[],
        );
        assert_ne!(first, second);
        assert_eq!(array(ctx.rt, first), array(ctx.rt, second));
        for (index, ((name, durability), armor_type)) in TYPES
            .into_iter()
            .zip([11, 16, 15, 13, 16])
            .zip(array(ctx.rt, first))
            .enumerate()
        {
            assert_eq!(
                armor_type,
                read_static_field(&mut ctx, TYPE, name, TYPE_DESC)
            );
            let name_value = ctx.rt.intern_str(name);
            assert_eq!(
                armor_type,
                invoke(
                    &mut ctx,
                    TYPE,
                    "valueOf",
                    "(Ljava/lang/String;)Lnet/minecraft/world/item/ArmorItem$Type;",
                    true,
                    &[name_value]
                )
            );
            assert_eq!(
                invoke(&mut ctx, TYPE, "ordinal", "()I", false, &[armor_type]),
                V::I(index as i32)
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    TYPE,
                    "getDurability",
                    "(I)I",
                    false,
                    &[armor_type, V::I(37)]
                ),
                V::I(durability * 37)
            );
            assert_eq!(
                invoke(&mut ctx, TYPE, "hasTrims", "()Z", false, &[armor_type]),
                V::bool(name != "BODY")
            );
            let serialized = invoke(
                &mut ctx,
                TYPE,
                "getSerializedName",
                "()Ljava/lang/String;",
                false,
                &[armor_type],
            );
            assert_eq!(
                ctx.rt.str_of(serialized),
                Some(name.to_ascii_lowercase().as_str())
            );
        }
        let invalid = ctx.rt.intern_str("NOT_ARMOR");
        invoke(
            &mut ctx,
            TYPE,
            "valueOf",
            "(Ljava/lang/String;)Lnet/minecraft/world/item/ArmorItem$Type;",
            true,
            &[invalid],
        );
        let exception = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("IllegalArgumentException"));
        invoke(
            &mut ctx,
            TYPE,
            "valueOf",
            "(Ljava/lang/String;)Lnet/minecraft/world/item/ArmorItem$Type;",
            true,
            &[V::NULL],
        );
        let exception = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("NullPointerException"));
    }

    #[test]
    fn layer_constructor_resolves_inner_outer_texture_and_dyeable() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ArmorHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let asset = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "mekanism:hazmat",
        );
        let suffix = ctx.rt.intern_str("_overlay");
        let layer = ctx.rt.new_instance(LAYER, 0);
        invoke(
            &mut ctx,
            LAYER,
            "<init>",
            "(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/String;Z)V",
            false,
            &[layer, asset, suffix, V::TRUE],
        );
        assert!(ctx.rt.pending_exception.is_none());
        for (inner, expected) in [
            (
                false,
                "mekanism:textures/models/armor/hazmat_layer_1_overlay.png",
            ),
            (
                true,
                "mekanism:textures/models/armor/hazmat_layer_2_overlay.png",
            ),
        ] {
            let texture = invoke(
                &mut ctx,
                LAYER,
                "texture",
                "(Z)Lnet/minecraft/resources/ResourceLocation;",
                false,
                &[layer, V::bool(inner)],
            );
            assert_eq!(ctx.rt.handle_of(texture).unwrap().2, expected);
            assert_eq!(
                texture,
                invoke(
                    &mut ctx,
                    LAYER,
                    "texture",
                    "(Z)Lnet/minecraft/resources/ResourceLocation;",
                    false,
                    &[layer, V::bool(inner)]
                )
            );
        }
        assert_eq!(
            invoke(&mut ctx, LAYER, "dyeable", "()Z", false, &[layer]),
            V::TRUE
        );
        let simple = ctx.rt.new_instance(LAYER, 0);
        invoke(
            &mut ctx,
            LAYER,
            "<init>",
            "(Lnet/minecraft/resources/ResourceLocation;)V",
            false,
            &[simple, asset],
        );
        assert_eq!(
            invoke(&mut ctx, LAYER, "dyeable", "()Z", false, &[simple]),
            V::FALSE
        );
        let texture = invoke(
            &mut ctx,
            LAYER,
            "texture",
            "(Z)Lnet/minecraft/resources/ResourceLocation;",
            false,
            &[simple, V::FALSE],
        );
        assert_eq!(
            ctx.rt.handle_of(texture).unwrap().2,
            "mekanism:textures/models/armor/hazmat_layer_1.png"
        );
    }

    #[test]
    fn custom_material_preserves_constructor_fields_and_defense_map_mutations() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ArmorHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let boots = read_static_field(&mut ctx, TYPE, "BOOTS", TYPE_DESC);
        let amount = box_primitive(ctx.rt, "java/lang/Integer", V::I(7));
        let defense = ctx
            .rt
            .alloc("java/util/EnumMap", 0, Data::Map(vec![(boots, amount)]));
        let layers = ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new()));
        let supplier = ctx.rt.new_instance("example/RealSupplier", 0);
        let sound = ctx
            .rt
            .new_instance("net/minecraft/core/Holder$Reference", 0);
        let material = ctx.rt.new_instance(MATERIAL, 0);
        invoke(&mut ctx, MATERIAL, "<init>", "(Ljava/util/Map;ILnet/minecraft/core/Holder;Ljava/util/function/Supplier;Ljava/util/List;FF)V", false, &[material, defense, V::I(18), sound, supplier, layers, V::F(1.5), V::F(0.25)]);
        assert!(ctx.rt.pending_exception.is_none());
        for (name, desc, expected) in [
            ("defense", "()Ljava/util/Map;", defense),
            ("equipSound", "()Lnet/minecraft/core/Holder;", sound),
            (
                "repairIngredient",
                "()Ljava/util/function/Supplier;",
                supplier,
            ),
            ("layers", "()Ljava/util/List;", layers),
        ] {
            assert_eq!(
                invoke(&mut ctx, MATERIAL, name, desc, false, &[material]),
                expected
            );
        }
        assert_eq!(
            invoke(
                &mut ctx,
                MATERIAL,
                "getDefense",
                "(Lnet/minecraft/world/item/ArmorItem$Type;)I",
                false,
                &[material, boots]
            ),
            V::I(7)
        );
        let replacement = box_primitive(ctx.rt, "java/lang/Integer", V::I(12));
        invoke(
            &mut ctx,
            "java/util/Map",
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            false,
            &[defense, boots, replacement],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                MATERIAL,
                "getDefense",
                "(Lnet/minecraft/world/item/ArmorItem$Type;)I",
                false,
                &[material, boots]
            ),
            V::I(12)
        );
        let helmet = read_static_field(&mut ctx, TYPE, "HELMET", TYPE_DESC);
        assert_eq!(
            invoke(
                &mut ctx,
                MATERIAL,
                "getDefense",
                "(Lnet/minecraft/world/item/ArmorItem$Type;)I",
                false,
                &[material, helmet]
            ),
            V::I(0)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn material_layers_and_equipment_slots_preserve_registered_value_identity() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::default();
        let leather =
            get_static_field(&mut rt, &mut registries, MATERIALS, "LEATHER", HOLDER_DESC).unwrap();
        let material = rt.get_field(leather, "$value");
        assert_eq!(
            registries
                .lookup("minecraft:armor_material", "minecraft:leather")
                .unwrap()
                .value,
            material
        );
        let sound = get_static_field(
            &mut rt,
            &mut registries,
            "net/minecraft/sounds/SoundEvents",
            "ARMOR_EQUIP_LEATHER",
            HOLDER_DESC,
        )
        .unwrap();
        assert_eq!(rt.get_field(material, "equipSound"), sound);
        let event = rt.get_field(sound, "$value");
        assert_eq!(
            registries
                .lookup(
                    "minecraft:sound_event",
                    "minecraft:item.armor.equip_leather"
                )
                .unwrap()
                .value,
            event
        );
        assert_eq!(
            rt.handle_of(rt.get_field(event, "location")).unwrap().2,
            "minecraft:item.armor.equip_leather"
        );
        let layers = rt.get_field(material, "layers");
        let layers = match &rt.get_obj(layers).unwrap().data {
            Data::List(items) => items.clone(),
            _ => panic!("expected layers"),
        };
        assert_eq!(layers.len(), 2);
        assert_eq!(rt.get_field(layers[0], "dyeable"), V::TRUE);
        assert_eq!(rt.get_field(layers[1], "dyeable"), V::FALSE);
        assert_eq!(
            rt.handle_of(rt.get_field(layers[1], "outerTexture"))
                .unwrap()
                .2,
            "minecraft:textures/models/armor/leather_layer_1_overlay.png"
        );
        for (name, slot_name) in TYPES
            .into_iter()
            .zip(["HEAD", "CHEST", "LEGS", "FEET", "BODY"])
        {
            let kind = get_static_field(&mut rt, &mut registries, TYPE, name, TYPE_DESC).unwrap();
            let slot = get_static_field(
                &mut rt,
                &mut registries,
                SLOT,
                slot_name,
                "Lnet/minecraft/world/entity/EquipmentSlot;",
            )
            .unwrap();
            assert_eq!(rt.get_field(kind, "slot"), slot);
        }
        assert!(
            get_static_field(&mut rt, &mut registries, MATERIALS, "FAKE", HOLDER_DESC).is_none()
        );
        assert!(get_static_field(&mut rt, &mut registries, TYPE, "HELMET", "I").is_none());
        let invalid = rt.new_instance(LAYER, 0);
        let asset = resource_location(&mut rt, "minecraft:leather");
        let suffix = rt.intern_str("_INVALID");
        initialize_layer(&mut rt, invalid, asset, suffix, V::FALSE);
        let exception = rt.take_exception().unwrap();
        assert!(rt
            .exception_summary(exception)
            .contains("ResourceLocationException"));
    }

    #[test]
    fn vanilla_layers_reject_mutation_but_custom_layers_keep_caller_mutability() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ArmorHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let holder = read_static_field(&mut ctx, MATERIALS, "LEATHER", HOLDER_DESC);
        let material = ctx.rt.get_field(holder, "$value");
        let layers = invoke(
            &mut ctx,
            MATERIAL,
            "layers",
            "()Ljava/util/List;",
            false,
            &[material],
        );
        let before = match &ctx.rt.get_obj(layers).unwrap().data {
            Data::List(values) => values.clone(),
            _ => panic!("expected layers"),
        };
        for (name, desc, parameters) in [
            ("clear", "()V", vec![]),
            ("add", "(Ljava/lang/Object;)Z", vec![before[0]]),
            ("add", "(ILjava/lang/Object;)V", vec![V::I(0), before[0]]),
            (
                "set",
                "(ILjava/lang/Object;)Ljava/lang/Object;",
                vec![V::I(0), before[1]],
            ),
            ("remove", "(I)Ljava/lang/Object;", vec![V::I(0)]),
            ("remove", "(Ljava/lang/Object;)Z", vec![before[0]]),
            ("addAll", "(Ljava/util/Collection;)Z", vec![layers]),
            (
                "addAll",
                "(ILjava/util/Collection;)Z",
                vec![V::I(0), layers],
            ),
            ("removeAll", "(Ljava/util/Collection;)Z", vec![layers]),
            ("retainAll", "(Ljava/util/Collection;)Z", vec![layers]),
            ("sort", "(Ljava/util/Comparator;)V", vec![V::NULL]),
            (
                "replaceAll",
                "(Ljava/util/function/UnaryOperator;)V",
                vec![V::NULL],
            ),
            (
                "removeIf",
                "(Ljava/util/function/Predicate;)Z",
                vec![V::NULL],
            ),
            ("addFirst", "(Ljava/lang/Object;)V", vec![before[0]]),
            ("addLast", "(Ljava/lang/Object;)V", vec![before[0]]),
            ("removeFirst", "()Ljava/lang/Object;", vec![]),
            ("removeLast", "()Ljava/lang/Object;", vec![]),
        ] {
            let mut arguments = vec![layers];
            arguments.extend(parameters);
            invoke(&mut ctx, "java/util/List", name, desc, false, &arguments);
            let error = ctx
                .rt
                .take_exception()
                .unwrap_or_else(|| panic!("{name} must fail"));
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("UnsupportedOperationException"));
            let after = match &ctx.rt.get_obj(layers).unwrap().data {
                Data::List(values) => values.clone(),
                _ => panic!("expected layers"),
            };
            assert_eq!(after, before, "{name} changed global material layers");
        }
        assert_eq!(
            ctx.rt.class_of(layers),
            Some("java/util/ImmutableCollections$List12")
        );
        assert_eq!(
            invoke(&mut ctx, "java/util/List", "size", "()I", false, &[layers]),
            V::I(2)
        );

        let custom_layers = ctx.rt.alloc("java/util/ArrayList", 0, Data::List(before));
        let defense = ctx.rt.alloc("java/util/Map", 0, Data::Map(Vec::new()));
        let custom = ctx.rt.new_instance(MATERIAL, 0);
        invoke(&mut ctx, MATERIAL, "<init>", "(Ljava/util/Map;ILnet/minecraft/core/Holder;Ljava/util/function/Supplier;Ljava/util/List;FF)V", false, &[custom, defense, V::I(1), V::NULL, V::NULL, custom_layers, V::F(0.0), V::F(0.0)]);
        let returned = invoke(
            &mut ctx,
            MATERIAL,
            "layers",
            "()Ljava/util/List;",
            false,
            &[custom],
        );
        assert_eq!(returned, custom_layers);
        invoke(
            &mut ctx,
            "java/util/List",
            "clear",
            "()V",
            false,
            &[returned],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/List",
                "size",
                "()I",
                false,
                &[returned]
            ),
            V::I(0)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
}

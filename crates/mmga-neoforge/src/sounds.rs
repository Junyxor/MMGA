//! Minecraft 1.21.1 sound event values. Playback is deliberately outside this module.
//!
//! Field names, declared types, and sound IDs below were checked against Mojang's
//! official 1.21.1 client bytecode (`SoundEvents` = `avp`, `SoundEvent` = `avo`)
//! and client mappings. Client SHA-1: 30c73b1c5da787909b2f73340419fdf13b9def88;
//! mappings SHA-1: 2244b6f072256667bcd9a73df124d6c58de77992.
//! This is an explicit subset used by Create and note-block instruments; unknown
//! fields stay unsupported.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{box_primitive, Data, Rt, V};

const SOUND_EVENTS: &str = "net/minecraft/sounds/SoundEvents";
const SOUND_EVENT: &str = "net/minecraft/sounds/SoundEvent";
const SOUND_DESC: &str = "Lnet/minecraft/sounds/SoundEvent;";
const HOLDER_DESC: &str = "Lnet/minecraft/core/Holder;";
const REFERENCE_DESC: &str = "Lnet/minecraft/core/Holder$Reference;";
const SOUND_REGISTRY: &str = "minecraft:sound_event";

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if owner != SOUND_EVENTS {
        return None;
    }
    let (path, declared_desc) = match name {
        "AMETHYST_CLUSTER_BREAK" => ("block.amethyst_cluster.break", SOUND_DESC),
        "AMETHYST_CLUSTER_FALL" => ("block.amethyst_cluster.fall", SOUND_DESC),
        "AMETHYST_CLUSTER_HIT" => ("block.amethyst_cluster.hit", SOUND_DESC),
        "AMETHYST_CLUSTER_PLACE" => ("block.amethyst_cluster.place", SOUND_DESC),
        "AMETHYST_CLUSTER_STEP" => ("block.amethyst_cluster.step", SOUND_DESC),
        "SMALL_AMETHYST_BUD_BREAK" => ("block.small_amethyst_bud.break", SOUND_DESC),
        "SMALL_AMETHYST_BUD_PLACE" => ("block.small_amethyst_bud.place", SOUND_DESC),
        "MEDIUM_AMETHYST_BUD_BREAK" => ("block.medium_amethyst_bud.break", SOUND_DESC),
        "MEDIUM_AMETHYST_BUD_PLACE" => ("block.medium_amethyst_bud.place", SOUND_DESC),
        "LARGE_AMETHYST_BUD_BREAK" => ("block.large_amethyst_bud.break", SOUND_DESC),
        "LARGE_AMETHYST_BUD_PLACE" => ("block.large_amethyst_bud.place", SOUND_DESC),
        "ANVIL_LAND" => ("block.anvil.land", SOUND_DESC),
        "ARMOR_EQUIP_GOLD" => ("item.armor.equip_gold", HOLDER_DESC),
        "ARMOR_EQUIP_LEATHER" => ("item.armor.equip_leather", HOLDER_DESC),
        "ARMOR_EQUIP_NETHERITE" => ("item.armor.equip_netherite", HOLDER_DESC),
        "BARREL_OPEN" => ("block.barrel.open", SOUND_DESC),
        "BELL_BLOCK" => ("block.bell.use", SOUND_DESC),
        "BOAT_PADDLE_LAND" => ("entity.boat.paddle_land", SOUND_DESC),
        "BOOK_PUT" => ("item.book.put", SOUND_DESC),
        "CHEST_OPEN" => ("block.chest.open", SOUND_DESC),
        "CHISELED_BOOKSHELF_BREAK" => ("block.chiseled_bookshelf.break", SOUND_DESC),
        "FROG_TONGUE" => ("entity.frog.tongue", SOUND_DESC),
        "GENERIC_EAT" => ("entity.generic.eat", SOUND_DESC),
        "GENERIC_EXPLODE" => ("entity.generic.explode", REFERENCE_DESC),
        "GILDED_BLACKSTONE_BREAK" => ("block.gilded_blackstone.break", SOUND_DESC),
        "GRAVEL_PLACE" => ("block.gravel.place", SOUND_DESC),
        "IRON_TRAPDOOR_CLOSE" => ("block.iron_trapdoor.close", SOUND_DESC),
        "ITEM_FRAME_ADD_ITEM" => ("entity.item_frame.add_item", SOUND_DESC),
        "ITEM_FRAME_BREAK" => ("entity.item_frame.break", SOUND_DESC),
        "ITEM_FRAME_REMOVE_ITEM" => ("entity.item_frame.remove_item", SOUND_DESC),
        "ITEM_FRAME_ROTATE_ITEM" => ("entity.item_frame.rotate_item", SOUND_DESC),
        "ITEM_BREAK" => ("entity.item.break", SOUND_DESC),
        "ITEM_PICKUP" => ("entity.item.pickup", SOUND_DESC),
        "WEEPING_VINES_BREAK" => ("block.weeping_vines.break", SOUND_DESC),
        "NETHERITE_BLOCK_BREAK" => ("block.netherite_block.break", SOUND_DESC),
        "NETHERITE_BLOCK_PLACE" => ("block.netherite_block.place", SOUND_DESC),
        "NETHERITE_BLOCK_HIT" => ("block.netherite_block.hit", SOUND_DESC),
        "NETHERRACK_BREAK" => ("block.netherrack.break", SOUND_DESC),
        "NETHERRACK_HIT" => ("block.netherrack.hit", SOUND_DESC),
        "NOTE_BLOCK_BASEDRUM" => ("block.note_block.basedrum", REFERENCE_DESC),
        "NOTE_BLOCK_BASS" => ("block.note_block.bass", REFERENCE_DESC),
        "NOTE_BLOCK_BELL" => ("block.note_block.bell", REFERENCE_DESC),
        "NOTE_BLOCK_HAT" => ("block.note_block.hat", REFERENCE_DESC),
        "NOTE_BLOCK_HARP" => ("block.note_block.harp", REFERENCE_DESC),
        "NOTE_BLOCK_SNARE" => ("block.note_block.snare", REFERENCE_DESC),
        "NOTE_BLOCK_FLUTE" => ("block.note_block.flute", REFERENCE_DESC),
        "NOTE_BLOCK_GUITAR" => ("block.note_block.guitar", REFERENCE_DESC),
        "NOTE_BLOCK_CHIME" => ("block.note_block.chime", REFERENCE_DESC),
        "NOTE_BLOCK_XYLOPHONE" => ("block.note_block.xylophone", REFERENCE_DESC),
        "NOTE_BLOCK_IRON_XYLOPHONE" => ("block.note_block.iron_xylophone", REFERENCE_DESC),
        "NOTE_BLOCK_COW_BELL" => ("block.note_block.cow_bell", REFERENCE_DESC),
        "NOTE_BLOCK_DIDGERIDOO" => ("block.note_block.didgeridoo", REFERENCE_DESC),
        "NOTE_BLOCK_BIT" => ("block.note_block.bit", REFERENCE_DESC),
        "NOTE_BLOCK_BANJO" => ("block.note_block.banjo", REFERENCE_DESC),
        "NOTE_BLOCK_PLING" => ("block.note_block.pling", REFERENCE_DESC),
        "NOTE_BLOCK_IMITATE_ZOMBIE" => ("block.note_block.imitate.zombie", REFERENCE_DESC),
        "NOTE_BLOCK_IMITATE_SKELETON" => ("block.note_block.imitate.skeleton", REFERENCE_DESC),
        "NOTE_BLOCK_IMITATE_CREEPER" => ("block.note_block.imitate.creeper", REFERENCE_DESC),
        "NOTE_BLOCK_IMITATE_ENDER_DRAGON" => ("block.note_block.imitate.ender_dragon", REFERENCE_DESC),
        "NOTE_BLOCK_IMITATE_WITHER_SKELETON" => ("block.note_block.imitate.wither_skeleton", REFERENCE_DESC),
        "NOTE_BLOCK_IMITATE_PIGLIN" => ("block.note_block.imitate.piglin", REFERENCE_DESC),
        "UI_BUTTON_CLICK" => ("ui.button.click", REFERENCE_DESC),
        "SAND_BREAK" => ("block.sand.break", SOUND_DESC),
        "SHULKER_OPEN" => ("entity.shulker.open", SOUND_DESC),
        "SLIME_BLOCK_PLACE" => ("block.slime_block.place", SOUND_DESC),
        "UI_STONECUTTER_TAKE_RESULT" => ("ui.stonecutter.take_result", SOUND_DESC),
        "WARDEN_TENDRIL_CLICKS" => ("entity.warden.tendril_clicks", SOUND_DESC),
        "WOODEN_TRAPDOOR_CLOSE" => ("block.wooden_trapdoor.close", SOUND_DESC),
        "WOODEN_TRAPDOOR_OPEN" => ("block.wooden_trapdoor.open", SOUND_DESC),
        "WOODEN_BUTTON_CLICK_OFF" => ("block.wooden_button.click_off", SOUND_DESC),
        "WOOD_PLACE" => ("block.wood.place", SOUND_DESC),
        "WOOD_BREAK" => ("block.wood.break", SOUND_DESC),
        "WOOD_STEP" => ("block.wood.step", SOUND_DESC),
        "WOOD_HIT" => ("block.wood.hit", SOUND_DESC),
        "WOOD_FALL" => ("block.wood.fall", SOUND_DESC),
        "STONE_BREAK" => ("block.stone.break", SOUND_DESC),
        "STONE_STEP" => ("block.stone.step", SOUND_DESC),
        "STONE_PLACE" => ("block.stone.place", SOUND_DESC),
        "STONE_HIT" => ("block.stone.hit", SOUND_DESC),
        "STONE_FALL" => ("block.stone.fall", SOUND_DESC),
        "GRAVEL_BREAK" => ("block.gravel.break", SOUND_DESC),
        "GRAVEL_STEP" => ("block.gravel.step", SOUND_DESC),
        "GRAVEL_HIT" => ("block.gravel.hit", SOUND_DESC),
        "GRAVEL_FALL" => ("block.gravel.fall", SOUND_DESC),
        "METAL_BREAK" => ("block.metal.break", SOUND_DESC),
        "METAL_STEP" => ("block.metal.step", SOUND_DESC),
        "METAL_PLACE" => ("block.metal.place", SOUND_DESC),
        "METAL_HIT" => ("block.metal.hit", SOUND_DESC),
        "METAL_FALL" => ("block.metal.fall", SOUND_DESC),
        "GLASS_BREAK" => ("block.glass.break", SOUND_DESC),
        "GLASS_STEP" => ("block.glass.step", SOUND_DESC),
        "GLASS_PLACE" => ("block.glass.place", SOUND_DESC),
        "GLASS_HIT" => ("block.glass.hit", SOUND_DESC),
        "GLASS_FALL" => ("block.glass.fall", SOUND_DESC),
        "WOOL_BREAK" => ("block.wool.break", SOUND_DESC),
        "WOOL_HIT" => ("block.wool.hit", SOUND_DESC),
        _ => return None,
    };
    if desc != declared_desc {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let full_id = format!("minecraft:{path}");
    let location = rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        &full_id,
    );
    let sound = new_sound_event(rt, location, 16.0, false);
    registries.register_direct(
        rt,
        SOUND_REGISTRY,
        &full_id,
        "minecraft",
        sound,
        ContentProperties {
            kind: "sound_event".into(),
            template: full_id.clone(),
            ..ContentProperties::default()
        },
    );
    let field_value = if declared_desc == SOUND_DESC {
        sound
    } else {
        // Match Registry.registerForHolder: the holder owns an actual registered
        // value and resource key, not merely an identifier-shaped placeholder.
        registries.canonical_reference(SOUND_REGISTRY, &full_id)?
    };
    rt.put_static(owner, name, field_value);
    Some(field_value)
}

pub fn call_sound_event(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if member.owner != SOUND_EVENT {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str(), is_static) {
        (
            "createVariableRangeEvent",
            "(Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/sounds/SoundEvent;",
            true,
        ) => Some(new_sound_event(rt, *args.first()?, 16.0, false)),
        (
            "createFixedRangeEvent",
            "(Lnet/minecraft/resources/ResourceLocation;F)Lnet/minecraft/sounds/SoundEvent;",
            true,
        ) => Some(new_sound_event(rt, *args.first()?, args.get(1)?.f(), true)),
        ("<init>", "(Lnet/minecraft/resources/ResourceLocation;FZ)V", false) => {
            let recv = *args.first()?;
            rt.put_field(recv, "location", *args.get(1)?);
            rt.put_field(recv, "range", *args.get(2)?);
            rt.put_field(recv, "newSystem", *args.get(3)?);
            Some(V::Void)
        }
        ("getLocation", "()Lnet/minecraft/resources/ResourceLocation;", false) => {
            Some(rt.get_field(*args.first()?, "location"))
        }
        ("getRange", "(F)F", false) => {
            let recv = *args.first()?;
            let volume = args.get(1)?.f();
            let range = if rt.get_field(recv, "newSystem").i() != 0 {
                rt.get_field(recv, "range").f()
            } else if volume > 1.0 {
                16.0 * volume
            } else {
                16.0
            };
            Some(V::F(range))
        }
        ("fixedRange", "()Ljava/util/Optional;", false) => {
            let recv = *args.first()?;
            let range = if rt.get_field(recv, "newSystem").i() != 0 {
                Some(box_primitive(
                    rt,
                    "java/lang/Float",
                    rt.get_field(recv, "range"),
                ))
            } else {
                None
            };
            Some(rt.alloc("java/util/Optional", 0, Data::Optional(range)))
        }
        _ => None,
    }
}

fn new_sound_event(rt: &mut Rt, location: V, range: f32, fixed: bool) -> V {
    let sound = rt.new_instance(SOUND_EVENT, 0);
    rt.put_field(sound, "location", location);
    rt.put_field(sound, "range", V::F(range));
    rt.put_field(sound, "newSystem", V::bool(fixed));
    sound
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::{call_keys_authlib_and_libs, resource_key_parts};

    fn invoke(rt: &mut Rt, name: &str, desc: &str, is_static: bool, args: &[V]) -> V {
        call_sound_event(
            rt,
            &MemberRef::new(SOUND_EVENT, name, desc),
            is_static,
            args,
        )
        .expect("supported sound method")
    }

    #[test]
    fn create_sound_constants_have_registered_values_and_stable_references() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        let holder = get_static_field(
            &mut rt,
            &mut registries,
            SOUND_EVENTS,
            "GENERIC_EXPLODE",
            REFERENCE_DESC,
        )
        .expect("Create's first vanilla sound");
        assert_eq!(
            rt.class_of(holder),
            Some("net/minecraft/core/Holder$Reference")
        );
        let sound = call_keys_authlib_and_libs(
            &mut rt,
            &mut registries,
            &MemberRef::new(
                "net/minecraft/core/Holder$Reference",
                "value",
                "()Ljava/lang/Object;",
            ),
            false,
            &[holder],
        )
        .unwrap();
        assert_eq!(rt.class_of(sound), Some(SOUND_EVENT));
        assert_eq!(
            registries
                .lookup(SOUND_REGISTRY, "minecraft:entity.generic.explode")
                .unwrap()
                .value,
            sound
        );
        assert_eq!(
            resource_key_parts(&rt, rt.get_field(holder, "$key")),
            Some((
                SOUND_REGISTRY.into(),
                "minecraft:entity.generic.explode".into()
            ))
        );
        let location = invoke(
            &mut rt,
            "getLocation",
            "()Lnet/minecraft/resources/ResourceLocation;",
            false,
            &[sound],
        );
        assert_eq!(
            rt.handle_of(location).unwrap().2,
            "minecraft:entity.generic.explode"
        );
        assert_eq!(
            get_static_field(
                &mut rt,
                &mut registries,
                SOUND_EVENTS,
                "GENERIC_EXPLODE",
                REFERENCE_DESC
            ),
            Some(holder)
        );

        let bell = get_static_field(
            &mut rt,
            &mut registries,
            SOUND_EVENTS,
            "BELL_BLOCK",
            SOUND_DESC,
        )
        .unwrap();
        assert_eq!(rt.class_of(bell), Some(SOUND_EVENT));
        assert_eq!(
            registries
                .lookup(SOUND_REGISTRY, "minecraft:block.bell.use")
                .unwrap()
                .value,
            bell
        );
        assert_eq!(registries.total_mod_entries(), 0);
    }

    #[test]
    fn sound_ranges_preserve_vanilla_volume_and_fixed_range_rules() {
        let mut rt = Rt::new();
        let location = rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "example:bell",
        );
        let variable = invoke(
            &mut rt,
            "createVariableRangeEvent",
            "(Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/sounds/SoundEvent;",
            true,
            &[location],
        );
        for (volume, range) in [
            (0.0, 16.0),
            (1.0, 16.0),
            (2.0, 32.0),
            (-3.0, 16.0),
            (f32::NAN, 16.0),
        ] {
            assert_eq!(
                invoke(
                    &mut rt,
                    "getRange",
                    "(F)F",
                    false,
                    &[variable, V::F(volume)]
                ),
                V::F(range)
            );
        }
        let fixed = invoke(
            &mut rt,
            "createFixedRangeEvent",
            "(Lnet/minecraft/resources/ResourceLocation;F)Lnet/minecraft/sounds/SoundEvent;",
            true,
            &[location, V::F(48.0)],
        );
        assert_eq!(
            invoke(&mut rt, "getRange", "(F)F", false, &[fixed, V::F(4.0)]),
            V::F(48.0)
        );
        assert_eq!(
            invoke(
                &mut rt,
                "getLocation",
                "()Lnet/minecraft/resources/ResourceLocation;",
                false,
                &[fixed]
            ),
            location
        );
        assert_ne!(fixed, variable);
    }

    #[test]
    fn unknown_sound_fields_and_signatures_remain_unhandled() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        assert_eq!(
            get_static_field(
                &mut rt,
                &mut registries,
                SOUND_EVENTS,
                "NOT_A_SOUND",
                SOUND_DESC
            ),
            None
        );
        assert_eq!(
            get_static_field(
                &mut rt,
                &mut registries,
                SOUND_EVENTS,
                "GENERIC_EXPLODE",
                SOUND_DESC
            ),
            None
        );
        assert_eq!(
            call_sound_event(
                &mut rt,
                &MemberRef::new(SOUND_EVENT, "getRange", "()F"),
                false,
                &[V::NULL]
            ),
            None
        );
        assert_eq!(
            call_sound_event(
                &mut rt,
                &MemberRef::new(SOUND_EVENT, "play", "()V"),
                false,
                &[V::NULL]
            ),
            None
        );
        assert!(registries.entries.get(SOUND_REGISTRY).is_none());
    }
}

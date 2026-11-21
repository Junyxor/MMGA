//! Known vanilla Enchantments ResourceKey fields for Minecraft 1.21.1.
//!
//! `Enchantments.SILK_TOUCH` is key metadata only. The enchantment object is still a
//! registry value and is not fabricated here.

use mmga_runtime::{Rt, V};

const ENCHANTMENTS: &str = "net/minecraft/world/item/enchantment/Enchantments";
const KEY_DESC: &str = "Lnet/minecraft/resources/ResourceKey;";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != ENCHANTMENTS || desc != KEY_DESC {
        return None;
    }
    let location = match name {
        "SILK_TOUCH" => "minecraft:silk_touch",
        "FORTUNE" => "minecraft:fortune",
        "FROST_WALKER" => "minecraft:frost_walker",
        _ => return None,
    };
    Some(crate::minecraft::new_resource_key(
        rt,
        "minecraft:enchantment",
        location,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silk_touch_static_field_is_the_canonical_enchantment_key_only() {
        let mut rt = Rt::new();
        let first = get_static_field(&mut rt, ENCHANTMENTS, "SILK_TOUCH", KEY_DESC).unwrap();
        let second = get_static_field(&mut rt, ENCHANTMENTS, "SILK_TOUCH", KEY_DESC).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            crate::minecraft::resource_key_parts(&rt, first),
            Some((
                "minecraft:enchantment".into(),
                "minecraft:silk_touch".into()
            ))
        );
        assert!(get_static_field(&mut rt, ENCHANTMENTS, "SILK_TOUCH", "I").is_none());
        let fortune = get_static_field(&mut rt, ENCHANTMENTS, "FORTUNE", KEY_DESC).unwrap();
        assert_eq!(
            crate::minecraft::resource_key_parts(&rt, fortune),
            Some(("minecraft:enchantment".into(), "minecraft:fortune".into()))
        );
        let frost_walker =
            get_static_field(&mut rt, ENCHANTMENTS, "FROST_WALKER", KEY_DESC).unwrap();
        assert_eq!(
            crate::minecraft::resource_key_parts(&rt, frost_walker),
            Some((
                "minecraft:enchantment".into(),
                "minecraft:frost_walker".into()
            ))
        );
        assert!(get_static_field(&mut rt, ENCHANTMENTS, "EFFICIENCY", KEY_DESC).is_none());
        assert!(get_static_field(
            &mut rt,
            "net/minecraft/core/registries/Registries",
            "ENCHANTMENT",
            KEY_DESC
        )
        .is_none());
    }
}

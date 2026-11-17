//! Explicit NeoForge 1.21.1 common item tag constants.
//!
//! Source: NeoForged/NeoForge commit 89e887b367ab0c11cb58ce8500920efadc17b803,
//! src/main/java/net/neoforged/neoforge/common/Tags.java, blob
//! 6f54eb71427d4a1ed1ba5e3b57c56d76847a2815. These are tag keys, not claims that
//! the data-pack tag contents have been loaded or that membership is implemented.

use mmga_runtime::{Rt, V};

const ITEMS: &str = "net/neoforged/neoforge/common/Tags$Items";
const TAG: &str = "net/minecraft/tags/TagKey";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != ITEMS || desc != "Lnet/minecraft/tags/TagKey;" {
        return None;
    }
    let location = match name {
        "ENCHANTING_FUELS" => "neoforge:enchanting_fuels",
        "DUSTS" => "c:dusts",
        "DUSTS_REDSTONE" => "c:dusts/redstone",
        "DUSTS_GLOWSTONE" => "c:dusts/glowstone",
        "GEMS" => "c:gems",
        "GEMS_DIAMOND" => "c:gems/diamond",
        "GEMS_EMERALD" => "c:gems/emerald",
        "GEMS_AMETHYST" => "c:gems/amethyst",
        "GEMS_LAPIS" => "c:gems/lapis",
        "GEMS_PRISMARINE" => "c:gems/prismarine",
        "GEMS_QUARTZ" => "c:gems/quartz",
        "INGOTS" => "c:ingots",
        "INGOTS_COPPER" => "c:ingots/copper",
        "INGOTS_GOLD" => "c:ingots/gold",
        "INGOTS_IRON" => "c:ingots/iron",
        "INGOTS_NETHERITE" => "c:ingots/netherite",
        "NUGGETS" => "c:nuggets",
        "NUGGETS_GOLD" => "c:nuggets/gold",
        "NUGGETS_IRON" => "c:nuggets/iron",
        "ORES" => "c:ores",
        "ORES_COAL" => "c:ores/coal",
        "ORES_COPPER" => "c:ores/copper",
        "ORES_DIAMOND" => "c:ores/diamond",
        "ORES_EMERALD" => "c:ores/emerald",
        "ORES_GOLD" => "c:ores/gold",
        "ORES_IRON" => "c:ores/iron",
        "ORES_LAPIS" => "c:ores/lapis",
        "ORES_NETHERITE_SCRAP" => "c:ores/netherite_scrap",
        "ORES_QUARTZ" => "c:ores/quartz",
        "ORES_REDSTONE" => "c:ores/redstone",
        "ORES_IN_GROUND_DEEPSLATE" => "c:ores_in_ground/deepslate",
        "ORES_IN_GROUND_NETHERRACK" => "c:ores_in_ground/netherrack",
        "ORES_IN_GROUND_STONE" => "c:ores_in_ground/stone",
        "RAW_MATERIALS" => "c:raw_materials",
        "RAW_MATERIALS_COPPER" => "c:raw_materials/copper",
        "RAW_MATERIALS_GOLD" => "c:raw_materials/gold",
        "RAW_MATERIALS_IRON" => "c:raw_materials/iron",
        "STORAGE_BLOCKS" => "c:storage_blocks",
        "STORAGE_BLOCKS_BONE_MEAL" => "c:storage_blocks/bone_meal",
        "STORAGE_BLOCKS_COAL" => "c:storage_blocks/coal",
        "STORAGE_BLOCKS_COPPER" => "c:storage_blocks/copper",
        "STORAGE_BLOCKS_DIAMOND" => "c:storage_blocks/diamond",
        "STORAGE_BLOCKS_DRIED_KELP" => "c:storage_blocks/dried_kelp",
        "STORAGE_BLOCKS_EMERALD" => "c:storage_blocks/emerald",
        "STORAGE_BLOCKS_GOLD" => "c:storage_blocks/gold",
        "STORAGE_BLOCKS_IRON" => "c:storage_blocks/iron",
        "STORAGE_BLOCKS_LAPIS" => "c:storage_blocks/lapis",
        "STORAGE_BLOCKS_NETHERITE" => "c:storage_blocks/netherite",
        "STORAGE_BLOCKS_RAW_COPPER" => "c:storage_blocks/raw_copper",
        "STORAGE_BLOCKS_RAW_GOLD" => "c:storage_blocks/raw_gold",
        "STORAGE_BLOCKS_RAW_IRON" => "c:storage_blocks/raw_iron",
        "STORAGE_BLOCKS_REDSTONE" => "c:storage_blocks/redstone",
        "STORAGE_BLOCKS_SLIME" => "c:storage_blocks/slime",
        "STORAGE_BLOCKS_WHEAT" => "c:storage_blocks/wheat",
        _ => return None,
    };
    if let Some(tag) = rt.get_static(owner, name) {
        return Some(tag);
    }
    let tag = rt.new_pinned_handle(TAG, "tag_key", 0, format!("minecraft:item|{location}"));
    rt.put_static(owner, name, tag);
    Some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::{op, MemberRef};
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};
    struct Host(crate::RegistryStore);
    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            crate::minecraft::call_keys_authlib_and_libs(rt, &mut self.0, member, is_static, args)
        }
    }
    #[test]
    fn primary_resource_keys_keep_item_registry_and_common_namespace() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host(crate::RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (field, id) in [
            ("ORES_IRON", "c:ores/iron"),
            ("ORES_GOLD", "c:ores/gold"),
            ("ORES_COPPER", "c:ores/copper"),
            ("STORAGE_BLOCKS_RAW_IRON", "c:storage_blocks/raw_iron"),
            ("ENCHANTING_FUELS", "neoforge:enchanting_fuels"),
        ] {
            let tag = read_static_field(&mut ctx, ITEMS, field, "Lnet/minecraft/tags/TagKey;");
            assert!(ctx.rt.pending_exception.is_none(), "missing {field}");
            assert_eq!(
                tag,
                read_static_field(&mut ctx, ITEMS, field, "Lnet/minecraft/tags/TagKey;")
            );
            let location = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    TAG,
                    "location",
                    "()Lnet/minecraft/resources/ResourceLocation;",
                ),
                &[tag],
            );
            assert_eq!(ctx.rt.handle_of(location).unwrap().2, id);
            let registry = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(TAG, "registry", "()Lnet/minecraft/resources/ResourceKey;"),
                &[tag],
            );
            assert_eq!(
                crate::minecraft::resource_key_parts(ctx.rt, registry),
                Some(("minecraft:root".into(), "minecraft:item".into()))
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(host.0.total_mod_entries(), 0);
    }
    #[test]
    fn unknown_fields_registry_owners_and_descriptors_remain_unimplemented() {
        let mut rt = Rt::new();
        for (owner, field, desc) in [
            (ITEMS, "ORES_UNOBTAINIUM", "Lnet/minecraft/tags/TagKey;"),
            (
                "net/neoforged/neoforge/common/Tags$Blocks",
                "ORES_IRON",
                "Lnet/minecraft/tags/TagKey;",
            ),
            (ITEMS, "ORES_IRON", "I"),
        ] {
            assert!(get_static_field(&mut rt, owner, field, desc).is_none());
        }
    }
}

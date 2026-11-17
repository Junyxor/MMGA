//! Explicit Minecraft 1.21.1 BlockTags used by Create's block builders.
//!
//! Verified against official BlockTags (`awe`) initializers and mappings.
//! These are registry/location keys only: no tag membership or datapack
//! contents are invented by exposing the constants.
use mmga_runtime::{Rt, V};
const OWNER: &str = "net/minecraft/tags/BlockTags";
#[cfg(test)]
const TAG: &str = "net/minecraft/tags/TagKey";
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != OWNER || desc != "Lnet/minecraft/tags/TagKey;" {
        return None;
    }
    let path = match name {
        "MINEABLE_WITH_PICKAXE" => "mineable/pickaxe",
        "MINEABLE_WITH_AXE" => "mineable/axe",
        "MINEABLE_WITH_HOE" => "mineable/hoe",
        "MINEABLE_WITH_SHOVEL" => "mineable/shovel",
        "NEEDS_DIAMOND_TOOL" => "needs_diamond_tool",
        "NEEDS_IRON_TOOL" => "needs_iron_tool",
        "NEEDS_STONE_TOOL" => "needs_stone_tool",
        "CLIMBABLE" => "climbable",
        "RAILS" => "rails",
        "BEACON_BASE_BLOCKS" => "beacon_base_blocks",
        _ => return None,
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = crate::tag_codecs::intern_key(rt, "minecraft:block", &format!("minecraft:{path}"));
    rt.put_static(owner, name, value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryStore;
    use mmga_classfile::{op, MemberRef};
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};
    struct TagHost(RegistryStore);
    impl HostApi for TagHost {
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
    fn create_mining_tag_constants_have_exact_vanilla_block_keys() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TagHost(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, path) in [
            ("MINEABLE_WITH_PICKAXE", "mineable/pickaxe"),
            ("MINEABLE_WITH_AXE", "mineable/axe"),
            ("MINEABLE_WITH_HOE", "mineable/hoe"),
            ("MINEABLE_WITH_SHOVEL", "mineable/shovel"),
            ("NEEDS_DIAMOND_TOOL", "needs_diamond_tool"),
            ("NEEDS_IRON_TOOL", "needs_iron_tool"),
            ("NEEDS_STONE_TOOL", "needs_stone_tool"),
            ("CLIMBABLE", "climbable"),
            ("RAILS", "rails"),
            ("BEACON_BASE_BLOCKS", "beacon_base_blocks"),
        ] {
            let tag = read_static_field(&mut ctx, OWNER, name, "Lnet/minecraft/tags/TagKey;");
            assert!(ctx.rt.pending_exception.is_none(), "{name}");
            assert_eq!(
                tag,
                read_static_field(&mut ctx, OWNER, name, "Lnet/minecraft/tags/TagKey;")
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
            assert_eq!(
                ctx.rt.handle_of(location).unwrap().2,
                format!("minecraft:{path}")
            );
            let key = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(TAG, "registry", "()Lnet/minecraft/resources/ResourceKey;"),
                &[tag],
            );
            assert_eq!(
                crate::minecraft::resource_key_parts(ctx.rt, key),
                Some(("minecraft:root".into(), "minecraft:block".into()))
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
        assert!(
            get_static_field(ctx.rt, OWNER, "UNVERIFIED", "Lnet/minecraft/tags/TagKey;").is_none()
        );
        assert!(get_static_field(
            ctx.rt,
            "net/minecraft/tags/ItemTags",
            "MINEABLE_WITH_PICKAXE",
            "Lnet/minecraft/tags/TagKey;"
        )
        .is_none());
        assert!(get_static_field(ctx.rt, OWNER, "MINEABLE_WITH_PICKAXE", "I").is_none());
    }
}

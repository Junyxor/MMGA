//! Minecraft 1.21.1 ItemTags used by the original Create AllItems initializer.
//! Verified from official `awn` bytecode/mappings (client SHA-1
//! 30c73b1c5da787909b2f73340419fdf13b9def88; mappings SHA-1
//! 2244b6f072256667bcd9a73df124d6c58de77992). Keys do not supply tag membership.

use mmga_runtime::{Rt, V};
const OWNER: &str = "net/minecraft/tags/ItemTags";
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != OWNER || desc != "Lnet/minecraft/tags/TagKey;" {
        return None;
    }
    let path = match name {
        // awn.U / bL / bM / bj / bi / bh / bg in the official mappings.
        "PIGLIN_LOVED" => "piglin_loved",
        "DURABILITY_ENCHANTABLE" => "enchantable/durability",
        "BOW_ENCHANTABLE" => "enchantable/bow",
        "HEAD_ARMOR" => "head_armor",
        "CHEST_ARMOR" => "chest_armor",
        "LEG_ARMOR" => "leg_armor",
        "FOOT_ARMOR" => "foot_armor",
        _ => return None,
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = crate::tag_codecs::intern_key(rt, "minecraft:item", &format!("minecraft:{path}"));
    rt.put_static(owner, name, value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryStore;
    use mmga_classfile::{op, MemberRef};
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};
    struct Host(RegistryStore);
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
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            crate::tag_codecs::call(
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
    #[test]
    fn create_item_tags_keep_verified_locations_and_factory_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host(RegistryStore::new());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, path) in [
            ("PIGLIN_LOVED", "piglin_loved"),
            ("DURABILITY_ENCHANTABLE", "enchantable/durability"),
            ("BOW_ENCHANTABLE", "enchantable/bow"),
            ("HEAD_ARMOR", "head_armor"),
            ("CHEST_ARMOR", "chest_armor"),
            ("LEG_ARMOR", "leg_armor"),
            ("FOOT_ARMOR", "foot_armor"),
        ] {
            let key = read_static_field(&mut ctx, OWNER, name, "Lnet/minecraft/tags/TagKey;");
            assert!(
                ctx.rt.pending_exception.is_none(),
                "missing original Create ItemTags field {name}"
            );
            assert_eq!(
                ctx.rt.handle_of(key).unwrap().2,
                format!("minecraft:item|minecraft:{path}")
            );
            assert_eq!(
                key,
                read_static_field(&mut ctx, OWNER, name, "Lnet/minecraft/tags/TagKey;")
            );
            let registry =
                crate::minecraft::new_resource_key(ctx.rt, "minecraft:root", "minecraft:item");
            let location = ctx.rt.new_pinned_handle(
                "net/minecraft/resources/ResourceLocation",
                "resource_location",
                0,
                format!("minecraft:{path}"),
            );
            let factory=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new("net/minecraft/tags/TagKey","create",
                "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/tags/TagKey;"),&[registry,location]);
            assert_eq!(factory, key);
            let block = crate::tag_codecs::intern_key(
                ctx.rt,
                "minecraft:block",
                &format!("minecraft:{path}"),
            );
            assert_ne!(
                block, key,
                "registry types must distinguish keys even for equal paths"
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(host.0.total_mod_entries(), 0);
    }
    #[test]
    fn unverified_names_wrong_owners_and_descriptors_do_not_generate_guessed_tags() {
        let mut rt = Rt::new();
        assert!(get_static_field(
            &mut rt,
            OWNER,
            "NOT_A_REAL_TAG",
            "Lnet/minecraft/tags/TagKey;"
        )
        .is_none());
        assert!(get_static_field(
            &mut rt,
            "net/minecraft/tags/BlockTags",
            "PIGLIN_LOVED",
            "Lnet/minecraft/tags/TagKey;"
        )
        .is_none());
        assert!(get_static_field(&mut rt, OWNER, "PIGLIN_LOVED", "I").is_none());
    }
}

//! Registered Minecraft 1.21.1 attribute values used by mod initialization.
//!
//! IDs and defaults are checked against Mojang's `Attributes` (`buw`) bytecode.
//! Its register helper calls Registry.registerForHolder and returns a Reference.
//! RangedAttribute range checks and other accessors remain unsupported.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};

const ATTRIBUTES: &str = "net/minecraft/world/entity/ai/attributes/Attributes";
const ATTRIBUTE: &str = "net/minecraft/world/entity/ai/attributes/Attribute";
const RANGED: &str = "net/minecraft/world/entity/ai/attributes/RangedAttribute";
const REGISTRY: &str = "minecraft:attribute";

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if owner != ATTRIBUTES || desc != "Lnet/minecraft/core/Holder;" {
        return None;
    }
    let (path, default) = match name {
        "ARMOR" => ("generic.armor", 0.0),
        "ARMOR_TOUGHNESS" => ("generic.armor_toughness", 0.0),
        "ATTACK_DAMAGE" => ("generic.attack_damage", 2.0),
        "ATTACK_KNOCKBACK" => ("generic.attack_knockback", 0.0),
        "ATTACK_SPEED" => ("generic.attack_speed", 4.0),
        "BLOCK_INTERACTION_RANGE" => ("player.block_interaction_range", 4.5),
        "BURNING_TIME" => ("generic.burning_time", 1.0),
        "ENTITY_INTERACTION_RANGE" => ("player.entity_interaction_range", 3.0),
        "EXPLOSION_KNOCKBACK_RESISTANCE" => ("generic.explosion_knockback_resistance", 0.0),
        "FALL_DAMAGE_MULTIPLIER" => ("generic.fall_damage_multiplier", 1.0),
        "FLYING_SPEED" => ("generic.flying_speed", 0.4),
        "FOLLOW_RANGE" => ("generic.follow_range", 32.0),
        "GRAVITY" => ("generic.gravity", 0.08),
        "JUMP_STRENGTH" => ("generic.jump_strength", f64::from(0.42_f32)),
        "KNOCKBACK_RESISTANCE" => ("generic.knockback_resistance", 0.0),
        "LUCK" => ("generic.luck", 0.0),
        "MAX_HEALTH" => ("generic.max_health", 20.0),
        "MOVEMENT_EFFICIENCY" => ("generic.movement_efficiency", 0.0),
        "MOVEMENT_SPEED" => ("generic.movement_speed", 0.7),
        "OXYGEN_BONUS" => ("generic.oxygen_bonus", 0.0),
        "SAFE_FALL_DISTANCE" => ("generic.safe_fall_distance", 3.0),
        "SCALE" => ("generic.scale", 1.0),
        "SNEAKING_SPEED" => ("player.sneaking_speed", 0.3),
        "STEP_HEIGHT" => ("generic.step_height", 0.6),
        "SUBMERGED_MINING_SPEED" => ("player.submerged_mining_speed", 0.2),
        "SWEEPING_DAMAGE_RATIO" => ("player.sweeping_damage_ratio", 0.0),
        "WATER_MOVEMENT_EFFICIENCY" => ("generic.water_movement_efficiency", 0.0),
        _ => return None,
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let id = format!("minecraft:{path}");
    let attribute = rt.new_instance(RANGED, 0);
    let description = rt.intern_str(&format!("attribute.name.{path}"));
    rt.put_field(attribute, "descriptionId", description);
    rt.put_field(attribute, "defaultValue", V::D(default));
    registries.register_direct(
        rt,
        REGISTRY,
        &id,
        "minecraft",
        attribute,
        ContentProperties {
            kind: "attribute".into(),
            template: id.clone(),
            ..ContentProperties::default()
        },
    );
    if rt.pending_exception.is_some() { return Some(V::NULL); }
    let holder = registries.canonical_reference(REGISTRY, &id)?;
    rt.put_static(owner, name, holder);
    Some(holder)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static
        && matches!(member.owner.as_str(), ATTRIBUTE | RANGED)
        && member.name == "getDefaultValue"
        && member.desc == "()D"
    {
        return Some(rt.get_field(*args.first()?, "defaultValue"));
    }
    None
}

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    sub_class == RANGED && super_class == ATTRIBUTE
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};

    #[derive(Default)]
    struct AttributeHost(RegistryStore);

    impl HostApi for AttributeHost {
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
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args).or_else(|| {
                crate::minecraft::call_keys_authlib_and_libs(
                    rt,
                    &mut self.0,
                    member,
                    is_static,
                    args,
                )
            })
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
                || crate::minecraft::is_subtype(sub_class, super_class)
        }
    }

    #[test]
    fn attack_speed_holder_and_registry_share_the_same_real_value() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = AttributeHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let holder = read_static_field(
            &mut ctx,
            ATTRIBUTES,
            "ATTACK_SPEED",
            "Lnet/minecraft/core/Holder;",
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            ctx.rt.class_of(holder),
            Some("net/minecraft/core/Holder$Reference")
        );
        assert_eq!(
            holder,
            read_static_field(
                &mut ctx,
                ATTRIBUTES,
                "ATTACK_SPEED",
                "Lnet/minecraft/core/Holder;"
            )
        );
        let attribute = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("net/minecraft/core/Holder", "value", "()Ljava/lang/Object;"),
            &[holder],
        );
        let registry =
            ctx.rt
                .new_pinned_handle("net/minecraft/core/Registry", "registry", 0, REGISTRY);
        let id = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "minecraft:generic.attack_speed",
        );
        let lookup = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "net/minecraft/core/Registry",
                "get",
                "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
            ),
            &[registry, id],
        );
        assert_eq!(lookup, attribute);
        let key = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "net/minecraft/core/Registry",
                "getKey",
                "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;",
            ),
            &[registry, attribute],
        );
        assert_eq!(
            ctx.rt.handle_of(key).unwrap().2,
            "minecraft:generic.attack_speed"
        );
        let default = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(ATTRIBUTE, "getDefaultValue", "()D"),
            &[attribute],
        );
        assert_eq!(default, V::D(4.0));
        assert!(linked.is_subtype(RANGED, ATTRIBUTE, ctx.host));
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(host.0.total_mod_entries(), 0);
    }

    #[test]
    fn unknown_fields_and_unimplemented_range_operations_remain_errors() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        assert!(
            get_static_field(&mut rt, &mut registries, ATTRIBUTES, "ATTACK_SPEED", "I").is_none()
        );
        assert!(get_static_field(
            &mut rt,
            &mut registries,
            ATTRIBUTES,
            "UNKNOWN",
            "Lnet/minecraft/core/Holder;"
        )
        .is_none());
        assert!(call(
            &mut rt,
            &MemberRef::new(RANGED, "sanitizeValue", "(D)D"),
            false,
            &[V::NULL, V::D(1.0)]
        )
        .is_none());
    }
}

//! Minecraft 1.21.1 ParticleTypes.SMOKE metadata needed by mod constructors.
//!
//! The official `ParticleTypes` initializer calls `register("smoke", false)`, whose
//! simple-particle factory returns a `SimpleParticleType` backed by a ParticleType
//! with `overrideLimiter == false`. This module does not model particle rendering or
//! world particle spawning.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};

const TYPES: &str = "net/minecraft/core/particles/ParticleTypes";
const PARTICLE_TYPE: &str = "net/minecraft/core/particles/ParticleType";
const SIMPLE_TYPE: &str = "net/minecraft/core/particles/SimpleParticleType";
const SMOKE_DESC: &str = "Lnet/minecraft/core/particles/SimpleParticleType;";
const REGISTRY: &str = "minecraft:particle_type";
const SMOKE_ID: &str = "minecraft:smoke";

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    (owner == TYPES && name == "SMOKE" && desc == SMOKE_DESC).then(|| simple_smoke(rt, registries))
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static
        || !matches!(
            member.owner.as_str(),
            PARTICLE_TYPE | SIMPLE_TYPE | "net/minecraft/core/particles/ParticleOptions"
        )
    {
        return None;
    }
    let receiver = *args.first()?;
    if rt.class_of(receiver) != Some(SIMPLE_TYPE) {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str()) {
        ("getOverrideLimiter", "()Z") => Some(rt.get_field(receiver, "overrideLimiter")),
        (
            "getType",
            "()Lnet/minecraft/core/particles/SimpleParticleType;"
            | "()Lnet/minecraft/core/particles/ParticleType;",
        ) => Some(receiver),
        ("codec", "()Lcom/mojang/serialization/MapCodec;")
        | ("streamCodec", "()Lnet/minecraft/network/codec/StreamCodec;") => {
            Some(rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("SMOKE particle {} is not implemented", member.name),
            ))
        }
        _ => None,
    }
}

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (sub_class == SIMPLE_TYPE
        && matches!(
            super_class,
            PARTICLE_TYPE | "net/minecraft/core/particles/ParticleOptions"
        ))
        || (matches!(sub_class, SIMPLE_TYPE | PARTICLE_TYPE) && super_class == "java/lang/Object")
}

pub fn register_types(linked: &mut mmga_runtime::LinkedSpace) {
    use mmga_runtime::{NativeClassDef, NativeModuleRegistry};
    linked.register_native_module(NativeModuleRegistry {
        classes: &[
            NativeClassDef {
                name: PARTICLE_TYPE,
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: SIMPLE_TYPE,
                superclass: Some(PARTICLE_TYPE),
                interfaces: &["net/minecraft/core/particles/ParticleOptions"],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: "net/minecraft/core/particles/ParticleOptions",
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: true,
                methods: &[],
            },
        ],
        lambdas: &[],
    });
}

fn simple_smoke(rt: &mut Rt, registries: &mut RegistryStore) -> V {
    if let Some(value) = rt.get_static(TYPES, "SMOKE") {
        return value;
    }
    let value = rt.new_instance(SIMPLE_TYPE, 0);
    rt.put_field(value, "overrideLimiter", V::FALSE);
    registries.register_direct(
        rt,
        REGISTRY,
        SMOKE_ID,
        "minecraft",
        value,
        ContentProperties {
            kind: "particle_type".into(),
            template: SMOKE_ID.into(),
            ..ContentProperties::default()
        },
    );
    if rt.pending_exception.is_some() {
        return V::NULL;
    }
    let Some(holder) = registries.canonical_reference(REGISTRY, SMOKE_ID) else {
        return rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "SMOKE particle registry reference is unavailable after registration",
        );
    };
    rt.put_field(value, "builtInRegistryHolder", holder);
    rt.put_static(TYPES, "SMOKE", value);
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryStore;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace};

    struct Host;

    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            get_static_field(rt, &mut RegistryStore::new(), owner, name, desc)
        }

        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(rt, member, is_static, args)
        }

        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            is_subtype(sub_class, super_class)
        }
    }

    #[test]
    fn smoke_is_the_canonical_simple_particle_with_official_limiter_setting() {
        let mut linked = LinkedSpace::new();
        register_types(&mut linked);
        let mut rt = Rt::new();
        let mut host = Host;
        let mut registries = RegistryStore::new();
        let smoke = get_static_field(&mut rt, &mut registries, TYPES, "SMOKE", SMOKE_DESC).unwrap();
        let again = get_static_field(&mut rt, &mut registries, TYPES, "SMOKE", SMOKE_DESC).unwrap();
        assert_eq!(smoke, again);
        assert_eq!(rt.class_of(smoke), Some(SIMPLE_TYPE));
        assert_eq!(rt.get_field(smoke, "overrideLimiter"), V::FALSE);
        assert_eq!(
            rt.get_field(smoke, "builtInRegistryHolder"),
            registries.canonical_reference(REGISTRY, SMOKE_ID).unwrap()
        );
        assert_eq!(registries.lookup(REGISTRY, SMOKE_ID).unwrap().value, smoke);
        assert!(is_subtype(SIMPLE_TYPE, PARTICLE_TYPE));
        assert!(is_subtype(
            SIMPLE_TYPE,
            "net/minecraft/core/particles/ParticleOptions"
        ));
        assert!(linked.is_subtype(SIMPLE_TYPE, PARTICLE_TYPE, &host));
        assert_eq!(
            get_static_field(
                &mut rt,
                &mut registries,
                TYPES,
                "SMOKE",
                "Lnet/minecraft/core/particles/ParticleType;"
            ),
            None
        );
        assert_eq!(
            get_static_field(&mut rt, &mut registries, TYPES, "FLAME", SMOKE_DESC),
            None
        );

        let override_limiter = invoke_member(
            &mut ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            },
            op::INVOKEVIRTUAL,
            &MemberRef::new(PARTICLE_TYPE, "getOverrideLimiter", "()Z"),
            &[smoke],
        );
        assert_eq!(override_limiter, V::FALSE);
        assert!(rt.pending_exception.is_none());
    }

    #[test]
    fn smoke_codec_access_fails_explicitly_until_supported() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        let smoke = get_static_field(&mut rt, &mut registries, TYPES, "SMOKE", SMOKE_DESC).unwrap();
        let codec = call(
            &mut rt,
            &MemberRef::new(
                SIMPLE_TYPE,
                "codec",
                "()Lcom/mojang/serialization/MapCodec;",
            ),
            false,
            &[smoke],
        )
        .unwrap();
        assert!(codec.is_null());
        let error = rt.take_exception().unwrap();
        assert_eq!(
            rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
}

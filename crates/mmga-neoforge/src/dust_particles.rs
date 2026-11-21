//! Minecraft 1.21.1 dust particle values. Client classes `lh`, `ln`, `lm$1`
//! and mappings define the color reference, scale clamp, and registered type.
//! Dust codecs and the complete particle registry remain unsupported.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{Data, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt, V};

const DUST: &str = "net/minecraft/core/particles/DustParticleOptions";
const SCALABLE: &str = "net/minecraft/core/particles/ScalableParticleOptionsBase";
const OPTIONS: &str = "net/minecraft/core/particles/ParticleOptions";
const TYPE: &str = "net/minecraft/core/particles/ParticleType";
const TYPES: &str = "net/minecraft/core/particles/ParticleTypes";
const REGISTERED_DUST_CLASS: &str = "net/minecraft/core/particles/ParticleTypes$1";
const VECTOR: &str = "org/joml/Vector3f";
const VECTOR_VIEW: &str = "org/joml/Vector3fc";
const REGISTRY: &str = "minecraft:particle_type";
const DUST_ID: &str = "minecraft:dust";
const MIN_SCALE: f32 = 0.01;
const MAX_SCALE: f32 = 4.0;

pub fn register_types(linked: &mut LinkedSpace) {
    linked.register_native_module(NativeModuleRegistry {
        classes: &[
            NativeClassDef {
                name: DUST,
                superclass: Some(SCALABLE),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: SCALABLE,
                superclass: Some("java/lang/Object"),
                interfaces: &[OPTIONS],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: REGISTERED_DUST_CLASS,
                superclass: Some(TYPE),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: VECTOR,
                superclass: Some("java/lang/Object"),
                interfaces: &[VECTOR_VIEW, "java/io/Externalizable", "java/lang/Cloneable"],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: VECTOR_VIEW,
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: true,
                methods: &[],
            },
        ],
        lambdas: &[],
    });
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    matches!(
        (sub, sup),
        (DUST, SCALABLE | OPTIONS)
            | (SCALABLE, OPTIONS)
            | (REGISTERED_DUST_CLASS, TYPE)
            | (
                VECTOR,
                VECTOR_VIEW
                    | "java/io/Externalizable"
                    | "java/io/Serializable"
                    | "java/lang/Cloneable"
            )
    )
}

/// Install this known vanilla entry before mod callbacks or registry freezing.
pub fn bootstrap(rt: &mut Rt, registries: &mut RegistryStore) {
    registered_dust(rt, registries);
}

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    match (owner, name, desc) {
        (TYPES, "DUST", "Lnet/minecraft/core/particles/ParticleType;") => {
            Some(registered_dust(rt, registries))
        }
        (DUST, "REDSTONE_PARTICLE_COLOR", "Lorg/joml/Vector3f;") => Some(redstone_color(rt)),
        (DUST, "REDSTONE", "Lnet/minecraft/core/particles/DustParticleOptions;") => {
            let value = if let Some(value) = rt.get_static(DUST, "REDSTONE") {
                value
            } else {
                let color = redstone_color(rt);
                let value = rt.new_instance(DUST, 0);
                initialize_dust(rt, value, color, 1.0);
                rt.put_static(DUST, "REDSTONE", value);
                value
            };
            Some(value)
        }
        (SCALABLE, "MIN_SCALE", "F") => Some(V::F(MIN_SCALE)),
        (SCALABLE, "MAX_SCALE", "F") => Some(V::F(MAX_SCALE)),
        (SCALABLE, "SCALE", "Lcom/mojang/serialization/Codec;")
        | (DUST, "CODEC", "Lcom/mojang/serialization/MapCodec;")
        | (DUST, "STREAM_CODEC", "Lnet/minecraft/network/codec/StreamCodec;") => {
            Some(dust_codec_unavailable(rt, name))
        }
        _ => None,
    }
}

pub fn call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if let Some(value) =
        guard_registry_access(rt, registries, member, receiver, args.get(1).copied())
    {
        return Some(value);
    }
    match (owner, name, desc) {
        (DUST, "<init>", "(Lorg/joml/Vector3f;F)V") => {
            let (color, V::F(scale)) = (*args.get(1)?, *args.get(2)?) else {
                return None;
            };
            initialize_dust(rt, receiver, color, scale);
            Some(V::Void)
        }
        (SCALABLE, "<init>", "(F)V") => {
            let V::F(scale) = *args.get(1)? else {
                return None;
            };
            rt.put_field(receiver, "scale", V::F(scale.clamp(MIN_SCALE, MAX_SCALE)));
            Some(V::Void)
        }
        (DUST, "getColor", "()Lorg/joml/Vector3f;") => {
            Some(initialized_field(rt, receiver, "color"))
        }
        (DUST | SCALABLE, "getScale", "()F") => Some(initialized_field(rt, receiver, "scale")),
        (DUST | SCALABLE | OPTIONS, "getType", "()Lnet/minecraft/core/particles/ParticleType;")
            if rt.class_of(receiver) == Some(DUST) =>
        {
            Some(registered_dust(rt, registries))
        }
        (TYPE | REGISTERED_DUST_CLASS, "getOverrideLimiter", "()Z")
            if rt.class_of(receiver) == Some(REGISTERED_DUST_CLASS) =>
        {
            Some(initialized_field(rt, receiver, "overrideLimiter"))
        }
        (TYPE | REGISTERED_DUST_CLASS, "codec", "()Lcom/mojang/serialization/MapCodec;")
        | (
            TYPE | REGISTERED_DUST_CLASS,
            "streamCodec",
            "()Lnet/minecraft/network/codec/StreamCodec;",
        ) if rt.class_of(receiver) == Some(REGISTERED_DUST_CLASS) => {
            // ParticleTypes$1 delegates to the two DustParticleOptions static codecs.
            Some(dust_codec_unavailable(rt, name))
        }
        (VECTOR, "<init>", "(FFF)V") | (VECTOR, "set", "(FFF)Lorg/joml/Vector3f;") => {
            let [V::F(x), V::F(y), V::F(z)] = *args.get(1..4)? else {
                return None;
            };
            set_vector(rt, receiver, x, y, z);
            Some(if name == "set" { receiver } else { V::Void })
        }
        (VECTOR | VECTOR_VIEW, "x" | "y" | "z", "()F") if rt.class_of(receiver) == Some(VECTOR) => {
            Some(initialized_field(rt, receiver, name))
        }
        (VECTOR, "equals", "(Ljava/lang/Object;)Z")
        | (VECTOR, "hashCode", "()I")
        | (VECTOR, "toString", "()Ljava/lang/String;") => Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            &format!("JOML Vector3f.{name} is not implemented"),
        )),
        _ => None,
    }
}

fn registered_dust(rt: &mut Rt, registries: &mut RegistryStore) -> V {
    if rt.pending_exception.is_some() {
        return V::NULL;
    }
    if let Some(value) = rt.get_static(TYPES, "DUST") {
        let registered = registries
            .lookup(REGISTRY, DUST_ID)
            .map(|entry| entry.value);
        let holder = registries.canonical_reference(REGISTRY, DUST_ID);
        if registered != Some(value)
            || holder.is_none_or(|holder| rt.get_field(holder, "$value") != value)
        {
            return rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "ParticleTypes.DUST cache and canonical registry value are inconsistent",
            );
        }
        return value;
    }
    let value = rt.new_instance(REGISTERED_DUST_CLASS, 0);
    rt.put_field(value, "overrideLimiter", V::FALSE);
    registries.register_direct(
        rt,
        REGISTRY,
        DUST_ID,
        "minecraft",
        value,
        ContentProperties {
            kind: "particle_type".into(),
            template: DUST_ID.into(),
            ..ContentProperties::default()
        },
    );
    if rt.pending_exception.is_some() {
        return V::NULL;
    }
    rt.put_static(TYPES, "DUST", value);
    value
}

fn initialize_dust(rt: &mut Rt, value: V, color: V, scale: f32) {
    // Mth.clamp preserves NaN; Rust's clamp does too, unlike chained min/max.
    rt.put_field(value, "scale", V::F(scale.clamp(MIN_SCALE, MAX_SCALE)));
    rt.put_field(value, "color", color);
}

fn redstone_color(rt: &mut Rt) -> V {
    if let Some(value) = rt.get_static(DUST, "REDSTONE_PARTICLE_COLOR") {
        return value;
    }
    // Vec3.fromRGB24(0xff0000).toVector3f(). The final field holds the mutable vector.
    let value = rt.new_instance(VECTOR, 0);
    set_vector(rt, value, 1.0, 0.0, 0.0);
    rt.put_static(DUST, "REDSTONE_PARTICLE_COLOR", value);
    value
}

fn set_vector(rt: &mut Rt, value: V, x: f32, y: f32, z: f32) {
    for (field, coordinate) in [("x", x), ("y", y), ("z", z)] {
        rt.put_field(value, field, V::F(coordinate));
    }
}

fn initialized_field(rt: &mut Rt, receiver: V, name: &str) -> V {
    if let Some(Data::Fields(fields)) = rt.get_obj(receiver).map(|obj| &obj.data) {
        if let Some(value) = fields.get(name) {
            return *value;
        }
    }
    rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("Particle value is missing initialized field {name}"),
    )
}

fn dust_codec_unavailable(rt: &mut Rt, name: &str) -> V {
    rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("Dust particle {name} requires unimplemented color and scale codecs"),
    )
}

fn guard_registry_access(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    receiver: V,
    target: Option<V>,
) -> Option<V> {
    if !matches!(
        member.owner.as_str(),
        "net/minecraft/core/Registry"
            | "net/minecraft/core/MappedRegistry"
            | "net/minecraft/core/WritableRegistry"
            | "net/minecraft/core/IdMap"
            | "java/lang/Iterable"
    ) || !matches!(rt.handle_of(receiver), Some(("registry", _, REGISTRY, _)))
    {
        return None;
    }
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if matches!(
        (name, desc),
        ("size", "()I")
            | ("stream" | "holders", "()Ljava/util/stream/Stream;")
            | ("iterator", "()Ljava/util/Iterator;")
            | (
                "keySet" | "entrySet" | "registryKeySet",
                "()Ljava/util/Set;"
            )
            | ("getAny", "()Ljava/util/Optional;")
            | (
                "keys",
                "(Lcom/mojang/serialization/DynamicOps;)Ljava/util/stream/Stream;"
            )
            | ("forEach", "(Ljava/util/function/Consumer;)V")
            | ("spliterator", "()Ljava/util/Spliterator;")
            | ("getId" | "getIdOrThrow", "(Ljava/lang/Object;)I")
            | ("byId" | "byIdOrThrow", "(I)Ljava/lang/Object;")
            | ("getHolder", "(I)Ljava/util/Optional;")
            | ("asHolderIdMap", "()Lnet/minecraft/core/IdMap;")
            | (
                "getRandom",
                "(Lnet/minecraft/util/RandomSource;)Ljava/util/Optional;"
            )
    ) {
        return Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Enumeration and numeric IDs of the incomplete vanilla particle registry are unavailable",
        ));
    }
    if !matches!(
        (name, desc),
        (
            "get" | "getOrThrow",
            "(Lnet/minecraft/resources/ResourceKey;)Ljava/lang/Object;"
        ) | (
            "get",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;"
        ) | (
            "getOptional" | "getHolder",
            "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;"
                | "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;"
        ) | (
            "containsKey",
            "(Lnet/minecraft/resources/ResourceKey;)Z"
                | "(Lnet/minecraft/resources/ResourceLocation;)Z"
        ) | (
            "getHolderOrThrow",
            "(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/core/Holder$Reference;"
        )
    ) {
        return None;
    }
    let id = match rt.handle_of(target?)? {
        ("resource_location", _, location, _) => location.to_owned(),
        ("resource_key", _, key, _) => {
            let (registry, location) = key.split_once('|')?;
            if registry != REGISTRY {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Particle registry lookup with a key for another registry is unavailable",
                ));
            }
            location.to_owned()
        }
        _ => return None,
    };
    if registries.resolve_alias(REGISTRY, &id) == DUST_ID {
        registered_dust(rt, registries);
        if rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
    }
    if (id.starts_with("minecraft:") || id.starts_with("neoforge:"))
        && registries
            .lookup(REGISTRY, &id)
            .is_none_or(|entry| entry.value.is_null())
    {
        return Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            &format!("Particle definition {id} is unavailable in the incomplete vanilla registry"),
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreparedLoad;
    use mmga_classfile::op;
    use mmga_runtime::{alloc_new_object, invoke_member, read_static_field, ExecCtx};

    fn invoke(
        ctx: &mut ExecCtx<'_>,
        opcode: u8,
        owner: &str,
        name: &str,
        desc: &str,
        args: &[V],
    ) -> V {
        invoke_member(ctx, opcode, &MemberRef::new(owner, name, desc), args)
    }

    fn redstone(ctx: &mut ExecCtx<'_>) -> V {
        read_static_field(
            ctx,
            DUST,
            "REDSTONE",
            "Lnet/minecraft/core/particles/DustParticleOptions;",
        )
    }

    fn assert_unsupported(rt: &mut Rt, expected: &str) {
        let exception = rt
            .take_exception()
            .expect("unsupported operation must raise a Java exception");
        assert_eq!(
            rt.class_of(exception),
            Some("java/lang/UnsupportedOperationException")
        );
        let summary = rt.exception_summary(exception);
        assert!(summary.contains(expected), "{summary}");
    }

    #[test]
    fn redstone_keeps_the_shared_color_and_uses_the_registered_dust_type() {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        let mut rt = Rt::new();
        bootstrap(&mut rt, &mut prepared.loader.state.registries);
        let store = &mut prepared.loader.state.registries;
        let entry = store
            .lookup(REGISTRY, DUST_ID)
            .expect("dust registry entry")
            .value;
        let holder = store
            .canonical_reference(REGISTRY, DUST_ID)
            .expect("bound dust holder");
        store.freeze_checked(&mut rt);
        assert!(rt.pending_exception.is_none());

        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let dust = redstone(&mut ctx);
        assert_eq!(ctx.rt.class_of(dust), Some(DUST));
        assert_eq!(redstone(&mut ctx), dust);
        assert!(ctx.linked.is_subtype(DUST, OPTIONS, ctx.host));
        let color = read_static_field(
            &mut ctx,
            DUST,
            "REDSTONE_PARTICLE_COLOR",
            "Lorg/joml/Vector3f;",
        );
        let get_color = MemberRef::new(DUST, "getColor", "()Lorg/joml/Vector3f;");
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &get_color, &[dust]),
            color
        );
        assert!(ctx.rt.get_field(dust, "builtInRegistryHolder").is_null());
        assert_eq!(ctx.rt.class_of(color), Some(VECTOR));
        for (coord, expected) in [("x", 1.0), ("y", 0.0), ("z", 0.0)] {
            assert_eq!(
                invoke(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    VECTOR_VIEW,
                    coord,
                    "()F",
                    &[color]
                ),
                V::F(expected)
            );
        }
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                SCALABLE,
                "getScale",
                "()F",
                &[dust]
            ),
            V::F(1.0)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                VECTOR,
                "set",
                "(FFF)Lorg/joml/Vector3f;",
                &[color, V::F(0.25), V::F(0.5), V::F(0.75)]
            ),
            color
        );
        let color_again = invoke_member(&mut ctx, op::INVOKEVIRTUAL, &get_color, &[dust]);
        assert_eq!(color_again, color);
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                VECTOR,
                "y",
                "()F",
                &[color_again]
            ),
            V::F(0.5)
        );

        let ty = invoke(
            &mut ctx,
            op::INVOKEINTERFACE,
            OPTIONS,
            "getType",
            "()Lnet/minecraft/core/particles/ParticleType;",
            &[dust],
        );
        assert_eq!(ctx.rt.class_of(ty), Some(REGISTERED_DUST_CLASS));
        assert_eq!(ty, entry);
        assert_eq!(
            read_static_field(
                &mut ctx,
                TYPES,
                "DUST",
                "Lnet/minecraft/core/particles/ParticleType;"
            ),
            ty
        );
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                TYPE,
                "getOverrideLimiter",
                "()Z",
                &[ty]
            ),
            V::FALSE
        );
        let registry = read_static_field(
            &mut ctx,
            "net/minecraft/core/registries/BuiltInRegistries",
            "PARTICLE_TYPE",
            "Lnet/minecraft/core/Registry;",
        );
        let key = crate::minecraft::new_resource_key(ctx.rt, REGISTRY, DUST_ID);
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEINTERFACE,
                "net/minecraft/core/Registry",
                "get",
                "(Lnet/minecraft/resources/ResourceKey;)Ljava/lang/Object;",
                &[registry, key]
            ),
            ty
        );
        let optional = invoke(
            &mut ctx,
            op::INVOKEINTERFACE,
            "net/minecraft/core/Registry",
            "getHolder",
            "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
            &[registry, key],
        );
        assert!(
            matches!(ctx.rt.get_obj(optional).map(|obj| &obj.data), Some(Data::Optional(Some(reference))) if *reference == holder)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn dust_constructor_clamps_scale_and_preserves_supplied_color() {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let color = alloc_new_object(&mut ctx, VECTOR);
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKESPECIAL,
                VECTOR,
                "<init>",
                "(FFF)V",
                &[color, V::F(0.2), V::F(0.3), V::F(0.4)]
            ),
            V::Void
        );
        let nan = f32::from_bits(0x7fc1_2345);
        for (input, expected) in [
            (f32::NEG_INFINITY, 0.01),
            (-0.0, 0.01),
            (2.75, 2.75),
            (9.0, 4.0),
            (f32::INFINITY, 4.0),
            (nan, nan),
        ] {
            let dust = alloc_new_object(&mut ctx, DUST);
            assert_eq!(
                invoke(
                    &mut ctx,
                    op::INVOKESPECIAL,
                    DUST,
                    "<init>",
                    "(Lorg/joml/Vector3f;F)V",
                    &[dust, color, V::F(input)]
                ),
                V::Void
            );
            let scale = invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                DUST,
                "getScale",
                "()F",
                &[dust],
            );
            assert!(matches!(scale, V::F(value) if value.to_bits() == expected.to_bits()));
            assert_eq!(
                invoke(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    DUST,
                    "getColor",
                    "()Lorg/joml/Vector3f;",
                    &[dust]
                ),
                color
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
        let dust = alloc_new_object(&mut ctx, DUST);
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKESPECIAL,
                DUST,
                "<init>",
                "(Lorg/joml/Vector3f;F)V",
                &[dust, V::NULL, V::F(1.0)]
            ),
            V::Void
        );
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                DUST,
                "getColor",
                "()Lorg/joml/Vector3f;",
                &[dust]
            ),
            V::NULL
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "the official constructor retains even a null color"
        );
    }

    #[test]
    fn unsupported_codecs_and_incomplete_particle_registry_fail_explicitly() {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let ty = read_static_field(
            &mut ctx,
            TYPES,
            "DUST",
            "Lnet/minecraft/core/particles/ParticleType;",
        );
        assert!(!ty.is_null());
        for (method, field, desc) in [
            ("codec", "CODEC", "Lcom/mojang/serialization/MapCodec;"),
            (
                "streamCodec",
                "STREAM_CODEC",
                "Lnet/minecraft/network/codec/StreamCodec;",
            ),
        ] {
            let result = invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                TYPE,
                method,
                &format!("(){desc}"),
                &[ty],
            );
            assert_eq!(result, V::NULL);
            assert_unsupported(ctx.rt, method);
            assert_eq!(read_static_field(&mut ctx, DUST, field, desc), V::NULL);
            assert_unsupported(ctx.rt, field);
        }
        let registry = read_static_field(
            &mut ctx,
            "net/minecraft/core/registries/BuiltInRegistries",
            "PARTICLE_TYPE",
            "Lnet/minecraft/core/Registry;",
        );
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEINTERFACE,
                "net/minecraft/core/Registry",
                "stream",
                "()Ljava/util/stream/Stream;",
                &[registry]
            ),
            V::NULL
        );
        assert_unsupported(ctx.rt, "incomplete vanilla particle registry");
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEINTERFACE,
                "net/minecraft/core/IdMap",
                "getId",
                "(Ljava/lang/Object;)I",
                &[registry, ty]
            ),
            V::NULL
        );
        assert_unsupported(ctx.rt, "numeric IDs");
        let omitted = crate::minecraft::new_resource_key(ctx.rt, REGISTRY, "minecraft:white_smoke");
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEINTERFACE,
                "net/minecraft/core/Registry",
                "getOptional",
                "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
                &[registry, omitted]
            ),
            V::NULL
        );
        assert_unsupported(ctx.rt, "minecraft:white_smoke");
    }

    #[test]
    fn registry_conflicts_freezing_and_pending_exceptions_never_cache_a_fake_type() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        registries.freeze_checked(&mut rt);
        bootstrap(&mut rt, &mut registries);
        let error = rt
            .take_exception()
            .expect("a frozen registry rejects a missing vanilla entry");
        assert_eq!(rt.class_of(error), Some("java/lang/IllegalStateException"));
        assert!(rt.get_static(TYPES, "DUST").is_none());
        assert!(registries.lookup(REGISTRY, DUST_ID).is_none());

        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        let existing = rt.new_instance("test/ExistingParticle", 0);
        registries.register_direct(
            &mut rt,
            REGISTRY,
            DUST_ID,
            "test",
            existing,
            ContentProperties::default(),
        );
        bootstrap(&mut rt, &mut registries);
        let error = rt
            .take_exception()
            .expect("a duplicate entry must not be silently adopted");
        assert_eq!(rt.class_of(error), Some("java/lang/IllegalStateException"));
        assert_eq!(
            registries.lookup(REGISTRY, DUST_ID).unwrap().value,
            existing
        );
        assert!(rt.get_static(TYPES, "DUST").is_none());

        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        rt.throw_exception("test/OriginalFailure", "keep the original error");
        let pending = rt.pending_exception;
        bootstrap(&mut rt, &mut registries);
        assert_eq!(rt.pending_exception, pending);
        assert!(rt.get_static(TYPES, "DUST").is_none());
        assert!(registries.lookup(REGISTRY, DUST_ID).is_none());
        rt.take_exception();
        bootstrap(&mut rt, &mut registries);
        assert!(rt.pending_exception.is_none());
        registries
            .entries
            .get_mut(REGISTRY)
            .unwrap()
            .remove(DUST_ID);
        bootstrap(&mut rt, &mut registries);
        assert_unsupported(&mut rt, "canonical registry value are inconsistent");
    }

    #[test]
    fn wrong_signatures_and_uncovered_vector_methods_do_not_claim_success() {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        let mut rt = Rt::new();
        let store = &mut prepared.loader.state.registries;
        for (owner, name, desc) in [
            (DUST, "REDSTONE", "Ljava/lang/Object;"),
            (DUST, "REDSTONE_PARTICLE_COLOR", "Lorg/joml/Vector3fc;"),
            (
                TYPES,
                "DUST",
                "Lnet/minecraft/core/particles/SimpleParticleType;",
            ),
            (
                DUST,
                "UNKNOWN",
                "Lnet/minecraft/core/particles/DustParticleOptions;",
            ),
        ] {
            assert!(get_static_field(&mut rt, store, owner, name, desc).is_none());
        }
        assert!(store.lookup(REGISTRY, DUST_ID).is_none());
        let vector = rt.new_instance(VECTOR, 0);
        for (name, desc) in [("x", "()D"), ("normalize", "()Lorg/joml/Vector3f;")] {
            assert!(call(
                &mut rt,
                store,
                &MemberRef::new(VECTOR, name, desc),
                false,
                &[vector]
            )
            .is_none());
            assert!(rt.pending_exception.is_none());
        }
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                VECTOR,
                "normalize",
                "()Lorg/joml/Vector3f;",
                &[vector]
            ),
            V::NULL
        );
        assert_unsupported(ctx.rt, "normalize");
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                "java/lang/Object",
                "equals",
                "(Ljava/lang/Object;)Z",
                &[vector, vector]
            ),
            V::NULL
        );
        assert_unsupported(ctx.rt, "JOML Vector3f.equals");
        assert_eq!(
            invoke(
                &mut ctx,
                op::INVOKEVIRTUAL,
                DUST,
                "getColor",
                "()Lorg/joml/Vector3f;",
                &[V::NULL]
            ),
            V::NULL
        );
        let error = ctx.rt.take_exception().expect("null receiver throws");
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
    }
}

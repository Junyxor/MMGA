//! Constant loot number-provider values with explicit unsupported random/world providers.
use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};
use serde_json::Value as Json;

const PROVIDERS: &str = "net/minecraft/world/level/storage/loot/providers/number/NumberProviders";
const PROVIDER: &str = "net/minecraft/world/level/storage/loot/providers/number/NumberProvider";
const CONSTANT: &str = "net/minecraft/world/level/storage/loot/providers/number/ConstantValue";
const TYPE: &str = "net/minecraft/world/level/storage/loot/providers/number/LootNumberProviderType";

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    let mode = match (owner, name, desc) {
        (PROVIDERS, "CODEC", "Lcom/mojang/serialization/Codec;") => 0,
        (CONSTANT, "INLINE_CODEC", "Lcom/mojang/serialization/Codec;") => 1,
        (CONSTANT, "CODEC", "Lcom/mojang/serialization/MapCodec;") => 2,
        (
            PROVIDERS,
            "CONSTANT",
            "Lnet/minecraft/world/level/storage/loot/providers/number/LootNumberProviderType;",
        ) => 3,
        _ => return None,
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let map = if let Some(value) = rt.get_static(CONSTANT, "CODEC") {
        value
    } else {
        let value = crate::dfu::number_provider_codec(rt, 2);
        rt.put_static(CONSTANT, "CODEC", value);
        value
    };
    let typ = if let Some(value) = rt.get_static(PROVIDERS, "CONSTANT") {
        value
    } else {
        let value = rt.new_instance(TYPE, 0);
        rt.put_field(value, "codec", map);
        registries.register_direct(
            rt,
            "minecraft:loot_number_provider_type",
            "minecraft:constant",
            "minecraft",
            value,
            ContentProperties {
                kind: "loot_number_provider_type".into(),
                template: "minecraft:constant".into(),
                ..Default::default()
            },
        );
        rt.put_static(PROVIDERS, "CONSTANT", value);
        value
    };
    let value = if mode == 3 {
        typ
    } else if mode == 2 {
        map
    } else {
        crate::dfu::number_provider_codec(rt, mode)
    };
    rt.put_static(owner, name, value);
    Some(value)
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == TYPE && !is_static {
        return match (name, desc) {
            ("<init>", "(Lcom/mojang/serialization/MapCodec;)V") => {
                rt.put_field(args[0], "codec", args[1]);
                Some(V::Void)
            }
            ("codec", "()Lcom/mojang/serialization/MapCodec;") => {
                Some(rt.get_field(args[0], "codec"))
            }
            _ => None,
        };
    }
    if !matches!(owner, CONSTANT | PROVIDER) {
        return None;
    }
    if is_static {
        return if owner == CONSTANT
            && name == "exactly"
            && desc == "(F)Lnet/minecraft/world/level/storage/loot/providers/number/ConstantValue;"
        {
            Some(constant(rt, args[0].f()))
        } else {
            None
        };
    }
    let recv = *args.first()?;
    if rt.class_of(recv) != Some(CONSTANT) {
        return None;
    }
    let value = rt.get_field(recv, "value").f();
    match (name, desc) {
        ("<init>", "(F)V") if owner == CONSTANT => {
            rt.put_field(recv, "value", args[1]);
            Some(V::Void)
        }
        ("getFloat", "(Lnet/minecraft/world/level/storage/loot/LootContext;)F")
        | ("value", "()F") => Some(V::F(value)),
        ("getInt", "(Lnet/minecraft/world/level/storage/loot/LootContext;)I") => {
            Some(V::I((f64::from(value) + 0.5).floor() as i32))
        }
        (
            "getType",
            "()Lnet/minecraft/world/level/storage/loot/providers/number/LootNumberProviderType;",
        ) => rt.get_static(PROVIDERS, "CONSTANT"),
        ("equals", "(Ljava/lang/Object;)Z") => {
            let same = rt.class_of(args[1]) == Some(CONSTANT);
            let other = rt.get_field(args[1], "value").f();
            Some(V::bool(same && float_bits(value) == float_bits(other)))
        }
        ("hashCode", "()I") => Some(V::I(if value == 0.0 {
            0
        } else {
            float_bits(value) as i32
        })),
        ("toString", "()Ljava/lang/String;") => Some(rt.new_str(format!(
            "ConstantValue[value={}]",
            rt.value_to_string(V::F(value))
        ))),
        _ => None,
    }
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == CONSTANT
        && matches!(
            sup,
            PROVIDER
                | "net/minecraft/world/level/storage/loot/LootContextUser"
                | "java/lang/Record"
        )
}
pub fn decode_json(rt: &mut Rt, input: &Json, mode: i32) -> Result<V, String> {
    let number = if mode == 1 {
        input
    } else if mode == 2 {
        input
            .as_object()
            .and_then(|m| m.get("value"))
            .ok_or("ConstantValue requires value")?
    } else if input.is_number() {
        input
    } else {
        let map = input.as_object().ok_or("Not a number provider")?;
        match map.get("type").and_then(Json::as_str) {
            Some("constant" | "minecraft:constant") => {
                map.get("value").ok_or("ConstantValue requires value")?
            }
            Some(
                "uniform"
                | "minecraft:uniform"
                | "binomial"
                | "minecraft:binomial"
                | "score"
                | "minecraft:score"
                | "storage"
                | "minecraft:storage"
                | "enchantment_level"
                | "minecraft:enchantment_level",
            )
            | None => {
                rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Random and world-context number providers are not implemented",
                );
                return Err("Unsupported number provider".into());
            }
            Some(other) => return Err(format!("Unknown number provider type {other}")),
        }
    };
    let value = number
        .as_f64()
        .ok_or_else(|| format!("Not a number: {number}"))? as f32;
    Ok(constant(rt, value))
}
pub fn encode_json(rt: &mut Rt, value: V, mode: i32) -> Result<Json, String> {
    if rt.class_of(value) != Some(CONSTANT) {
        rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Nonconstant number provider encoding not implemented",
        );
        return Err("Unsupported number provider".into());
    }
    let value = rt.get_field(value, "value").f();
    if !value.is_finite() {
        rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Nonfinite JsonPrimitive number encoding not implemented",
        );
        return Err("Nonfinite provider value".into());
    }
    Ok(if mode == 2 {
        serde_json::json!({"value":value})
    } else {
        Json::from(value)
    })
}
fn constant(rt: &mut Rt, value: f32) -> V {
    let provider = rt.new_instance(CONSTANT, 0);
    rt.put_field(provider, "value", V::F(value));
    provider
}
fn float_bits(value: f32) -> u32 {
    if value.is_nan() {
        0x7fc00000
    } else {
        value.to_bits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constant_provider_decodes_inline_or_typed_and_does_not_need_a_world() {
        let mut rt = Rt::new();
        for input in [
            serde_json::json!(2.5),
            serde_json::json!({"type":"minecraft:constant","value":2.5}),
        ] {
            let provider = decode_json(&mut rt, &input, 0).unwrap();
            assert_eq!(
                call(
                    &mut rt,
                    &MemberRef::new(
                        PROVIDER,
                        "getFloat",
                        "(Lnet/minecraft/world/level/storage/loot/LootContext;)F"
                    ),
                    false,
                    &[provider, V::NULL]
                ),
                Some(V::F(2.5))
            );
            assert_eq!(
                call(
                    &mut rt,
                    &MemberRef::new(
                        PROVIDER,
                        "getInt",
                        "(Lnet/minecraft/world/level/storage/loot/LootContext;)I"
                    ),
                    false,
                    &[provider, V::NULL]
                ),
                Some(V::I(3))
            );
            assert_eq!(
                encode_json(&mut rt, provider, 0).unwrap(),
                serde_json::json!(2.5)
            );
        }
        let negative = decode_json(&mut rt, &serde_json::json!(-1.5), 0).unwrap();
        assert_eq!(
            call(
                &mut rt,
                &MemberRef::new(
                    PROVIDER,
                    "getInt",
                    "(Lnet/minecraft/world/level/storage/loot/LootContext;)I"
                ),
                false,
                &[negative, V::NULL]
            ),
            Some(V::I(-1))
        );
    }
    #[test]
    fn random_and_world_context_providers_never_return_constant_defaults() {
        let mut rt = Rt::new();
        assert!(decode_json(
            &mut rt,
            &serde_json::json!({"type":"minecraft:uniform","min":1,"max":3}),
            0
        )
        .is_err());
        let error = rt.take_exception().unwrap();
        assert!(rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert!(decode_json(
            &mut rt,
            &serde_json::json!({"type":"minecraft:constant","value":"bad"}),
            0
        )
        .is_err());
        assert!(rt.pending_exception.is_none());
    }

    #[test]
    fn constant_type_registry_identity_and_map_codec_are_real_values() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        let codec = get_static_field(
            &mut rt,
            &mut registries,
            PROVIDERS,
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        let typ = get_static_field(
            &mut rt,
            &mut registries,
            PROVIDERS,
            "CONSTANT",
            "Lnet/minecraft/world/level/storage/loot/providers/number/LootNumberProviderType;",
        )
        .unwrap();
        assert_eq!(
            registries
                .lookup("minecraft:loot_number_provider_type", "minecraft:constant")
                .unwrap()
                .value,
            typ
        );
        let map = get_static_field(
            &mut rt,
            &mut registries,
            CONSTANT,
            "CODEC",
            "Lcom/mojang/serialization/MapCodec;",
        )
        .unwrap();
        assert_eq!(rt.get_field(typ, "codec"), map);
        assert_eq!(
            get_static_field(
                &mut rt,
                &mut registries,
                PROVIDERS,
                "CODEC",
                "Lcom/mojang/serialization/Codec;"
            ),
            Some(codec)
        );
        let constant = decode_json(&mut rt, &serde_json::json!({"value":-2.5}), 2).unwrap();
        assert_eq!(call(&mut rt,&MemberRef::new(CONSTANT,"getType","()Lnet/minecraft/world/level/storage/loot/providers/number/LootNumberProviderType;"),false,&[constant]),Some(typ));
        assert_eq!(
            encode_json(&mut rt, constant, 2).unwrap(),
            serde_json::json!({"value":-2.5})
        );
        assert!(get_static_field(
            &mut rt,
            &mut registries,
            PROVIDERS,
            "MADE_UP",
            "Lcom/mojang/serialization/Codec;"
        )
        .is_none());
    }
}

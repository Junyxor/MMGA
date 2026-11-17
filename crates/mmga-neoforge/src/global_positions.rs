//! Official Minecraft 1.21.1 GlobalPos (`jl`) and immutable BlockPos (`jd`) values.
//! JSON maps use dimension + three signed coordinates. World queries are separate.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, LinkedSpace, NullHost, Rt, V,
};
use serde_json::Value as Json;
const GLOBAL: &str = "net/minecraft/core/GlobalPos";
const POS: &str = "net/minecraft/core/BlockPos";
const VEC: &str = "net/minecraft/core/Vec3i";
const CALLBACK: &str = "mmga/internal/GlobalPositionCodec";
const CODEC: &str = "com/mojang/serialization/Codec";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == POS && name == "ZERO" && desc == "Lnet/minecraft/core/BlockPos;" {
        if let Some(v) = rt.get_static(owner, name) {
            return Some(v);
        }
        let value = position(rt, [0, 0, 0]);
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if !matches!(owner, GLOBAL | POS)
        || !matches!(
            (name, desc),
            ("CODEC", "Lcom/mojang/serialization/Codec;")
                | ("MAP_CODEC", "Lcom/mojang/serialization/MapCodec;")
        )
    {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    if owner == POS {
        if name != "CODEC" {
            return None;
        }
        let value = crate::dfu::block_position_codec(rt);
        rt.put_static(owner, name, value);
        return Some(value);
    }
    let map = if let Some(map) = rt.get_static(GLOBAL, "MAP_CODEC") {
        map
    } else {
        let base = crate::dfu::get_static_field(
            rt,
            "net/minecraft/resources/ResourceLocation",
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )?;
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let forward = callback(
            rt,
            "dimension",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
        );
        let backward = callback(
            rt,
            "dimensionId",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
        );
        let dim=crate::dfu::call(rt,&linked,&mut host,&MemberRef::new(CODEC,"xmap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;"),false,&[base,forward,backward])?;
        let pos = get_static_field(rt, POS, "CODEC", "Lcom/mojang/serialization/Codec;")?;
        let dim = crate::dfu::field_codec(rt, dim, "dimension");
        let pos = crate::dfu::field_codec(rt, pos, "pos");
        let dimget = callback(
            rt,
            "getDimension",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
        );
        let posget = callback(
            rt,
            "getPos",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
        );
        let constructor = callback(
            rt,
            "construct",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            true,
        );
        let map = crate::dfu::record_map_codec(rt, &[(dim, dimget), (pos, posget)], constructor);
        rt.put_static(GLOBAL, "MAP_CODEC", map);
        map
    };
    let value = if name == "MAP_CODEC" {
        map
    } else {
        crate::dfu::codec_from_map(rt, map)
    };
    rt.put_static(owner, name, value);
    Some(value)
}
fn callback(rt: &mut Rt, name: &str, desc: &str, binary: bool) -> V {
    let interface = if binary {
        "java/util/function/BiFunction"
    } else {
        "java/util/function/Function"
    };
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: "apply".into(),
            sam_desc: desc.into(),
            instantiated_desc: desc.into(),
            captures: vec![],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(CALLBACK, name, desc),
            },
        })),
    )
}
fn position(rt: &mut Rt, coords: [i32; 3]) -> V {
    let value = rt.new_instance(POS, 0);
    for (field, value2) in ["x", "y", "z"].into_iter().zip(coords) {
        rt.put_field(value, field, V::I(value2));
    }
    value
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == CALLBACK && is_static {
        return match (name, desc) {
            ("dimension", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
                let ("resource_location", _, id, _) = ctx.rt.handle_of(args[0])? else {
                    return None;
                };
                let id = id.to_owned();
                Some(crate::minecraft::new_resource_key(
                    ctx.rt,
                    "minecraft:dimension",
                    &id,
                ))
            }
            ("dimensionId", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
                if args[0].is_null() {
                    return Some(
                        ctx.rt.throw_exception(
                            "java/lang/NullPointerException",
                            "GlobalPos dimension",
                        ),
                    );
                }
                let (_, id) = crate::minecraft::resource_key_parts(ctx.rt, args[0])?;
                Some(ctx.rt.new_pinned_handle(
                    "net/minecraft/resources/ResourceLocation",
                    "resource_location",
                    0,
                    id,
                ))
            }
            ("getDimension" | "getPos", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
                if ctx.rt.class_of(args[0]) != Some(GLOBAL) {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "GlobalPos getter requires actual record",
                    ));
                }
                Some(
                    ctx.rt
                        .get_field(args[0], if name == "getPos" { "pos" } else { "dimension" }),
                )
            }
            ("construct", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;") => {
                let value = ctx.rt.new_instance(GLOBAL, 0);
                ctx.rt.put_field(value, "dimension", args[0]);
                ctx.rt.put_field(value, "pos", args[1]);
                Some(value)
            }
            _ => None,
        };
    }
    if owner==GLOBAL&&is_static&&name=="of"&&desc=="(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/core/BlockPos;)Lnet/minecraft/core/GlobalPos;"{let value=ctx.rt.new_instance(GLOBAL,0);ctx.rt.put_field(value,"dimension",args[0]);ctx.rt.put_field(value,"pos",args[1]);return Some(value);}
    if is_static {
        return None;
    }
    let value = *args.first()?;
    let class = ctx.rt.class_of(value)?;
    if class == GLOBAL && matches!(owner, GLOBAL | "java/lang/Object") {
        return match (name, desc) {
            ("<init>", "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/core/BlockPos;)V") => {
                ctx.rt.put_field(value, "dimension", args[1]);
                ctx.rt.put_field(value, "pos", args[2]);
                Some(V::Void)
            }
            ("dimension", "()Lnet/minecraft/resources/ResourceKey;") => {
                Some(ctx.rt.get_field(value, "dimension"))
            }
            ("pos", "()Lnet/minecraft/core/BlockPos;") => Some(ctx.rt.get_field(value, "pos")),
            ("equals", "(Ljava/lang/Object;)Z") => {
                if value == args[1] {
                    return Some(V::TRUE);
                }
                if ctx.rt.class_of(args[1]) != Some(GLOBAL) {
                    return Some(V::FALSE);
                }
                for field in ["dimension", "pos"] {
                    let a = ctx.rt.get_field(value, field);
                    let b = ctx.rt.get_field(args[1], field);
                    if a == b {
                        continue;
                    }
                    if a.is_null() {
                        return Some(V::FALSE);
                    }
                    let equal = invoke_member(
                        ctx,
                        op::INVOKEVIRTUAL,
                        &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                        &[a, b],
                    );
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    if equal.i() == 0 {
                        return Some(V::FALSE);
                    }
                }
                Some(V::TRUE)
            }
            ("hashCode", "()I") => {
                let mut hash = 0i32;
                for field in ["dimension", "pos"] {
                    let a = ctx.rt.get_field(value, field);
                    let next = if a.is_null() {
                        0
                    } else {
                        invoke_member(
                            ctx,
                            op::INVOKEVIRTUAL,
                            &MemberRef::new("java/lang/Object", "hashCode", "()I"),
                            &[a],
                        )
                        .i()
                    };
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    hash = hash.wrapping_mul(31).wrapping_add(next);
                }
                Some(V::I(hash))
            }
            ("toString", "()Ljava/lang/String;") => Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "GlobalPos full record text formatting is not implemented",
            )),
            _ => None,
        };
    }
    if class != POS || !matches!(owner, POS | VEC | "java/lang/Object") {
        return None;
    }
    match (name, desc) {
        ("<init>", "(III)V") => {
            for (field, v) in ["x", "y", "z"].into_iter().zip(&args[1..]) {
                ctx.rt.put_field(value, field, *v);
            }
            Some(V::Void)
        }
        ("getX" | "getY" | "getZ", "()I") => Some(ctx.rt.get_field(
            value,
            match name {
                "getX" => "x",
                "getY" => "y",
                _ => "z",
            },
        )),
        ("immutable", "()Lnet/minecraft/core/BlockPos;") => Some(value),
        ("hashCode", "()I") => Some(V::I(
            ctx.rt
                .get_field(value, "y")
                .i()
                .wrapping_add(ctx.rt.get_field(value, "z").i().wrapping_mul(31))
                .wrapping_mul(31)
                .wrapping_add(ctx.rt.get_field(value, "x").i()),
        )),
        ("equals", "(Ljava/lang/Object;)Z") => Some(V::bool(
            ctx.rt.class_of(args[1]) == Some(POS)
                && ["x", "y", "z"].into_iter().all(|field| {
                    ctx.rt.get_field(value, field) == ctx.rt.get_field(args[1], field)
                }),
        )),
        _ => None,
    }
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == GLOBAL && sup == "java/lang/Record")
        || (sub == POS && matches!(sup, VEC | "java/lang/Comparable"))
}
pub fn decode_pos(ctx: &mut ExecCtx<'_>, input: &Json) -> Result<V, (String, Option<V>)> {
    let items = input
        .as_array()
        .ok_or_else(|| (format!("Not an int stream: {input}"), None))?;
    let mut coords = Vec::new();
    for item in items.iter().take(4) {
        coords.push(crate::dfu::decode_json_int(ctx.rt, item).map_err(|error| (error, None))?);
    }
    if coords.len() < 3 {
        return Err((format!("Input is not a list of 3 ints: {input}"), None));
    }
    let value = position(ctx.rt, [coords[0], coords[1], coords[2]]);
    if coords.len() > 3 {
        Err((
            format!("Input is not a list of 3 ints: {input}"),
            Some(value),
        ))
    } else {
        Ok(value)
    }
}
pub fn encode_pos(ctx: &mut ExecCtx<'_>, value: V) -> Result<Json, String> {
    if ctx.rt.class_of(value) != Some(POS) {
        ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "BlockPos codec requires immutable BlockPos",
        );
        return Err("Wrong BlockPos".into());
    }
    Ok(serde_json::json!([
        ctx.rt.get_field(value, "x").i(),
        ctx.rt.get_field(value, "y").i(),
        ctx.rt.get_field(value, "z").i()
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_position_roundtrips_actual_dimension_key_and_signed_coordinates() {
        let mut prepared = crate::PreparedLoad::from_jars(vec![], true).unwrap();
        let mut rt = Rt::new();
        let codec = get_static_field(&mut rt, GLOBAL, "CODEC", "Lcom/mojang/serialization/Codec;")
            .expect("GlobalPos schema must be real");
        let ops = crate::dfu::get_static_field(
            &mut rt,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let input =
            rt.new_str("{\"dimension\":\"minecraft:overworld\",\"pos\":[-123,64,2147483647]}");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let input = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "com/google/gson/JsonParser",
                "parseString",
                "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ),
            &[input],
        );
        let decoded=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),&[codec,ops,input]);
        let value = ctx.rt.get_field(decoded, "$value");
        assert_eq!(ctx.rt.get_field(decoded, "$success"), V::TRUE);
        let pos = ctx.rt.get_field(value, "pos");
        assert_eq!(ctx.rt.get_field(pos, "x"), V::I(-123));
        assert_eq!(ctx.rt.get_field(pos, "z"), V::I(i32::MAX));
        let dim = ctx.rt.get_field(value, "dimension");
        assert_eq!(
            crate::minecraft::resource_key_parts(ctx.rt, dim),
            Some(("minecraft:dimension".into(), "minecraft:overworld".into()))
        );
        let encoded=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),&[codec,ops,value]);
        assert_eq!(ctx.rt.get_field(encoded, "$success"), V::TRUE);
        let data = ctx.rt.get_field(encoded, "$value");
        let text = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Object", "toString", "()Ljava/lang/String;"),
            &[data],
        );
        assert_eq!(
            ctx.rt.str_of(text),
            Some("{\"dimension\":\"minecraft:overworld\",\"pos\":[-123,64,2147483647]}")
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn block_position_bad_lengths_keep_true_partial_and_record_constructors_keep_references() {
        let mut prepared = crate::PreparedLoad::from_jars(vec![], true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert!(decode_pos(&mut ctx, &serde_json::json!([1, 2]))
            .unwrap_err()
            .1
            .is_none());
        let error = decode_pos(&mut ctx, &serde_json::json!([1, 2, 3, 4])).unwrap_err();
        let partial = error
            .1
            .expect("long list must keep first three coordinates");
        assert_eq!(
            encode_pos(&mut ctx, partial).unwrap(),
            serde_json::json!([1, 2, 3])
        );
        let dim =
            crate::minecraft::new_resource_key(ctx.rt, "minecraft:dimension", "example:custom");
        let global = ctx.rt.new_instance(GLOBAL, 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                GLOBAL,
                "<init>",
                "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/core/BlockPos;)V",
            ),
            &[global, dim, partial],
        );
        assert_eq!(ctx.rt.get_field(global, "dimension"), dim);
        assert_eq!(ctx.rt.get_field(global, "pos"), partial);
        let same=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(GLOBAL,"of","(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/core/BlockPos;)Lnet/minecraft/core/GlobalPos;"),&[dim,partial]);
        assert_eq!(
            invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                &[global, same]
            ),
            V::TRUE
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
}

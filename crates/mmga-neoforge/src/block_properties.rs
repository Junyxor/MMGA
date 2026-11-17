//! BlockBehaviour.Properties values from the official Minecraft 1.21.1 `dtb$d`.
//! Defaults retain real function objects and setters retain the caller's objects.
//! A properties builder is not a constructed block or a world implementation.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, read_static_field, ClosureData, ClosureTarget, Data, ExecCtx, V,
};

const CLASS: &str = "net/minecraft/world/level/block/state/BlockBehaviour$Properties";
const CALLBACK: &str = "mmga/internal/BlockPropertiesFunction";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == CALLBACK && is_static {
        return callback_call(ctx, name, args);
    }
    if owner != CLASS {
        return None;
    }
    if is_static {
        if name == "of" && desc == format!("()L{CLASS};") {
            let value = ctx.rt.new_instance(CLASS, 0);
            initialize(ctx, value);
            return Some(if ctx.rt.pending_exception.is_some() {
                V::NULL
            } else {
                value
            });
        }
        return None;
    }
    let recv = *args.first()?;
    if name == "<init>" && desc == "()V" {
        initialize(ctx, recv);
        return Some(V::Void);
    }
    if desc == format!("()L{CLASS};") {
        match name {
            "noCollission" => {
                ctx.rt.put_field(recv, "hasCollision", V::FALSE);
                ctx.rt.put_field(recv, "canOcclude", V::FALSE);
            }
            "noOcclusion" => ctx.rt.put_field(recv, "canOcclude", V::FALSE),
            "instabreak" => {
                ctx.rt.put_field(recv, "destroyTime", V::F(0.0));
                ctx.rt.put_field(recv, "explosionResistance", V::F(0.0));
            }
            "noLootTable" => {
                let key = crate::minecraft::new_resource_key(
                    ctx.rt,
                    "minecraft:loot_table",
                    "minecraft:empty",
                );
                ctx.rt.put_field(recv, "drops", key);
            }
            _ => {
                let field = match name {
                    "randomTicks" => "isRandomlyTicking",
                    "dynamicShape" => "dynamicShape",
                    "ignitedByLava" => "ignitedByLava",
                    "liquid" => "liquid",
                    "forceSolidOn" => "forceSolidOn",
                    "forceSolidOff" => "forceSolidOff",
                    "air" => "isAir",
                    "requiresCorrectToolForDrops" => "requiresCorrectToolForDrops",
                    "replaceable" => "replaceable",
                    "noTerrainParticles" => "spawnTerrainParticles",
                    _ => return None,
                };
                ctx.rt
                    .put_field(recv, field, V::bool(name != "noTerrainParticles"));
            }
        }
        return Some(recv);
    }
    if name == "strength" && desc == format!("(FF)L{CLASS};") {
        ctx.rt.put_field(recv, "destroyTime", args[1]);
        ctx.rt.put_field(
            recv,
            "explosionResistance",
            V::F(java_max_zero(args[2].f())),
        );
        return Some(recv);
    }
    if desc == format!("(F)L{CLASS};") {
        let field = match name {
            "friction" => "friction",
            "speedFactor" => "speedFactor",
            "jumpFactor" => "jumpFactor",
            "destroyTime" => "destroyTime",
            "explosionResistance" => "explosionResistance",
            "strength" => "destroyTime",
            _ => return None,
        };
        ctx.rt.put_field(
            recv,
            field,
            if name == "explosionResistance" {
                V::F(java_max_zero(args[1].f()))
            } else {
                args[1]
            },
        );
        if name == "strength" {
            ctx.rt.put_field(
                recv,
                "explosionResistance",
                V::F(java_max_zero(args[1].f())),
            );
        }
        return Some(recv);
    }
    let field=match (name,desc){
        ("sound",d)if d==format!("(Lnet/minecraft/world/level/block/SoundType;)L{CLASS};")=>"soundType",
        ("lightLevel",d)if d==format!("(Ljava/util/function/ToIntFunction;)L{CLASS};")=>"lightEmission",
        ("mapColor",d)if d==format!("(Ljava/util/function/Function;)L{CLASS};")=>"mapColor",
        ("mapColor",d)if d==format!("(Lnet/minecraft/world/level/material/MapColor;)L{CLASS};")=>{
            let callback=closure(ctx.rt,"java/util/function/Function","apply","(Ljava/lang/Object;)Ljava/lang/Object;","constantColor",&[args[1]]);
            ctx.rt.put_field(recv,"mapColor",callback);return Some(recv);
        }
        ("mapColor",d)if d==format!("(Lnet/minecraft/world/item/DyeColor;)L{CLASS};")=>{
            let callback=closure(ctx.rt,"java/util/function/Function","apply","(Ljava/lang/Object;)Ljava/lang/Object;","dyeColor",&[args[1]]);
            ctx.rt.put_field(recv,"mapColor",callback);return Some(recv);
        }
        ("pushReaction",d)if d==format!("(Lnet/minecraft/world/level/material/PushReaction;)L{CLASS};")=>"pushReaction",
        ("instrument",d)if d==format!("(Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument;)L{CLASS};")=>"instrument",
        ("isValidSpawn",d)if d==format!("(Lnet/minecraft/world/level/block/state/BlockBehaviour$StateArgumentPredicate;)L{CLASS};")=>"isValidSpawn",
        (name,d)if d==format!("(Lnet/minecraft/world/level/block/state/BlockBehaviour$StatePredicate;)L{CLASS};")&&matches!(name,"isRedstoneConductor"|"isSuffocating"|"isViewBlocking"|"hasPostProcess"|"emissiveRendering")=>name,
        _=>return None,
    };
    ctx.rt.put_field(recv, field, args[1]);
    Some(recv)
}

fn java_max_zero(value: f32) -> f32 {
    if value.is_nan() {
        value
    } else if value <= 0.0 {
        0.0
    } else {
        value
    }
}

fn initialize(ctx: &mut ExecCtx<'_>, value: V) {
    for (field, v) in [
        ("hasCollision", V::TRUE),
        ("explosionResistance", V::F(0.0)),
        ("destroyTime", V::F(0.0)),
        ("requiresCorrectToolForDrops", V::FALSE),
        ("isRandomlyTicking", V::FALSE),
        ("friction", V::F(0.6)),
        ("speedFactor", V::F(1.0)),
        ("jumpFactor", V::F(1.0)),
        ("canOcclude", V::TRUE),
        ("isAir", V::FALSE),
        ("ignitedByLava", V::FALSE),
        ("liquid", V::FALSE),
        ("forceSolidOff", V::FALSE),
        ("forceSolidOn", V::FALSE),
        ("spawnTerrainParticles", V::TRUE),
        ("replaceable", V::FALSE),
        ("dynamicShape", V::FALSE),
        ("drops", V::NULL),
        ("offsetFunction", V::NULL),
    ] {
        ctx.rt.put_field(value, field, v);
    }
    for (field, owner, name, desc) in [
        (
            "soundType",
            "net/minecraft/world/level/block/SoundType",
            "STONE",
            "Lnet/minecraft/world/level/block/SoundType;",
        ),
        (
            "pushReaction",
            "net/minecraft/world/level/material/PushReaction",
            "NORMAL",
            "Lnet/minecraft/world/level/material/PushReaction;",
        ),
        (
            "instrument",
            "net/minecraft/world/level/block/state/properties/NoteBlockInstrument",
            "HARP",
            "Lnet/minecraft/world/level/block/state/properties/NoteBlockInstrument;",
        ),
        (
            "requiredFeatures",
            "net/minecraft/world/flag/FeatureFlags",
            "VANILLA_SET",
            "Lnet/minecraft/world/flag/FeatureFlagSet;",
        ),
    ] {
        let field_value = read_static_field(ctx, owner, name, desc);
        if ctx.rt.pending_exception.is_some() {
            return;
        }
        ctx.rt.put_field(value, field, field_value);
    }
    let none = read_static_field(
        ctx,
        "net/minecraft/world/level/material/MapColor",
        "NONE",
        "Lnet/minecraft/world/level/material/MapColor;",
    );
    if ctx.rt.pending_exception.is_some() {
        return;
    }
    let color = closure(
        ctx.rt,
        "java/util/function/Function",
        "apply",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        "constantColor",
        &[none],
    );
    ctx.rt.put_field(value, "mapColor", color);
    let light = closure(
        ctx.rt,
        "java/util/function/ToIntFunction",
        "applyAsInt",
        "(Ljava/lang/Object;)I",
        "zeroLight",
        &[],
    );
    ctx.rt.put_field(value, "lightEmission", light);
    let pred_interface = "net/minecraft/world/level/block/state/BlockBehaviour$StatePredicate";
    let pred_desc="(Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)Z";
    for (field, name) in [
        ("isRedstoneConductor", "solidRender"),
        ("isSuffocating", "suffocating"),
        ("hasPostProcess", "falsePredicate"),
        ("emissiveRendering", "falsePredicate"),
    ] {
        let predicate = closure(ctx.rt, pred_interface, "test", pred_desc, name, &[]);
        ctx.rt.put_field(value, field, predicate);
    }
    let suffocating = ctx.rt.get_field(value, "isSuffocating");
    ctx.rt.put_field(value, "isViewBlocking", suffocating);
    let spawn=closure(ctx.rt,"net/minecraft/world/level/block/state/BlockBehaviour$StateArgumentPredicate","test","(Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;Ljava/lang/Object;)Z","spawn",&[]);
    ctx.rt.put_field(value, "isValidSpawn", spawn);
}

fn closure(
    rt: &mut mmga_runtime::Rt,
    interface: &str,
    sam: &str,
    desc: &str,
    name: &str,
    captures: &[V],
) -> V {
    let target_desc = if captures.is_empty() {
        desc.to_owned()
    } else {
        format!("(Ljava/lang/Object;{}", &desc[1..])
    };
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: sam.into(),
            sam_desc: desc.into(),
            instantiated_desc: desc.into(),
            captures: captures.to_vec(),
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(CALLBACK, name, target_desc),
            },
        })),
    )
}
fn callback_call(ctx: &mut ExecCtx<'_>, name: &str, args: &[V]) -> Option<V> {
    match name {
        "constantColor" => Some(args[0]),
        "zeroLight" => Some(V::I(0)),
        "falsePredicate" => Some(V::FALSE),
        "dyeColor" => Some(invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "net/minecraft/world/item/DyeColor",
                "getMapColor",
                "()Lnet/minecraft/world/level/material/MapColor;",
            ),
            &[args[0]],
        )),
        "solidRender" | "suffocating" => {
            if name == "suffocating" {
                let motion = invoke_member(
                    ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(
                        "net/minecraft/world/level/block/state/BlockState",
                        "blocksMotion",
                        "()Z",
                    ),
                    &[args[0]],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if motion.i() == 0 {
                    return Some(V::FALSE);
                }
            }
            Some(invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/minecraft/world/level/block/state/BlockState",
                    "isSolidRender",
                    "(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)Z",
                ),
                args,
            ))
        }
        "spawn" => {
            let up = read_static_field(
                ctx,
                "net/minecraft/core/Direction",
                "UP",
                "Lnet/minecraft/core/Direction;",
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let sturdy=invoke_member(ctx,op::INVOKEVIRTUAL,&MemberRef::new("net/minecraft/world/level/block/state/BlockState","isFaceSturdy","(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;Lnet/minecraft/core/Direction;)Z"),&[args[0],args[1],args[2],up]);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if sturdy.i() == 0 {
                return Some(V::FALSE);
            }
            let light = invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/minecraft/world/level/block/state/BlockState",
                    "getLightEmission",
                    "()I",
                ),
                &[args[0]],
            );
            Some(if ctx.rt.pending_exception.is_some() {
                V::NULL
            } else {
                V::bool(light.i() < 14)
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{LinkedSpace, Rt};

    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            if name == "of" {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(CLASS, name, desc),
            args,
        )
    }
    #[test]
    fn defaults_and_strength_match_vanilla_and_preserve_distinct_builders() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let props = invoke(&mut ctx, "of", &format!("()L{CLASS};"), &[]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "Properties.of must initialize actual defaults"
        );
        let second = invoke(&mut ctx, "of", &format!("()L{CLASS};"), &[]);
        assert_ne!(props, second);
        assert_eq!(ctx.rt.get_field(props, "friction"), V::F(0.6));
        assert_eq!(ctx.rt.get_field(props, "destroyTime"), V::F(0.0));
        assert_eq!(ctx.rt.get_field(props, "hasCollision"), V::TRUE);
        assert_eq!(ctx.rt.get_field(props, "canOcclude"), V::TRUE);
        let sound = read_static_field(
            &mut ctx,
            "net/minecraft/world/level/block/SoundType",
            "STONE",
            "Lnet/minecraft/world/level/block/SoundType;",
        );
        assert_eq!(ctx.rt.get_field(props, "soundType"), sound);
        let function = ctx.rt.get_field(props, "mapColor");
        let color = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "java/util/function/Function",
                "apply",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
            ),
            &[function, V::NULL],
        );
        let none = read_static_field(
            &mut ctx,
            "net/minecraft/world/level/material/MapColor",
            "NONE",
            "Lnet/minecraft/world/level/material/MapColor;",
        );
        assert_eq!(color, none);
        assert_eq!(
            invoke(
                &mut ctx,
                "strength",
                &format!("(FF)L{CLASS};"),
                &[props, V::F(-1.0), V::F(-5.0)]
            ),
            props
        );
        assert_eq!(ctx.rt.get_field(props, "destroyTime"), V::F(-1.0));
        assert_eq!(ctx.rt.get_field(props, "explosionResistance"), V::F(0.0));
        invoke(
            &mut ctx,
            "strength",
            &format!("(FF)L{CLASS};"),
            &[props, V::F(2.2), V::F(11.0)],
        );
        assert_eq!(ctx.rt.get_field(props, "explosionResistance"), V::F(11.0));
        assert_eq!(ctx.rt.get_field(second, "destroyTime"), V::F(0.0));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn fluent_values_keep_references_and_world_predicates_do_not_fake_success() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let callback = rt.new_instance("java/util/function/ToIntFunction", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let props = invoke(&mut ctx, "of", &format!("()L{CLASS};"), &[]);
        invoke(
            &mut ctx,
            "lightLevel",
            &format!("(Ljava/util/function/ToIntFunction;)L{CLASS};"),
            &[props, callback],
        );
        assert_eq!(ctx.rt.get_field(props, "lightEmission"), callback);
        invoke(
            &mut ctx,
            "sound",
            &format!("(Lnet/minecraft/world/level/block/SoundType;)L{CLASS};"),
            &[props, V::NULL],
        );
        assert_eq!(ctx.rt.get_field(props, "soundType"), V::NULL);
        invoke(&mut ctx, "noCollission", &format!("()L{CLASS};"), &[props]);
        assert_eq!(ctx.rt.get_field(props, "hasCollision"), V::FALSE);
        assert_eq!(ctx.rt.get_field(props, "canOcclude"), V::FALSE);
        let predicate = ctx.rt.get_field(props, "isRedstoneConductor");
        invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new("net/minecraft/world/level/block/state/BlockBehaviour$StatePredicate","test","(Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)Z"),&[predicate,V::NULL,V::NULL,V::NULL]);
        let error = ctx
            .rt
            .take_exception()
            .expect("world predicate must run original state operation or fail");
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        assert!(call(
            &mut ctx,
            &MemberRef::new(
                CLASS,
                "ofFullCopy",
                &format!("(Lnet/minecraft/world/level/block/state/BlockBehaviour;)L{CLASS};")
            ),
            true,
            &[V::NULL]
        )
        .is_none());
        let _ = LinkedSpace::new();
    }
}

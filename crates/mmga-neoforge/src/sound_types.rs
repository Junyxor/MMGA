//! AE2's amethyst SoundType constants, verified against Mojang 1.21.1 `dnm`.
//! Sound references are real registered SoundEvents; playback is not implemented.
use crate::RegistryStore;
use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};
const CLASS: &str = "net/minecraft/world/level/block/SoundType";
const DESC: &str = "Lnet/minecraft/world/level/block/SoundType;";

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if owner != CLASS || desc != DESC {
        return None;
    }
    let (break_sound, place_sound) = match name {
        "AMETHYST_CLUSTER" => ("AMETHYST_CLUSTER_BREAK", "AMETHYST_CLUSTER_PLACE"),
        "SMALL_AMETHYST_BUD" => ("SMALL_AMETHYST_BUD_BREAK", "SMALL_AMETHYST_BUD_PLACE"),
        "MEDIUM_AMETHYST_BUD" => ("MEDIUM_AMETHYST_BUD_BREAK", "MEDIUM_AMETHYST_BUD_PLACE"),
        "LARGE_AMETHYST_BUD" => ("LARGE_AMETHYST_BUD_BREAK", "LARGE_AMETHYST_BUD_PLACE"),
        _ => return None,
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let mut sounds = Vec::with_capacity(5);
    for field in [
        break_sound,
        "AMETHYST_CLUSTER_STEP",
        place_sound,
        "AMETHYST_CLUSTER_HIT",
        "AMETHYST_CLUSTER_FALL",
    ] {
        sounds.push(crate::sounds::get_static_field(
            rt,
            registries,
            "net/minecraft/sounds/SoundEvents",
            field,
            "Lnet/minecraft/sounds/SoundEvent;",
        )?);
    }
    let value = rt.new_instance(CLASS, 0);
    initialize(
        rt,
        value,
        &[
            V::F(1.0),
            V::F(1.0),
            sounds[0],
            sounds[1],
            sounds[2],
            sounds[3],
            sounds[4],
        ],
    );
    rt.put_static(owner, name, value);
    Some(value)
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static || member.owner != CLASS {
        return None;
    }
    let recv = *args.first()?;
    if member.name == "<init>" && member.desc == "(FFLnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;)V"{
        if args.len()!=8{return None;}initialize(rt,recv,&args[1..]);return Some(V::Void);
    }
    let field = match (member.name.as_str(), member.desc.as_str()) {
        ("getVolume", "()F") => "volume",
        ("getPitch", "()F") => "pitch",
        ("getBreakSound", "()Lnet/minecraft/sounds/SoundEvent;") => "breakSound",
        ("getStepSound", "()Lnet/minecraft/sounds/SoundEvent;") => "stepSound",
        ("getPlaceSound", "()Lnet/minecraft/sounds/SoundEvent;") => "placeSound",
        ("getHitSound", "()Lnet/minecraft/sounds/SoundEvent;") => "hitSound",
        ("getFallSound", "()Lnet/minecraft/sounds/SoundEvent;") => "fallSound",
        _ => return None,
    };
    Some(rt.get_field(recv, field))
}
fn initialize(rt: &mut Rt, receiver: V, args: &[V]) {
    for (field, value) in [
        "volume",
        "pitch",
        "breakSound",
        "stepSound",
        "placeSound",
        "hitSound",
        "fallSound",
    ]
    .into_iter()
    .zip(args)
    {
        rt.put_field(receiver, field, *value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace};

    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(CLASS, name, desc),
            args,
        )
    }
    struct TestHost;
    impl HostApi for TestHost {
        fn get_static_field(
            &mut self,
            _rt: &mut Rt,
            _owner: &str,
            _name: &str,
            _desc: &str,
        ) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
    }
    #[test]
    fn quartz_cluster_types_keep_the_official_sound_order_and_registry_identity() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost;
        for (name, break_name, place_name) in [
            (
                "AMETHYST_CLUSTER",
                "AMETHYST_CLUSTER_BREAK",
                "AMETHYST_CLUSTER_PLACE",
            ),
            (
                "SMALL_AMETHYST_BUD",
                "SMALL_AMETHYST_BUD_BREAK",
                "SMALL_AMETHYST_BUD_PLACE",
            ),
            (
                "MEDIUM_AMETHYST_BUD",
                "MEDIUM_AMETHYST_BUD_BREAK",
                "MEDIUM_AMETHYST_BUD_PLACE",
            ),
            (
                "LARGE_AMETHYST_BUD",
                "LARGE_AMETHYST_BUD_BREAK",
                "LARGE_AMETHYST_BUD_PLACE",
            ),
        ] {
            let typ = get_static_field(&mut rt, &mut registries, CLASS, name, DESC)
                .expect("AE2 amethyst constants must exist");
            assert_eq!(
                get_static_field(&mut rt, &mut registries, CLASS, name, DESC),
                Some(typ)
            );
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            assert_eq!(invoke(&mut ctx, "getVolume", "()F", &[typ]), V::F(1.0));
            assert_eq!(invoke(&mut ctx, "getPitch", "()F", &[typ]), V::F(1.0));
            for (method, sound_name) in [
                ("getBreakSound", break_name),
                ("getStepSound", "AMETHYST_CLUSTER_STEP"),
                ("getPlaceSound", place_name),
                ("getHitSound", "AMETHYST_CLUSTER_HIT"),
                ("getFallSound", "AMETHYST_CLUSTER_FALL"),
            ] {
                let actual = invoke(
                    &mut ctx,
                    method,
                    "()Lnet/minecraft/sounds/SoundEvent;",
                    &[typ],
                );
                let expected = crate::sounds::get_static_field(
                    ctx.rt,
                    &mut registries,
                    "net/minecraft/sounds/SoundEvents",
                    sound_name,
                    "Lnet/minecraft/sounds/SoundEvent;",
                )
                .unwrap();
                assert_eq!(actual, expected);
                let id = ctx
                    .rt
                    .handle_of(ctx.rt.get_field(actual, "location"))
                    .unwrap()
                    .2;
                assert_eq!(registries.lookup("sound_event", id).unwrap().value, actual);
            }
            assert!(ctx.rt.pending_exception.is_none());
        }
    }
    #[test]
    fn custom_sound_type_constructor_retains_null_and_real_references() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost;
        let receiver = rt.new_instance(CLASS, 0);
        let sound = rt.new_instance("net/minecraft/sounds/SoundEvent", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(&mut ctx,op::INVOKESPECIAL,&MemberRef::new(CLASS,"<init>","(FFLnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundEvent;)V"),&[receiver,V::F(0.7),V::F(1.4),sound,V::NULL,sound,V::NULL,sound]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(invoke(&mut ctx, "getVolume", "()F", &[receiver]), V::F(0.7));
        assert_eq!(
            invoke(
                &mut ctx,
                "getStepSound",
                "()Lnet/minecraft/sounds/SoundEvent;",
                &[receiver]
            ),
            V::NULL
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "getFallSound",
                "()Lnet/minecraft/sounds/SoundEvent;",
                &[receiver]
            ),
            sound
        );
        assert!(call(
            ctx.rt,
            &MemberRef::new(CLASS, "getPitch", "()I"),
            false,
            &[receiver]
        )
        .is_none());
    }
}

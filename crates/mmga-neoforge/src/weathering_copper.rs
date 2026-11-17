//! The four Minecraft 1.21.1 copper WeatherState values (`dpe$a`).
//! Checked against the official client SHA-1 30c73b1c5da787909b2f73340419fdf13b9def88
//! and mappings SHA-1 2244b6f072256667bcd9a73df124d6c58de77992.
//! Enum metadata does not implement copper block changes or world weather ticks.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};
const STATE: &str = "net/minecraft/world/level/block/WeatheringCopper$WeatherState";
const DESC: &str = "Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;";
const VALUES: &str = "()[Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;";
const NAMES: [&str; 4] = ["UNAFFECTED", "EXPOSED", "WEATHERED", "OXIDIZED"];
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != STATE || desc != DESC {
        return None;
    }
    constant(rt, name)
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    if !is_static
        && name == "getSerializedName"
        && desc == "()Ljava/lang/String;"
        && matches!(
            member.owner.as_str(),
            STATE | "net/minecraft/util/StringRepresentable"
        )
        && rt.class_of(*args.first()?) == Some(STATE)
    {
        return Some(rt.get_field(args[0], "name"));
    }
    if member.owner != STATE {
        return None;
    }
    match (name, desc, is_static) {
        ("values", VALUES, true) => {
            let values = NAMES
                .into_iter()
                .map(|name| constant(rt, name).unwrap())
                .collect();
            Some(rt.alloc(
                "[Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;",
                0,
                Data::Array(values),
            ))
        }
        (
            "valueOf",
            "(Ljava/lang/String;)Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;",
            true,
        ) => {
            let value = *args.first()?;
            if value.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let name = rt.str_of(value)?.to_owned();
            Some(constant(rt,&name).unwrap_or_else(||rt.throw_exception("java/lang/IllegalArgumentException",&format!("No enum constant net.minecraft.world.level.block.WeatheringCopper.WeatherState.{name}"))))
        }
        _ => None,
    }
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == STATE
        && matches!(
            sup,
            "java/lang/Enum"
                | "java/lang/Comparable"
                | "java/io/Serializable"
                | "net/minecraft/util/StringRepresentable"
        )
}
fn constant(rt: &mut Rt, name: &str) -> Option<V> {
    let index = NAMES.iter().position(|candidate| *candidate == name)?;
    if let Some(value) = rt.get_static(STATE, name) {
        return Some(value);
    }
    let value = rt.new_instance(STATE, 0);
    let label = rt.intern_str(name);
    let serialized = rt.intern_str(&name.to_ascii_lowercase());
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(index as i32));
    rt.put_field(value, "name", serialized);
    rt.put_static(STATE, name, value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace};
    struct Host;
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
            _: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, stat: bool, args: &[V]) -> V {
        invoke_member(
            ctx,
            if stat {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(STATE, name, desc),
            args,
        )
    }
    fn array(rt: &Rt, value: V) -> Vec<V> {
        match &rt.get_obj(value).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("WeatherState[]"),
        }
    }
    #[test]
    fn values_keep_canonical_names_ordinals_and_independent_array_storage() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = invoke(&mut ctx, "values", VALUES, true, &[]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "original WeatherState.values API missing"
        );
        let values = array(ctx.rt, first);
        assert_eq!(values.len(), 4);
        for (index, name) in NAMES.into_iter().enumerate() {
            let value = read_static_field(&mut ctx, STATE, name, DESC);
            assert_eq!(values[index], value);
            assert_eq!(
                invoke(&mut ctx, "ordinal", "()I", false, &[value]),
                V::I(index as i32)
            );
            let label = invoke(&mut ctx, "name", "()Ljava/lang/String;", false, &[value]);
            assert_eq!(ctx.rt.str_of(label), Some(name));
            let serialized = invoke(
                &mut ctx,
                "getSerializedName",
                "()Ljava/lang/String;",
                false,
                &[value],
            );
            assert_eq!(
                ctx.rt.str_of(serialized),
                Some(name.to_ascii_lowercase().as_str())
            );
            let label = ctx.rt.intern_str(name);
            assert_eq!(invoke(&mut ctx,"valueOf","(Ljava/lang/String;)Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;",true,&[label]),value);
        }
        if let Data::Array(values) = &mut ctx.rt.get_obj_mut(first).unwrap().data {
            values[0] = V::NULL;
        }
        let second = invoke(&mut ctx, "values", VALUES, true, &[]);
        assert_ne!(first, second);
        assert_eq!(array(ctx.rt, second), values);
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn value_of_and_unknown_members_fail_without_default_weather_states() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for text in ["unaffected", "CORRODED", ""] {
            let text = ctx.rt.new_str(text);
            invoke(&mut ctx,"valueOf","(Ljava/lang/String;)Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;",true,&[text]);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/IllegalArgumentException")
            );
        }
        invoke(
            &mut ctx,
            "valueOf",
            "(Ljava/lang/String;)Lnet/minecraft/world/level/block/WeatheringCopper$WeatherState;",
            true,
            &[V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert!(get_static_field(ctx.rt, STATE, "CORRODED", DESC).is_none());
        assert!(get_static_field(ctx.rt, STATE, "OXIDIZED", "I").is_none());
    }
}

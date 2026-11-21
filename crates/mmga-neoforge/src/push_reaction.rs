//! Minecraft 1.21.1 piston reaction enum (`epj` in the official client).
//! Values describe the reaction; piston/world mechanics are not implemented here.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const REACTION: &str = "net/minecraft/world/level/material/PushReaction";
const DESC: &str = "Lnet/minecraft/world/level/material/PushReaction;";
const NAMES: [&str; 5] = ["NORMAL", "DESTROY", "BLOCK", "IGNORE", "PUSH_ONLY"];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != REACTION || desc != DESC {
        return None;
    }
    constant(rt, name)
}
pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if member.owner != REACTION {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str(), is_static) {
        ("values", "()[Lnet/minecraft/world/level/material/PushReaction;", true) => {
            let values = NAMES
                .iter()
                .map(|name| constant(rt, name).unwrap())
                .collect();
            Some(rt.alloc(
                "[Lnet/minecraft/world/level/material/PushReaction;",
                0,
                Data::Array(values),
            ))
        }
        (
            "valueOf",
            "(Ljava/lang/String;)Lnet/minecraft/world/level/material/PushReaction;",
            true,
        ) => {
            let name = *args.first()?;
            if name.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let name = rt.str_of(name)?.to_owned();
            Some(constant(rt, &name).unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!(
                        "No enum constant net.minecraft.world.level.material.PushReaction.{name}"
                    ),
                )
            }))
        }
        _ => None,
    }
}
fn constant(rt: &mut Rt, name: &str) -> Option<V> {
    let index = NAMES.iter().position(|&candidate| candidate == name)?;
    if let Some(value) = rt.get_static(REACTION, name) {
        return Some(value);
    }
    let value = rt.new_instance(REACTION, 0);
    let label = rt.intern_str(name);
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(index as i32));
    rt.put_static(REACTION, name, value);
    Some(value)
}
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    sub_class == REACTION
        && matches!(
            super_class,
            "java/lang/Enum" | "java/lang/Comparable" | "java/io/Serializable"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, Data, ExecCtx, HostApi, LinkedSpace};
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
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, is_static: bool, args: &[V]) -> V {
        invoke_member(
            ctx,
            if is_static {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(REACTION, name, desc),
            args,
        )
    }
    #[test]
    fn piston_reaction_constants_are_canonical_and_values_returns_a_copy() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let array = invoke(
            &mut ctx,
            "values",
            "()[Lnet/minecraft/world/level/material/PushReaction;",
            true,
            &[],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let values = match &ctx.rt.get_obj(array).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("array"),
        };
        assert_eq!(values.len(), 5);
        for (index, name) in NAMES.iter().enumerate() {
            let constant = read_static_field(&mut ctx, REACTION, name, DESC);
            assert_eq!(constant, values[index]);
            assert_eq!(
                invoke(&mut ctx, "ordinal", "()I", false, &[constant]),
                V::I(index as i32)
            );
            let label = invoke(
                &mut ctx,
                "toString",
                "()Ljava/lang/String;",
                false,
                &[constant],
            );
            assert_eq!(ctx.rt.str_of(label), Some(*name));
            let string = ctx.rt.intern_str(name);
            assert_eq!(
                invoke(
                    &mut ctx,
                    "valueOf",
                    "(Ljava/lang/String;)Lnet/minecraft/world/level/material/PushReaction;",
                    true,
                    &[string]
                ),
                constant
            );
        }
        if let Data::Array(values) = &mut ctx.rt.get_obj_mut(array).unwrap().data {
            values[0] = V::NULL;
        }
        let copy = invoke(
            &mut ctx,
            "values",
            "()[Lnet/minecraft/world/level/material/PushReaction;",
            true,
            &[],
        );
        assert_ne!(array, copy);
        assert!(matches!(&ctx.rt.get_obj(copy).unwrap().data,Data::Array(copy)if copy==&values));
        assert!(linked.is_subtype(REACTION, "java/lang/Enum", ctx.host));
    }
    #[test]
    fn invalid_enum_names_and_unknown_members_fail() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for name in ["normal", "NOT_A_REACTION"] {
            let name = ctx.rt.intern_str(name);
            invoke(
                &mut ctx,
                "valueOf",
                "(Ljava/lang/String;)Lnet/minecraft/world/level/material/PushReaction;",
                true,
                &[name],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/IllegalArgumentException")
            );
        }
        invoke(
            &mut ctx,
            "valueOf",
            "(Ljava/lang/String;)Lnet/minecraft/world/level/material/PushReaction;",
            true,
            &[V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert!(get_static_field(ctx.rt, REACTION, "UNKNOWN", DESC).is_none());
        assert!(get_static_field(ctx.rt, REACTION, "NORMAL", "I").is_none());
    }
}

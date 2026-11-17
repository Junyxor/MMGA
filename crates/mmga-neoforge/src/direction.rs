//! The canonical Minecraft 1.21.1 Direction enum used by mod initialization.
//!
//! Checked against Mojang's official `Direction` (`ji`) bytecode and mappings:
//! client SHA-1 30c73b1c5da787909b2f73340419fdf13b9def88, mappings SHA-1
//! 2244b6f072256667bcd9a73df124d6c58de77992. Geometry, axis operations, codecs,
//! and rotation methods are not implemented by this enum-only module.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const DIRECTION: &str = "net/minecraft/core/Direction";
const DIRECTION_DESC: &str = "Lnet/minecraft/core/Direction;";
const VALUES_DESC: &str = "()[Lnet/minecraft/core/Direction;";
const NAMES: [&str; 6] = ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != DIRECTION || desc != DIRECTION_DESC {
        return None;
    }
    constant(rt, name)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if member.owner != DIRECTION {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str(), is_static) {
        ("values", VALUES_DESC, true) => {
            let values = NAMES
                .into_iter()
                .map(|name| constant(rt, name).unwrap())
                .collect();
            Some(rt.alloc("[Lnet/minecraft/core/Direction;", 0, Data::Array(values)))
        }
        ("valueOf", "(Ljava/lang/String;)Lnet/minecraft/core/Direction;", true) => {
            let value = *args.first()?;
            if value.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let name = rt.str_of(value)?.to_owned();
            Some(constant(rt, &name).unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!("No enum constant net.minecraft.core.Direction.{name}"),
                )
            }))
        }
        ("getName" | "getSerializedName" | "toString", "()Ljava/lang/String;", false) => {
            Some(rt.get_field(*args.first()?, "name"))
        }
        _ => None,
    }
}

fn constant(rt: &mut Rt, name: &str) -> Option<V> {
    let ordinal = NAMES.iter().position(|&candidate| candidate == name)?;
    if let Some(value) = rt.get_static(DIRECTION, name) {
        return Some(value);
    }
    let value = rt.new_instance(DIRECTION, 0);
    let enum_name = rt.intern_str(name);
    let serialized_name = rt.intern_str(&name.to_ascii_lowercase());
    rt.put_field(value, "$name", enum_name);
    rt.put_field(value, "$ordinal", V::I(ordinal as i32));
    rt.put_field(value, "name", serialized_name);
    rt.put_static(DIRECTION, name, value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, Data, ExecCtx, HostApi, LinkedSpace};

    struct DirectionHost;
    impl HostApi for DirectionHost {
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
    }

    fn values(ctx: &mut ExecCtx<'_>) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(DIRECTION, "values", VALUES_DESC),
            &[],
        )
    }

    fn items(rt: &Rt, value: V) -> Vec<V> {
        match &rt.get_obj(value).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("expected Direction[]"),
        }
    }

    #[test]
    fn values_preserve_mekanism_enum_order_and_getstatic_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        // EnumUtils obtains values() before reading four horizontal GETSTATICs.
        let first = values(&mut ctx);
        assert!(ctx.rt.pending_exception.is_none());
        let canonical = items(ctx.rt, first);
        assert_eq!(canonical.len(), 6);
        for (ordinal, (name, value)) in NAMES.into_iter().zip(&canonical).enumerate() {
            assert_eq!(
                *value,
                read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC)
            );
            assert_eq!(
                *value,
                read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC)
            );
            assert_eq!(ctx.rt.str_of(ctx.rt.get_field(*value, "$name")), Some(name));
            assert_eq!(ctx.rt.get_field(*value, "$ordinal"), V::I(ordinal as i32));
            let string = ctx.rt.intern_str(name);
            let from_name = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    DIRECTION,
                    "valueOf",
                    "(Ljava/lang/String;)Lnet/minecraft/core/Direction;",
                ),
                &[string],
            );
            assert_eq!(*value, from_name);
            for name_method in ["getName", "getSerializedName", "toString"] {
                let label = invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(DIRECTION, name_method, "()Ljava/lang/String;"),
                    &[*value],
                );
                assert_eq!(
                    ctx.rt.str_of(label),
                    Some(name.to_ascii_lowercase().as_str())
                );
            }
        }
        if let Data::Array(array) = &mut ctx.rt.get_obj_mut(first).unwrap().data {
            array[0] = V::NULL;
        }
        let second = values(&mut ctx);
        assert_ne!(first, second);
        assert_eq!(items(ctx.rt, second), canonical);
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn invalid_enum_names_and_unknown_signatures_fail_explicitly() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for name in ["NORTHEAST", "north", ""] {
            let string = ctx.rt.intern_str(name);
            invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    DIRECTION,
                    "valueOf",
                    "(Ljava/lang/String;)Lnet/minecraft/core/Direction;",
                ),
                &[string],
            );
            let error = ctx.rt.take_exception().expect("unknown enum must fail");
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("IllegalArgumentException"));
        }
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                DIRECTION,
                "valueOf",
                "(Ljava/lang/String;)Lnet/minecraft/core/Direction;",
            ),
            &[V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        assert!(get_static_field(ctx.rt, DIRECTION, "NORTHEAST", DIRECTION_DESC).is_none());
        assert!(get_static_field(ctx.rt, DIRECTION, "NORTH", "I").is_none());
        assert!(call(
            ctx.rt,
            &MemberRef::new(DIRECTION, "values", "()Ljava/lang/Object;"),
            true,
            &[]
        )
        .is_none());
    }
}

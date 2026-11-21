//! Vanilla dispenser behavior metadata used by Create's fluid bootstrap.
//!
//! `DefaultDispenseItemBehavior()` only initializes the behavior object in
//! Minecraft 1.21.1. The actual block source, item consumption, sound, and
//! world mutation paths remain unsupported until a modeled world invokes them.

use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};

const DEFAULT: &str = "net/minecraft/core/dispenser/DefaultDispenseItemBehavior";
const BEHAVIOR: &str = "net/minecraft/core/dispenser/DispenseItemBehavior";

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static || member.owner != DEFAULT {
        return None;
    }
    if member.name == "<init>" && member.desc == "()V" {
        let receiver = *args.first()?;
        if receiver.is_null() {
            return Some(rt.throw_exception(
                "java/lang/NullPointerException",
                "DefaultDispenseItemBehavior receiver is null",
            ));
        }
        return Some(V::Void);
    }
    None
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == DEFAULT && matches!(sup, BEHAVIOR | "java/lang/Object")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace};

    struct Host;
    impl HostApi for Host {
        fn get_static_field(&mut self, _: &mut Rt, _: &str, _: &str, _: &str) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(rt, member, is_static, args)
        }
    }

    #[test]
    fn default_behavior_constructor_is_executable_and_type_identity_is_preserved() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let receiver = ctx.rt.new_instance(DEFAULT, 0);
        let result = invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(DEFAULT, "<init>", "()V"),
            &[receiver],
        );
        assert_eq!(result, V::Void);
        assert!(ctx.rt.pending_exception.is_none());
        assert!(is_subtype(DEFAULT, BEHAVIOR));
        assert!(is_subtype(DEFAULT, "java/lang/Object"));
        assert!(!is_subtype(DEFAULT, "net/minecraft/world/item/Item"));
    }
}

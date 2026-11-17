//! Common chat-component factories required by published mod bytecode.
//!
//! Minecraft 1.21.1's `CommonComponents.space()` calls `Component.literal(" ")` on every
//! invocation (official mapped class `wy`, method `a()`). It must not return the shared
//! `CommonComponents.SPACE` object because the returned component is mutable.

use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};
use std::collections::BTreeMap;

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static
        || member.owner != "net/minecraft/network/chat/CommonComponents"
        || member.name != "space"
        || member.desc != "()Lnet/minecraft/network/chat/MutableComponent;"
        || !args.is_empty()
    {
        return None;
    }
    let space = rt.intern_str(" ");
    crate::minecraft::call_chat(
        rt,
        &BTreeMap::new(),
        &MemberRef::new(
            "net/minecraft/network/chat/Component",
            "literal",
            "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
        ),
        true,
        &[space],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_display_source_space_is_a_fresh_mutable_literal() {
        let mut rt = Rt::new();
        let member = MemberRef::new(
            "net/minecraft/network/chat/CommonComponents",
            "space",
            "()Lnet/minecraft/network/chat/MutableComponent;",
        );
        let first = call(&mut rt, &member, true, &[]).unwrap();
        let second = call(&mut rt, &member, true, &[]).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            rt.class_of(first),
            Some("net/minecraft/network/chat/MutableComponent")
        );

        let suffix = rt.intern_str("rpm");
        crate::minecraft::call_chat(
            &mut rt,
            &BTreeMap::new(),
            &MemberRef::new(
                "net/minecraft/network/chat/MutableComponent",
                "append",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ),
            false,
            &[first, suffix],
        )
        .unwrap();
        for (component, expected) in [(first, " rpm"), (second, " ")] {
            let text = crate::minecraft::call_chat(
                &mut rt,
                &BTreeMap::new(),
                &MemberRef::new(
                    "net/minecraft/network/chat/Component",
                    "getString",
                    "()Ljava/lang/String;",
                ),
                false,
                &[component],
            )
            .unwrap();
            assert_eq!(rt.str_of(text), Some(expected));
        }
        assert!(rt.pending_exception.is_none());
    }

    #[test]
    fn unsupported_common_component_calls_remain_unhandled() {
        let mut rt = Rt::new();
        let member = MemberRef::new(
            "net/minecraft/network/chat/CommonComponents",
            "space",
            "()Ljava/lang/String;",
        );
        assert_eq!(call(&mut rt, &member, true, &[]), None);
        let member = MemberRef::new(
            "net/minecraft/network/chat/CommonComponents",
            "space",
            "()Lnet/minecraft/network/chat/MutableComponent;",
        );
        assert_eq!(call(&mut rt, &member, false, &[V::NULL]), None);
    }
}

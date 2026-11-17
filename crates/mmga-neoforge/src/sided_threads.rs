//! FML's logical-side ThreadGroup constants. These are metadata, not a thread
//! scheduler. The loader thread belongs to an unsided main group until the
//! host explicitly runs a server tick inside the server execution group.
use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};

const GROUPS: &str = "net/neoforged/fml/util/thread/SidedThreadGroups";
const GROUP: &str = "net/neoforged/fml/util/thread/SidedThreadGroup";
const SIDE: &str = "net/neoforged/fml/LogicalSide";

fn side(rt: &mut Rt, name: &str) -> V {
    if let Some(value) = rt.get_static(SIDE, name) {
        return value;
    }
    let value = rt.new_instance(SIDE, 0);
    let label = rt.intern_str(name);
    rt.put_field(value, "$name", label);
    rt.put_field(
        value,
        "$ordinal",
        V::I(if name == "CLIENT" { 0 } else { 1 }),
    );
    rt.put_static(SIDE, name, value);
    value
}

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if !matches!(name, "CLIENT" | "SERVER") {
        return None;
    }
    if owner == SIDE && desc == format!("L{SIDE};") {
        return Some(side(rt, name));
    }
    if owner != GROUPS || desc != format!("L{GROUP};") {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = rt.new_instance(GROUP, 0);
    let label = rt.intern_str(name);
    let logical_side = side(rt, name);
    rt.put_field(value, "name", label);
    rt.put_field(value, "side", logical_side);
    rt.put_static(owner, name, value);
    Some(value)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static
        && member.owner == GROUP
        && member.name == "getSide"
        && member.desc == format!("()L{SIDE};")
    {
        return Some(rt.get_field(args[0], "side"));
    }
    if !is_static && member.owner == SIDE {
        match (member.name.as_str(), member.desc.as_str()) {
            ("isClient", "()Z") => {
                return Some(V::bool(
                    rt.str_of(rt.get_field(args[0], "$name")) == Some("CLIENT"),
                ))
            }
            ("isServer", "()Z") => {
                return Some(V::bool(
                    rt.str_of(rt.get_field(args[0], "$name")) == Some("SERVER"),
                ))
            }
            _ => {}
        }
    }
    // Any attempt to start a Java thread or execute a ThreadFactory remains
    // unhandled. Thread identity itself is maintained by the runtime JDK layer.
    None
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == GROUP
        && matches!(
            sup,
            "java/lang/ThreadGroup" | "java/util/concurrent/ThreadFactory"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreparedLoad;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx};

    #[test]
    fn fml_groups_are_stable_and_do_not_label_loader_thread_as_server() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let server = read_static_field(&mut ctx, GROUPS, "SERVER", &format!("L{GROUP};"));
        assert!(ctx.rt.pending_exception.is_none());
        let again = read_static_field(&mut ctx, GROUPS, "SERVER", &format!("L{GROUP};"));
        assert_eq!(server, again);
        let side = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(GROUP, "getSide", &format!("()L{SIDE};")),
            &[server],
        );
        let expected = read_static_field(&mut ctx, SIDE, "SERVER", &format!("L{SIDE};"));
        assert_eq!(side, expected);
        let thread = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new("java/lang/Thread", "currentThread", "()Ljava/lang/Thread;"),
            &[],
        );
        let current_group = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "java/lang/Thread",
                "getThreadGroup",
                "()Ljava/lang/ThreadGroup;",
            ),
            &[thread],
        );
        assert_ne!(server, current_group);
        let name = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/ThreadGroup", "getName", "()Ljava/lang/String;"),
            &[server],
        );
        assert_eq!(ctx.rt.str_of(name), Some("SERVER"));
        assert!(ctx.rt.pending_exception.is_none());
    }
}

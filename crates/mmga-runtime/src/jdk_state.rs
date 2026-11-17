//! Stateful JDK value helpers. ThreadLocal storage follows the current native
//! execution thread; no Java thread scheduling or Thread.start is fabricated.
use crate::{Data, ExecCtx, V};
use mmga_classfile::{op, MemberRef};

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == "java/util/function/Function"
        && is_static
        && name == "identity"
        && desc == "()Ljava/util/function/Function;"
    {
        if let Some(value) = ctx.rt.get_static(owner, "$identity") {
            return Some(value);
        }
        let value = ctx.rt.new_instance("mmga/internal/IdentityFunction", 0);
        ctx.rt.put_static(owner, "$identity", value);
        return Some(value);
    }
    if !is_static
        && matches!(
            owner,
            "java/util/function/Function" | "mmga/internal/IdentityFunction"
        )
        && name == "apply"
        && desc == "(Ljava/lang/Object;)Ljava/lang/Object;"
        && ctx.rt.class_of(args[0]) == Some("mmga/internal/IdentityFunction")
    {
        return Some(args[1]);
    }
    if is_static
        && owner == "java/lang/ThreadLocal"
        && name == "withInitial"
        && desc == "(Ljava/util/function/Supplier;)Ljava/lang/ThreadLocal;"
    {
        if args[0].is_null() {
            return Some(
                ctx.rt
                    .throw_exception("java/lang/NullPointerException", "ThreadLocal supplier"),
            );
        }
        let local = ctx
            .rt
            .new_instance("java/lang/ThreadLocal$SuppliedThreadLocal", 0);
        let values = ctx.rt.alloc("java/util/HashMap", 0, Data::Map(Vec::new()));
        ctx.rt.put_field(local, "$thread_values", values);
        ctx.rt.put_field(local, "$initial_supplier", args[0]);
        return Some(local);
    }
    if is_static
        || !matches!(
            owner,
            "java/lang/ThreadLocal" | "java/lang/ThreadLocal$SuppliedThreadLocal"
        )
    {
        return None;
    }
    let local = args[0];
    match (name, desc) {
        ("<init>", "()V") => {
            let values = ctx.rt.alloc("java/util/HashMap", 0, Data::Map(Vec::new()));
            ctx.rt.put_field(local, "$thread_values", values);
            Some(V::Void)
        }
        ("initialValue", "()Ljava/lang/Object;") => {
            let supplier = ctx.rt.get_field(local, "$initial_supplier");
            if supplier.is_null() {
                Some(V::NULL)
            } else {
                Some(crate::invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
                    &[supplier],
                ))
            }
        }
        ("get", "()Ljava/lang/Object;") | ("set", "(Ljava/lang/Object;)V") | ("remove", "()V") => {
            let values = ctx.rt.get_field(local, "$thread_values");
            let key = ctx
                .rt
                .intern_str(&format!("{:?}", std::thread::current().id()));
            let existing = match ctx.rt.get_obj(values).map(|o| &o.data) {
                Some(Data::Map(entries)) => {
                    entries.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
                }
                _ => {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/IllegalStateException",
                        "ThreadLocal was not initialized",
                    ))
                }
            };
            if name == "get" {
                if let Some(value) = existing {
                    return Some(value);
                }
                let initialized = crate::invoke_member(
                    ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(
                        "java/lang/ThreadLocal",
                        "initialValue",
                        "()Ljava/lang/Object;",
                    ),
                    &[local],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if let Some(Data::Map(entries)) = ctx.rt.get_obj_mut(values).map(|o| &mut o.data) {
                    entries.retain(|(k, _)| *k != key);
                    entries.push((key, initialized));
                }
                return Some(initialized);
            }
            if let Some(Data::Map(entries)) = ctx.rt.get_obj_mut(values).map(|o| &mut o.data) {
                entries.retain(|(k, _)| *k != key);
                if name == "set" {
                    entries.push((key, args[1]));
                }
            }
            Some(V::Void)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NullHost, Rt};

    fn call_vm(rt: &mut Rt, owner: &str, name: &str, desc: &str, stat: bool, args: &[V]) -> V {
        invoke_member(
            &mut ExecCtx {
                rt,
                linked: &LinkedSpace::new(),
                host: &mut NullHost,
                depth: 0,
            },
            if stat {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    #[test]
    fn function_identity_preserves_null_and_reference_identity() {
        let mut rt = Rt::new();
        let identity = call_vm(
            &mut rt,
            "java/util/function/Function",
            "identity",
            "()Ljava/util/function/Function;",
            true,
            &[],
        );
        assert!(rt.pending_exception.is_none());
        let value = rt.new_instance("example/Object", 0);
        for item in [value, V::NULL] {
            assert_eq!(
                call_vm(
                    &mut rt,
                    "java/util/function/Function",
                    "apply",
                    "(Ljava/lang/Object;)Ljava/lang/Object;",
                    false,
                    &[identity, item]
                ),
                item
            );
        }
    }

    #[test]
    fn thread_local_isolates_threads_and_reinitializes_after_remove() {
        let mut rt = Rt::new();
        let local = rt.new_instance("java/lang/ThreadLocal", 0);
        call_vm(
            &mut rt,
            "java/lang/ThreadLocal",
            "<init>",
            "()V",
            false,
            &[local],
        );
        assert!(rt.pending_exception.is_none());
        assert_eq!(
            call_vm(
                &mut rt,
                "java/lang/ThreadLocal",
                "get",
                "()Ljava/lang/Object;",
                false,
                &[local]
            ),
            V::NULL
        );
        let value = rt.intern_str("main value");
        call_vm(
            &mut rt,
            "java/lang/ThreadLocal",
            "set",
            "(Ljava/lang/Object;)V",
            false,
            &[local, value],
        );
        let mut rt = std::thread::spawn(move || {
            assert_eq!(
                call_vm(
                    &mut rt,
                    "java/lang/ThreadLocal",
                    "get",
                    "()Ljava/lang/Object;",
                    false,
                    &[local]
                ),
                V::NULL
            );
            let value = rt.intern_str("other value");
            call_vm(
                &mut rt,
                "java/lang/ThreadLocal",
                "set",
                "(Ljava/lang/Object;)V",
                false,
                &[local, value],
            );
            rt
        })
        .join()
        .unwrap();
        assert_eq!(
            call_vm(
                &mut rt,
                "java/lang/ThreadLocal",
                "get",
                "()Ljava/lang/Object;",
                false,
                &[local]
            ),
            value
        );
        call_vm(
            &mut rt,
            "java/lang/ThreadLocal",
            "remove",
            "()V",
            false,
            &[local],
        );
        assert_eq!(
            call_vm(
                &mut rt,
                "java/lang/ThreadLocal",
                "get",
                "()Ljava/lang/Object;",
                false,
                &[local]
            ),
            V::NULL
        );
    }
}

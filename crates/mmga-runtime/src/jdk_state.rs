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
    if is_static && owner == "java/lang/Thread" && name == "currentThread" && desc == "()Ljava/lang/Thread;" {
        let cache = format!("$thread:{:?}", std::thread::current().id());
        if let Some(value) = ctx.rt.get_static(owner, &cache) { return Some(value); }
        let thread = ctx.rt.new_instance(owner, 0);
        let group = match ctx.rt.get_static("java/lang/ThreadGroup", "$main") {
            Some(group) => group,
            None => {
                let group = ctx.rt.new_instance("java/lang/ThreadGroup", 0);
                let name = ctx.rt.intern_str("main");
                ctx.rt.put_field(group, "name", name);
                ctx.rt.put_static("java/lang/ThreadGroup", "$main", group);
                group
            }
        };
        let thread_name = ctx.rt.new_str(std::thread::current().name().unwrap_or("main"));
        ctx.rt.put_field(thread, "group", group);
        ctx.rt.put_field(thread, "name", thread_name);
        ctx.rt.put_static(owner, &cache, thread);
        return Some(thread);
    }
    if !is_static && owner == "java/lang/Thread" {
        match (name, desc) {
            ("getThreadGroup", "()Ljava/lang/ThreadGroup;") => return Some(ctx.rt.get_field(args[0], "group")),
            ("getName", "()Ljava/lang/String;") => return Some(ctx.rt.get_field(args[0], "name")),
            _ => {}
        }
    }
    if !is_static && owner == "java/lang/ThreadGroup" && name == "getName" && desc == "()Ljava/lang/String;" {
        return Some(ctx.rt.get_field(args[0], "name"));
    }
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
    if owner == "java/util/function/UnaryOperator"
        && is_static
        && name == "identity"
        && desc == "()Ljava/util/function/UnaryOperator;"
    {
        if let Some(value) = ctx.rt.get_static(owner, "$identity") {
            return Some(value);
        }
        let value = ctx.rt.new_instance("mmga/internal/IdentityUnaryOperator", 0);
        ctx.rt.put_static(owner, "$identity", value);
        return Some(value);
    }
    if !is_static
        && matches!(
            owner,
            "java/util/function/Function"
                | "java/util/function/UnaryOperator"
                | "mmga/internal/IdentityFunction"
                | "mmga/internal/IdentityUnaryOperator"
        )
        && name == "apply"
        && desc == "(Ljava/lang/Object;)Ljava/lang/Object;"
        && matches!(
            ctx.rt.class_of(args[0]),
            Some("mmga/internal/IdentityFunction" | "mmga/internal/IdentityUnaryOperator")
        )
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
    let inherited_thread_local_method = !is_static
        && name != "<init>"
        && ctx.rt.class_of(args[0]).is_some_and(|class| {
            ctx.linked.is_subtype(class, "java/lang/ThreadLocal", ctx.host)
        })
        && ctx.linked.is_subtype(owner, "java/lang/ThreadLocal", ctx.host);
    if is_static || !(matches!(owner, "java/lang/ThreadLocal" | "java/lang/ThreadLocal$SuppliedThreadLocal")
        || inherited_thread_local_method) {
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
    fn unary_operator_identity_is_a_cached_unary_operator_function_and_preserves_inputs() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let identity = crate::invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "java/util/function/UnaryOperator",
                "identity",
                "()Ljava/util/function/UnaryOperator;",
            ),
            &[],
        );
        let next = crate::invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "java/util/function/UnaryOperator",
                "identity",
                "()Ljava/util/function/UnaryOperator;",
            ),
            &[],
        );
        assert_eq!(identity, next, "the JDK returns a cached identity lambda");
        assert_eq!(ctx.rt.class_of(identity), Some("mmga/internal/IdentityUnaryOperator"));
        assert!(linked.is_subtype(ctx.rt.class_of(identity).unwrap(), "java/util/function/UnaryOperator", ctx.host));
        assert!(linked.is_subtype(ctx.rt.class_of(identity).unwrap(), "java/util/function/Function", ctx.host));

        let reference = ctx.rt.new_instance("example/Value", 0);
        for value in [reference, V::NULL] {
            let returned = crate::invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(
                    "java/util/function/UnaryOperator",
                    "apply",
                    "(Ljava/lang/Object;)Ljava/lang/Object;",
                ),
                &[identity, value],
            );
            assert_eq!(returned, value);
            assert!(ctx.rt.pending_exception.is_none());
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

    #[test]
    fn inherited_thread_local_methods_resolve_from_subclass_owner() {
        use crate::{NativeClassDef, NativeModuleRegistry};
        fn initial(ctx:&mut ExecCtx<'_>,args:&[V])->V {
            let count=ctx.rt.get_field(args[0],"calls").i()+1;
            ctx.rt.put_field(args[0],"calls",V::I(count));
            ctx.rt.intern_str(&format!("initial-{count}"))
        }
        let mut linked=LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry{classes:&[NativeClassDef{
            name:"test/CustomLocal",superclass:Some("java/lang/ThreadLocal"),interfaces:&[],is_interface:false,
            methods:&[("initialValue.()Ljava/lang/Object;",initial,false,false)],
        }],lambdas:&[]});
        let mut rt=Rt::new();let local=rt.new_instance("test/CustomLocal",0);let mut host=NullHost;
        let mut ctx=ExecCtx{rt:&mut rt,linked:&linked,host:&mut host,depth:0};
        invoke_member(&mut ctx,op::INVOKESPECIAL,&MemberRef::new("java/lang/ThreadLocal","<init>","()V"),&[local]);
        let first=invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("test/CustomLocal","get","()Ljava/lang/Object;"),&[local]);
        assert!(ctx.rt.pending_exception.is_none());assert_eq!(ctx.rt.str_of(first),Some("initial-1"));
        invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("test/CustomLocal","set","(Ljava/lang/Object;)V"),&[local,V::NULL]);
        assert_eq!(invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("test/CustomLocal","get","()Ljava/lang/Object;"),&[local]),V::NULL);
        assert_eq!(ctx.rt.get_field(local,"calls"),V::I(1));
        invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("test/CustomLocal","remove","()V"),&[local]);
        let next=invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new("test/CustomLocal","get","()Ljava/lang/Object;"),&[local]);
        assert_eq!(ctx.rt.str_of(next),Some("initial-2"));assert!(ctx.rt.pending_exception.is_none());
    }
}

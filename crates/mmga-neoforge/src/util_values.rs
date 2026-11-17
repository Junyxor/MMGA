//! Minecraft 1.21.1 Util.make factories, verified against official `ad` bytecode.
//! Supplier.get and Consumer.accept execute through the VM; their results,
//! identity and exceptions are preserved rather than replaced by defaults.
use mmga_classfile::MemberRef;
use mmga_runtime::{invoke_member, ExecCtx, V};
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static || member.owner != "net/minecraft/Util" || member.name != "make" {
        return None;
    }
    match member.desc.as_str() {
        "(Ljava/util/function/Supplier;)Ljava/lang/Object;" => Some(invoke_member(
            ctx,
            mmga_classfile::op::INVOKEINTERFACE,
            &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
            &[*args.first()?],
        )),
        "(Ljava/lang/Object;Ljava/util/function/Consumer;)Ljava/lang/Object;" => {
            invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new(
                    "java/util/function/Consumer",
                    "accept",
                    "(Ljava/lang/Object;)V",
                ),
                &[*args.get(1)?, *args.first()?],
            );
            Some(if ctx.rt.pending_exception.is_some() {
                V::NULL
            } else {
                args[0]
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, HostApi, LinkedSpace, Rt};
    #[derive(Default)]
    struct FactoryHost {
        calls: usize,
        received: V,
        throw: bool,
    }
    impl HostApi for FactoryHost {
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
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            if member.owner == "java/util/function/Supplier" && member.name == "get" {
                self.calls += 1;
                if self.throw {
                    return Some(rt.throw_exception("example/FactoryFailure", "supplier"));
                }
                return Some(rt.get_field(args[0], "result"));
            }
            if member.owner == "java/util/function/Consumer" && member.name == "accept" {
                self.calls += 1;
                self.received = args[1];
                if self.throw {
                    return Some(rt.throw_exception("example/FactoryFailure", "consumer"));
                }
                rt.put_field(args[1], "initialized", V::TRUE);
                return Some(V::Void);
            }
            call(
                &mut ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                },
                member,
                is_static,
                args,
            )
        }
    }
    fn make(ctx: &mut ExecCtx<'_>, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new("net/minecraft/Util", "make", desc),
            args,
        )
    }
    #[test]
    fn make_executes_factories_once_and_preserves_object_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FactoryHost::default();
        let object = rt.new_instance("example/Value", 0);
        let supplier = rt.new_instance("example/Supplier", 0);
        rt.put_field(supplier, "result", object);
        let consumer = rt.new_instance("example/Consumer", 0);
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            assert_eq!(
                make(
                    &mut ctx,
                    "(Ljava/util/function/Supplier;)Ljava/lang/Object;",
                    &[supplier]
                ),
                object
            );
            assert_eq!(
                make(
                    &mut ctx,
                    "(Ljava/lang/Object;Ljava/util/function/Consumer;)Ljava/lang/Object;",
                    &[object, consumer]
                ),
                object
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(ctx.rt.get_field(object, "initialized"), V::TRUE);
        }
        assert_eq!(host.calls, 2);
        assert_eq!(host.received, object);
    }
    #[test]
    fn make_preserves_callback_exception_and_checks_null_function() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FactoryHost {
            throw: true,
            ..FactoryHost::default()
        };
        let supplier = rt.new_instance("example/Supplier", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        make(
            &mut ctx,
            "(Ljava/util/function/Supplier;)Ljava/lang/Object;",
            &[supplier],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("FactoryFailure"));
        make(
            &mut ctx,
            "(Ljava/util/function/Supplier;)Ljava/lang/Object;",
            &[V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        make(
            &mut ctx,
            "(Ljava/lang/Object;Ljava/util/function/Consumer;)Ljava/lang/Object;",
            &[V::NULL, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
    }
}

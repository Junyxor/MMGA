//! Stored constructor state for Minecraft's JSON resource reload listener.
//!
//! Official 1.21.1 `aui.<init>(Gson,String)` only calls the empty `auj` constructor
//! and stores the two references, including null. Preparation and asynchronous
//! reload require ResourceManager and executor semantics and remain unsupported.

use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};

const JSON: &str = "net/minecraft/server/packs/resources/SimpleJsonResourceReloadListener";
const PREPARABLE: &str = "net/minecraft/server/packs/resources/SimplePreparableReloadListener";
const LISTENER: &str = "net/minecraft/server/packs/resources/PreparableReloadListener";

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == JSON && matches!(sup, PREPARABLE | LISTENER)) || (sub == PREPARABLE && sup == LISTENER)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static || member.name != "<init>" {
        return None;
    }
    match (member.owner.as_str(), member.desc.as_str()) {
        (JSON, "(Lcom/google/gson/Gson;Ljava/lang/String;)V") => {
            rt.put_field(*args.first()?, "gson", *args.get(1)?);
            rt.put_field(args[0], "directory", *args.get(2)?);
            Some(V::Void)
        }
        (PREPARABLE, "()V") => Some(V::Void),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace};

    struct ReloadHost;
    impl HostApi for ReloadHost {
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
            call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            is_subtype(sub, sup)
        }
    }

    #[test]
    fn backpack_reload_listener_keeps_gson_directory_and_official_null_behavior() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReloadHost;
        let gson = rt.new_instance("com/google/gson/Gson", 0);
        let directory = rt.intern_str("registry");
        let listener = rt.new_instance("example/BackpackRegistryLoader", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let ctor = MemberRef::new(
            JSON,
            "<init>",
            "(Lcom/google/gson/Gson;Ljava/lang/String;)V",
        );
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &ctor,
            &[listener, gson, directory],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(listener, "gson"), gson);
        assert_eq!(ctx.rt.get_field(listener, "directory"), directory);
        let with_null = ctx.rt.new_instance("example/BackpackRegistryLoader", 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &ctor,
            &[with_null, V::NULL, V::NULL],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(with_null, "gson"), V::NULL);
        assert_eq!(ctx.rt.get_field(with_null, "directory"), V::NULL);
        assert!(ctx.linked.is_subtype(JSON, LISTENER, ctx.host));
    }

    #[test]
    fn unsupported_resource_preparation_never_reports_an_empty_success() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ReloadHost;
        let listener = rt.new_instance(JSON, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new(JSON,"prepare","(Lnet/minecraft/server/packs/resources/ResourceManager;Lnet/minecraft/util/profiling/ProfilerFiller;)Ljava/util/Map;"),&[listener,V::NULL,V::NULL]);
        let error = ctx
            .rt
            .take_exception()
            .expect("unimplemented resource scan must fail");
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert!(call(
            ctx.rt,
            &MemberRef::new(JSON, "<init>", "()V"),
            false,
            &[listener]
        )
        .is_none());
    }
}

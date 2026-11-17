//! FML ordered event posting, checked against FancyModLoader 1.21.1 source blobs
//! ec848c1ac32375d0d2eae2334ad271d2ec8a3522 (ModLoader) and
//! ec73f6004863cded6914d0c0a12e2767a713a0ac (ModLoadingContext).

use super::FmlLoader;
use crate::bus::{dispatch_event_on_bus, event_class_metadata, BusId, EventPriority};
use mmga_runtime::{ExecCtx, LinkedSpace, Rt, V};

pub(super) fn post_wrapped_in_mod_order(
    loader: &mut FmlLoader,
    rt: &mut Rt,
    linked: &LinkedSpace,
    event: V,
) -> V {
    if event.is_null() {
        return rt.throw_exception(
            "java/lang/NullPointerException",
            "ModLoader ordered post requires an event",
        );
    }
    let Some(class) = rt.class_of(event).map(str::to_owned) else {
        return rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "ModLoader ordered post requires an Event instance",
        );
    };
    let metadata = match event_class_metadata(linked, loader, &class) {
        Ok(metadata) => metadata,
        Err(message) => return rt.throw_exception("java/lang/IllegalArgumentException", &message),
    };
    if !metadata.is_mod_bus {
        return rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "ModLoader ordered post requires IModBusEvent",
        );
    }
    if loader
        .loading_issues
        .iter()
        .any(|issue| issue.severity == "ERROR")
    {
        // FML explicitly refuses to notify listeners after a recorded loading error.
        return V::Void;
    }
    let mod_ids: Vec<_> = loader
        .mods
        .iter()
        .map(|loaded| loaded.mod_id.clone())
        .collect();
    for priority in [
        EventPriority::Highest,
        EventPriority::High,
        EventPriority::Normal,
        EventPriority::Low,
        EventPriority::Lowest,
    ] {
        for mod_id in &mod_ids {
            let bus = BusId::Mod(mod_id.clone());
            let listeners: Vec<_> = loader
                .state
                .bus
                .matching_listeners_with_linked(&bus, &class, linked, loader)
                .into_iter()
                .filter(|listener| listener.priority == priority)
                .collect();
            loader.constructing = Some(mod_id.clone());
            let (ran, _) = {
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host: loader,
                    depth: 0,
                };
                dispatch_event_on_bus(&mut ctx, &listeners, event)
            };
            // Always perform the post wrapper, including when the original Java
            // callback failed. No second VM call can replace that pending exception.
            loader.constructing = None;
            loader.state.bus.record_dispatch(ran);
            if rt.pending_exception.is_some() {
                return V::NULL;
            }
        }
    }
    V::Void
}

#[cfg(test)]
mod tests {
    use super::super::{LoadedMod, PreparedLoad, RecordedLoadingIssue};
    use crate::bus::{BusId, EventCallback, EventClassMetadata, EventPriority};
    use mmga_classfile::{access, op, ClassFile, Const, MemberRef};
    use mmga_runtime::{
        invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, NativeModuleRegistry, Rt, V,
    };
    use std::collections::BTreeMap;

    const EVENT: &str = "test/OrderedModEvent";
    const POST: &str = "net/neoforged/fml/ModLoader";
    const CONTEXT: &str = "net/neoforged/fml/ModLoadingContext";
    fn fixture() -> PreparedLoad {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        let class = ClassFile {
            major_version: 65,
            minor_version: 0,
            pool: vec![Const::Unused],
            access: access::PUBLIC,
            name: EVENT.into(),
            superclass: Some("net/neoforged/bus/api/Event".into()),
            interfaces: vec!["net/neoforged/fml/event/IModBusEvent".into()],
            fields: vec![],
            methods: vec![],
            annotations: vec![],
            bootstrap_methods: vec![],
            signature: None,
            record_components: vec![],
        };
        prepared
            .linked
            .add_bytecode_classes(BTreeMap::from([(class.name.clone(), class)]));
        prepared
            .linked
            .register_native_module(NativeModuleRegistry {
                classes: &[],
                lambdas: &[(201, record), (202, fail)],
            });
        prepared.loader.mods = ["first", "second"]
            .into_iter()
            .map(|id| LoadedMod {
                mod_id: id.into(),
                version: "1".into(),
                display_name: id.into(),
                source_name: "fixture".into(),
                entry_class: None,
                instance: None,
                entrypoints: vec![],
                subscribers: vec![],
            })
            .collect();
        prepared
    }
    fn context_id(ctx: &mut ExecCtx<'_>) -> V {
        let context = invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(CONTEXT, "get", "()Lnet/neoforged/fml/ModLoadingContext;"),
            &[],
        );
        let container = invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                CONTEXT,
                "getActiveContainer",
                "()Lnet/neoforged/fml/ModContainer;",
            ),
            &[context],
        );
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "net/neoforged/fml/ModContainer",
                "getModId",
                "()Ljava/lang/String;",
            ),
            &[container],
        )
    }
    fn record(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let active = context_id(ctx);
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        let id = ctx.rt.str_of(active).unwrap().to_owned();
        let label = ctx.rt.str_of(args[1]).unwrap().to_owned();
        let text = ctx.rt.new_str(format!("{id}:{label}"));
        let records = ctx.rt.get_field(args[0], "records");
        if let Some(Data::List(records)) = ctx.rt.get_obj_mut(records).map(|o| &mut o.data) {
            records.push(text);
        }
        ctx.rt.put_field(args[0], "event", args[2]);
        V::Void
    }
    fn fail(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        record(ctx, args);
        let exception = ctx
            .rt
            .throw_exception("test/OrderedListenerFailure", "original listener failure");
        let original = ctx.rt.pending_exception.map(V::R).unwrap();
        ctx.rt.put_field(args[0], "failure", original);
        exception
    }
    fn observer(rt: &mut Rt) -> V {
        let observer = rt.new_instance("test/Observer", 0);
        let records = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
        rt.put_field(observer, "records", records);
        observer
    }
    fn listen(
        prepared: &mut PreparedLoad,
        rt: &mut Rt,
        observer: V,
        mod_id: &str,
        priority: EventPriority,
        label: &str,
        fails: bool,
    ) {
        let label = rt.intern_str(label);
        let callback = rt.alloc(
            "java/util/function/Consumer",
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: "java/util/function/Consumer".into(),
                sam_name: "accept".into(),
                sam_desc: "(Ljava/lang/Object;)V".into(),
                instantiated_desc: format!("(L{EVENT};)V"),
                captures: vec![observer, label],
                target: ClosureTarget::NativeLambda(if fails { 202 } else { 201 }),
            })),
        );
        prepared
            .loader
            .state
            .bus
            .register_listener_with_metadata(
                BusId::Mod(mod_id.into()),
                EVENT.into(),
                priority,
                false,
                callback,
                EventCallback::Consumer(callback),
                EventClassMetadata {
                    is_mod_bus: true,
                    is_abstract: false,
                },
            )
            .unwrap();
    }
    fn records(rt: &Rt, observer: V) -> Vec<String> {
        let list = rt.get_field(observer, "records");
        let Some(Data::List(values)) = rt.get_obj(list).map(|o| &o.data) else {
            panic!("records are not a list")
        };
        values
            .iter()
            .map(|v| rt.str_of(*v).unwrap().to_owned())
            .collect()
    }
    fn post(ctx: &mut ExecCtx<'_>, event: V) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                POST,
                "postEventWrapContainerInModOrder",
                "(Lnet/neoforged/bus/api/Event;)V",
            ),
            &[event],
        )
    }

    #[test]
    fn ordered_post_runs_priority_before_mod_order_keeps_event_and_clears_context() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let observer = observer(&mut rt);
        // Reverse installation order to prove that mod load order, not listener insertion,
        // controls order across containers. Each priority pass traverses that mod order.
        for id in ["second", "first"] {
            listen(
                &mut prepared,
                &mut rt,
                observer,
                id,
                EventPriority::Low,
                "low",
                false,
            );
            listen(
                &mut prepared,
                &mut rt,
                observer,
                id,
                EventPriority::High,
                "high",
                false,
            );
        }
        prepared.loader.constructing = Some("outer".into());
        let event = rt.new_instance(EVENT, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert_eq!(post(&mut ctx, event), V::Void);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "ordered post must execute all registered callbacks"
        );
        assert_eq!(
            records(ctx.rt, observer),
            vec!["first:high", "second:high", "first:low", "second:low"]
        );
        assert_eq!(ctx.rt.get_field(observer, "event"), event);
        let cleared = context_id(&mut ctx);
        assert_eq!(ctx.rt.str_of(cleared), Some("minecraft"));
        assert!(prepared.loader.constructing.is_none());
    }
    #[test]
    fn ordered_post_preserves_callback_exception_and_cleans_active_container_before_return() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let observer = observer(&mut rt);
        listen(
            &mut prepared,
            &mut rt,
            observer,
            "first",
            EventPriority::Highest,
            "failure",
            true,
        );
        listen(
            &mut prepared,
            &mut rt,
            observer,
            "second",
            EventPriority::Highest,
            "unreached",
            false,
        );
        let event = rt.new_instance(EVENT, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert_eq!(post(&mut ctx, event), V::NULL);
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(exception),
            Some("test/OrderedListenerFailure")
        );
        assert_eq!(ctx.rt.get_field(observer, "failure"), exception);
        assert_eq!(records(ctx.rt, observer), vec!["first:failure"]);
        let cleared = context_id(&mut ctx);
        assert_eq!(ctx.rt.str_of(cleared), Some("minecraft"));
        assert!(prepared.loader.constructing.is_none());
    }
    #[test]
    fn ordered_post_rejects_bad_events_and_broken_loading_state_does_not_dispatch() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let observer = observer(&mut rt);
        listen(
            &mut prepared,
            &mut rt,
            observer,
            "first",
            EventPriority::Normal,
            "event",
            false,
        );
        let game = rt.new_instance("net/neoforged/neoforge/event/server/ServerStartedEvent", 0);
        let event = rt.new_instance(EVENT, 0);
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            for (event, class) in [
                (V::NULL, "java/lang/NullPointerException"),
                (game, "java/lang/IllegalArgumentException"),
            ] {
                post(&mut ctx, event);
                let caught = ctx.rt.take_exception().unwrap();
                assert_eq!(ctx.rt.class_of(caught), Some(class));
            }
            assert!(records(ctx.rt, observer).is_empty());
        }
        prepared.loader.loading_issues.push(RecordedLoadingIssue {
            severity: "ERROR".into(),
            mod_id: None,
            translation_key: "test:error".into(),
            message: "failed load".into(),
        });
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert_eq!(post(&mut ctx, event), V::Void);
        assert!(ctx.rt.pending_exception.is_none());
        assert!(records(ctx.rt, observer).is_empty());
    }
}

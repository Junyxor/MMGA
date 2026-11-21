//! NeoForge 21.1 TicketController construction and callback retention.
//!
//! Source/API target: NeoForge 21.1.266, `common.world.chunk.TicketController`.
//! Official source commit 89e887b367ab0c11cb58ce8500920efadc17b803, blob
//! 9569ac289875c5e60ad9b4c0e0d0693ebbed3e6b: a record with a non-null ID
//! and nullable callback, plus a one-argument convenience constructor.
//! A controller is identified by a ResourceLocation and delegates persisted-ticket
//! validation to the supplied LoadingValidationCallback. Construction only retains
//! those objects; registration and world ticket operations happen later.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, ExecCtx, V};

const CONTROLLER: &str = "net/neoforged/neoforge/common/world/chunk/TicketController";
#[cfg(test)]
const RESOURCE_LOCATION: &str = "net/minecraft/resources/ResourceLocation";
#[cfg(test)]
const CALLBACK: &str = "net/neoforged/neoforge/common/world/chunk/LoadingValidationCallback";
const CONSTRUCTOR: &str = "(Lnet/minecraft/resources/ResourceLocation;Lnet/neoforged/neoforge/common/world/chunk/LoadingValidationCallback;)V";

fn component_call(ctx: &mut ExecCtx<'_>, value: V, name: &str, desc: &str, rest: &[V]) -> V {
    let mut args = Vec::with_capacity(rest.len() + 1);
    args.push(value);
    args.extend_from_slice(rest);
    invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", name, desc),
        &args,
    )
}

fn record_equals(ctx: &mut ExecCtx<'_>, receiver: V, other: V) -> V {
    if receiver == other {
        return V::TRUE;
    }
    if ctx.rt.class_of(other) != Some(CONTROLLER) {
        return V::FALSE;
    }
    // JDK 21 ObjectMethods.makeEquals wraps each successive field test around
    // the previous test, so the last record component is tested first.
    for field in ["callback", "id"] {
        let left = ctx.rt.get_field(receiver, field);
        let right = ctx.rt.get_field(other, field);
        if left == right {
            continue;
        }
        if left.is_null() {
            return V::FALSE;
        }
        let equal = component_call(ctx, left, "equals", "(Ljava/lang/Object;)Z", &[right]);
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        if equal.i() == 0 {
            return V::FALSE;
        }
    }
    V::TRUE
}

fn record_hash(ctx: &mut ExecCtx<'_>, receiver: V) -> V {
    let mut hash = 0i32;
    for field in ["id", "callback"] {
        let value = ctx.rt.get_field(receiver, field);
        let component_hash = if value.is_null() {
            0
        } else {
            let value = component_call(ctx, value, "hashCode", "()I", &[]);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            value.i()
        };
        hash = hash.wrapping_mul(31).wrapping_add(component_hash);
    }
    V::I(hash)
}

fn record_string(ctx: &mut ExecCtx<'_>, receiver: V) -> V {
    let mut components = Vec::with_capacity(2);
    for field in ["id", "callback"] {
        let value = ctx.rt.get_field(receiver, field);
        let text = if value.is_null() {
            "null".to_owned()
        } else {
            let text = component_call(ctx, value, "toString", "()Ljava/lang/String;", &[]);
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            ctx.rt.value_to_string(text)
        };
        components.push(format!("{field}={text}"));
    }
    ctx.rt
        .new_str(format!("TicketController[{}]", components.join(", ")))
}

/// Handles construction, accessors, and Java record methods without creating tickets.
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    if matches!(
        member.owner.as_str(),
        CONTROLLER | "java/lang/Object" | "java/lang/Record"
    ) && ctx.rt.class_of(receiver) == Some(CONTROLLER)
    {
        match (member.name.as_str(), member.desc.as_str()) {
            ("equals", "(Ljava/lang/Object;)Z") => {
                return Some(record_equals(ctx, receiver, *args.get(1)?));
            }
            ("hashCode", "()I") => return Some(record_hash(ctx, receiver)),
            ("toString", "()Ljava/lang/String;") => return Some(record_string(ctx, receiver)),
            _ => {}
        }
    }
    if member.owner != CONTROLLER {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str()) {
        ("id", "()Lnet/minecraft/resources/ResourceLocation;") => {
            return Some(ctx.rt.get_field(receiver, "id"))
        }
        ("callback", "()Lnet/neoforged/neoforge/common/world/chunk/LoadingValidationCallback;") => {
            return Some(ctx.rt.get_field(receiver, "callback"))
        }
        _ => {}
    }
    if member.name != "<init>"
        || !matches!(
            member.desc.as_str(),
            CONSTRUCTOR | "(Lnet/minecraft/resources/ResourceLocation;)V"
        )
    {
        return None;
    }
    let id = *args.get(1)?;
    let callback = if member.desc == CONSTRUCTOR {
        *args.get(2)?
    } else {
        V::NULL
    };
    if id.is_null() {
        return Some(
            ctx.rt
                .throw_exception("java/lang/NullPointerException", "id must not be null"),
        );
    }
    // Keep the actual values: the callback is invoked by later ticket validation.
    ctx.rt.put_field(receiver, "id", id);
    ctx.rt.put_field(receiver, "callback", callback);
    Some(V::Void)
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == CONTROLLER && sup == "java/lang/Record"
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{HostApi, LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost, Rt};

    struct RecordHost;
    impl HostApi for RecordHost {
        fn get_static_field(&mut self, _: &mut Rt, _: &str, _: &str, _: &str) -> Option<V> {
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
            super::call(
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
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
        }
    }
    fn component_equals(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "equalsCalls").i() + 1;
        ctx.rt.put_field(args[0], "equalsCalls", V::I(count));
        if ctx.rt.get_field(args[0], "failEquals") == V::TRUE {
            let error = ctx.rt.get_field(args[0], "error");
            ctx.rt.pending_exception = Some(error.r());
            return V::NULL;
        }
        V::bool(
            ctx.rt.class_of(args[0]) == ctx.rt.class_of(args[1])
                && ctx.rt.val_eq(
                    ctx.rt.get_field(args[0], "value"),
                    ctx.rt.get_field(args[1], "value"),
                ),
        )
    }
    fn component_hash(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "hashCalls").i() + 1;
        ctx.rt.put_field(args[0], "hashCalls", V::I(count));
        if ctx.rt.get_field(args[0], "failHash") == V::TRUE {
            let error = ctx.rt.get_field(args[0], "error");
            ctx.rt.pending_exception = Some(error.r());
            return V::NULL;
        }
        ctx.rt.get_field(args[0], "hash")
    }
    fn component_string(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "stringCalls").i() + 1;
        ctx.rt.put_field(args[0], "stringCalls", V::I(count));
        if ctx.rt.get_field(args[0], "failString") == V::TRUE {
            let error = ctx.rt.get_field(args[0], "error");
            ctx.rt.pending_exception = Some(error.r());
            return V::NULL;
        }
        ctx.rt.get_field(args[0], "text")
    }
    fn record_linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: RESOURCE_LOCATION,
                    superclass: Some("java/lang/Object"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[
                        (
                            "equals.(Ljava/lang/Object;)Z",
                            component_equals,
                            false,
                            false,
                        ),
                        ("hashCode.()I", component_hash, false, false),
                        (
                            "toString.()Ljava/lang/String;",
                            component_string,
                            false,
                            false,
                        ),
                    ],
                },
                NativeClassDef {
                    name: "test/RecordCallback",
                    superclass: Some("java/lang/Object"),
                    interfaces: &[CALLBACK],
                    is_interface: false,
                    methods: &[
                        (
                            "equals.(Ljava/lang/Object;)Z",
                            component_equals,
                            false,
                            false,
                        ),
                        ("hashCode.()I", component_hash, false, false),
                        (
                            "toString.()Ljava/lang/String;",
                            component_string,
                            false,
                            false,
                        ),
                    ],
                },
                NativeClassDef {
                    name: "test/OtherRecord",
                    superclass: Some("java/lang/Record"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
            ],
            lambdas: &[],
        });
        linked
    }
    fn component(rt: &mut Rt, class: &str, text: &str, hash: i32) -> V {
        let object = rt.new_instance(class, 0);
        let text = rt.new_str(text);
        let error = rt.new_instance("test/OriginalComponentFailure", 0);
        rt.put_field(object, "value", text);
        rt.put_field(object, "text", text);
        rt.put_field(object, "hash", V::I(hash));
        rt.put_field(object, "error", error);
        object
    }
    fn record(ctx: &mut ExecCtx<'_>, id: V, callback: V) -> V {
        let value = ctx.rt.new_instance(CONTROLLER, 0);
        assert_eq!(construct(ctx, value, id, callback), V::Void);
        value
    }
    fn object_method(ctx: &mut ExecCtx<'_>, receiver: V, name: &str, desc: &str, rest: &[V]) -> V {
        let mut args = vec![receiver];
        args.extend_from_slice(rest);
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Object", name, desc),
            &args,
        )
    }

    #[test]
    fn record_methods_use_equal_components_null_callbacks_and_declaration_hash_order() {
        let mut rt = Rt::new();
        let linked = record_linked();
        let mut host = RecordHost;
        let id = component(&mut rt, RESOURCE_LOCATION, "ae2:default", i32::MAX);
        let equal_id = component(&mut rt, RESOURCE_LOCATION, "ae2:default", i32::MAX);
        let unequal_id = component(&mut rt, RESOURCE_LOCATION, "ae2:other", 3);
        let callback = component(&mut rt, "test/RecordCallback", "actual callback", 17);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let left = record(&mut ctx, id, callback);
        let right = record(&mut ctx, equal_id, callback);
        let unequal = record(&mut ctx, unequal_id, callback);
        let no_callback = record(&mut ctx, id, V::NULL);
        let same_no_callback = record(&mut ctx, equal_id, V::NULL);
        for other in [left, right] {
            assert_eq!(
                object_method(&mut ctx, left, "equals", "(Ljava/lang/Object;)Z", &[other]),
                V::TRUE
            );
        }
        assert_eq!(
            object_method(
                &mut ctx,
                left,
                "equals",
                "(Ljava/lang/Object;)Z",
                &[unequal]
            ),
            V::FALSE
        );
        assert_eq!(
            object_method(
                &mut ctx,
                left,
                "equals",
                "(Ljava/lang/Object;)Z",
                &[no_callback]
            ),
            V::FALSE
        );
        assert_eq!(
            object_method(
                &mut ctx,
                no_callback,
                "equals",
                "(Ljava/lang/Object;)Z",
                &[same_no_callback]
            ),
            V::TRUE
        );
        assert_eq!(
            object_method(&mut ctx, left, "hashCode", "()I", &[]),
            V::I(i32::MAX.wrapping_mul(31).wrapping_add(17))
        );
        assert_eq!(
            object_method(&mut ctx, right, "hashCode", "()I", &[]),
            V::I(i32::MAX.wrapping_mul(31).wrapping_add(17))
        );
        assert_eq!(
            object_method(&mut ctx, no_callback, "hashCode", "()I", &[]),
            V::I(i32::MAX.wrapping_mul(31))
        );
        let text = object_method(&mut ctx, left, "toString", "()Ljava/lang/String;", &[]);
        assert_eq!(
            ctx.rt.str_of(text),
            Some("TicketController[id=ae2:default, callback=actual callback]")
        );
        let text = object_method(
            &mut ctx,
            no_callback,
            "toString",
            "()Ljava/lang/String;",
            &[],
        );
        assert_eq!(
            ctx.rt.str_of(text),
            Some("TicketController[id=ae2:default, callback=null]")
        );
        assert!(ctx.rt.get_field(id, "equalsCalls").i() > 0);
        assert!(ctx.rt.get_field(id, "hashCalls").i() > 0);
        assert_eq!(ctx.rt.get_field(callback, "stringCalls"), V::I(1));
        let other = ctx.rt.new_instance("test/OtherRecord", 0);
        ctx.rt.put_field(other, "id", id);
        ctx.rt.put_field(other, "callback", callback);
        for value in [V::NULL, other, callback] {
            assert_eq!(
                object_method(&mut ctx, left, "equals", "(Ljava/lang/Object;)Z", &[value]),
                V::FALSE
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn record_methods_propagate_original_component_exceptions_and_stop_in_jdk_order() {
        let mut rt = Rt::new();
        let linked = record_linked();
        let mut host = RecordHost;
        let id = component(&mut rt, RESOURCE_LOCATION, "ae2:default", 11);
        let equal_id = component(&mut rt, RESOURCE_LOCATION, "ae2:default", 11);
        let callback = component(&mut rt, "test/RecordCallback", "callback", 17);
        let equal_callback = component(&mut rt, "test/RecordCallback", "callback", 17);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let left = record(&mut ctx, id, callback);
        let right = record(&mut ctx, equal_id, equal_callback);
        // JDK 21 checks callback before ID for equals, but hashes ID before callback.
        for (component, flag, method, desc, rest) in [
            (
                callback,
                "failEquals",
                "equals",
                "(Ljava/lang/Object;)Z",
                vec![right],
            ),
            (
                id,
                "failEquals",
                "equals",
                "(Ljava/lang/Object;)Z",
                vec![right],
            ),
            (id, "failHash", "hashCode", "()I", vec![]),
            (callback, "failHash", "hashCode", "()I", vec![]),
            (id, "failString", "toString", "()Ljava/lang/String;", vec![]),
            (
                callback,
                "failString",
                "toString",
                "()Ljava/lang/String;",
                vec![],
            ),
        ] {
            ctx.rt.put_field(component, flag, V::TRUE);
            object_method(&mut ctx, left, method, desc, &rest);
            let original = ctx.rt.get_field(component, "error");
            assert_eq!(
                ctx.rt.take_exception(),
                Some(original),
                "{flag} must preserve exact exception reference"
            );
            ctx.rt.put_field(component, flag, V::FALSE);
        }
        ctx.rt.put_field(callback, "failEquals", V::TRUE);
        ctx.rt.put_field(id, "equalsCalls", V::I(0));
        object_method(&mut ctx, left, "equals", "(Ljava/lang/Object;)Z", &[right]);
        assert_eq!(
            ctx.rt.take_exception(),
            Some(ctx.rt.get_field(callback, "error"))
        );
        assert_eq!(ctx.rt.get_field(id, "equalsCalls"), V::I(0));
        ctx.rt.put_field(id, "failHash", V::TRUE);
        ctx.rt.put_field(callback, "hashCalls", V::I(0));
        object_method(&mut ctx, left, "hashCode", "()I", &[]);
        assert_eq!(ctx.rt.take_exception(), Some(ctx.rt.get_field(id, "error")));
        assert_eq!(ctx.rt.get_field(callback, "hashCalls"), V::I(0));
    }

    fn construct(ctx: &mut ExecCtx<'_>, receiver: V, id: V, callback: V) -> V {
        call(
            ctx,
            &MemberRef::new(CONTROLLER, "<init>", CONSTRUCTOR),
            false,
            &[receiver, id, callback],
        )
        .expect("recognized TicketController constructor")
    }

    #[test]
    fn constructor_retains_the_exact_id_and_callback_without_running_validation() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host: Box<dyn HostApi> = Box::new(NullHost);
        let id = rt.new_pinned_handle(RESOURCE_LOCATION, "resource_location", 0, "ae2:default");
        let callback = rt.new_instance("test/Callback", 0);
        rt.put_field(callback, "calls", V::I(0));
        let controller = rt.new_instance(CONTROLLER, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: host.as_mut(),
            depth: 0,
        };

        assert_eq!(construct(&mut ctx, controller, id, callback), V::Void);
        assert_eq!(ctx.rt.get_field(controller, "id"), id);
        assert_eq!(ctx.rt.get_field(controller, "callback"), callback);
        assert_eq!(ctx.rt.get_field(callback, "calls"), V::I(0));
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn constructor_rejects_null_id_and_accepts_nullable_callback() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let id = rt.new_pinned_handle(RESOURCE_LOCATION, "resource_location", 0, "ae2:default");
        let callback = rt.new_instance("test/Callback", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };

        let receiver = ctx.rt.new_instance(CONTROLLER, 0);
        construct(&mut ctx, receiver, V::NULL, callback);
        let error = ctx.rt.take_exception().expect("null ID should throw");
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        assert_eq!(ctx.rt.get_field(receiver, "id"), V::NULL);
        assert_eq!(ctx.rt.get_field(receiver, "callback"), V::NULL);

        let receiver = ctx.rt.new_instance(CONTROLLER, 0);
        construct(&mut ctx, receiver, id, V::NULL);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(receiver, "id"), id);
        assert_eq!(ctx.rt.get_field(receiver, "callback"), V::NULL);
        let one_arg = ctx.rt.new_instance(CONTROLLER, 0);
        assert_eq!(
            call(
                &mut ctx,
                &MemberRef::new(
                    CONTROLLER,
                    "<init>",
                    "(Lnet/minecraft/resources/ResourceLocation;)V"
                ),
                false,
                &[one_arg, id]
            ),
            Some(V::Void)
        );
        assert_eq!(ctx.rt.get_field(one_arg, "id"), id);
        assert_eq!(ctx.rt.get_field(one_arg, "callback"), V::NULL);
    }

    #[test]
    fn only_the_exact_constructor_signature_is_handled() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert!(call(
            &mut ctx,
            &MemberRef::new(CONTROLLER, "<init>", "()V"),
            false,
            &[V::NULL]
        )
        .is_none());
        assert!(call(
            &mut ctx,
            &MemberRef::new(CONTROLLER, "forceChunk", "()Z"),
            false,
            &[V::NULL]
        )
        .is_none());
        assert!(call(
            &mut ctx,
            &MemberRef::new(CONTROLLER, "<init>", CONSTRUCTOR),
            true,
            &[]
        )
        .is_none());
        assert!(!is_subtype(CONTROLLER, RESOURCE_LOCATION));
        assert!(is_subtype(CONTROLLER, "java/lang/Record"));
        assert!(!is_subtype(CALLBACK, "java/lang/Record"));
    }
}

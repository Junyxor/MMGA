use crate::{invoke_member, Data, ExecCtx, LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost, Rt, V};
use mmga_classfile::{op, MemberRef};

fn endpoint(ctx: &mut ExecCtx<'_>, receiver: V, method: &str, value: V) -> V {
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/List", method, "(Ljava/lang/Object;)V"),
        &[receiver, value],
    )
}

#[test]
fn ae2_list_endpoints_preserve_order_nulls_and_void_results() {
    let mut rt = Rt::new();
    let linked = LinkedSpace::new();
    let mut host = NullHost;
    let first = rt.intern_str("first");
    let last = rt.intern_str("last");
    let list = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
    let mut ctx = ExecCtx { rt: &mut rt, linked: &linked, host: &mut host, depth: 0 };

    assert_eq!(endpoint(&mut ctx, list, "addFirst", first), V::Void);
    assert!(ctx.rt.pending_exception.is_none());
    assert_eq!(endpoint(&mut ctx, list, "addFirst", V::NULL), V::Void);
    assert_eq!(endpoint(&mut ctx, list, "addLast", last), V::Void);
    let Data::List(values) = &ctx.rt.get_obj(list).unwrap().data else { panic!() };
    assert_eq!(values, &[V::NULL, first, last]);

    for (method, expected) in [("getFirst", V::NULL), ("getLast", last)] {
        let value = invoke_member(
            &mut ctx, op::INVOKEINTERFACE,
            &MemberRef::new("java/util/List", method, "()Ljava/lang/Object;"), &[list],
        );
        assert_eq!(value, expected);
    }
    assert!(ctx.rt.pending_exception.is_none());
}

#[test]
fn list_endpoint_mutation_rejects_immutable_lists_null_receivers_and_wrong_signatures() {
    let mut rt = Rt::new();
    let linked = LinkedSpace::new();
    let mut host = NullHost;
    let immutable_lists = [
        "java/util/ImmutableCollections$ListN",
        "com/google/common/collect/ImmutableList",
        "com/google/common/collect/RegularImmutableList",
        "com/google/common/collect/SingletonImmutableList",
    ].map(|class| rt.alloc(class, 0, Data::List(Vec::new())));
    let mut ctx = ExecCtx { rt: &mut rt, linked: &linked, host: &mut host, depth: 0 };
    for name in ["addFirst", "addLast"] {
        for immutable in immutable_lists {
            endpoint(&mut ctx, immutable, name, V::NULL);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(ctx.rt.class_of(error), Some("java/lang/UnsupportedOperationException"));
            assert!(matches!(&ctx.rt.get_obj(immutable).unwrap().data, Data::List(v) if v.is_empty()));
        }
        endpoint(&mut ctx, V::NULL, name, V::NULL);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/NullPointerException"));
    }
    let mutable = ctx.rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
    for name in ["addFirst", "addLast"] {
        assert!(crate::stdlib_call(
            &mut ctx, &MemberRef::new("java/util/List", name, "(Ljava/lang/Object;)Z"),
            false, &[mutable, V::NULL],
        ).is_none());
    }
    assert!(ctx.rt.pending_exception.is_none());
    assert!(matches!(&ctx.rt.get_obj(mutable).unwrap().data, Data::List(v) if v.is_empty()));
}

#[test]
fn list_endpoints_invoke_overridden_add_and_preserve_original_exception() {
    fn add(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_static("test/RejectingList", "argumentCount", V::I(args.len() as i32));
        ctx.rt.put_static("test/RejectingList", "element", *args.last().unwrap());
        if args.len() == 3 {
            ctx.rt.put_static("test/RejectingList", "index", args[1]);
        }
        let exception = ctx.rt.get_static("test/RejectingList", "exception").unwrap();
        ctx.rt.pending_exception = Some(exception.r());
        V::NULL
    }
    let mut linked = LinkedSpace::new();
    linked.register_native_module(NativeModuleRegistry {
        classes: &[NativeClassDef {
            name: "test/RejectingList",
            superclass: Some("java/util/ArrayList"),
            interfaces: &[],
            is_interface: false,
            methods: &[
                ("add.(ILjava/lang/Object;)V", add, false, false),
                ("add.(Ljava/lang/Object;)Z", add, false, false),
            ],
        }],
        lambdas: &[],
    });
    let mut rt = Rt::new();
    let mut host = NullHost;
    let list = rt.alloc("test/RejectingList", 0, Data::List(Vec::new()));
    let exception = rt.new_instance("test/AddFailure", 0);
    let element = rt.new_str("original element");
    rt.put_static("test/RejectingList", "exception", exception);
    let mut ctx = ExecCtx { rt: &mut rt, linked: &linked, host: &mut host, depth: 0 };
    for (method, count) in [("addFirst", 3), ("addLast", 2)] {
        assert_eq!(endpoint(&mut ctx, list, method, element), V::NULL);
        assert_eq!(ctx.rt.take_exception(), Some(exception));
        assert_eq!(ctx.rt.get_static("test/RejectingList", "argumentCount"), Some(V::I(count)));
        assert_eq!(ctx.rt.get_static("test/RejectingList", "element"), Some(element));
        assert_eq!(ctx.rt.get_static("test/RejectingList", "index"), Some(V::I(0)));
        assert!(matches!(&ctx.rt.get_obj(list).unwrap().data, Data::List(v) if v.is_empty()));
    }
}

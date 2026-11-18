//! JDK Collections.addAll: invoke Collection.add for every vararg in encounter order.
//! OpenJDK bytecode uses `modified |= c.add(element)`, never short-circuit OR.

use crate::{invoke_member, Data, ExecCtx, V};
use mmga_classfile::{op, parse_field_descriptor, DescriptorType, MemberRef};

const COLLECTIONS: &str = "java/util/Collections";
const DESC: &str = "(Ljava/util/Collection;[Ljava/lang/Object;)Z";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static || member.owner != COLLECTIONS || member.name != "addAll" || member.desc != DESC {
        return None;
    }
    if ctx.rt.pending_exception.is_some() {
        return Some(V::NULL);
    }
    let collection = *args.first()?;
    let array = *args.get(1)?;
    if array.is_null() {
        return Some(ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "Collections.addAll varargs array is null",
        ));
    }
    let object = ctx.rt.get_obj(array)?;
    let reference_array = matches!(
        parse_field_descriptor(&object.class),
        Ok(DescriptorType::Array(component))
            if matches!(*component, DescriptorType::Object(_) | DescriptorType::Array(_))
    );
    let length = match &object.data {
        Data::Array(values) if reference_array => values.len(),
        _ => {
            return Some(ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "Collections.addAll requires a reference array",
            ));
        }
    };
    let mut modified = false;
    for index in 0..length {
        // Collection.add may mutate a later slot of the original array.
        let element = match ctx.rt.get_obj(array).map(|obj| &obj.data) {
            Some(Data::Array(values)) => match values.get(index) {
                Some(element) => *element,
                None => {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/UnsupportedOperationException",
                        "Collections.addAll array changed length during callback",
                    ));
                }
            },
            _ => {
                return Some(ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Collections.addAll array representation changed during callback",
                ));
            }
        };
        let added = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/Collection", "add", "(Ljava/lang/Object;)Z"),
            &[collection, element],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let V::I(added) = added else {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Collection.add did not return a boolean runtime value",
            ));
        };
        modified |= added != 0;
    }
    Some(V::bool(modified))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost, Rt};

    fn add_all(ctx: &mut ExecCtx<'_>, collection: V, array: V) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(COLLECTIONS, "addAll", DESC),
            &[collection, array],
        )
    }

    fn list(rt: &Rt, value: V) -> Vec<V> {
        let Some(Data::List(values)) = rt.get_obj(value).map(|obj| &obj.data) else {
            panic!("expected list storage");
        };
        values.clone()
    }

    fn observed_add(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let receiver = args[0];
        let calls = ctx.rt.get_field(receiver, "calls").i() + 1;
        ctx.rt.put_field(receiver, "calls", V::I(calls));
        let seen = ctx.rt.get_field(receiver, "seen");
        let Some(Data::List(seen)) = ctx.rt.get_obj_mut(seen).map(|obj| &mut obj.data) else {
            panic!("expected callback log");
        };
        seen.push(args[1]);
        let exception = ctx.rt.get_field(receiver, "exception");
        if calls == ctx.rt.get_field(receiver, "throwAt").i() {
            ctx.rt.pending_exception = Some(exception.r());
            return V::NULL;
        }
        let input = ctx.rt.get_field(receiver, "input");
        let replacement = ctx.rt.get_field(receiver, "replacement");
        if calls == 1 && !input.is_null() {
            let Some(Data::Array(values)) = ctx.rt.get_obj_mut(input).map(|obj| &mut obj.data)
            else {
                panic!("expected source array");
            };
            values[1] = replacement;
        }
        let added = ctx.rt.get_field(receiver, "added");
        let Some(Data::List(added)) = ctx.rt.get_obj_mut(added).map(|obj| &mut obj.data) else {
            panic!("expected callback storage");
        };
        added.push(args[1]);
        V::bool(calls == 1 && ctx.rt.get_field(receiver, "allFalse") != V::TRUE)
    }

    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/ObservedCollection",
                superclass: Some("java/lang/Object"),
                interfaces: &["java/util/Collection"],
                is_interface: false,
                methods: &[("add.(Ljava/lang/Object;)Z", observed_add, false, false)],
            }],
            lambdas: &[],
        });
        linked
    }

    fn observed(rt: &mut Rt) -> V {
        let collection = rt.new_instance("test/ObservedCollection", 0);
        let seen = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
        let added = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
        rt.put_field(collection, "seen", seen);
        rt.put_field(collection, "added", added);
        collection
    }

    #[test]
    fn calls_every_add_after_true_and_observes_array_mutation_from_callbacks() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let collection = observed(&mut rt);
        let first = rt.new_str("first");
        let second = rt.new_str("second");
        let replacement = rt.new_str("replacement");
        let array = rt.alloc(
            "[Ljava/lang/String;",
            0,
            Data::Array(vec![first, second, V::NULL]),
        );
        rt.put_field(collection, "input", array);
        rt.put_field(collection, "replacement", replacement);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(add_all(&mut ctx, collection, array), V::TRUE);
        assert_eq!(ctx.rt.get_field(collection, "calls"), V::I(3));
        assert_eq!(
            list(ctx.rt, ctx.rt.get_field(collection, "seen")),
            vec![first, replacement, V::NULL]
        );
        assert_eq!(
            list(ctx.rt, ctx.rt.get_field(collection, "added")),
            vec![first, replacement, V::NULL]
        );
        assert!(ctx.rt.pending_exception.is_none());
        let unmodified = observed(ctx.rt);
        ctx.rt.put_field(unmodified, "allFalse", V::TRUE);
        assert_eq!(add_all(&mut ctx, unmodified, array), V::FALSE);
        assert_eq!(ctx.rt.get_field(unmodified, "calls"), V::I(3));
    }

    #[test]
    fn failure_keeps_prior_additions_preserves_original_exception_and_stops_visiting() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let collection = observed(&mut rt);
        let first = rt.new_str("first");
        let second = rt.new_str("throws");
        let last = rt.new_str("unvisited");
        let array = rt.alloc(
            "[Ljava/lang/Object;",
            0,
            Data::Array(vec![first, second, last]),
        );
        let exception = rt.new_instance("test/AddFailure", 0);
        rt.put_field(collection, "throwAt", V::I(2));
        rt.put_field(collection, "exception", exception);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(add_all(&mut ctx, collection, array), V::NULL);
        assert_eq!(ctx.rt.take_exception(), Some(exception));
        assert_eq!(ctx.rt.get_field(collection, "calls"), V::I(2));
        assert_eq!(
            list(ctx.rt, ctx.rt.get_field(collection, "seen")),
            vec![first, second]
        );
        assert_eq!(
            list(ctx.rt, ctx.rt.get_field(collection, "added")),
            vec![first]
        );
    }

    #[test]
    fn list_set_and_empty_immutable_target_keep_actual_add_semantics() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let first = rt.new_str("same");
        let array = rt.alloc(
            "[Ljava/lang/Object;",
            0,
            Data::Array(vec![first, first, V::NULL]),
        );
        let collection = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
        let set = rt.alloc("java/util/HashSet", 0, Data::Set(vec![]));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(add_all(&mut ctx, collection, array), V::TRUE);
        assert_eq!(list(ctx.rt, collection), vec![first, first, V::NULL]);
        assert_eq!(add_all(&mut ctx, set, array), V::TRUE);
        assert_eq!(add_all(&mut ctx, set, array), V::FALSE);
        assert!(
            matches!(ctx.rt.get_obj(set).map(|obj| &obj.data), Some(Data::Set(values)) if values == &vec![first, V::NULL])
        );
        let empty = ctx.rt.alloc("[Ljava/lang/Object;", 0, Data::Array(vec![]));
        let immutable = ctx.rt.alloc(
            "java/util/ImmutableCollections$ListN",
            0,
            Data::List(vec![]),
        );
        assert_eq!(add_all(&mut ctx, immutable, empty), V::FALSE);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(add_all(&mut ctx, immutable, array), V::NULL);
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(exception),
            Some("java/lang/UnsupportedOperationException")
        );
        assert!(list(ctx.rt, immutable).is_empty());
    }

    #[test]
    fn nulls_covariant_arrays_and_exact_signatures_follow_the_jdk_loop() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let empty = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(vec![]));
        let array = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(vec![V::NULL]));
        let collection = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(add_all(&mut ctx, V::NULL, empty), V::FALSE);
        assert!(ctx.rt.pending_exception.is_none());
        for (target, source) in [(V::NULL, V::NULL), (collection, V::NULL), (V::NULL, array)] {
            assert_eq!(add_all(&mut ctx, target, source), V::NULL);
            let exception = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(exception),
                Some("java/lang/NullPointerException")
            );
        }
        let primitive_array = ctx.rt.alloc("[I", 0, Data::Array(vec![V::I(7)]));
        assert_eq!(add_all(&mut ctx, collection, primitive_array), V::NULL);
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(exception),
            Some("java/lang/ClassCastException")
        );
        let nested = ctx.rt.alloc("[[I", 0, Data::Array(vec![primitive_array]));
        assert_eq!(add_all(&mut ctx, collection, nested), V::TRUE);
        assert_eq!(list(ctx.rt, collection), vec![primitive_array]);
        assert!(call(
            &mut ctx,
            &MemberRef::new(
                COLLECTIONS,
                "addAll",
                "(Ljava/util/Collection;Ljava/lang/Object;)Z"
            ),
            true,
            &[collection, array]
        )
        .is_none());
        assert!(call(
            &mut ctx,
            &MemberRef::new(COLLECTIONS, "addAll", DESC),
            false,
            &[collection, array]
        )
        .is_none());
    }
}

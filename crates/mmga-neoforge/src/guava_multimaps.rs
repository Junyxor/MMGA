//! Guava 32.1.2 AbstractMultimap.putAll executes real iterable traversal.
//! HashMultimap uniqueness and ArrayListMultimap duplicates are delegated to
//! their existing put implementation. Empty input does not create a key bucket.
use mmga_classfile::MemberRef;
use mmga_runtime::{invoke_member, ExecCtx, V};
const MULTIMAP: &str = "com/google/common/collect/Multimap";
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static
        || member.name != "putAll"
        || !matches!(
            member.owner.as_str(),
            MULTIMAP
                | "com/google/common/collect/SetMultimap"
                | "com/google/common/collect/ListMultimap"
                | "com/google/common/collect/HashMultimap"
                | "com/google/common/collect/LinkedHashMultimap"
                | "com/google/common/collect/ArrayListMultimap"
        )
    {
        return None;
    }
    let receiver = *args.first()?;
    match member.desc.as_str() {
        "(Ljava/lang/Object;Ljava/lang/Iterable;)Z" => {
            let values = *args.get(2)?;
            if values.is_null() {
                return Some(
                    ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "Multimap.putAll values",
                    ),
                );
            }
            let iterator = invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new("java/lang/Iterable", "iterator", "()Ljava/util/Iterator;"),
                &[values],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let mut changed = false;
            loop {
                let more = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Iterator", "hasNext", "()Z"),
                    &[iterator],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if more.i() == 0 {
                    break;
                }
                let value = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Iterator", "next", "()Ljava/lang/Object;"),
                    &[iterator],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let added = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new(MULTIMAP, "put", "(Ljava/lang/Object;Ljava/lang/Object;)Z"),
                    &[receiver, *args.get(1)?, value],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                changed |= added.i() != 0;
            }
            Some(V::bool(changed))
        }
        "(Lcom/google/common/collect/Multimap;)Z" => {
            let other = *args.get(1)?;
            let entries = invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new(MULTIMAP, "entries", "()Ljava/util/Collection;"),
                &[other],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let iterator = invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new("java/lang/Iterable", "iterator", "()Ljava/util/Iterator;"),
                &[entries],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let mut changed = false;
            loop {
                let more = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Iterator", "hasNext", "()Z"),
                    &[iterator],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if more.i() == 0 {
                    break;
                }
                let entry = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Iterator", "next", "()Ljava/lang/Object;"),
                    &[iterator],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let key = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Map$Entry", "getKey", "()Ljava/lang/Object;"),
                    &[entry],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let value = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Map$Entry", "getValue", "()Ljava/lang/Object;"),
                    &[entry],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let added = invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new(MULTIMAP, "put", "(Ljava/lang/Object;Ljava/lang/Object;)Z"),
                    &[receiver, key, value],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                changed |= added.i() != 0;
            }
            Some(V::bool(changed))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryStore;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, Data, HostApi, LinkedSpace, Rt};
    #[derive(Default)]
    struct TestHost {
        registries: RegistryStore,
        iterations: usize,
        steps: usize,
    }
    impl HostApi for TestHost {
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
            if member.owner == "java/lang/Iterable"
                && rt.class_of(args[0]) == Some("test/FailingIterable")
                && member.name == "iterator"
            {
                self.iterations += 1;
                return Some(rt.new_instance("test/FailingIterator", 0));
            }
            if member.owner == "java/util/Iterator"
                && rt.class_of(args[0]) == Some("test/FailingIterator")
            {
                if member.name == "hasNext" {
                    return Some(V::TRUE);
                }
                if member.name == "next" {
                    self.steps += 1;
                    return Some(if self.steps == 1 {
                        rt.intern_str("first")
                    } else {
                        rt.throw_exception("test/IterationFailure", "second element")
                    });
                }
            }
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
            .or_else(|| {
                crate::minecraft::call_keys_authlib_and_libs(
                    rt,
                    &mut self.registries,
                    member,
                    is_static,
                    args,
                )
            })
        }
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        stat: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if stat {
                op::INVOKESTATIC
            } else {
                op::INVOKEINTERFACE
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn create(ctx: &mut ExecCtx<'_>, kind: &str) -> V {
        invoke(ctx, kind, "create", &format!("()L{kind};"), true, &[])
    }
    #[test]
    fn iterable_put_all_preserves_set_uniqueness_and_list_duplicates() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (kind, count) in [
            ("com/google/common/collect/HashMultimap", 2),
            ("com/google/common/collect/ArrayListMultimap", 3),
        ] {
            let map = create(&mut ctx, kind);
            let key = ctx.rt.intern_str("tags");
            let value = ctx.rt.intern_str("minecraft:mineable/pickaxe");
            let values = ctx.rt.alloc(
                "java/util/ArrayList",
                0,
                Data::List(vec![value, value, V::NULL]),
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    MULTIMAP,
                    "putAll",
                    "(Ljava/lang/Object;Ljava/lang/Iterable;)Z",
                    false,
                    &[map, key, values]
                ),
                V::TRUE
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(
                invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[map]),
                V::I(count)
            );
            let empty = ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new()));
            let absent = ctx.rt.intern_str("empty");
            assert_eq!(
                invoke(
                    &mut ctx,
                    MULTIMAP,
                    "putAll",
                    "(Ljava/lang/Object;Ljava/lang/Iterable;)Z",
                    false,
                    &[map, absent, empty]
                ),
                V::FALSE
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    MULTIMAP,
                    "containsKey",
                    "(Ljava/lang/Object;)Z",
                    false,
                    &[map, absent]
                ),
                V::FALSE
            );
            invoke(
                &mut ctx,
                MULTIMAP,
                "putAll",
                "(Ljava/lang/Object;Ljava/lang/Iterable;)Z",
                false,
                &[map, key, V::NULL],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("NullPointerException"));
        }
    }
    #[test]
    fn iteration_failure_keeps_prior_entries_and_never_restarts_iterator() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost::default();
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            let map = create(&mut ctx, "com/google/common/collect/HashMultimap");
            let key = ctx.rt.intern_str("key");
            let iterable = ctx.rt.new_instance("test/FailingIterable", 0);
            invoke(
                &mut ctx,
                MULTIMAP,
                "putAll",
                "(Ljava/lang/Object;Ljava/lang/Iterable;)Z",
                false,
                &[map, key, iterable],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx.rt.exception_summary(error).contains("IterationFailure"));
            assert_eq!(
                invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[map]),
                V::I(1)
            );
        }
        assert_eq!(host.iterations, 1);
        assert_eq!(host.steps, 2);
    }

    #[test]
    fn existing_hash_multimap_put_size_and_entries_see_set_buckets() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let source = create(&mut ctx, "com/google/common/collect/HashMultimap");
        let key = ctx.rt.intern_str("key");
        let value = ctx.rt.intern_str("value");
        assert_eq!(
            invoke(
                &mut ctx,
                MULTIMAP,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                false,
                &[source, key, value]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                MULTIMAP,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                false,
                &[source, key, value]
            ),
            V::FALSE
        );
        assert_eq!(
            invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[source]),
            V::I(1)
        );
        assert_eq!(
            invoke(&mut ctx, MULTIMAP, "isEmpty", "()Z", false, &[source]),
            V::FALSE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                MULTIMAP,
                "containsKey",
                "(Ljava/lang/Object;)Z",
                false,
                &[source, key]
            ),
            V::TRUE
        );
        let dest = create(&mut ctx, "com/google/common/collect/ArrayListMultimap");
        assert_eq!(
            invoke(
                &mut ctx,
                MULTIMAP,
                "putAll",
                "(Lcom/google/common/collect/Multimap;)Z",
                false,
                &[dest, source]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[dest]),
            V::I(1)
        );
        let entries = invoke(
            &mut ctx,
            MULTIMAP,
            "entries",
            "()Ljava/util/Collection;",
            false,
            &[source],
        );
        assert!(
            matches!(ctx.rt.get_obj(entries).map(|o|&o.data),Some(Data::List(items))if items.len()==1)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
}

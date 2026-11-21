//! Guava 32.1.2 AbstractMultimap.putAll executes real iterable traversal.
//! HashMultimap uniqueness and ArrayListMultimap duplicates are delegated to
//! their existing put implementation. Empty input does not create a key bucket.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, Rt, V};
const MULTIMAP: &str = "com/google/common/collect/Multimap";
const LINKED_LIST: &str = "com/google/common/collect/LinkedListMultimap";
const LINKED_VIEW: &str = "mmga/internal/LinkedMultimapView";
const LINKED_ITERATOR: &str = "mmga/internal/LinkedMultimapIterator";
const LINKED_ENTRY: &str = "mmga/internal/LinkedMultimapEntry";

/// Guava v32.1.2 LinkedListMultimap.java, source blob
/// d88564ca549e528a94e56eb363a46474ae724ab4. A single ordered node sequence
/// is sufficient for the ListMultimap operations used by AE2 and keeps duplicate
/// key/value pairs and keyed iteration in their true encounter order.
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == LINKED_LIST
        && matches!(
            sup,
            MULTIMAP
                | "com/google/common/collect/ListMultimap"
                | "com/google/common/collect/AbstractMultimap"
                | "java/io/Serializable"
        ))
        || (sub == LINKED_ITERATOR && sup == "java/util/Iterator")
        || (sub == LINKED_ENTRY && sup == "java/util/Map$Entry")
}

fn linked_nodes(rt: &Rt, map: V) -> Vec<V> {
    match rt.get_obj(rt.get_field(map, "$entries")).map(|o| &o.data) {
        Some(Data::List(nodes)) => nodes.clone(),
        _ => Vec::new(),
    }
}

fn linked_equal(ctx: &mut ExecCtx<'_>, left: V, right: V) -> bool {
    if left == right {
        return true;
    }
    if left.is_null() {
        return false;
    }
    invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
        &[left, right],
    )
    .i() != 0
}

fn linked_put(rt: &mut Rt, map: V, key: V, value: V) -> V {
    let node = rt.new_instance(LINKED_ENTRY, 0);
    rt.put_field(node, "key", key);
    rt.put_field(node, "value", value);
    rt.put_field(node, "$removed", V::FALSE);
    rt.put_field(node, "$map", map);
    let entries = rt.get_field(map, "$entries");
    if let Some(Data::List(nodes)) = rt.get_obj_mut(entries).map(|o| &mut o.data) {
        nodes.push(node);
    }
    let size = rt.get_field(map, "$size").i() + 1;
    rt.put_field(map, "$size", V::I(size));
    V::TRUE
}

fn linked_remove_node(rt: &mut Rt, node: V) {
    if rt.get_field(node, "$removed") == V::TRUE {
        return;
    }
    let map = rt.get_field(node, "$map");
    let entries = rt.get_field(map, "$entries");
    let removed = if let Some(Data::List(nodes)) = rt.get_obj_mut(entries).map(|o| &mut o.data) {
        if let Some(index) = nodes.iter().position(|candidate| *candidate == node) {
            nodes.remove(index);
            true
        } else {
            false
        }
    } else {
        false
    };
    if removed {
        let size = rt.get_field(map, "$size").i() - 1;
        rt.put_field(map, "$size", V::I(size));
        rt.put_field(node, "$removed", V::TRUE);
    }
}

fn linked_view(rt: &mut Rt, map: V, mode: i32, key: V) -> V {
    let view = rt.new_instance(LINKED_VIEW, 0);
    rt.put_field(view, "$map", map);
    rt.put_field(view, "$mode", V::I(mode));
    rt.put_field(view, "$key", key);
    view
}

fn linked_view_nodes(ctx: &mut ExecCtx<'_>, view: V) -> Vec<V> {
    let map = ctx.rt.get_field(view, "$map");
    let mode = ctx.rt.get_field(view, "$mode").i();
    let key = ctx.rt.get_field(view, "$key");
    let mut found = Vec::new();
    for node in linked_nodes(ctx.rt, map) {
        if mode != 0 || linked_equal(ctx, key, ctx.rt.get_field(node, "key")) {
            found.push(node);
        }
        if ctx.rt.pending_exception.is_some() {
            break;
        }
    }
    found
}

fn linked_call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    if is_static {
        if member.owner != LINKED_LIST
            || name != "create"
            || desc != "()Lcom/google/common/collect/LinkedListMultimap;"
        {
            return None;
        }
        let map = ctx.rt.new_instance(LINKED_LIST, 0);
        let entries = ctx
            .rt
            .alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
        ctx.rt.put_field(map, "$entries", entries);
        ctx.rt.put_field(map, "$size", V::I(0));
        return Some(map);
    }

    let receiver = *args.first()?;
    match ctx.rt.class_of(receiver)? {
        LINKED_LIST => match (name, desc) {
            ("put", "(Ljava/lang/Object;Ljava/lang/Object;)Z") => {
                Some(linked_put(ctx.rt, receiver, args[1], args[2]))
            }
            ("get", "(Ljava/lang/Object;)Ljava/util/List;")
            | ("get", "(Ljava/lang/Object;)Ljava/util/Collection;") => {
                Some(linked_view(ctx.rt, receiver, 0, args[1]))
            }
            ("entries", "()Ljava/util/Collection;") | ("entries", "()Ljava/util/List;") => {
                Some(linked_view(ctx.rt, receiver, 2, V::NULL))
            }
            ("values", "()Ljava/util/Collection;") | ("values", "()Ljava/util/List;") => {
                Some(linked_view(ctx.rt, receiver, 1, V::NULL))
            }
            ("containsKey", "(Ljava/lang/Object;)Z") => {
                let view = linked_view(ctx.rt, receiver, 0, args[1]);
                let found = linked_view_nodes(ctx, view);
                Some(if ctx.rt.pending_exception.is_some() {
                    V::NULL
                } else {
                    V::bool(!found.is_empty())
                })
            }
            ("remove", "(Ljava/lang/Object;Ljava/lang/Object;)Z") => {
                for node in linked_nodes(ctx.rt, receiver) {
                    if linked_equal(ctx, args[1], ctx.rt.get_field(node, "key"))
                        && linked_equal(ctx, args[2], ctx.rt.get_field(node, "value"))
                    {
                        if ctx.rt.pending_exception.is_some() {
                            return Some(V::NULL);
                        }
                        linked_remove_node(ctx.rt, node);
                        return Some(V::TRUE);
                    }
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                Some(V::FALSE)
            }
            ("size", "()I") => Some(ctx.rt.get_field(receiver, "$size")),
            ("isEmpty", "()Z") => Some(V::bool(ctx.rt.get_field(receiver, "$size").i() == 0)),
            ("clear", "()V") => {
                for node in linked_nodes(ctx.rt, receiver) {
                    linked_remove_node(ctx.rt, node);
                }
                Some(V::Void)
            }
            _ => None,
        },
        LINKED_VIEW => match (name, desc) {
            ("get", "(I)Ljava/lang/Object;") => {
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let Some(node) = nodes.get(args[1].i() as usize).copied() else {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/IndexOutOfBoundsException",
                        "LinkedListMultimap list index",
                    ));
                };
                Some(if ctx.rt.get_field(receiver, "$mode").i() == 2 {
                    node
                } else {
                    ctx.rt.get_field(node, "value")
                })
            }
            ("add", "(Ljava/lang/Object;)Z") => {
                if ctx.rt.get_field(receiver, "$mode").i() != 0 {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/UnsupportedOperationException",
                        "Cannot add through multimap entries or values view",
                    ));
                }
                let map = ctx.rt.get_field(receiver, "$map");
                let key = ctx.rt.get_field(receiver, "$key");
                Some(linked_put(ctx.rt, map, key, args[1]))
            }
            ("contains", "(Ljava/lang/Object;)Z") | ("remove", "(Ljava/lang/Object;)Z") => {
                if ctx.rt.get_field(receiver, "$mode").i() == 2 {
                    return None;
                }
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                for node in nodes {
                    let equal = linked_equal(ctx, args[1], ctx.rt.get_field(node, "value"));
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    if equal {
                        if name == "remove" {
                            linked_remove_node(ctx.rt, node);
                        }
                        return Some(V::TRUE);
                    }
                }
                Some(V::FALSE)
            }
            ("forEach", "(Ljava/util/function/Consumer;)V") => {
                let callback = args[1];
                if callback.is_null() {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/NullPointerException", "action"),
                    );
                }
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let entries = ctx.rt.get_field(receiver, "$mode").i() == 2;
                for node in nodes {
                    let value = if entries {
                        node
                    } else {
                        ctx.rt.get_field(node, "value")
                    };
                    invoke_member(
                        ctx,
                        op::INVOKEINTERFACE,
                        &MemberRef::new(
                            "java/util/function/Consumer",
                            "accept",
                            "(Ljava/lang/Object;)V",
                        ),
                        &[callback, value],
                    );
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                Some(V::Void)
            }
            ("isEmpty", "()Z") => {
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    Some(V::NULL)
                } else {
                    Some(V::bool(nodes.is_empty()))
                }
            }
            ("size", "()I") => {
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    Some(V::NULL)
                } else {
                    Some(V::I(nodes.len() as i32))
                }
            }
            ("iterator", "()Ljava/util/Iterator;") => {
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let iter = ctx.rt.new_instance(LINKED_ITERATOR, 0);
                let nodes = ctx.rt.alloc("[Ljava/lang/Object;", 0, Data::Array(nodes));
                ctx.rt.put_field(iter, "$nodes", nodes);
                ctx.rt.put_field(iter, "$view", receiver);
                ctx.rt.put_field(iter, "$position", V::I(0));
                ctx.rt.put_field(iter, "$current", V::NULL);
                Some(iter)
            }
            ("clear", "()V") => {
                let nodes = linked_view_nodes(ctx, receiver);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                for node in nodes {
                    linked_remove_node(ctx.rt, node);
                }
                Some(V::Void)
            }
            _ => None,
        },
        LINKED_ITERATOR => match (name, desc) {
            ("hasNext", "()Z") => {
                let position = ctx.rt.get_field(receiver, "$position").i().max(0) as usize;
                let nodes = ctx.rt.get_field(receiver, "$nodes");
                Some(V::bool(matches!(
                    ctx.rt.get_obj(nodes).map(|o| &o.data),
                    Some(Data::Array(values)) if position < values.len()
                )))
            }
            ("next", "()Ljava/lang/Object;") => {
                let position = ctx.rt.get_field(receiver, "$position").i().max(0) as usize;
                let nodes = ctx.rt.get_field(receiver, "$nodes");
                let Some(Data::Array(values)) = ctx.rt.get_obj(nodes).map(|o| &o.data) else {
                    return Some(ctx.rt.throw_exception(
                        "java/util/NoSuchElementException",
                        "LinkedListMultimap iterator",
                    ));
                };
                let Some(&node) = values.get(position) else {
                    return Some(ctx.rt.throw_exception(
                        "java/util/NoSuchElementException",
                        "LinkedListMultimap iterator",
                    ));
                };
                let source = ctx.rt.get_field(receiver, "$view");
                let value = if ctx.rt.get_field(source, "$mode").i() == 2 {
                    node
                } else {
                    ctx.rt.get_field(node, "value")
                };
                ctx.rt
                    .put_field(receiver, "$position", V::I(position as i32 + 1));
                ctx.rt.put_field(receiver, "$current", node);
                Some(value)
            }
            ("remove", "()V") => {
                let current = ctx.rt.get_field(receiver, "$current");
                if current.is_null() {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/IllegalStateException",
                        "next() must be called before remove()",
                    ));
                }
                linked_remove_node(ctx.rt, current);
                ctx.rt.put_field(receiver, "$current", V::NULL);
                Some(V::Void)
            }
            _ => None,
        },
        LINKED_ENTRY => match (name, desc) {
            ("getKey", "()Ljava/lang/Object;") => Some(ctx.rt.get_field(receiver, "key")),
            ("getValue", "()Ljava/lang/Object;") => Some(ctx.rt.get_field(receiver, "value")),
            ("setValue", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
                let previous = ctx.rt.get_field(receiver, "value");
                ctx.rt.put_field(receiver, "value", args[1]);
                Some(previous)
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if let Some(value) = linked_call(ctx, member, is_static, args) {
        return Some(value);
    }
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
                | "com/google/common/collect/LinkedListMultimap"
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

    #[test]
    fn linked_list_multimap_preserves_global_order_duplicate_pairs_and_per_key_order() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = create(&mut ctx, "com/google/common/collect/LinkedListMultimap");
        let a = ctx.rt.intern_str("a");
        let b = ctx.rt.intern_str("b");
        let one = ctx.rt.intern_str("one");
        let two = ctx.rt.intern_str("two");
        for (key, value) in [(a, one), (b, two), (a, one), (a, two)] {
            assert_eq!(
                invoke(
                    &mut ctx,
                    MULTIMAP,
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                    false,
                    &[map, key, value]
                ),
                V::TRUE
            );
        }
        assert_eq!(
            invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[map]),
            V::I(4)
        );
        let entries = invoke(
            &mut ctx,
            MULTIMAP,
            "entries",
            "()Ljava/util/Collection;",
            false,
            &[map],
        );
        let iter = invoke(
            &mut ctx,
            "java/lang/Iterable",
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[entries],
        );
        let mut actual = Vec::new();
        while invoke(
            &mut ctx,
            "java/util/Iterator",
            "hasNext",
            "()Z",
            false,
            &[iter],
        )
        .i() != 0
        {
            let entry = invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[iter],
            );
            let key = invoke(
                &mut ctx,
                "java/util/Map$Entry",
                "getKey",
                "()Ljava/lang/Object;",
                false,
                &[entry],
            );
            let value = invoke(
                &mut ctx,
                "java/util/Map$Entry",
                "getValue",
                "()Ljava/lang/Object;",
                false,
                &[entry],
            );
            actual.push((key, value));
        }
        assert_eq!(actual, vec![(a, one), (b, two), (a, one), (a, two)]);
        let values = invoke(
            &mut ctx,
            MULTIMAP,
            "get",
            "(Ljava/lang/Object;)Ljava/util/List;",
            false,
            &[map, a],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Collection",
                "size",
                "()I",
                false,
                &[values]
            ),
            V::I(3)
        );
        let values_iter = invoke(
            &mut ctx,
            "java/util/Collection",
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[values],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[values_iter]
            ),
            one
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[values_iter]
            ),
            one
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[values_iter]
            ),
            two
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn linked_list_keyed_iterator_removes_only_its_current_duplicate_entry() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TestHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = create(&mut ctx, "com/google/common/collect/LinkedListMultimap");
        let key = ctx.rt.intern_str("key");
        let other = ctx.rt.intern_str("other");
        let value = ctx.rt.intern_str("same");
        let later = ctx.rt.intern_str("later");
        for (k, v) in [(key, value), (other, value), (key, value), (key, later)] {
            invoke(
                &mut ctx,
                MULTIMAP,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                false,
                &[map, k, v],
            );
        }
        let list = invoke(
            &mut ctx,
            MULTIMAP,
            "get",
            "(Ljava/lang/Object;)Ljava/util/List;",
            false,
            &[map, key],
        );
        let iter = invoke(
            &mut ctx,
            "java/util/Collection",
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[list],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[iter]
            ),
            value
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "remove",
            "()V",
            false,
            &[iter],
        );
        assert_eq!(
            invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[map]),
            V::I(3)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[iter]
            ),
            value
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[iter]
            ),
            later
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn linked_multimap_live_views_use_actual_equality_and_callback_failures() {
        fn equal(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            let calls = ctx.rt.get_field(args[0], "calls").i() + 1;
            ctx.rt.put_field(args[0], "calls", V::I(calls));
            if ctx.rt.get_field(args[0], "fail") == V::TRUE {
                return ctx
                    .rt
                    .throw_exception("test/EqualsFailure", "actual equals failure");
            }
            V::bool(ctx.rt.get_field(args[0], "id") == ctx.rt.get_field(args[1], "id"))
        }
        fn accept(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            ctx.rt.put_field(args[0], "seen", args[1]);
            ctx.rt
                .throw_exception("test/CallbackFailure", "actual callback failure")
        }
        use mmga_runtime::{NativeClassDef, NativeModuleRegistry};
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: "test/EqualKey",
                    superclass: Some("java/lang/Object"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[("equals.(Ljava/lang/Object;)Z", equal, false, false)],
                },
                NativeClassDef {
                    name: "test/ValueConsumer",
                    superclass: Some("java/lang/Object"),
                    interfaces: &["java/util/function/Consumer"],
                    is_interface: false,
                    methods: &[("accept.(Ljava/lang/Object;)V", accept, false, false)],
                },
            ],
            lambdas: &[],
        });
        let mut host = TestHost::default();
        let key = rt.new_instance("test/EqualKey", 0);
        let equal_key = rt.new_instance("test/EqualKey", 0);
        rt.put_field(key, "id", V::I(10));
        rt.put_field(equal_key, "id", V::I(10));
        let consumer = rt.new_instance("test/ValueConsumer", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map = create(&mut ctx, "com/google/common/collect/LinkedListMultimap");
        let list = invoke(
            &mut ctx,
            MULTIMAP,
            "get",
            "(Ljava/lang/Object;)Ljava/util/List;",
            false,
            &[map, equal_key],
        );
        assert_eq!(
            invoke(&mut ctx, "java/util/List", "size", "()I", false, &[list]),
            V::I(0)
        );
        let value = ctx.rt.intern_str("value");
        invoke(
            &mut ctx,
            MULTIMAP,
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Z",
            false,
            &[map, key, value],
        );
        assert_eq!(
            invoke(&mut ctx, "java/util/List", "size", "()I", false, &[list]),
            V::I(1)
        );
        assert!(ctx.rt.get_field(equal_key, "calls").i() > 0);
        invoke(
            &mut ctx,
            "java/util/Collection",
            "forEach",
            "(Ljava/util/function/Consumer;)V",
            false,
            &[list, consumer],
        );
        let error = ctx.rt.take_exception().expect("original callback throws");
        assert_eq!(ctx.rt.class_of(error), Some("test/CallbackFailure"));
        assert_eq!(ctx.rt.get_field(consumer, "seen"), value);
        ctx.rt.put_field(equal_key, "fail", V::TRUE);
        invoke(&mut ctx, "java/util/List", "clear", "()V", false, &[list]);
        let error = ctx.rt.take_exception().expect("original equality throws");
        assert_eq!(ctx.rt.class_of(error), Some("test/EqualsFailure"));
        assert_eq!(
            invoke(&mut ctx, MULTIMAP, "size", "()I", false, &[map]),
            V::I(1)
        );
    }
}

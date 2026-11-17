//! ImmutableMap collection views retain their source and reject all mutation.
//! These are the public Map/Collection operations used by AE2's colored items.
use crate::{invoke_member, Data, ExecCtx, V};
use mmga_classfile::{op, MemberRef};

const MAP: &str = "com/google/common/collect/ImmutableMap";
const KEYS: &str = "com/google/common/collect/ImmutableMapKeySet";
const ENTRIES: &str = "com/google/common/collect/ImmutableMapEntrySet";
const VALUES: &str = "com/google/common/collect/ImmutableMapValues";
const ITERATOR: &str = "com/google/common/collect/UnmodifiableIterator";
const ENTRY: &str = "java/util/AbstractMap$SimpleImmutableEntry";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    let class = ctx.rt.class_of(receiver)?.to_owned();
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    if class == ENTRY && name == "setValue" && desc == "(Ljava/lang/Object;)Ljava/lang/Object;" {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "immutable Map.Entry",
        ));
    }
    if class == ITERATOR && name == "remove" && desc == "()V" {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "immutable iterator",
        ));
    }
    if class == MAP {
        let view_class = match (name, desc) {
            ("keySet", "()Ljava/util/Set;" | "()Lcom/google/common/collect/ImmutableSet;") => KEYS,
            ("entrySet", "()Ljava/util/Set;" | "()Lcom/google/common/collect/ImmutableSet;") => {
                ENTRIES
            }
            (
                "values",
                "()Ljava/util/Collection;" | "()Lcom/google/common/collect/ImmutableCollection;",
            ) => VALUES,
            _ => return None,
        };
        let view = ctx.rt.new_instance(view_class, 0);
        ctx.rt.put_field(view, "$map", receiver);
        return Some(view);
    }
    if !matches!(class.as_str(), KEYS | ENTRIES | VALUES) {
        return None;
    }
    if matches!(
        (name, desc),
        ("add" | "remove", "(Ljava/lang/Object;)Z")
            | (
                "addAll" | "removeAll" | "retainAll",
                "(Ljava/util/Collection;)Z"
            )
            | ("removeIf", "(Ljava/util/function/Predicate;)Z")
            | ("clear", "()V")
    ) {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "immutable Map collection view",
        ));
    }
    let map = ctx.rt.get_field(receiver, "$map");
    let Some(Data::Map(entries)) = ctx.rt.get_obj(map).map(|o| &o.data) else {
        return None;
    };
    let entries = entries.clone();
    match (name, desc) {
        ("size", "()I") => return Some(V::I(entries.len() as i32)),
        ("isEmpty", "()Z") => return Some(V::bool(entries.is_empty())),
        ("contains", "(Ljava/lang/Object;)Z") => {
            if args[1].is_null() {
                return Some(V::FALSE);
            }
            let (wanted_key, wanted_value) = if class == ENTRIES {
                if !ctx.linked.is_subtype(
                    ctx.rt.class_of(args[1])?,
                    "java/util/Map$Entry",
                    ctx.host,
                ) {
                    return Some(V::FALSE);
                }
                let key = invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Map$Entry", "getKey", "()Ljava/lang/Object;"),
                    &[args[1]],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let value = invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Map$Entry", "getValue", "()Ljava/lang/Object;"),
                    &[args[1]],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                (key, value)
            } else {
                (args[1], args[1])
            };
            for (key, value) in entries {
                let equal_key = class == VALUES || equals(ctx, wanted_key, key);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let equal_value = class == KEYS || (equal_key && equals(ctx, wanted_value, value));
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if equal_key && equal_value {
                    return Some(V::TRUE);
                }
            }
            return Some(V::FALSE);
        }
        ("iterator", "()Ljava/util/Iterator;")
        | ("toArray", "()[Ljava/lang/Object;")
        | ("forEach", "(Ljava/util/function/Consumer;)V") => {}
        _ => return None,
    }
    if name == "forEach" && args[1].is_null() {
        return Some(
            ctx.rt
                .throw_exception("java/lang/NullPointerException", "consumer"),
        );
    }
    let items: Vec<V> = entries
        .into_iter()
        .map(|(key, value)| match class.as_str() {
            KEYS => key,
            VALUES => value,
            _ => {
                let entry = ctx.rt.new_instance(ENTRY, 0);
                ctx.rt.put_field(entry, "key", key);
                ctx.rt.put_field(entry, "value", value);
                entry
            }
        })
        .collect();
    Some(match name {
        "iterator" => ctx.rt.alloc(ITERATOR, 0, Data::Iter { items, pos: 0 }),
        "toArray" => ctx.rt.alloc("[Ljava/lang/Object;", 0, Data::Array(items)),
        "forEach" => {
            for value in items {
                invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Consumer",
                        "accept",
                        "(Ljava/lang/Object;)V",
                    ),
                    &[args[1], value],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
            }
            V::Void
        }
        _ => unreachable!(),
    })
}

fn equals(ctx: &mut ExecCtx<'_>, left: V, right: V) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NullHost, Rt};
    use mmga_classfile::op;

    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    #[test]
    fn ae2_entry_iteration_preserves_entries_and_blocks_interface_mutation() {
        let mut rt = Rt::new();
        let red = rt.intern_str("red");
        let blue = rt.intern_str("blue");
        let item = rt.intern_str("ae2:paint_ball");
        let map = rt.alloc(
            "com/google/common/collect/ImmutableMap",
            0,
            Data::Map(vec![(red, item), (blue, item)]),
        );
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let entries = invoke(
            &mut ctx,
            "java/util/Map",
            "entrySet",
            "()Ljava/util/Set;",
            &[map],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "entrySet must expose actual entries"
        );
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", &[entries]),
            V::I(2)
        );
        let iterator = invoke(
            &mut ctx,
            "java/util/Set",
            "iterator",
            "()Ljava/util/Iterator;",
            &[entries],
        );
        for key in [red, blue] {
            assert_eq!(
                invoke(
                    &mut ctx,
                    "java/util/Iterator",
                    "hasNext",
                    "()Z",
                    &[iterator]
                ),
                V::TRUE
            );
            let entry = invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                &[iterator],
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    "java/util/Map$Entry",
                    "getKey",
                    "()Ljava/lang/Object;",
                    &[entry]
                ),
                key
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    "java/util/Map$Entry",
                    "getValue",
                    "()Ljava/lang/Object;",
                    &[entry]
                ),
                item
            );
            invoke(
                &mut ctx,
                "java/util/Map$Entry",
                "setValue",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[entry, V::NULL],
            );
            let error = ctx
                .rt
                .take_exception()
                .expect("entry must remain immutable through the interface");
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("UnsupportedOperationException"));
        }
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "hasNext",
                "()Z",
                &[iterator]
            ),
            V::FALSE
        );
        invoke(&mut ctx, "java/util/Iterator", "remove", "()V", &[iterator]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Map",
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[map, red]
            ),
            item
        );
    }

    #[test]
    fn views_keep_duplicate_values_and_reject_even_empty_mutations() {
        let mut rt = Rt::new();
        let key = rt.intern_str("one");
        let other = rt.intern_str("two");
        let value = rt.intern_str("same");
        let map = rt.alloc(
            "com/google/common/collect/ImmutableMap",
            0,
            Data::Map(vec![(key, value), (other, value)]),
        );
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, desc) in [
            ("keySet", "()Ljava/util/Set;"),
            ("values", "()Ljava/util/Collection;"),
            ("entrySet", "()Ljava/util/Set;"),
        ] {
            let view = invoke(&mut ctx, "java/util/Map", name, desc, &[map]);
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(
                invoke(&mut ctx, "java/util/Collection", "size", "()I", &[view]),
                V::I(2)
            );
            let array = invoke(
                &mut ctx,
                "java/util/Collection",
                "toArray",
                "()[Ljava/lang/Object;",
                &[view],
            );
            let actual = match &ctx.rt.get_obj(array).unwrap().data {
                Data::Array(xs) => xs.clone(),
                _ => panic!(),
            };
            if name == "values" {
                assert_eq!(actual, vec![value, value]);
            }
            invoke(&mut ctx, "java/util/Collection", "clear", "()V", &[view]);
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("UnsupportedOperationException"));
            invoke(
                &mut ctx,
                "java/util/Collection",
                "remove",
                "(Ljava/lang/Object;)Z",
                &[view, V::NULL],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("UnsupportedOperationException"));
        }
    }
}

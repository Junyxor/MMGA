//! EnumSet uses the actual enum universe and keeps ordinal iteration order.
//! Java 21 EnumSet/RegularEnumSet behavior was checked with the installed javap.

use crate::{invoke_member, Data, ExecCtx, Rt, V};
use mmga_classfile::{op, MemberRef};

const ENUM_SET: &str = "java/util/EnumSet";
const REGULAR: &str = "java/util/RegularEnumSet";
const JUMBO: &str = "java/util/JumboEnumSet";
const VIEW: &str = "java/util/Collections$UnmodifiableSet";
const ITERATOR: &str = "java/util/RegularEnumSet$EnumSetIterator";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static {
        if owner == "java/util/Collections"
            && name == "unmodifiableSet"
            && desc == "(Ljava/util/Set;)Ljava/util/Set;"
            && args.first().is_some_and(|value| is_enumset(ctx.rt, *value))
        {
            let view = ctx.rt.new_instance(VIEW, 0);
            ctx.rt.put_field(view, "$enumSetDelegate", args[0]);
            return Some(view);
        }
        if owner != ENUM_SET {
            return None;
        }
        return match (name, desc) {
            ("noneOf" | "allOf", "(Ljava/lang/Class;)Ljava/util/EnumSet;") => {
                let class = *args.first()?;
                if class.is_null() {
                    return Some(
                        ctx.rt.throw_exception(
                            "java/lang/NullPointerException",
                            "EnumSet elementType",
                        ),
                    );
                }
                let Some(("class", _, class_name, _)) = ctx.rt.handle_of(class) else {
                    return Some(unsupported(ctx.rt, "EnumSet Class representation"));
                };
                let class_name = class_name.to_owned();
                Some(match new_set(ctx, &class_name) {
                    Ok(set) => {
                        if name == "allOf" {
                            let universe = universe_values(ctx.rt, set);
                            write_members(ctx.rt, set, universe);
                        }
                        set
                    }
                    Err(error) => error,
                })
            }
            (
                "copyOf",
                "(Ljava/util/Collection;)Ljava/util/EnumSet;"
                | "(Ljava/util/EnumSet;)Ljava/util/EnumSet;",
            ) => {
                let input = *args.first()?;
                if input.is_null() {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "EnumSet.copyOf(null)",
                    ));
                }
                if is_enumset(ctx.rt, input) {
                    return Some(clone_set(ctx.rt, input));
                }
                if desc == "(Ljava/util/EnumSet;)Ljava/util/EnumSet;" {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "EnumSet.copyOf requires EnumSet",
                    ));
                }
                let empty = invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Collection", "isEmpty", "()Z"),
                    &[input],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if empty.i() != 0 {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/IllegalArgumentException",
                        "Collection is empty",
                    ));
                }
                let iter = invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Collection", "iterator", "()Ljava/util/Iterator;"),
                    &[input],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let first = next(ctx, iter);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let class = match declaring_enum(ctx, first) {
                    Ok(class) => class,
                    Err(error) => return Some(error),
                };
                let set = match new_set(ctx, &class) {
                    Ok(set) => set,
                    Err(error) => return Some(error),
                };
                add(ctx, set, first);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                loop {
                    let more = has_next(ctx, iter);
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    if more.i() == 0 {
                        break;
                    }
                    let value = next(ctx, iter);
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    add(ctx, set, value);
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                Some(set)
            }
            ("of", _)
                if desc.ends_with(")Ljava/util/EnumSet;")
                    && desc.starts_with("(Ljava/lang/Enum;") =>
            {
                let first = *args.first()?;
                let class = match declaring_enum(ctx, first) {
                    Ok(class) => class,
                    Err(error) => return Some(error),
                };
                let set = match new_set(ctx, &class) {
                    Ok(set) => set,
                    Err(error) => return Some(error),
                };
                let values = if desc == "(Ljava/lang/Enum;[Ljava/lang/Enum;)Ljava/util/EnumSet;" {
                    let rest = *args.get(1)?;
                    if rest.is_null() {
                        return Some(
                            ctx.rt.throw_exception(
                                "java/lang/NullPointerException",
                                "EnumSet.of rest",
                            ),
                        );
                    }
                    let Some(Data::Array(rest)) = ctx.rt.get_obj(rest).map(|object| &object.data)
                    else {
                        return Some(unsupported(ctx.rt, "EnumSet.of rest array"));
                    };
                    let mut all = vec![first];
                    all.extend(rest);
                    all
                } else {
                    args.to_vec()
                };
                for value in values {
                    add(ctx, set, value);
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                Some(set)
            }
            ("complementOf", "(Ljava/util/EnumSet;)Ljava/util/EnumSet;") => {
                let input = *args.first()?;
                if input.is_null() {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "EnumSet.complementOf(null)",
                    ));
                }
                if !is_enumset(ctx.rt, input) {
                    return Some(unsupported(ctx.rt, "EnumSet complement input"));
                }
                let set = clone_set(ctx.rt, input);
                let values = universe_values(ctx.rt, set)
                    .into_iter()
                    .filter(|value| !members(ctx.rt, input).contains(value))
                    .collect();
                write_members(ctx.rt, set, values);
                Some(set)
            }
            _ => None,
        };
    }
    let receiver = *args.first()?;
    if ctx.rt.class_of(receiver) == Some(ITERATOR) {
        return Some(iterator_call(ctx, receiver, name, desc));
    }
    let delegate = ctx.rt.get_field(receiver, "$enumSetDelegate");
    let readonly = !delegate.is_null();
    let set = if readonly { delegate } else { receiver };
    if !is_enumset(ctx.rt, set) {
        return None;
    }
    if name == "getClass" {
        return None;
    }
    if readonly
        && matches!(
            name,
            "add" | "addAll" | "remove" | "removeAll" | "retainAll" | "clear" | "removeIf"
        )
    {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "unmodifiable EnumSet view",
        ));
    }
    Some(match (name, desc) {
        ("size", "()I") => V::I(members(ctx.rt, set).len() as i32),
        ("isEmpty", "()Z") => V::bool(members(ctx.rt, set).is_empty()),
        ("contains", "(Ljava/lang/Object;)Z") => {
            V::bool(members(ctx.rt, set).contains(args.get(1)?))
        }
        ("add", "(Ljava/lang/Object;)Z" | "(Ljava/lang/Enum;)Z") => add(ctx, set, *args.get(1)?),
        ("remove", "(Ljava/lang/Object;)Z") => remove(ctx.rt, set, *args.get(1)?),
        ("clear", "()V") => {
            write_members(ctx.rt, set, Vec::new());
            V::Void
        }
        ("clone", "()Ljava/util/EnumSet;" | "()Ljava/lang/Object;") if !readonly => {
            clone_set(ctx.rt, set)
        }
        ("iterator", "()Ljava/util/Iterator;") => {
            let iter = ctx.rt.new_instance(ITERATOR, 0);
            let values = ctx
                .rt
                .alloc("[Ljava/lang/Object;", 0, Data::Array(members(ctx.rt, set)));
            ctx.rt.put_field(iter, "set", set);
            ctx.rt.put_field(iter, "snapshot", values);
            ctx.rt.put_field(iter, "index", V::I(0));
            ctx.rt.put_field(iter, "last", V::NULL);
            ctx.rt.put_field(iter, "readonly", V::bool(readonly));
            iter
        }
        ("toArray", "()[Ljava/lang/Object;") => {
            ctx.rt
                .alloc("[Ljava/lang/Object;", 0, Data::Array(members(ctx.rt, set)))
        }
        ("stream", "()Ljava/util/stream/Stream;") => ctx.rt.alloc(
            "java/util/stream/Stream",
            0,
            Data::Stream(members(ctx.rt, set)),
        ),
        ("forEach", "(Ljava/util/function/Consumer;)V") => {
            let consumer = *args.get(1)?;
            if consumer.is_null() {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/NullPointerException", "EnumSet consumer"),
                );
            }
            for value in members(ctx.rt, set) {
                invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Consumer",
                        "accept",
                        "(Ljava/lang/Object;)V",
                    ),
                    &[consumer, value],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
            }
            V::Void
        }
        ("addAll" | "containsAll", "(Ljava/util/Collection;)Z") => {
            let input = *args.get(1)?;
            if input.is_null() {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/NullPointerException", "EnumSet collection"),
                );
            }
            let iterator = invoke_member(
                ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/util/Collection", "iterator", "()Ljava/util/Iterator;"),
                &[input],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let mut changed = false;
            loop {
                let more = has_next(ctx, iterator);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if more.i() == 0 {
                    break;
                }
                let value = next(ctx, iterator);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if name == "containsAll" {
                    if !members(ctx.rt, set).contains(&value) {
                        return Some(V::FALSE);
                    }
                } else {
                    changed |= add(ctx, set, value).i() != 0;
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
            }
            V::bool(name == "containsAll" || changed)
        }
        ("equals", "(Ljava/lang/Object;)Z") => {
            let other = *args.get(1)?;
            if other == receiver {
                return Some(V::TRUE);
            }
            if other.is_null()
                || !ctx.linked.is_subtype(
                    ctx.rt.class_of(other).unwrap_or(""),
                    "java/util/Set",
                    ctx.host,
                )
            {
                return Some(V::FALSE);
            }
            let values = match collection_values(ctx, other) {
                Ok(values) => values,
                Err(error) => return Some(error),
            };
            let own = members(ctx.rt, set);
            V::bool(own.len() == values.len() && values.iter().all(|value| own.contains(value)))
        }
        ("hashCode", "()I") => {
            let mut hash = 0i32;
            for value in members(ctx.rt, set) {
                let part = invoke_member(
                    ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Object", "hashCode", "()I"),
                    &[value],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                hash = hash.wrapping_add(part.i());
            }
            V::I(hash)
        }
        _ => unsupported(ctx.rt, &format!("EnumSet operation {name}.{desc}")),
    })
}

fn is_enumset(rt: &Rt, value: V) -> bool {
    matches!(rt.class_of(value), Some(REGULAR | JUMBO))
        && !rt.get_field(value, "universe").is_null()
}
fn declaring_enum(ctx: &mut ExecCtx<'_>, value: V) -> Result<String, V> {
    if value.is_null() {
        return Err(ctx
            .rt
            .throw_exception("java/lang/NullPointerException", "EnumSet element"));
    }
    let Some(class) = ctx.rt.class_of(value) else {
        return Err(ctx
            .rt
            .throw_exception("java/lang/ClassCastException", "not an enum"));
    };
    let mut class = class.to_owned();
    if !ctx.linked.is_subtype(&class, "java/lang/Enum", ctx.host) {
        return Err(ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "EnumSet requires enum elements",
        ));
    }
    while let Some(parent) = ctx.linked.superclass_of(&class) {
        if parent == "java/lang/Enum" {
            break;
        }
        class = parent.to_owned();
    }
    Ok(class)
}
fn new_set(ctx: &mut ExecCtx<'_>, class: &str) -> Result<V, V> {
    // Assignability also holds for enum constant-body subclasses. When actual
    // superclass metadata exists, only the declaring enum is a legal element type.
    let direct_enum = match ctx.linked.superclass_of(class) {
        Some(parent) => parent == "java/lang/Enum",
        None => ctx.host.is_api_subtype(class, "java/lang/Enum"),
    };
    if !direct_enum {
        return Err(
            if ctx.linked.superclass_of(class).is_some()
                || matches!(
                    class,
                    "java/lang/Object"
                        | "java/lang/Class"
                        | "java/lang/String"
                        | "java/lang/Enum"
                        | "java/lang/Integer"
                        | "java/lang/Long"
                        | "java/lang/Short"
                        | "java/lang/Byte"
                        | "java/lang/Boolean"
                        | "java/lang/Character"
                        | "java/lang/Float"
                        | "java/lang/Double"
                )
            {
                ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    &format!("{class} not an enum"),
                )
            } else {
                unsupported(ctx.rt, &format!("EnumSet enum metadata for {class}"))
            },
        );
    }
    let universe = invoke_member(
        ctx,
        op::INVOKESTATIC,
        &MemberRef::new(class, "values", &format!("()[L{class};")),
        &[],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err(V::NULL);
    }
    let Some(Data::Array(values)) = ctx.rt.get_obj(universe).map(|object| &object.data) else {
        return Err(unsupported(ctx.rt, "EnumSet enum values array"));
    };
    let values = values.clone();
    for (index, value) in values.iter().enumerate() {
        if value.is_null()
            || ctx.rt.get_field(*value, "$name").is_null()
            || ctx.rt.get_field(*value, "$ordinal") != V::I(index as i32)
        {
            return Err(unsupported(ctx.rt, "EnumSet canonical enum universe"));
        }
    }
    let set = ctx
        .rt
        .new_instance(if values.len() <= 64 { REGULAR } else { JUMBO }, 0);
    let class = ctx
        .rt
        .new_pinned_handle("java/lang/Class", "class", 0, class);
    let values = ctx.rt.alloc("[Ljava/lang/Enum;", 0, Data::Array(values));
    let members = ctx.rt.alloc("java/util/Set", 0, Data::Set(Vec::new()));
    ctx.rt.put_field(set, "elementType", class);
    ctx.rt.put_field(set, "universe", values);
    ctx.rt.put_field(set, "$members", members);
    Ok(set)
}
fn members(rt: &Rt, set: V) -> Vec<V> {
    match rt
        .get_obj(rt.get_field(set, "$members"))
        .map(|object| &object.data)
    {
        Some(Data::Set(values)) => values.clone(),
        _ => Vec::new(),
    }
}
fn universe_values(rt: &Rt, set: V) -> Vec<V> {
    match rt
        .get_obj(rt.get_field(set, "universe"))
        .map(|object| &object.data)
    {
        Some(Data::Array(values)) => values.clone(),
        _ => Vec::new(),
    }
}
fn write_members(rt: &mut Rt, set: V, values: Vec<V>) {
    if let Some(Data::Set(members)) = rt
        .get_obj_mut(rt.get_field(set, "$members"))
        .map(|object| &mut object.data)
    {
        *members = values;
    }
}
fn clone_set(rt: &mut Rt, set: V) -> V {
    let copy = rt.new_instance(rt.class_of(set).unwrap_or(REGULAR).to_owned(), 0);
    rt.put_field(copy, "elementType", rt.get_field(set, "elementType"));
    rt.put_field(copy, "universe", rt.get_field(set, "universe"));
    let values = rt.alloc("java/util/Set", 0, Data::Set(members(rt, set)));
    rt.put_field(copy, "$members", values);
    copy
}
fn add(ctx: &mut ExecCtx<'_>, set: V, value: V) -> V {
    let class = match declaring_enum(ctx, value) {
        Ok(class) => class,
        Err(error) => return error,
    };
    let expected = ctx
        .rt
        .handle_of(ctx.rt.get_field(set, "elementType"))
        .map(|(_, _, class, _)| class);
    if expected != Some(class.as_str()) {
        return ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "EnumSet element type mismatch",
        );
    }
    if !universe_values(ctx.rt, set).contains(&value) {
        return unsupported(ctx.rt, "EnumSet noncanonical enum constant");
    }
    let mut values = members(ctx.rt, set);
    if values.contains(&value) {
        return V::FALSE;
    }
    values.push(value);
    values.sort_by_key(|value| ctx.rt.get_field(*value, "$ordinal").i());
    write_members(ctx.rt, set, values);
    V::TRUE
}
fn remove(rt: &mut Rt, set: V, value: V) -> V {
    let mut values = members(rt, set);
    let old = values.len();
    values.retain(|candidate| *candidate != value);
    write_members(rt, set, values.clone());
    V::bool(values.len() != old)
}
fn next(ctx: &mut ExecCtx<'_>, iter: V) -> V {
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/Iterator", "next", "()Ljava/lang/Object;"),
        &[iter],
    )
}
fn has_next(ctx: &mut ExecCtx<'_>, iter: V) -> V {
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/Iterator", "hasNext", "()Z"),
        &[iter],
    )
}
fn collection_values(ctx: &mut ExecCtx<'_>, collection: V) -> Result<Vec<V>, V> {
    let iter = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/Collection", "iterator", "()Ljava/util/Iterator;"),
        &[collection],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err(V::NULL);
    }
    let mut values = Vec::new();
    loop {
        let more = has_next(ctx, iter);
        if ctx.rt.pending_exception.is_some() {
            return Err(V::NULL);
        }
        if more.i() == 0 {
            break;
        }
        let value = next(ctx, iter);
        if ctx.rt.pending_exception.is_some() {
            return Err(V::NULL);
        }
        values.push(value);
    }
    Ok(values)
}
fn iterator_call(ctx: &mut ExecCtx<'_>, iter: V, name: &str, desc: &str) -> V {
    let snapshot = ctx.rt.get_field(iter, "snapshot");
    let values = match ctx.rt.get_obj(snapshot).map(|object| &object.data) {
        Some(Data::Array(values)) => values.clone(),
        _ => return unsupported(ctx.rt, "EnumSet iterator snapshot"),
    };
    let index = ctx.rt.get_field(iter, "index").i() as usize;
    match (name, desc) {
        ("hasNext", "()Z") => V::bool(index < values.len()),
        ("next", "()Ljava/lang/Object;") => {
            let Some(value) = values.get(index).copied() else {
                return ctx.rt.throw_exception(
                    "java/util/NoSuchElementException",
                    "EnumSet iterator exhausted",
                );
            };
            ctx.rt.put_field(iter, "index", V::I(index as i32 + 1));
            ctx.rt.put_field(iter, "last", value);
            value
        }
        ("remove", "()V") => {
            if ctx.rt.get_field(iter, "readonly").i() != 0 {
                return ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "unmodifiable EnumSet iterator",
                );
            }
            let last = ctx.rt.get_field(iter, "last");
            if last.is_null() {
                return ctx.rt.throw_exception(
                    "java/lang/IllegalStateException",
                    "EnumSet iterator has no last element",
                );
            }
            remove(ctx.rt, ctx.rt.get_field(iter, "set"), last);
            ctx.rt.put_field(iter, "last", V::NULL);
            V::Void
        }
        _ => unsupported(ctx.rt, &format!("EnumSet iterator {name}.{desc}")),
    }
}
fn unsupported(rt: &mut Rt, reason: &str) -> V {
    rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("MMGA does not implement {reason}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost, Rt};
    use mmga_classfile::op;

    fn universe(ctx: &mut ExecCtx<'_>, class: &str, count: usize) -> V {
        let mut values = Vec::new();
        for index in 0..count {
            let name = format!("E{index}");
            let constant = match ctx.rt.get_static(class, &name) {
                Some(value) => value,
                None => {
                    let concrete = if class == "test/Small" && index == 1 {
                        "test/Small$1"
                    } else {
                        class
                    };
                    let value = ctx.rt.new_instance(concrete, 0);
                    let name_value = ctx.rt.intern_str(&name);
                    ctx.rt.put_field(value, "$name", name_value);
                    ctx.rt.put_field(value, "$ordinal", V::I(index as i32));
                    ctx.rt.put_static(class, &name, value);
                    value
                }
            };
            values.push(constant);
        }
        ctx.rt.alloc(&format!("[L{class};"), 0, Data::Array(values))
    }
    fn small(ctx: &mut ExecCtx<'_>, _: &[V]) -> V {
        universe(ctx, "test/Small", 3)
    }
    fn large(ctx: &mut ExecCtx<'_>, _: &[V]) -> V {
        universe(ctx, "test/Large", 66)
    }
    fn no_values(ctx: &mut ExecCtx<'_>, _: &[V]) -> V {
        universe(ctx, "test/Empty", 0)
    }
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: "test/Small",
                    superclass: Some("java/lang/Enum"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[("values.()[Ltest/Small;", small, true, false)],
                },
                NativeClassDef {
                    name: "test/Small$1",
                    superclass: Some("test/Small"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
                NativeClassDef {
                    name: "test/Large",
                    superclass: Some("java/lang/Enum"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[("values.()[Ltest/Large;", large, true, false)],
                },
                NativeClassDef {
                    name: "test/Empty",
                    superclass: Some("java/lang/Enum"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[("values.()[Ltest/Empty;", no_values, true, false)],
                },
            ],
            lambdas: &[],
        });
        linked
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
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn values(ctx: &mut ExecCtx<'_>, class: &str) -> Vec<V> {
        let result = invoke(ctx, class, "values", &format!("()[L{class};"), true, &[]);
        match &ctx.rt.get_obj(result).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("enum[]"),
        }
    }
    #[test]
    fn collection_iteration_failure_keeps_prior_bulk_additions() {
        struct FailingHost {
            first: V,
            steps: usize,
        }
        impl crate::HostApi for FailingHost {
            fn get_static_field(&mut self, _: &mut Rt, _: &str, _: &str, _: &str) -> Option<V> {
                None
            }
            fn call_method(
                &mut self,
                rt: &mut Rt,
                _: &LinkedSpace,
                member: &MemberRef,
                is_static: bool,
                args: &[V],
            ) -> Option<V> {
                if is_static {
                    return None;
                }
                if rt.class_of(args[0]) == Some("test/FailingCollection")
                    && member.name == "iterator"
                {
                    return Some(rt.new_instance("test/FailingIterator", 0));
                }
                if rt.class_of(args[0]) == Some("test/FailingIterator") {
                    if member.name == "hasNext" {
                        return Some(V::TRUE);
                    }
                    if member.name == "next" {
                        self.steps += 1;
                        return Some(if self.steps == 1 {
                            self.first
                        } else {
                            rt.throw_exception("test/IterationFailure", "second entry")
                        });
                    }
                }
                None
            }
        }
        let mut rt = Rt::new();
        let linked = linked();
        let mut null = NullHost;
        let constants = values(
            &mut ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut null,
                depth: 0,
            },
            "test/Small",
        );
        let mut host = FailingHost {
            first: constants[1],
            steps: 0,
        };
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            let class = ctx
                .rt
                .new_pinned_handle("java/lang/Class", "class", 0, "test/Small");
            let set = invoke(
                &mut ctx,
                "java/util/EnumSet",
                "noneOf",
                "(Ljava/lang/Class;)Ljava/util/EnumSet;",
                true,
                &[class],
            );
            let collection = ctx.rt.new_instance("test/FailingCollection", 0);
            invoke(
                &mut ctx,
                "java/util/Set",
                "addAll",
                "(Ljava/util/Collection;)Z",
                false,
                &[set, collection],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(ctx.rt.class_of(error), Some("test/IterationFailure"));
            assert_eq!(
                invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[set]),
                V::I(1)
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    "java/util/Set",
                    "contains",
                    "(Ljava/lang/Object;)Z",
                    false,
                    &[set, constants[1]]
                ),
                V::TRUE
            );
        }
        assert_eq!(host.steps, 2);
    }
    #[test]
    fn copy_of_preserves_real_enum_identity_order_and_empty_enumset_type() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let constants = values(&mut ctx, "test/Small");
        let input = ctx.rt.alloc(
            "java/util/ArrayList",
            0,
            Data::List(vec![constants[2], constants[0], constants[2], constants[1]]),
        );
        let set = invoke(
            &mut ctx,
            "java/util/EnumSet",
            "copyOf",
            "(Ljava/util/Collection;)Ljava/util/EnumSet;",
            true,
            &[input],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "real EnumSet.copyOf missing"
        );
        assert!(ctx.linked.is_subtype(
            ctx.rt.class_of(set).unwrap(),
            "java/util/EnumSet",
            ctx.host
        ));
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[set]),
            V::I(3)
        );
        let iterator = invoke(
            &mut ctx,
            "java/util/Set",
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[set],
        );
        for value in &constants {
            assert_eq!(
                invoke(
                    &mut ctx,
                    "java/util/Iterator",
                    "next",
                    "()Ljava/lang/Object;",
                    false,
                    &[iterator]
                ),
                *value
            );
        }
        invoke(&mut ctx, "java/util/Set", "clear", "()V", false, &[set]);
        let copy = invoke(
            &mut ctx,
            "java/util/EnumSet",
            "copyOf",
            "(Ljava/util/Collection;)Ljava/util/EnumSet;",
            true,
            &[set],
        );
        assert_ne!(copy, set);
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[copy]),
            V::I(0)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Set",
                "add",
                "(Ljava/lang/Object;)Z",
                false,
                &[copy, constants[1]]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[set]),
            V::I(0)
        );
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn empty_collections_null_and_mixed_enum_types_report_real_exceptions() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let empty = ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new()));
        invoke(
            &mut ctx,
            "java/util/EnumSet",
            "copyOf",
            "(Ljava/util/Collection;)Ljava/util/EnumSet;",
            true,
            &[empty],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalArgumentException")
        );
        let constants = values(&mut ctx, "test/Small");
        let set = invoke(
            &mut ctx,
            "java/util/EnumSet",
            "of",
            "(Ljava/lang/Enum;)Ljava/util/EnumSet;",
            true,
            &[constants[0]],
        );
        assert!(ctx.rt.pending_exception.is_none());
        invoke(
            &mut ctx,
            "java/util/Set",
            "add",
            "(Ljava/lang/Object;)Z",
            false,
            &[set, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/NullPointerException")
        );
        let other = values(&mut ctx, "test/Large")[0];
        invoke(
            &mut ctx,
            "java/util/Set",
            "add",
            "(Ljava/lang/Object;)Z",
            false,
            &[set, other],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Set",
                "contains",
                "(Ljava/lang/Object;)Z",
                false,
                &[set, V::NULL]
            ),
            V::FALSE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Set",
                "contains",
                "(Ljava/lang/Object;)Z",
                false,
                &[set, other]
            ),
            V::FALSE
        );
        let not_enum = ctx
            .rt
            .new_pinned_handle("java/lang/Class", "class", 0, "java/lang/String");
        invoke(
            &mut ctx,
            "java/util/EnumSet",
            "noneOf",
            "(Ljava/lang/Class;)Ljava/util/EnumSet;",
            true,
            &[not_enum],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
        let unknown =
            ctx.rt
                .new_pinned_handle("java/lang/Class", "class", 0, "java/time/DayOfWeek");
        invoke(
            &mut ctx,
            "java/util/EnumSet",
            "noneOf",
            "(Ljava/lang/Class;)Ljava/util/EnumSet;",
            true,
            &[unknown],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn large_universes_and_unmodifiable_views_keep_real_members_and_block_mutation() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let class = ctx
            .rt
            .new_pinned_handle("java/lang/Class", "class", 0, "test/Large");
        let all = invoke(
            &mut ctx,
            "java/util/EnumSet",
            "allOf",
            "(Ljava/lang/Class;)Ljava/util/EnumSet;",
            true,
            &[class],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.class_of(all), Some("java/util/JumboEnumSet"));
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[all]),
            V::I(66)
        );
        let view = invoke(
            &mut ctx,
            "java/util/Collections",
            "unmodifiableSet",
            "(Ljava/util/Set;)Ljava/util/Set;",
            true,
            &[all],
        );
        assert_ne!(view, all);
        invoke(&mut ctx, "java/util/Set", "clear", "()V", false, &[view]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        let iterator = invoke(
            &mut ctx,
            "java/util/Set",
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[view],
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
            false,
            &[iterator],
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "remove",
            "()V",
            false,
            &[iterator],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        invoke(&mut ctx, "java/util/Set", "clear", "()V", false, &[all]);
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[view]),
            V::I(0)
        );
        let class = ctx
            .rt
            .new_pinned_handle("java/lang/Class", "class", 0, "test/Empty");
        let empty = invoke(
            &mut ctx,
            "java/util/EnumSet",
            "noneOf",
            "(Ljava/lang/Class;)Ljava/util/EnumSet;",
            true,
            &[class],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(&mut ctx, "java/util/Set", "size", "()I", false, &[empty]),
            V::I(0)
        );
    }
}

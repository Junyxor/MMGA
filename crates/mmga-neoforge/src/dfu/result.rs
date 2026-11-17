use super::*;

pub(super) fn success(rt: &mut Rt, value: V) -> V {
    let result = rt.new_instance("com/mojang/serialization/DataResult$Success", 0);
    rt.put_field(result, "$success", V::TRUE);
    rt.put_field(result, "$value", value);
    let life = lifecycle(rt, "experimental", 0);
    rt.put_field(result, "$lifecycle", life);
    result
}
pub(super) fn error(rt: &mut Rt, message: &str, partial: Option<V>) -> V {
    let result = rt.new_instance("com/mojang/serialization/DataResult$Error", 0);
    let message = rt.new_str(message);
    rt.put_field(result, "$message", message);
    rt.put_field(result, "$success", V::FALSE);
    rt.put_field(result, "$partial", partial.unwrap_or(V::NULL));
    let life = lifecycle(rt, "experimental", 0);
    rt.put_field(result, "$lifecycle", life);
    result
}
pub(super) fn value(rt: &Rt, result: V) -> Option<V> {
    (rt.get_field(result, "$success") == V::TRUE).then(|| rt.get_field(result, "$value"))
}
pub(super) fn message(ctx: &mut ExecCtx<'_>, result: V) -> String {
    let supplier = ctx.rt.get_field(result, "$message_supplier");
    if !supplier.is_null() {
        let text = function(ctx, supplier, &[]);
        return ctx
            .rt
            .str_of(text)
            .unwrap_or("<non-string DataResult error>")
            .to_owned();
    }
    ctx.rt
        .str_of(ctx.rt.get_field(result, "$message"))
        .unwrap_or("DFU error")
        .to_owned()
}
fn lifecycle(rt: &mut Rt, kind: &str, version: i32) -> V {
    let key = format!("{kind}:{version}");
    if let Some(v) = rt.get_static("com/mojang/serialization/Lifecycle", &key) {
        return v;
    }
    let value = rt.new_pinned_handle(
        "com/mojang/serialization/Lifecycle",
        "dfu_lifecycle",
        version as u64,
        kind,
    );
    rt.put_static("com/mojang/serialization/Lifecycle", &key, value);
    value
}
fn optional(rt: &mut Rt, v: Option<V>) -> V {
    rt.alloc("java/util/Optional", 0, Data::Optional(v))
}
fn clone_result(rt: &mut Rt, value: V) -> V {
    let obj = rt.get_obj(value).unwrap().clone();
    rt.alloc(obj.class, 0, obj.data)
}
fn combine_lifecycle(rt: &mut Rt, first: V, second: V) -> V {
    let a = rt
        .handle_of(first)
        .map(|(_, id, tag, _)| (id as i32, tag.to_owned()));
    let b = rt
        .handle_of(second)
        .map(|(_, id, tag, _)| (id as i32, tag.to_owned()));
    match (a, b) {
        (Some((_, a)), _) if a == "experimental" => first,
        (_, Some((_, b))) if b == "experimental" => second,
        (Some((a, at)), Some((b, bt))) if at == "deprecated" && bt == "deprecated" => {
            if a <= b {
                first
            } else {
                second
            }
        }
        (Some((_, a)), _) if a == "deprecated" => first,
        (_, Some((_, b))) if b == "deprecated" => second,
        _ => first,
    }
}

pub(super) fn call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == "com/mojang/serialization/Lifecycle" && is_static {
        return match (name, desc) {
            ("stable", "()Lcom/mojang/serialization/Lifecycle;") => {
                Some(lifecycle(ctx.rt, "stable", 0))
            }
            ("experimental", "()Lcom/mojang/serialization/Lifecycle;") => {
                Some(lifecycle(ctx.rt, "experimental", 0))
            }
            ("deprecated", "(I)Lcom/mojang/serialization/Lifecycle;") => {
                Some(lifecycle(ctx.rt, "deprecated", args[0].i()))
            }
            _ => None,
        };
    }
    if !matches!(
        owner,
        RESULT
            | "com/mojang/serialization/DataResult$Success"
            | "com/mojang/serialization/DataResult$Error"
    ) {
        return None;
    }
    if is_static {
        return match (name,desc){
            ("success","(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;")=>Some(success(ctx.rt,args[0])),
            ("success","(Ljava/lang/Object;Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;")=>{let v=success(ctx.rt,args[0]);ctx.rt.put_field(v,"$lifecycle",args[1]);Some(v)},
            ("error","(Ljava/util/function/Supplier;)Lcom/mojang/serialization/DataResult;"|"(Ljava/util/function/Supplier;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"|"(Ljava/util/function/Supplier;Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;"|"(Ljava/util/function/Supplier;Ljava/lang/Object;Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;")=>{
                let partial=if desc.contains(";Ljava/lang/Object;"){Some(args[1])}else{None};
                if partial.is_some_and(V::is_null){return Some(ctx.rt.throw_exception("java/lang/NullPointerException","DataResult partial result is null"));}
                let v=error(ctx.rt,"",partial);ctx.rt.put_field(v,"$message_supplier",args[0]);
                if desc.contains("Lifecycle;"){ctx.rt.put_field(v,"$lifecycle",*args.last()?);}
                Some(v)
            }
            _=>None,
        };
    }
    let recv = *args.first()?;
    let successful = ctx.rt.get_field(recv, "$success") == V::TRUE;
    let value = ctx
        .rt
        .get_field(recv, if successful { "$value" } else { "$partial" });
    match (name, desc) {
        ("isSuccess", "()Z") => Some(V::bool(successful)),
        ("isError", "()Z") => Some(V::bool(!successful)),
        ("result", "()Ljava/util/Optional;") => Some(if successful && value.is_null() {
            ctx.rt
                .throw_exception("java/lang/NullPointerException", "Optional.of(null)")
        } else {
            optional(ctx.rt, successful.then_some(value))
        }),
        ("error", "()Ljava/util/Optional;") => {
            Some(optional(ctx.rt, (!successful).then_some(recv)))
        }
        ("partialValue", "()Ljava/util/Optional;") if !successful => {
            Some(optional(ctx.rt, (!value.is_null()).then_some(value)))
        }
        ("hasResultOrPartial", "()Z") => Some(V::bool(successful || !value.is_null())),
        (
            "resultOrPartial",
            "()Ljava/util/Optional;" | "(Ljava/util/function/Consumer;)Ljava/util/Optional;",
        ) => {
            if successful && value.is_null() {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/NullPointerException", "Optional.of(null)"),
                );
            }
            if !successful && args.len() == 2 {
                let msg = message(ctx, recv);
                let msg = ctx.rt.new_str(msg);
                invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Consumer",
                        "accept",
                        "(Ljava/lang/Object;)V",
                    ),
                    &[args[1], msg],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
            }
            Some(optional(ctx.rt, (!value.is_null()).then_some(value)))
        }
        (
            "getOrThrow" | "getPartialOrThrow",
            "()Ljava/lang/Object;" | "(Ljava/util/function/Function;)Ljava/lang/Object;",
        ) => {
            if successful || (name == "getPartialOrThrow" && !value.is_null()) {
                return Some(value);
            }
            let msg = message(ctx, recv);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if args.len() == 1 {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/IllegalStateException", &msg),
                );
            }
            let msg = ctx.rt.new_str(msg);
            let exception = function(ctx, args[1], &[msg]);
            if ctx.rt.pending_exception.is_none() {
                if exception.is_null() {
                    ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "exception supplier returned null",
                    );
                } else {
                    ctx.rt.pending_exception = Some(exception.r());
                }
            }
            Some(V::NULL)
        }
        (
            "map" | "flatMap",
            "(Ljava/util/function/Function;)Lcom/mojang/serialization/DataResult;",
        ) => {
            if !successful && value.is_null() {
                return Some(recv);
            }
            let mapped = function(ctx, args[1], &[value]);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if name == "map" {
                let out = if successful {
                    success(ctx.rt, mapped)
                } else {
                    let out = clone_result(ctx.rt, recv);
                    ctx.rt.put_field(out, "$partial", mapped);
                    out
                };
                let life = ctx.rt.get_field(recv, "$lifecycle");
                ctx.rt.put_field(out, "$lifecycle", life);
                return Some(out);
            }
            if successful {
                let combined = combine_lifecycle(
                    ctx.rt,
                    ctx.rt.get_field(recv, "$lifecycle"),
                    ctx.rt.get_field(mapped, "$lifecycle"),
                );
                let out = clone_result(ctx.rt, mapped);
                ctx.rt.put_field(out, "$lifecycle", combined);
                return Some(out);
            }
            let first = message(ctx, recv);
            let next = if let Some(v) = self::value(ctx.rt, mapped) {
                error(ctx.rt, &first, Some(v))
            } else {
                let second = message(ctx, mapped);
                let partial = ctx.rt.get_field(mapped, "$partial");
                error(
                    ctx.rt,
                    &format!("{first}; {second}"),
                    (!partial.is_null()).then_some(partial),
                )
            };
            let combined = combine_lifecycle(
                ctx.rt,
                ctx.rt.get_field(recv, "$lifecycle"),
                ctx.rt.get_field(mapped, "$lifecycle"),
            );
            ctx.rt.put_field(next, "$lifecycle", combined);
            Some(next)
        }
        ("message", "()Ljava/lang/String;") => {
            let msg = message(ctx, recv);
            Some(ctx.rt.new_str(msg))
        }
        ("lifecycle", "()Lcom/mojang/serialization/Lifecycle;") => {
            Some(ctx.rt.get_field(recv, "$lifecycle"))
        }
        (
            "setLifecycle",
            "(Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;",
        ) => {
            let out = clone_result(ctx.rt, recv);
            ctx.rt.put_field(out, "$lifecycle", args[1]);
            Some(out)
        }
        (
            "setPartial",
            "(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"
            | "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/DataResult;",
        ) => {
            if successful {
                return Some(recv);
            }
            let partial = if desc.contains("Supplier") {
                function(ctx, args[1], &[])
            } else {
                args[1]
            };
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if partial.is_null() {
                return Some(
                    ctx.rt.throw_exception(
                        "java/lang/NullPointerException",
                        "partial result is null",
                    ),
                );
            }
            let out = clone_result(ctx.rt, recv);
            ctx.rt.put_field(out, "$partial", partial);
            Some(out)
        }
        (
            "addLifecycle",
            "(Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;",
        ) => {
            let combined = combine_lifecycle(ctx.rt, ctx.rt.get_field(recv, "$lifecycle"), args[1]);
            let out = clone_result(ctx.rt, recv);
            ctx.rt.put_field(out, "$lifecycle", combined);
            Some(out)
        }
        _ => None,
    }
}

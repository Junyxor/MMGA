//! Lazy FML IMC queue streams. Semantic reference: FancyModLoader 1.21.1
//! InterModComms a854836002e34e483a6611bdd22f45755442f352 and
//! InterModProcessEvent 14c7932e8ae4aeff29f101a1097f2f3779631770.

use super::{FmlLoader, ImcEntry};
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, LinkedSpace, Rt, V};

const MESSAGE: &str = "net/neoforged/fml/InterModComms$IMCMessage";
const COMMS: &str = "net/neoforged/fml/InterModComms";
const LIFECYCLE: &str = "net/neoforged/fml/event/lifecycle/ModLifecycleEvent";
const STREAM: &str = "mmga/internal/ImcMessageStream";
const STORAGE: &str = "mmga/internal/ImcRecipientQueues";
const NODE: &str = "mmga/internal/ImcQueueNode";
const STAGE: &str = "mmga/internal/ImcStreamStage";

pub(super) fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == STREAM
        && matches!(
            sup,
            "java/util/stream/Stream" | "java/util/stream/BaseStream" | "java/lang/AutoCloseable"
        ))
        || (sub == MESSAGE && sup == "java/lang/Record")
}
fn queues(rt: &mut Rt) -> V {
    if let Some(value) = rt.get_static(STORAGE, "queues") {
        return value;
    }
    let value = rt.alloc("java/util/HashMap", 0, Data::Map(vec![]));
    rt.put_static(STORAGE, "queues", value);
    value
}
fn queue(rt: &Rt, recipient: V) -> Option<V> {
    let queues = rt.get_static(STORAGE, "queues")?;
    let Data::Map(entries) = &rt.get_obj(queues)?.data else {
        return None;
    };
    entries
        .iter()
        .find(|(name, _)| rt.val_eq(*name, recipient))
        .map(|(_, value)| *value)
}
fn ensure_queue(rt: &mut Rt, recipient: V) -> V {
    if let Some(value) = queue(rt, recipient) {
        return value;
    }
    let queues = queues(rt);
    let value = rt.new_instance("mmga/internal/ImcRecipientQueue", 0);
    rt.put_field(value, "head", V::NULL);
    rt.put_field(value, "tail", V::NULL);
    if let Some(Data::Map(entries)) = rt.get_obj_mut(queues).map(|o| &mut o.data) {
        entries.push((recipient, value));
    }
    value
}
pub(super) fn send(
    loader: &mut FmlLoader,
    rt: &mut Rt,
    sender: V,
    recipient: V,
    method: V,
    supplier: V,
) -> V {
    let Some(target) = rt.str_of(recipient).map(str::to_owned) else {
        if recipient.is_null() {
            return V::FALSE;
        }
        return rt.throw_exception(
            "java/lang/ClassCastException",
            "IMC recipient must be String",
        );
    };
    if loader.version_of(&target).is_none() && !matches!(target.as_str(), "minecraft" | "neoforge")
    {
        return V::FALSE;
    }
    let message = rt.new_instance(MESSAGE, 0);
    for (field, value) in [
        ("senderModId", sender),
        ("modId", recipient),
        ("method", method),
        ("messageSupplier", supplier),
    ] {
        rt.put_field(message, field, value);
    }
    let node = rt.new_instance(NODE, 0);
    rt.put_field(node, "message", message);
    rt.put_field(node, "next", V::NULL);
    rt.put_field(node, "present", V::TRUE);
    let queue = ensure_queue(rt, recipient);
    let tail = rt.get_field(queue, "tail");
    if tail.is_null() {
        rt.put_field(queue, "head", node);
    } else {
        rt.put_field(tail, "next", node);
    }
    rt.put_field(queue, "tail", node);
    // These String fields are diagnostic labels. Java getters and matchers always
    // read the original nullable references in the real record above.
    let sender_text = rt.str_of(sender).unwrap_or("null").to_owned();
    let method_text = rt.str_of(method).unwrap_or("null").to_owned();
    loader.imc_messages.push(ImcEntry {
        sender_mod_id: sender_text,
        mod_id: target,
        method: method_text,
        supplier,
        message,
        node,
    });
    V::TRUE
}
fn first_live(rt: &Rt, mut node: V) -> (V, V) {
    while !node.is_null() {
        let message = rt.get_field(node, "message");
        if rt.get_field(node, "present") == V::TRUE && !message.is_null() {
            return (node, message);
        }
        node = rt.get_field(node, "next");
    }
    (V::NULL, V::NULL)
}
fn create_stream(rt: &mut Rt, recipient: V, predicate: V, accept_all: bool) -> V {
    let value = rt.new_instance(STREAM, 0);
    let queue = queue(rt, recipient).unwrap_or(V::NULL);
    let (node, message) = first_live(rt, rt.get_field(queue, "head"));
    let stages = rt.alloc("java/util/ArrayList", 0, Data::List(vec![]));
    for (field, item) in [
        ("queue", queue),
        ("next_node", node),
        ("next_message", message),
        ("predicate", predicate),
        ("accept_all", V::bool(accept_all)),
        ("root", value),
        ("stages", stages),
        ("used", V::FALSE),
        ("linked", V::FALSE),
    ] {
        rt.put_field(value, field, item);
    }
    value
}
fn remove_message(loader: &mut FmlLoader, rt: &mut Rt, queue: V, node: V) {
    rt.put_field(node, "present", V::FALSE);
    rt.put_field(node, "message", V::NULL);
    loader.imc_messages.retain(|entry| entry.node != node);
    let head = rt.get_field(queue, "head");
    let (first, _) = first_live(rt, head);
    let head = if first.is_null() {
        rt.get_field(queue, "tail")
    } else {
        first
    };
    rt.put_field(queue, "head", head);
}
fn stage_values(rt: &Rt, stream: V) -> Vec<V> {
    match rt.get_obj(rt.get_field(stream, "stages")).map(|o| &o.data) {
        Some(Data::List(values)) => values.clone(),
        _ => vec![],
    }
}
fn consumed(rt: &Rt, stream: V) -> bool {
    let root = rt.get_field(stream, "root");
    rt.get_field(stream, "used") == V::TRUE
        || rt.get_field(stream, "linked") == V::TRUE
        || rt.get_field(root, "used") == V::TRUE
}
fn already_used(rt: &mut Rt) -> V {
    rt.throw_exception(
        "java/lang/IllegalStateException",
        "stream has already been operated upon or closed",
    )
}
fn intermediate(rt: &mut Rt, stream: V, operation: &str, argument: V) -> V {
    if matches!(operation, "filter" | "map" | "peek") && argument.is_null() {
        return rt.throw_exception(
            "java/lang/NullPointerException",
            "IMC stream callback is null",
        );
    }
    if matches!(operation, "limit" | "skip") && argument.j() < 0 {
        return rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "negative stream bound",
        );
    }
    if consumed(rt, stream) {
        return already_used(rt);
    }
    let stage = rt.new_instance(STAGE, 0);
    let name = rt.intern_str(operation);
    rt.put_field(stage, "operation", name);
    rt.put_field(stage, "argument", argument);
    rt.put_field(stage, "seen", V::J(0));
    let mut stages = stage_values(rt, stream);
    stages.push(stage);
    let stages = rt.alloc("java/util/ArrayList", 0, Data::List(stages));
    let next = rt.new_instance(STREAM, 0);
    let root = rt.get_field(stream, "root");
    rt.put_field(next, "root", root);
    rt.put_field(next, "stages", stages);
    rt.put_field(next, "used", V::FALSE);
    rt.put_field(next, "linked", V::FALSE);
    rt.put_field(stream, "linked", V::TRUE);
    next
}
fn limit_reached(rt: &Rt, stages: &[V]) -> bool {
    stages.iter().any(|stage| {
        rt.str_of(rt.get_field(*stage, "operation")) == Some("limit")
            && rt.get_field(*stage, "seen").j() >= rt.get_field(*stage, "argument").j()
    })
}
pub(super) fn call(
    loader: &mut FmlLoader,
    rt: &mut Rt,
    linked: &LinkedSpace,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static
        && owner == COMMS
        && name == "getMessages"
        && matches!(
            desc,
            "(Ljava/lang/String;)Ljava/util/stream/Stream;"
                | "(Ljava/lang/String;Ljava/util/function/Predicate;)Ljava/util/stream/Stream;"
        )
    {
        let recipient = *args.first()?;
        if recipient.is_null() {
            return Some(
                rt.throw_exception("java/lang/NullPointerException", "IMC recipient is null"),
            );
        }
        return Some(create_stream(
            rt,
            recipient,
            args.get(1).copied().unwrap_or(V::NULL),
            args.len() == 1,
        ));
    }
    if !is_static
        && name == "getIMCStream"
        && matches!(
            desc,
            "()Ljava/util/stream/Stream;"
                | "(Ljava/util/function/Predicate;)Ljava/util/stream/Stream;"
        )
    {
        let event = *args.first()?;
        if !linked.is_subtype(rt.class_of(event)?, LIFECYCLE, loader) {
            return None;
        }
        let container = rt.get_field(event, "container");
        let recipient = {
            let mut ctx = ExecCtx {
                rt,
                linked,
                host: loader,
                depth: 0,
            };
            invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/fml/ModContainer",
                    "getModId",
                    "()Ljava/lang/String;",
                ),
                &[container],
            )
        };
        if rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        return Some(create_stream(
            rt,
            recipient,
            args.get(1).copied().unwrap_or(V::NULL),
            args.len() == 1,
        ));
    }
    if !is_static
        && name == "getContainer"
        && desc == "()Lnet/neoforged/fml/ModContainer;"
        && linked.is_subtype(rt.class_of(*args.first()?)?, LIFECYCLE, loader)
    {
        return Some(rt.get_field(args[0], "container"));
    }
    if !is_static
        && matches!(
            owner,
            LIFECYCLE | "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent"
        )
        && name == "<init>"
        && matches!(
            desc,
            "(Lnet/neoforged/fml/ModContainer;)V"
                | "(Lnet/neoforged/fml/ModContainer;Lnet/neoforged/fml/DeferredWorkQueue;)V"
        )
    {
        rt.put_field(args[0], "container", args[1]);
        rt.put_field(args[0], "isCanceled", V::FALSE);
        rt.put_field(args[0], "$canceled", V::FALSE);
        if let Some(queue) = args.get(2) {
            rt.put_field(args[0], "deferredWorkQueue", *queue);
        }
        return Some(V::Void);
    }
    if !is_static && rt.class_of(*args.first()?) == Some(MESSAGE) {
        let field = match (name, desc) {
            ("senderModId" | "getSenderModId", "()Ljava/lang/String;") => "senderModId",
            ("modId" | "getModId", "()Ljava/lang/String;") => "modId",
            ("method" | "getMethod", "()Ljava/lang/String;") => "method",
            ("messageSupplier" | "getMessageSupplier", "()Ljava/util/function/Supplier;") => {
                "messageSupplier"
            }
            ("equals", "(Ljava/lang/Object;)Z")
            | ("hashCode", "()I")
            | ("toString", "()Ljava/lang/String;") => {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "IMC record value methods are not implemented",
                ))
            }
            _ => return None,
        };
        return Some(rt.get_field(args[0], field));
    }
    if is_static || rt.class_of(*args.first()?) != Some(STREAM) {
        return None;
    }
    let stream = args[0];
    match (name, desc) {
        ("filter", "(Ljava/util/function/Predicate;)Ljava/util/stream/Stream;")
        | ("map", "(Ljava/util/function/Function;)Ljava/util/stream/Stream;")
        | ("peek", "(Ljava/util/function/Consumer;)Ljava/util/stream/Stream;")
        | ("limit" | "skip", "(J)Ljava/util/stream/Stream;") => {
            Some(intermediate(rt, stream, name, args[1]))
        }
        ("forEach" | "forEachOrdered", "(Ljava/util/function/Consumer;)V")
        | ("count", "()J")
        | ("toList", "()Ljava/util/List;")
        | ("findFirst" | "findAny", "()Ljava/util/Optional;")
        | ("anyMatch" | "allMatch" | "noneMatch", "(Ljava/util/function/Predicate;)Z") => Some(
            terminal(loader, rt, linked, stream, name, args.get(1).copied()),
        ),
        ("close", "()V") => {
            let root = rt.get_field(stream, "root");
            rt.put_field(root, "used", V::TRUE);
            Some(V::Void)
        }
        ("isParallel", "()Z") => Some(V::FALSE),
        ("sequential", "()Ljava/util/stream/BaseStream;") => Some(stream),
        _ => Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            &format!("IMC stream operation {} is not implemented", member.key()),
        )),
    }
}
fn terminal(
    loader: &mut FmlLoader,
    rt: &mut Rt,
    linked: &LinkedSpace,
    stream: V,
    operation: &str,
    callback: Option<V>,
) -> V {
    if matches!(
        operation,
        "forEach" | "forEachOrdered" | "anyMatch" | "allMatch" | "noneMatch"
    ) && callback.is_none_or(|v| v.is_null())
    {
        return rt.throw_exception(
            "java/lang/NullPointerException",
            "IMC stream terminal callback is null",
        );
    }
    if consumed(rt, stream) {
        return already_used(rt);
    }
    let root = rt.get_field(stream, "root");
    rt.put_field(stream, "used", V::TRUE);
    rt.put_field(root, "used", V::TRUE);
    let stages = stage_values(rt, stream);
    let mut values = Vec::new();
    let mut count = 0_i64;
    let mut matched = None;
    while !limit_reached(rt, &stages) {
        let node = rt.get_field(root, "next_node");
        let message = rt.get_field(root, "next_message");
        if node.is_null() {
            break;
        }
        // Iterator.next prefetches before predicate/action. A cached message stays
        // visible even if a different iterator removes the same node meanwhile.
        let (next, next_message) = first_live(rt, rt.get_field(node, "next"));
        rt.put_field(root, "next_node", next);
        rt.put_field(root, "next_message", next_message);
        let mut value = message;
        let mut stop = false;
        let completed = {
            let mut ctx = ExecCtx {
                rt,
                linked,
                host: loader,
                depth: 0,
            };
            let accepted = if ctx.rt.get_field(root, "accept_all") == V::TRUE {
                true
            } else {
                let predicate = ctx.rt.get_field(root, "predicate");
                let method = ctx.rt.get_field(message, "method");
                invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Predicate",
                        "test",
                        "(Ljava/lang/Object;)Z",
                    ),
                    &[predicate, method],
                )
                .i() != 0
            };
            if ctx.rt.pending_exception.is_some() {
                return V::NULL;
            }
            if !accepted {
                continue;
            }
            let mut emitted = true;
            for stage in &stages {
                let kind = ctx
                    .rt
                    .str_of(ctx.rt.get_field(*stage, "operation"))
                    .unwrap_or("")
                    .to_owned();
                let argument = ctx.rt.get_field(*stage, "argument");
                match kind.as_str() {
                    "filter" => {
                        if invoke_member(
                            &mut ctx,
                            op::INVOKEINTERFACE,
                            &MemberRef::new(
                                "java/util/function/Predicate",
                                "test",
                                "(Ljava/lang/Object;)Z",
                            ),
                            &[argument, value],
                        )
                        .i() == 0
                        {
                            emitted = false;
                        }
                    }
                    "map" => {
                        value = invoke_member(
                            &mut ctx,
                            op::INVOKEINTERFACE,
                            &MemberRef::new(
                                "java/util/function/Function",
                                "apply",
                                "(Ljava/lang/Object;)Ljava/lang/Object;",
                            ),
                            &[argument, value],
                        );
                    }
                    "peek" => {
                        invoke_member(
                            &mut ctx,
                            op::INVOKEINTERFACE,
                            &MemberRef::new(
                                "java/util/function/Consumer",
                                "accept",
                                "(Ljava/lang/Object;)V",
                            ),
                            &[argument, value],
                        );
                    }
                    "skip" | "limit" => {
                        let seen = ctx.rt.get_field(*stage, "seen").j();
                        ctx.rt.put_field(*stage, "seen", V::J(seen + 1));
                        if kind == "skip" && seen < argument.j() {
                            emitted = false;
                        }
                    }
                    _ => {
                        ctx.rt.throw_exception(
                            "java/lang/UnsupportedOperationException",
                            "Unknown IMC stream pipeline stage",
                        );
                    }
                }
                if ctx.rt.pending_exception.is_some() {
                    return V::NULL;
                }
                if !emitted {
                    break;
                }
            }
            if emitted {
                match operation {
                    "forEach" | "forEachOrdered" => {
                        invoke_member(
                            &mut ctx,
                            op::INVOKEINTERFACE,
                            &MemberRef::new(
                                "java/util/function/Consumer",
                                "accept",
                                "(Ljava/lang/Object;)V",
                            ),
                            &[callback.unwrap(), value],
                        );
                    }
                    "count" => count += 1,
                    "toList" => values.push(value),
                    "findFirst" | "findAny" => {
                        values.push(value);
                        stop = true;
                    }
                    "anyMatch" | "allMatch" | "noneMatch" => {
                        let yes = invoke_member(
                            &mut ctx,
                            op::INVOKEINTERFACE,
                            &MemberRef::new(
                                "java/util/function/Predicate",
                                "test",
                                "(Ljava/lang/Object;)Z",
                            ),
                            &[callback.unwrap(), value],
                        )
                        .i() != 0;
                        if (operation == "allMatch" && !yes) || (operation != "allMatch" && yes) {
                            matched = Some(operation == "anyMatch");
                            stop = true;
                        }
                    }
                    _ => {
                        ctx.rt.throw_exception(
                            "java/lang/UnsupportedOperationException",
                            "Unknown IMC terminal operation",
                        );
                    }
                }
            }
            ctx.rt.pending_exception.is_none()
        };
        if !completed {
            return V::NULL;
        }
        let queue = rt.get_field(root, "queue");
        remove_message(loader, rt, queue, node);
        if stop {
            break;
        }
    }
    match operation {
        "forEach" | "forEachOrdered" => V::Void,
        "count" => V::J(count),
        "toList" => rt.alloc(
            "java/util/ImmutableCollections$ListN",
            0,
            Data::List(values),
        ),
        "findFirst" | "findAny" => {
            if values.first().is_some_and(|v| v.is_null()) {
                return rt.throw_exception(
                    "java/lang/NullPointerException",
                    "findFirst cannot hold null",
                );
            }
            rt.alloc(
                "java/util/Optional",
                0,
                Data::Optional(values.first().copied()),
            )
        }
        "anyMatch" | "allMatch" | "noneMatch" => {
            V::bool(matched.unwrap_or(operation != "anyMatch"))
        }
        _ => rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Unknown IMC terminal operation",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{LoadedMod, PreparedLoad};
    use mmga_classfile::{op, MemberRef};
    use mmga_runtime::{
        invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, NativeModuleRegistry, Rt, V,
    };

    const PROCESS: &str = "net/neoforged/fml/event/lifecycle/InterModProcessEvent";
    const COMMS: &str = "net/neoforged/fml/InterModComms";
    const STREAM: &str = "java/util/stream/Stream";
    fn fixture() -> PreparedLoad {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        prepared.loader.mods = ["sender", "recipient", "other"]
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
            .linked
            .register_native_module(NativeModuleRegistry {
                classes: &[],
                lambdas: &[
                    (401, keep),
                    (402, supply),
                    (403, fail_consumer),
                    (404, fail_predicate),
                    (405, reject_message),
                    (406, fail_supplier),
                    (407, force_supplier),
                    (408, append_once),
                ],
            });
        prepared
    }
    fn observer(rt: &mut Rt) -> V {
        rt.new_instance("test/ImcObserver", 0)
    }
    fn keep(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "predicates").i();
        ctx.rt.put_field(args[0], "predicates", V::I(count + 1));
        assert_eq!(
            ctx.rt.class_of(args[1]),
            Some("java/lang/String"),
            "IMC matcher receives METHOD, not message"
        );
        V::bool(ctx.rt.str_of(args[1]) == Some("keep"))
    }
    fn supply(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "suppliers").i();
        ctx.rt.put_field(args[0], "suppliers", V::I(count + 1));
        ctx.rt.get_field(args[0], "payload")
    }
    fn fail_consumer(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("test/ImcConsumerFailure", "consumer failed")
    }
    fn fail_predicate(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("test/ImcPredicateFailure", "predicate failed")
    }
    fn reject_message(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        assert_eq!(
            ctx.rt.class_of(args[1]),
            Some("net/neoforged/fml/InterModComms$IMCMessage")
        );
        let count = ctx.rt.get_field(args[0], "downstream").i();
        ctx.rt.put_field(args[0], "downstream", V::I(count + 1));
        V::FALSE
    }
    fn fail_supplier(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("test/ImcSupplierFailure", "supplier failed")
    }
    fn force_supplier(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let supplier = invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "net/neoforged/fml/InterModComms$IMCMessage",
                "messageSupplier",
                "()Ljava/util/function/Supplier;",
            ),
            &[args[1]],
        );
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
            &[supplier],
        )
    }
    fn append_once(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "visited").i();
        ctx.rt.put_field(args[0], "visited", V::I(count + 1));
        if count == 0 {
            let sender = ctx.rt.get_field(args[0], "sender");
            let recipient = ctx.rt.get_field(args[0], "recipient");
            let method = ctx.rt.get_field(args[0], "method");
            send(ctx, sender, recipient, method, V::NULL);
        }
        V::Void
    }
    fn closure(rt: &mut Rt, interface: &str, name: &str, desc: &str, observer: V, id: u32) -> V {
        rt.alloc(
            interface,
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: interface.into(),
                sam_name: name.into(),
                sam_desc: desc.into(),
                instantiated_desc: desc.into(),
                captures: vec![observer],
                target: ClosureTarget::NativeLambda(id),
            })),
        )
    }
    fn send(ctx: &mut ExecCtx<'_>, sender: V, recipient: V, method: V, supplier: V) -> V {
        invoke_member(ctx,op::INVOKESTATIC,&MemberRef::new(COMMS,"sendTo","(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/util/function/Supplier;)Z"),&[sender,recipient,method,supplier])
    }
    fn stream(ctx: &mut ExecCtx<'_>, event: V, predicate: V) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                PROCESS,
                "getIMCStream",
                "(Ljava/util/function/Predicate;)Ljava/util/stream/Stream;",
            ),
            &[event, predicate],
        )
    }
    fn list(ctx: &mut ExecCtx<'_>, stream: V) -> V {
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "toList", "()Ljava/util/List;"),
            &[stream],
        )
    }
    #[test]
    fn imc_stream_is_lazy_uses_event_recipient_and_only_removes_matching_messages_on_consumption() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let seen = observer(&mut rt);
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            401,
        );
        let supplier = closure(
            &mut rt,
            "java/util/function/Supplier",
            "get",
            "()Ljava/lang/Object;",
            seen,
            402,
        );
        let payload = rt.new_instance("test/Payload", 0);
        rt.put_field(seen, "payload", payload);
        let sender = rt.new_str("sender");
        let recipient = rt.new_str("recipient");
        let other = rt.new_str("other");
        let keep = rt.new_str("keep");
        let ignore = rt.new_str("ignore");
        let event = rt.new_instance(PROCESS, 0);
        let container = prepared.loader.mod_container_handle(&mut rt, "recipient");
        rt.put_field(event, "container", container);
        prepared.loader.constructing = Some("other".into());
        let value = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            assert_eq!(send(&mut ctx, sender, recipient, keep, supplier), V::TRUE);
            send(&mut ctx, sender, recipient, ignore, supplier);
            send(&mut ctx, sender, other, keep, supplier);
            let value = stream(&mut ctx, event, predicate);
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(ctx.rt.get_field(seen, "predicates").i(), 0);
            assert_eq!(ctx.rt.get_field(seen, "suppliers").i(), 0);
            value
        };
        assert_eq!(prepared.loader.imc_messages.len(), 3);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let values = list(&mut ctx, value);
        assert!(ctx.rt.pending_exception.is_none());
        let Some(Data::List(values)) = ctx.rt.get_obj(values).map(|o| &o.data) else {
            panic!("IMC stream did not return a list")
        };
        assert_eq!(values.len(), 1);
        let message = values[0];
        assert_eq!(ctx.rt.get_field(message, "senderModId"), sender);
        assert_eq!(ctx.rt.get_field(message, "modId"), recipient);
        assert_eq!(ctx.rt.get_field(message, "method"), keep);
        let got = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "net/neoforged/fml/InterModComms$IMCMessage",
                "messageSupplier",
                "()Ljava/util/function/Supplier;",
            ),
            &[message],
        );
        assert_eq!(got, supplier);
        assert_eq!(ctx.rt.get_field(seen, "suppliers").i(), 0);
        assert_eq!(ctx.rt.get_field(seen, "predicates"), V::I(2));
        assert_eq!(
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
                &[got]
            ),
            payload
        );
        assert_eq!(prepared.loader.imc_messages.len(), 2);
    }
    #[test]
    fn imc_predicate_or_terminal_consumer_failure_keeps_current_message_queued() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let seen = observer(&mut rt);
        let sender = rt.new_str("sender");
        let recipient = rt.new_str("recipient");
        let method = rt.new_str("keep");
        let event = rt.new_instance(PROCESS, 0);
        let container = prepared.loader.mod_container_handle(&mut rt, "recipient");
        rt.put_field(event, "container", container);
        let failure = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            404,
        );
        let keep = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            401,
        );
        let consumer = closure(
            &mut rt,
            "java/util/function/Consumer",
            "accept",
            "(Ljava/lang/Object;)V",
            seen,
            403,
        );
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            send(&mut ctx, sender, recipient, method, V::NULL);
            let value = stream(&mut ctx, event, failure);
            list(&mut ctx, value);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(ctx.rt.class_of(error), Some("test/ImcPredicateFailure"));
        }
        assert_eq!(prepared.loader.imc_messages.len(), 1);
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            let value = stream(&mut ctx, event, keep);
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(STREAM, "forEach", "(Ljava/util/function/Consumer;)V"),
                &[value, consumer],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(ctx.rt.class_of(error), Some("test/ImcConsumerFailure"));
            list(&mut ctx, value);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/IllegalStateException")
            );
        }
        assert_eq!(prepared.loader.imc_messages.len(), 1);
    }
    #[test]
    fn empty_queue_stream_never_invokes_predicate_or_supplier_and_remains_empty_after_later_send() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let seen = observer(&mut rt);
        let sender = rt.new_str("sender");
        let recipient = rt.new_str("recipient");
        let method = rt.new_str("keep");
        let event = rt.new_instance(PROCESS, 0);
        let container = prepared.loader.mod_container_handle(&mut rt, "recipient");
        rt.put_field(event, "container", container);
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            404,
        );
        let supplier = closure(
            &mut rt,
            "java/util/function/Supplier",
            "get",
            "()Ljava/lang/Object;",
            seen,
            402,
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let empty = stream(&mut ctx, event, predicate);
        assert!(ctx.rt.pending_exception.is_none());
        send(&mut ctx, sender, recipient, method, supplier);
        let count = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "count", "()J"),
            &[empty],
        );
        assert_eq!(count, V::J(0));
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(seen, "suppliers").i(), 0);
        assert_eq!(prepared.loader.imc_messages.len(), 1);
    }
    #[test]
    fn downstream_filter_rejection_still_drains_but_supplier_failure_after_to_list_is_already_drained(
    ) {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let seen = observer(&mut rt);
        let sender = rt.new_str("sender");
        let recipient = rt.new_str("recipient");
        let method = rt.new_str("keep");
        let event = rt.new_instance(PROCESS, 0);
        let container = prepared.loader.mod_container_handle(&mut rt, "recipient");
        rt.put_field(event, "container", container);
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            401,
        );
        let reject = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            405,
        );
        let supplier = closure(
            &mut rt,
            "java/util/function/Supplier",
            "get",
            "()Ljava/lang/Object;",
            seen,
            406,
        );
        let consumer = closure(
            &mut rt,
            "java/util/function/Consumer",
            "accept",
            "(Ljava/lang/Object;)V",
            seen,
            407,
        );
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            send(&mut ctx, sender, recipient, method, supplier);
            let source = stream(&mut ctx, event, predicate);
            let filtered = invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(
                    STREAM,
                    "filter",
                    "(Ljava/util/function/Predicate;)Ljava/util/stream/Stream;",
                ),
                &[source, reject],
            );
            assert_eq!(ctx.rt.get_field(seen, "downstream").i(), 0);
            let result = list(&mut ctx, filtered);
            assert!(ctx.rt.pending_exception.is_none());
            assert!(
                matches!(&ctx.rt.get_obj(result).unwrap().data,Data::List(values) if values.is_empty())
            );
            assert_eq!(ctx.rt.get_field(seen, "downstream"), V::I(1));
        }
        assert!(prepared.loader.imc_messages.is_empty());
        let result = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            send(&mut ctx, sender, recipient, method, supplier);
            let source = stream(&mut ctx, event, predicate);
            list(&mut ctx, source)
        };
        assert!(prepared.loader.imc_messages.is_empty());
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "java/util/List",
                "forEach",
                "(Ljava/util/function/Consumer;)V",
            ),
            &[result, consumer],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("test/ImcSupplierFailure"));
        assert!(prepared.loader.imc_messages.is_empty());
    }
    #[test]
    fn live_iterator_sees_appends_before_successor_prefetch_exhausts_but_not_after() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let seen = observer(&mut rt);
        let sender = rt.new_str("sender");
        let recipient = rt.new_str("recipient");
        let method = rt.new_str("keep");
        rt.put_field(seen, "sender", sender);
        rt.put_field(seen, "recipient", recipient);
        rt.put_field(seen, "method", method);
        let event = rt.new_instance(PROCESS, 0);
        let container = prepared.loader.mod_container_handle(&mut rt, "recipient");
        rt.put_field(event, "container", container);
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            seen,
            401,
        );
        let append = closure(
            &mut rt,
            "java/util/function/Consumer",
            "accept",
            "(Ljava/lang/Object;)V",
            seen,
            408,
        );
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            send(&mut ctx, sender, recipient, method, V::NULL);
            send(&mut ctx, sender, recipient, method, V::NULL);
            let source = stream(&mut ctx, event, predicate);
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(STREAM, "forEach", "(Ljava/util/function/Consumer;)V"),
                &[source, append],
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(ctx.rt.get_field(seen, "visited"), V::I(3));
        }
        assert!(prepared.loader.imc_messages.is_empty());
        rt.put_field(seen, "visited", V::I(0));
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            send(&mut ctx, sender, recipient, method, V::NULL);
            let source = stream(&mut ctx, event, predicate);
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(STREAM, "forEach", "(Ljava/util/function/Consumer;)V"),
                &[source, append],
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(ctx.rt.get_field(seen, "visited"), V::I(1));
        }
        assert_eq!(prepared.loader.imc_messages.len(), 1);
    }
    #[test]
    fn unloaded_aliases_are_rejected_nullable_record_values_survive_and_to_list_is_immutable() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let sender = rt.new_str("sender");
        let recipient = rt.new_str("recipient");
        let method = rt.new_str("keep");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        for target in ["forge", "fml", "missing"] {
            let target = ctx.rt.new_str(target);
            assert_eq!(send(&mut ctx, sender, target, method, V::NULL), V::FALSE);
            let mods = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    "net/neoforged/fml/ModList",
                    "get",
                    "()Lnet/neoforged/fml/ModList;",
                ),
                &[],
            );
            assert_eq!(
                invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(
                        "net/neoforged/fml/ModList",
                        "isLoaded",
                        "(Ljava/lang/String;)Z"
                    ),
                    &[mods, target]
                ),
                V::FALSE
            );
        }
        assert_eq!(
            send(&mut ctx, V::NULL, recipient, V::NULL, V::NULL),
            V::TRUE
        );
        let source = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                COMMS,
                "getMessages",
                "(Ljava/lang/String;)Ljava/util/stream/Stream;",
            ),
            &[recipient],
        );
        let values = list(&mut ctx, source);
        let Some(Data::List(records)) = ctx.rt.get_obj(values).map(|o| &o.data) else {
            panic!("expected list")
        };
        assert_eq!(records.len(), 1);
        let record = records[0];
        for (name, desc) in [
            ("senderModId", "()Ljava/lang/String;"),
            ("method", "()Ljava/lang/String;"),
            ("messageSupplier", "()Ljava/util/function/Supplier;"),
        ] {
            assert_eq!(
                invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("net/neoforged/fml/InterModComms$IMCMessage", name, desc),
                    &[record]
                ),
                V::NULL
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
        for (name, desc, args) in [
            ("add", "(Ljava/lang/Object;)Z", vec![record]),
            ("clear", "()V", vec![]),
            (
                "set",
                "(ILjava/lang/Object;)Ljava/lang/Object;",
                vec![V::I(0), record],
            ),
        ] {
            let mut call_args = vec![values];
            call_args.extend(args);
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/util/List", name, desc),
                &call_args,
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/UnsupportedOperationException")
            );
        }
        assert!(prepared.loader.imc_messages.is_empty());
    }
}

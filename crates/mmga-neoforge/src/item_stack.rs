//! ItemStack values and JSON codecs without fabricating missing registry items or components.
//! The canonical empty stack has a null backing item and empty component maps, as in 1.21.1.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace, Rt, V};
use serde_json::Value as Json;

const STACK: &str = "net/minecraft/world/item/ItemStack";
const COMPONENTS: &str = "net/minecraft/core/component/DataComponentMap";
const EMPTY_COMPONENTS: &str = "net/minecraft/core/component/DataComponentMap$1";
const PATCH: &str = "net/minecraft/core/component/DataComponentPatch";

const PATCHED: &str = "net/minecraft/core/component/PatchedDataComponentMap";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    let valid = (owner == STACK
        && matches!(
            (name, desc),
            ("EMPTY", "Lnet/minecraft/world/item/ItemStack;")
                | (
                    "CODEC" | "OPTIONAL_CODEC",
                    "Lcom/mojang/serialization/Codec;"
                )
        ))
        || (owner == COMPONENTS
            && name == "EMPTY"
            && desc == "Lnet/minecraft/core/component/DataComponentMap;")
        || (owner == PATCH
            && name == "EMPTY"
            && desc == "Lnet/minecraft/core/component/DataComponentPatch;");
    if !valid {
        return None;
    }
    if let Some(v) = rt.get_static(owner, name) {
        return Some(v);
    }
    let value = if owner == STACK && name == "EMPTY" {
        let stack = rt.new_instance(STACK, 0);
        rt.put_field(stack, "item", V::NULL);
        rt.put_field(stack, "count", V::I(0));
        rt.put_field(stack, "popTime", V::I(0));
        let prototype = empty_components(rt);
        let components = patched(rt, prototype);
        rt.put_field(stack, "components", components);
        stack
    } else if owner == STACK {
        crate::dfu::item_stack_codec(rt, name == "OPTIONAL_CODEC")
    } else if owner == COMPONENTS {
        rt.new_instance(EMPTY_COMPONENTS, 0)
    } else {
        rt.new_instance(PATCH, 0)
    };
    rt.put_static(owner, name, value);
    Some(value)
}
pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if matches!(owner, COMPONENTS | EMPTY_COMPONENTS | PATCHED | PATCH) && !is_static {
        let recv = *args.first()?;
        let known_empty = recv == empty_components(rt)
            || recv == empty_patch(rt)
            || (rt.class_of(recv) == Some(PATCHED)
                && rt.get_field(recv, "$prototype") == empty_components(rt));
        if !known_empty {
            return None;
        }
        return match(name,desc){
            ("get","(Lnet/minecraft/core/component/DataComponentType;)Ljava/lang/Object;")=>Some(V::NULL),
            ("getOrDefault","(Lnet/minecraft/core/component/DataComponentType;Ljava/lang/Object;)Ljava/lang/Object;")=>Some(args[2]),
            ("has","(Lnet/minecraft/core/component/DataComponentType;)Z")=>Some(V::FALSE),
            ("size","()I")=>Some(V::I(0)),("isEmpty","()Z")=>Some(V::TRUE),
            ("asPatch","()Lnet/minecraft/core/component/DataComponentPatch;")=>Some(empty_patch(rt)),
            ("copy","()Lnet/minecraft/core/component/PatchedDataComponentMap;")=>{let prototype=rt.get_field(recv,"$prototype");Some(patched(rt,prototype))},
            _=>None,
        };
    }
    if owner != STACK {
        return None;
    }
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    match (name, desc) {
        ("<init>", "(Ljava/lang/Void;)V") => {
            let prototype = empty_components(ctx.rt);
            let components = patched(ctx.rt, prototype);
            ctx.rt.put_field(recv, "item", V::NULL);
            ctx.rt.put_field(recv, "count", V::I(0));
            ctx.rt.put_field(recv, "components", components);
            Some(V::Void)
        }
        (
            "<init>",
            "(Lnet/minecraft/world/level/ItemLike;)V" | "(Lnet/minecraft/world/level/ItemLike;I)V",
        ) => {
            let item = invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(
                    "net/minecraft/world/level/ItemLike",
                    "asItem",
                    "()Lnet/minecraft/world/item/Item;",
                ),
                &[args[1]],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let count = args.get(2).copied().unwrap_or(V::I(1)).i();
            Some(construct(&mut ctx, recv, item, count))
        }
        ("isEmpty", "()Z") => Some(V::bool(is_empty(ctx.rt, recv))),
        ("getCount", "()I") => Some(V::I(if is_empty(ctx.rt, recv) {
            0
        } else {
            ctx.rt.get_field(recv, "count").i()
        })),
        ("setCount", "(I)V") => {
            ctx.rt.put_field(recv, "count", args[1]);
            Some(V::Void)
        }
        ("getPopTime", "()I") => Some(ctx.rt.get_field(recv, "popTime")),
        ("setPopTime", "(I)V") => {
            ctx.rt.put_field(recv, "popTime", args[1]);
            Some(V::Void)
        }
        ("getItem", "()Lnet/minecraft/world/item/Item;") => Some(if is_empty(ctx.rt, recv) {
            mmga_runtime::read_static_field(
                &mut ctx,
                "net/minecraft/world/item/Items",
                "AIR",
                "Lnet/minecraft/world/item/Item;",
            )
        } else {
            ctx.rt.get_field(recv, "item")
        }),
        ("getComponents" | "getPrototype", "()Lnet/minecraft/core/component/DataComponentMap;") => {
            Some(if is_empty(ctx.rt, recv) {
                empty_components(ctx.rt)
            } else if name == "getComponents" {
                ctx.rt.get_field(recv, "components")
            } else {
                let components = ctx.rt.get_field(recv, "components");
                ctx.rt.get_field(components, "$prototype")
            })
        }
        ("getComponentsPatch", "()Lnet/minecraft/core/component/DataComponentPatch;") => {
            if is_empty(ctx.rt, recv) {
                Some(empty_patch(ctx.rt))
            } else {
                let components = ctx.rt.get_field(recv, "components");
                if ctx.rt.get_field(components, "$prototype") == empty_components(ctx.rt) {
                    Some(empty_patch(ctx.rt))
                } else {
                    None
                }
            }
        }
        ("copy", "()Lnet/minecraft/world/item/ItemStack;")
        | ("copyWithCount", "(I)Lnet/minecraft/world/item/ItemStack;") => {
            if is_empty(ctx.rt, recv) {
                return Some(empty_stack(ctx.rt));
            }
            let item = ctx.rt.get_field(recv, "item");
            let count = ctx.rt.get_field(recv, "count").i();
            let clone = ctx.rt.new_instance(STACK, 0);
            construct(&mut ctx, clone, item, count);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let time = ctx.rt.get_field(recv, "popTime");
            ctx.rt.put_field(clone, "popTime", time);
            if name == "copyWithCount" {
                ctx.rt.put_field(clone, "count", args[1]);
            }
            Some(clone)
        }
        _ => None,
    }
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == STACK && sup == "net/minecraft/core/component/DataComponentHolder")
        || (matches!(sub, EMPTY_COMPONENTS | PATCHED) && sup == COMPONENTS)
}

fn empty_stack(rt: &mut Rt) -> V {
    get_static_field(rt, STACK, "EMPTY", "Lnet/minecraft/world/item/ItemStack;").unwrap()
}
fn empty_components(rt: &mut Rt) -> V {
    get_static_field(
        rt,
        COMPONENTS,
        "EMPTY",
        "Lnet/minecraft/core/component/DataComponentMap;",
    )
    .unwrap()
}
fn empty_patch(rt: &mut Rt) -> V {
    get_static_field(
        rt,
        PATCH,
        "EMPTY",
        "Lnet/minecraft/core/component/DataComponentPatch;",
    )
    .unwrap()
}
fn patched(rt: &mut Rt, prototype: V) -> V {
    let map = rt.new_instance(PATCHED, 0);
    rt.put_field(map, "$prototype", prototype);
    map
}
fn is_empty(rt: &Rt, stack: V) -> bool {
    rt.get_static(STACK, "EMPTY") == Some(stack)
        || rt.get_field(stack, "count").i() <= 0
        || rt
            .get_static("net/minecraft/world/item/Items", "AIR")
            .is_some_and(|air| rt.get_field(stack, "item") == air)
}
fn construct(ctx: &mut ExecCtx<'_>, stack: V, item: V, count: i32) -> V {
    let prototype = invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new(
            "net/minecraft/world/item/Item",
            "components",
            "()Lnet/minecraft/core/component/DataComponentMap;",
        ),
        &[item],
    );
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    if prototype != empty_components(ctx.rt) {
        return ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Nonempty Item component prototypes are not implemented",
        );
    }
    let components = patched(ctx.rt, prototype);
    ctx.rt.put_field(stack, "item", item);
    ctx.rt.put_field(stack, "count", V::I(count));
    ctx.rt.put_field(stack, "popTime", V::I(0));
    ctx.rt.put_field(stack, "components", components);
    invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new(
            "net/minecraft/world/item/Item",
            "verifyComponentsAfterLoad",
            "(Lnet/minecraft/world/item/ItemStack;)V",
        ),
        &[item, stack],
    )
}

pub fn decode_json(ctx: &mut ExecCtx<'_>, input: &Json, optional: bool) -> Result<V, String> {
    let map = input
        .as_object()
        .ok_or_else(|| format!("Not an ItemStack map: {input}"))?;
    if optional && map.is_empty() {
        return Ok(empty_stack(ctx.rt));
    }
    let id = map
        .get("id")
        .and_then(Json::as_str)
        .ok_or("ItemStack requires string id")?;
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let namespace = if namespace.is_empty() {
        "minecraft"
    } else {
        namespace
    };
    if !namespace
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        || !path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
    {
        return Err(format!("Invalid item id {id}"));
    }
    let id = format!("{namespace}:{path}");
    if id == "minecraft:air" {
        return Err("Item must not be minecraft:air".into());
    }
    let location = ctx.rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        &id,
    );
    let registry = ctx.rt.new_pinned_handle(
        "net/minecraft/core/Registry",
        "registry",
        0,
        "minecraft:item",
    );
    let item = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            "net/minecraft/core/Registry",
            "get",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
        ),
        &[registry, location],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Registry lookup failed".into());
    }
    if item.is_null() {
        if namespace == "minecraft" {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Vanilla item {id} is not materialized in this host"),
            );
        }
        return Err(format!("Unknown item {id}"));
    }
    if let Some(patch) = map.get("components").filter(|v| !v.is_null()) {
        let Some(patch) = patch.as_object() else {
            return Err("ItemStack components must be a map".into());
        };
        if !patch.is_empty() {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "ItemStack component patch decoding is not implemented",
            );
            return Err("Unsupported component patch".into());
        }
    }
    // Minecraft's count field applies MapCodec.orElse(1), including malformed or out-of-range data.
    let count = map
        .get("count")
        .and_then(|n| crate::dfu::decode_json_int(ctx.rt, n).ok())
        .filter(|n| (1..=99).contains(n))
        .unwrap_or(1);
    let stack = ctx.rt.new_instance(STACK, 0);
    construct(ctx, stack, item, count);
    if ctx.rt.pending_exception.is_some() {
        return Err("ItemStack construction failed".into());
    }
    Ok(stack)
}
pub fn encode_json(ctx: &mut ExecCtx<'_>, value: V, optional: bool) -> Result<Json, String> {
    if ctx.rt.class_of(value) != Some(STACK) {
        return Err("Not an ItemStack".into());
    }
    if is_empty(ctx.rt, value) {
        return if optional {
            Ok(serde_json::json!({}))
        } else {
            Err("Item must not be minecraft:air".into())
        };
    }
    let item = ctx.rt.get_field(value, "item");
    let registry = ctx.rt.new_pinned_handle(
        "net/minecraft/core/Registry",
        "registry",
        0,
        "minecraft:item",
    );
    let id = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            "net/minecraft/core/Registry",
            "getKey",
            "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;",
        ),
        &[registry, item],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Registry lookup failed".into());
    }
    let id = ctx
        .rt
        .handle_of(id)
        .filter(|(kind, ..)| *kind == "resource_location")
        .map(|(_, _, id, _)| id.to_owned())
        .ok_or("Unregistered item")?;
    let count = ctx.rt.get_field(value, "count").i();
    if !(1..=99).contains(&count) {
        return Err(format!("ItemStack count {count} outside [1,99]"));
    }
    let components = ctx.rt.get_field(value, "components");
    if ctx.rt.get_field(components, "$prototype") != empty_components(ctx.rt) {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ItemStack component patch encoding is not implemented",
        );
        return Err("Unknown component map".into());
    }
    Ok(serde_json::json!({"id":id,"count":count}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::NullHost;
    #[test]
    fn empty_stack_is_canonical_and_count_mutation_cannot_make_it_nonempty() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let empty = get_static_field(
            &mut rt,
            STACK,
            "EMPTY",
            "Lnet/minecraft/world/item/ItemStack;",
        )
        .unwrap();
        assert_eq!(
            get_static_field(
                &mut rt,
                STACK,
                "EMPTY",
                "Lnet/minecraft/world/item/ItemStack;"
            ),
            Some(empty)
        );
        call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(STACK, "setCount", "(I)V"),
            false,
            &[empty, V::I(64)],
        )
        .unwrap();
        assert_eq!(
            call(
                &mut rt,
                &linked,
                &mut host,
                &MemberRef::new(STACK, "isEmpty", "()Z"),
                false,
                &[empty]
            ),
            Some(V::TRUE)
        );
        assert_eq!(
            call(
                &mut rt,
                &linked,
                &mut host,
                &MemberRef::new(STACK, "getCount", "()I"),
                false,
                &[empty]
            ),
            Some(V::I(0))
        );
        assert_eq!(
            call(
                &mut rt,
                &linked,
                &mut host,
                &MemberRef::new(STACK, "copy", "()Lnet/minecraft/world/item/ItemStack;"),
                false,
                &[empty]
            ),
            Some(empty)
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(
            decode_json(&mut ctx, &serde_json::json!({}), true).unwrap(),
            empty
        );
        assert_eq!(
            encode_json(&mut ctx, empty, true).unwrap(),
            serde_json::json!({})
        );
        assert!(encode_json(&mut ctx, empty, false).is_err());
    }
    fn prototype(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        empty_components(ctx.rt)
    }
    fn verify(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "verifyCalls").i();
        ctx.rt.put_field(args[0], "verifyCalls", V::I(calls + 1));
        V::Void
    }
    #[test]
    fn codec_uses_actual_registered_item_prototype_and_preserves_count() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        prepared
            .linked
            .register_native_module(mmga_runtime::NativeModuleRegistry {
                classes: &[mmga_runtime::NativeClassDef {
                    name: "test/Item",
                    superclass: Some("net/minecraft/world/item/Item"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[
                        (
                            "components.()Lnet/minecraft/core/component/DataComponentMap;",
                            prototype,
                            false,
                            false,
                        ),
                        (
                            "verifyComponentsAfterLoad.(Lnet/minecraft/world/item/ItemStack;)V",
                            verify,
                            false,
                            false,
                        ),
                    ],
                }],
                lambdas: &[],
            });
        let mut rt = Rt::new();
        let item = rt.new_instance("test/Item", 0);
        prepared.loader.state.registries.register_direct(
            &mut rt,
            "minecraft:item",
            "example:actual",
            "example",
            item,
            crate::ContentProperties::default(),
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let stack = decode_json(
            &mut ctx,
            &serde_json::json!({"id":"example:actual","count":3}),
            false,
        )
        .unwrap();
        assert_eq!(ctx.rt.get_field(stack, "item"), item);
        assert_eq!(ctx.rt.get_field(stack, "count"), V::I(3));
        assert_eq!(ctx.rt.get_field(item, "verifyCalls"), V::I(1));
        assert_eq!(
            encode_json(&mut ctx, stack, false).unwrap(),
            serde_json::json!({"id":"example:actual","count":3})
        );
        let defaulted = decode_json(
            &mut ctx,
            &serde_json::json!({"id":"example:actual","count":0}),
            false,
        )
        .unwrap();
        assert_eq!(ctx.rt.get_field(defaulted, "count"), V::I(1));
        let truncated = decode_json(
            &mut ctx,
            &serde_json::json!({"id":"example:actual","count":4.9}),
            false,
        )
        .unwrap();
        assert_eq!(ctx.rt.get_field(truncated, "count"), V::I(4));
        assert!(decode_json(
            &mut ctx,
            &serde_json::json!({"id":"example:missing"}),
            false
        )
        .is_err());
        assert!(ctx.rt.pending_exception.is_none());
        assert!(decode_json(
            &mut ctx,
            &serde_json::json!({"id":"example:actual","components":{"example:data":1}}),
            false
        )
        .is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

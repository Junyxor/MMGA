//! NeoForge FluidStack/SimpleFluidContent values and optional codecs.
//! Verified against official 1.21.1 FluidStack dffcdf187fba85aaaec6f8d6a62559b9674625a9
//! and SimpleFluidContent 163293d06be1bcc4305c37348e5ddf1a8011bf1b source blobs.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace, Rt, V,
};
use serde_json::Value as Json;

const FLUID_STACK: &str = "net/neoforged/neoforge/fluids/FluidStack";
const CONTENT: &str = "net/neoforged/neoforge/fluids/SimpleFluidContent";
const CALLBACK: &str = "mmga/internal/FluidContentCodec";
const STREAM: &str = "net/minecraft/network/codec/StreamCodec";
const PATCHED: &str = "net/minecraft/core/component/PatchedDataComponentMap";

const COMPONENTS: &str = "net/minecraft/core/component/DataComponentMap";
const PATCH: &str = "net/minecraft/core/component/DataComponentPatch";
const FLUIDS: &str = "net/minecraft/world/level/material/Fluids";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    let valid = (owner == FLUID_STACK
        && matches!(
            (name, desc),
            ("EMPTY", "Lnet/neoforged/neoforge/fluids/FluidStack;")
                | (
                    "CODEC" | "OPTIONAL_CODEC",
                    "Lcom/mojang/serialization/Codec;"
                )
                | (
                    "STREAM_CODEC" | "OPTIONAL_STREAM_CODEC",
                    "Lnet/minecraft/network/codec/StreamCodec;"
                )
        ))
        || (owner == CONTENT
            && matches!(
                (name, desc),
                (
                    "EMPTY",
                    "Lnet/neoforged/neoforge/fluids/SimpleFluidContent;"
                ) | ("CODEC", "Lcom/mojang/serialization/Codec;")
                    | ("STREAM_CODEC", "Lnet/minecraft/network/codec/StreamCodec;")
            ));
    if !valid {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let content = owner == CONTENT;
    let optional = content || name.starts_with("OPTIONAL_");
    let value = if name == "EMPTY" {
        if content {
            let stack = empty_stack(rt);
            wrap_content(rt, stack)
        } else {
            let stack = rt.new_instance(FLUID_STACK, 0);
            initialize_stack(rt, stack, V::NULL, 0);
            stack
        }
    } else if name.ends_with("STREAM_CODEC") {
        stream_codec(rt, optional, content)
    } else {
        crate::dfu::fluid_stack_codec(rt, optional, content)
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
    let owner = if owner == "java/lang/Object"
        && !is_static
        && args
            .first()
            .is_some_and(|recv| rt.class_of(*recv) == Some(CONTENT))
        && matches!(
            (name, desc),
            ("equals", "(Ljava/lang/Object;)Z") | ("hashCode", "()I")
        ) {
        CONTENT
    } else {
        owner
    };
    if !matches!(owner, FLUID_STACK | CONTENT | CALLBACK) {
        return None;
    }
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    if owner == CALLBACK && is_static {
        return match (name, desc) {
            ("encode", "(ZZLjava/lang/Object;Ljava/lang/Object;)V") => Some(encode_wire(
                &mut ctx,
                args[2],
                args[3],
                args[0] == V::TRUE,
                args[1] == V::TRUE,
            )),
            ("decode", "(ZZLjava/lang/Object;)Ljava/lang/Object;") => Some(decode_wire(
                &mut ctx,
                args[2],
                args[0] == V::TRUE,
                args[1] == V::TRUE,
            )),
            _ => None,
        };
    }
    if owner == CONTENT {
        if is_static {
            return match (name, desc) {
                ("copyOf", "(Lnet/neoforged/neoforge/fluids/FluidStack;)Lnet/neoforged/neoforge/fluids/SimpleFluidContent;") => {
                    if !check_class(ctx.rt, args[0], FLUID_STACK) { return Some(V::NULL); }
                    if is_empty(ctx.rt, args[0]) { Some(empty_content(ctx.rt)) } else {
                        let copied = copy_stack(&mut ctx, args[0], None);
                        Some(if ctx.rt.pending_exception.is_some() { V::NULL } else { wrap_content(ctx.rt, copied) })
                    }
                }
                _ => None,
            };
        }
        let recv = *args.first()?;
        if name == "<init>" && desc == "(Lnet/neoforged/neoforge/fluids/FluidStack;)V" {
            ctx.rt.put_field(recv, "fluidStack", args[1]);
            return Some(V::Void);
        }
        let stack = ctx.rt.get_field(recv, "fluidStack");
        return match (name, desc) {
            // SimpleFluidContent overrides Object equality/hashCode.  Its equality is
            // FluidStack.matches, including amount. Keep this path ahead
            // of Object's identity fallback; unknown component maps remain explicit errors.
            ("equals", "(Ljava/lang/Object;)Z") => {
                if recv == args[1] {
                    Some(V::TRUE)
                } else if args[1].is_null() || ctx.rt.class_of(args[1]) != Some(CONTENT) {
                    Some(V::FALSE)
                } else {
                    let other = ctx.rt.get_field(args[1], "fluidStack");
                    Some(fluid_content_matches(&mut ctx, stack, other))
                }
            }
            ("hashCode", "()I") => Some(fluid_content_hash(&mut ctx, stack)),
            ("copy", "()Lnet/neoforged/neoforge/fluids/FluidStack;") => {
                Some(copy_stack(&mut ctx, stack, None))
            }
            ("isEmpty", "()Z") => Some(V::bool(is_empty(ctx.rt, stack))),
            ("getAmount", "()I") => Some(V::I(amount(ctx.rt, stack))),
            ("getComponents", "()Lnet/minecraft/core/component/DataComponentMap;") => {
                Some(ctx.rt.get_field(stack, "components"))
            }
            ("getFluid", "()Lnet/minecraft/world/level/material/Fluid;") => {
                Some(get_fluid(&mut ctx, stack))
            }
            _ => None,
        };
    }
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    match (name, desc) {
        ("<init>", "(Ljava/lang/Void;)V") => {
            initialize_stack(ctx.rt, recv, V::NULL, 0);
            Some(V::Void)
        }
        ("<init>", "(Lnet/minecraft/world/level/material/Fluid;I)V") => {
            initialize_stack(ctx.rt, recv, args[1], args[2].i());
            Some(V::Void)
        }
        ("<init>", "(Lnet/minecraft/core/Holder;I)V") => {
            let fluid = invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("net/minecraft/core/Holder", "value", "()Ljava/lang/Object;"),
                &[args[1]],
            );
            if ctx.rt.pending_exception.is_none() {
                initialize_stack(ctx.rt, recv, fluid, args[2].i());
            }
            Some(if ctx.rt.pending_exception.is_some() {
                V::NULL
            } else {
                V::Void
            })
        }
        ("isEmpty", "()Z") => Some(V::bool(is_empty(ctx.rt, recv))),
        ("getAmount", "()I") => Some(V::I(amount(ctx.rt, recv))),
        ("setAmount", "(I)V") => {
            ctx.rt.put_field(recv, "amount", args[1]);
            Some(V::Void)
        }
        ("getFluid", "()Lnet/minecraft/world/level/material/Fluid;") => {
            Some(get_fluid(&mut ctx, recv))
        }
        ("getComponents", "()Lnet/minecraft/core/component/PatchedDataComponentMap;")
        | ("getComponents", "()Lnet/minecraft/core/component/DataComponentMap;") => {
            Some(ctx.rt.get_field(recv, "components"))
        }
        ("getComponentsPatch", "()Lnet/minecraft/core/component/DataComponentPatch;") => {
            if is_empty(ctx.rt, recv) || has_empty_components(ctx.rt, recv) {
                Some(empty_patch(ctx.rt))
            } else {
                None
            }
        }
        ("isComponentsPatchEmpty", "()Z") => {
            if is_empty(ctx.rt, recv) || has_empty_components(ctx.rt, recv) {
                Some(V::TRUE)
            } else {
                None
            }
        }
        ("copy", "()Lnet/neoforged/neoforge/fluids/FluidStack;") => {
            Some(copy_stack(&mut ctx, recv, None))
        }
        ("copyWithAmount", "(I)Lnet/neoforged/neoforge/fluids/FluidStack;") => {
            Some(copy_stack(&mut ctx, recv, Some(args[1].i())))
        }
        _ => None,
    }
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (matches!(sub, CONTENT | FLUID_STACK)
        && sup == "net/minecraft/core/component/DataComponentHolder")
        || (sub == FLUID_STACK && sup == "net/neoforged/neoforge/common/MutableDataComponentHolder")
}
fn empty_stack(rt: &mut Rt) -> V {
    get_static_field(
        rt,
        FLUID_STACK,
        "EMPTY",
        "Lnet/neoforged/neoforge/fluids/FluidStack;",
    )
    .unwrap()
}
fn empty_content(rt: &mut Rt) -> V {
    get_static_field(
        rt,
        CONTENT,
        "EMPTY",
        "Lnet/neoforged/neoforge/fluids/SimpleFluidContent;",
    )
    .unwrap()
}
fn empty_components(rt: &mut Rt) -> V {
    crate::item_stack::get_static_field(
        rt,
        COMPONENTS,
        "EMPTY",
        "Lnet/minecraft/core/component/DataComponentMap;",
    )
    .unwrap()
}
fn empty_patch(rt: &mut Rt) -> V {
    crate::item_stack::get_static_field(
        rt,
        PATCH,
        "EMPTY",
        "Lnet/minecraft/core/component/DataComponentPatch;",
    )
    .unwrap()
}
fn initialize_stack(rt: &mut Rt, stack: V, fluid: V, amount: i32) {
    let prototype = empty_components(rt);
    let patch = empty_patch(rt);
    let components = rt.new_instance(PATCHED, 0);
    rt.put_field(components, "$prototype", prototype);
    rt.put_field(components, "$fluid_patch", patch);
    rt.put_field(stack, "fluid", fluid);
    rt.put_field(stack, "amount", V::I(amount));
    rt.put_field(stack, "components", components);
}
fn wrap_content(rt: &mut Rt, stack: V) -> V {
    let value = rt.new_instance(CONTENT, 0);
    rt.put_field(value, "fluidStack", stack);
    value
}
fn is_empty(rt: &Rt, stack: V) -> bool {
    rt.get_static(FLUID_STACK, "EMPTY") == Some(stack)
        || rt.get_field(stack, "amount").i() <= 0
        || rt
            .get_static(FLUIDS, "EMPTY")
            .is_some_and(|fluid| fluid == rt.get_field(stack, "fluid"))
}
fn amount(rt: &Rt, stack: V) -> i32 {
    if is_empty(rt, stack) {
        0
    } else {
        rt.get_field(stack, "amount").i()
    }
}
fn get_fluid(ctx: &mut ExecCtx<'_>, stack: V) -> V {
    if is_empty(ctx.rt, stack) {
        mmga_runtime::read_static_field(
            ctx,
            FLUIDS,
            "EMPTY",
            "Lnet/minecraft/world/level/material/Fluid;",
        )
    } else {
        ctx.rt.get_field(stack, "fluid")
    }
}
fn has_empty_components(rt: &mut Rt, stack: V) -> bool {
    let components = rt.get_field(stack, "components");
    rt.class_of(components) == Some(PATCHED)
        && rt.get_field(components, "$prototype") == empty_components(rt)
        && rt.get_field(components, "$fluid_patch") == empty_patch(rt)
}
fn copy_stack(ctx: &mut ExecCtx<'_>, stack: V, requested: Option<i32>) -> V {
    if !check_class(ctx.rt, stack, FLUID_STACK) {
        return V::NULL;
    }
    if is_empty(ctx.rt, stack) {
        return empty_stack(ctx.rt);
    }
    if !has_empty_components(ctx.rt, stack) {
        return unsupported(
            ctx.rt,
            "Copying nonempty fluid component patches is not implemented",
        );
    }
    let copy = ctx.rt.new_instance(FLUID_STACK, 0);
    let fluid = ctx.rt.get_field(stack, "fluid");
    let amount = requested.unwrap_or_else(|| ctx.rt.get_field(stack, "amount").i());
    initialize_stack(ctx.rt, copy, fluid, amount);
    copy
}
fn check_class(rt: &mut Rt, value: V, expected: &str) -> bool {
    if value.is_null() {
        rt.throw_exception(
            "java/lang/NullPointerException",
            "Fluid codec value is null",
        );
        false
    } else if rt.class_of(value) != Some(expected) {
        rt.throw_exception(
            "java/lang/ClassCastException",
            &format!("Expected {expected}"),
        );
        false
    } else {
        true
    }
}
fn unsupported(rt: &mut Rt, message: &str) -> V {
    rt.throw_exception("java/lang/UnsupportedOperationException", message)
}

fn fluid_content_matches(ctx: &mut ExecCtx<'_>, left: V, right: V) -> V {
    if !check_class(ctx.rt, left, FLUID_STACK) || !check_class(ctx.rt, right, FLUID_STACK) {
        return V::NULL;
    }
    if left == right {
        return V::TRUE;
    }
    if amount(ctx.rt, left) != amount(ctx.rt, right) {
        return V::FALSE;
    }
    if is_empty(ctx.rt, left) || is_empty(ctx.rt, right) {
        return V::bool(is_empty(ctx.rt, left) && is_empty(ctx.rt, right));
    }
    if !has_empty_components(ctx.rt, left) || !has_empty_components(ctx.rt, right) {
        return unsupported(
            ctx.rt,
            "SimpleFluidContent equality requires FluidStack component equality",
        );
    }
    V::bool(ctx.rt.get_field(left, "fluid") == ctx.rt.get_field(right, "fluid"))
}

fn fluid_content_hash(ctx: &mut ExecCtx<'_>, stack: V) -> V {
    if !check_class(ctx.rt, stack, FLUID_STACK) {
        return V::NULL;
    }
    if !has_empty_components(ctx.rt, stack) {
        return unsupported(
            ctx.rt,
            "SimpleFluidContent hashCode requires FluidStack component hashing",
        );
    }
    let fluid = get_fluid(ctx, stack);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let fluid_hash = invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", "hashCode", "()I"),
        &[fluid],
    )
    .i();
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    // PatchedDataComponentMap.hashCode = prototype.hashCode + 31 * patch.hashCode.
    // The known-empty prototype has Object identity semantics; the empty patch map hash
    // is zero. Unknown component maps are rejected before calculating this value.
    let prototype = empty_components(ctx.rt);
    let components_hash = invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", "hashCode", "()I"),
        &[prototype],
    )
    .i();
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    V::I(
        amount(ctx.rt, stack)
            .wrapping_mul(31)
            .wrapping_add((31_i32.wrapping_add(fluid_hash)).wrapping_mul(31))
            .wrapping_add(components_hash),
    )
}

// Use ordinary StreamCodec.of closures so these values also execute when nested in
// list/map/composite graphs; no new opaque graph is accepted as a wire success.
fn stream_codec(rt: &mut Rt, optional: bool, content: bool) -> V {
    let encoder = callback(rt, false, optional, content);
    let decoder = callback(rt, true, optional, content);
    let linked = LinkedSpace::new();
    let mut host = mmga_runtime::NullHost;
    crate::codecs::call(rt, &linked, &mut host,
        &MemberRef::new(STREAM, "of", "(Lnet/minecraft/network/codec/StreamEncoder;Lnet/minecraft/network/codec/StreamDecoder;)Lnet/minecraft/network/codec/StreamCodec;"), true,
        &[encoder, decoder]).unwrap()
}
fn callback(rt: &mut Rt, decode: bool, optional: bool, content: bool) -> V {
    let (interface, name, sam_desc, target_desc) = if decode {
        (
            "net/minecraft/network/codec/StreamDecoder",
            "decode",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            "(ZZLjava/lang/Object;)Ljava/lang/Object;",
        )
    } else {
        (
            "net/minecraft/network/codec/StreamEncoder",
            "encode",
            "(Ljava/lang/Object;Ljava/lang/Object;)V",
            "(ZZLjava/lang/Object;Ljava/lang/Object;)V",
        )
    };
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: name.into(),
            sam_desc: sam_desc.into(),
            instantiated_desc: sam_desc.into(),
            captures: vec![V::bool(optional), V::bool(content)],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(CALLBACK, name, target_desc),
            },
        })),
    )
}
fn var_int(ctx: &mut ExecCtx<'_>, buffer: V, encode: Option<i32>) -> V {
    let codec = crate::codecs::get_static_field(
        ctx.rt,
        "net/minecraft/network/codec/ByteBufCodecs",
        "VAR_INT",
        "Lnet/minecraft/network/codec/StreamCodec;",
    )
    .unwrap();
    if let Some(value) = encode {
        let boxed = mmga_runtime::box_primitive(ctx.rt, "java/lang/Integer", V::I(value));
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
            &[codec, buffer, boxed],
        )
    } else {
        let value = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, buffer],
        );
        mmga_runtime::unbox_primitive(ctx.rt, value)
    }
}
fn encode_wire(ctx: &mut ExecCtx<'_>, buffer: V, value: V, optional: bool, content: bool) -> V {
    if !check_class(ctx.rt, value, if content { CONTENT } else { FLUID_STACK }) {
        return V::NULL;
    }
    let stack = if content {
        ctx.rt.get_field(value, "fluidStack")
    } else {
        value
    };
    if !check_class(ctx.rt, stack, FLUID_STACK) {
        return V::NULL;
    }
    if is_empty(ctx.rt, stack) {
        if !optional {
            return ctx.rt.throw_exception(
                "io/netty/handler/codec/EncoderException",
                "Empty FluidStack not allowed",
            );
        }
        return var_int(ctx, buffer, Some(0));
    }
    var_int(ctx, buffer, Some(amount(ctx.rt, stack)));
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    unsupported(ctx.rt, "Nonempty FluidStack wire encoding requires fluid holder IDs and DataComponentPatch.STREAM_CODEC")
}
fn decode_wire(ctx: &mut ExecCtx<'_>, buffer: V, optional: bool, content: bool) -> V {
    let amount = var_int(ctx, buffer, None).i();
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    if amount > 0 {
        return unsupported(ctx.rt, "Nonempty FluidStack wire decoding requires fluid holder IDs and DataComponentPatch.STREAM_CODEC");
    }
    if !optional {
        return ctx.rt.throw_exception(
            "io/netty/handler/codec/DecoderException",
            "Empty FluidStack not allowed",
        );
    }
    let empty = empty_stack(ctx.rt);
    if content {
        wrap_content(ctx.rt, empty)
    } else {
        empty
    }
}

pub fn decode_json(
    ctx: &mut ExecCtx<'_>,
    input: &Json,
    optional: bool,
    content: bool,
) -> Result<V, String> {
    let map = input
        .as_object()
        .ok_or_else(|| format!("Not a FluidStack map: {input}"))?;
    if optional && map.is_empty() {
        let stack = empty_stack(ctx.rt);
        return Ok(if content {
            wrap_content(ctx.rt, stack)
        } else {
            stack
        });
    }
    let raw_id = map
        .get("id")
        .and_then(Json::as_str)
        .ok_or("FluidStack requires string id")?;
    let (namespace, path) = raw_id.split_once(':').unwrap_or(("minecraft", raw_id));
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
        return Err(format!("Invalid fluid id {raw_id}"));
    }
    let id = format!("{namespace}:{path}");
    if id == "minecraft:empty" {
        return Err("Fluid must not be minecraft:empty".into());
    }
    let amount = crate::dfu::decode_json_int(
        ctx.rt,
        map.get("amount").ok_or("FluidStack requires amount")?,
    )?;
    if amount <= 0 {
        return Err(format!("Value must be positive: {amount}"));
    }
    if let Some(patch) = map.get("components") {
        let patch = patch
            .as_object()
            .ok_or("FluidStack components must be a map")?;
        if !patch.is_empty() {
            unsupported(
                ctx.rt,
                "FluidStack component patch JSON decoding is not implemented",
            );
            return Err("Unsupported fluid component patch".into());
        }
    }
    let registry = fluid_registry(ctx.rt);
    let location = ctx.rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        &id,
    );
    let fluid = invoke_member(
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
        return Err("Fluid registry lookup failed".into());
    }
    if fluid.is_null() {
        unsupported(
            ctx.rt,
            &format!("Cannot determine fluid {id} in the incomplete built-in fluid registry"),
        );
        return Err(format!("Unmaterialized fluid {id}"));
    }
    if fluid_id(ctx, registry, fluid)? != id {
        return Err(format!("Unknown fluid {id}"));
    }
    let stack = ctx.rt.new_instance(FLUID_STACK, 0);
    initialize_stack(ctx.rt, stack, fluid, amount);
    Ok(if content {
        wrap_content(ctx.rt, stack)
    } else {
        stack
    })
}
pub fn encode_json(
    ctx: &mut ExecCtx<'_>,
    value: V,
    optional: bool,
    content: bool,
) -> Result<Json, String> {
    if !check_class(ctx.rt, value, if content { CONTENT } else { FLUID_STACK }) {
        return Err("Invalid fluid codec value".into());
    }
    let stack = if content {
        ctx.rt.get_field(value, "fluidStack")
    } else {
        value
    };
    if !check_class(ctx.rt, stack, FLUID_STACK) {
        return Err("Invalid underlying FluidStack".into());
    }
    if is_empty(ctx.rt, stack) {
        return if optional {
            Ok(serde_json::json!({}))
        } else {
            Err("Fluid must not be minecraft:empty".into())
        };
    }
    if !has_empty_components(ctx.rt, stack) {
        unsupported(
            ctx.rt,
            "FluidStack component patch JSON encoding is not implemented",
        );
        return Err("Unsupported fluid component patch".into());
    }
    let registry = fluid_registry(ctx.rt);
    let fluid = ctx.rt.get_field(stack, "fluid");
    let id = fluid_id(ctx, registry, fluid)?;
    if id == "minecraft:empty" {
        return Err("Fluid must not be minecraft:empty".into());
    }
    Ok(serde_json::json!({"id":id,"amount":amount(ctx.rt, stack)}))
}
fn fluid_registry(rt: &mut Rt) -> V {
    rt.new_pinned_handle(
        "net/minecraft/core/Registry",
        "registry",
        0,
        "minecraft:fluid",
    )
}
fn fluid_id(ctx: &mut ExecCtx<'_>, registry: V, fluid: V) -> Result<String, String> {
    let id = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            "net/minecraft/core/Registry",
            "getKey",
            "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;",
        ),
        &[registry, fluid],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Fluid registry lookup failed".into());
    }
    ctx.rt
        .handle_of(id)
        .filter(|(kind, ..)| *kind == "resource_location")
        .map(|(_, _, tag, _)| tag.to_owned())
        .ok_or_else(|| "Unregistered fluid".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct FluidHost(crate::RegistryStore);
    impl HostApi for FluidHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            get_static_field(rt, owner, name, desc)
                .or_else(|| crate::item_stack::get_static_field(rt, owner, name, desc))
                .or_else(|| crate::codecs::get_static_field(rt, owner, name, desc))
                .or_else(|| crate::dfu::get_static_field(rt, owner, name, desc))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            if let Some(v) = call(rt, linked, self, member, is_static, args) {
                return Some(v);
            }
            crate::codecs::call(rt, linked, self, member, is_static, args)
                .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
                .or_else(|| {
                    crate::minecraft::call_keys_authlib_and_libs(
                        rt,
                        &mut self.0,
                        member,
                        is_static,
                        args,
                    )
                })
        }
    }
    fn buffer(rt: &mut Rt, bytes: Vec<u8>) -> V {
        rt.alloc(
            "net/minecraft/network/RegistryFriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes,
                reader_idx: 0,
            },
        )
    }
    fn exception_class(rt: &Rt) -> Option<&str> {
        rt.pending_exception.and_then(|ex| rt.class_of(V::R(ex)))
    }
    #[test]
    fn empty_fluid_content_json_and_wire_paths_preserve_empty_and_real_zero_encoding() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let empty = get_static_field(
            &mut rt,
            CONTENT,
            "EMPTY",
            "Lnet/neoforged/neoforge/fluids/SimpleFluidContent;",
        )
        .unwrap();
        let codec = get_static_field(
            &mut rt,
            CONTENT,
            "STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let buffer = rt.alloc(
            "net/minecraft/network/RegistryFriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![],
                reader_idx: 0,
            },
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(
            encode_json(&mut ctx, empty, true, true).unwrap(),
            serde_json::json!({})
        );
        let decoded = decode_json(&mut ctx, &serde_json::json!({}), true, true).unwrap();
        assert_eq!(ctx.rt.class_of(decoded), Some(CONTENT));
        assert_ne!(
            decoded, empty,
            "The codec calls new SimpleFluidContent even for empty stacks"
        );
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
            &[codec, buffer, empty],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(
            matches!(&ctx.rt.get_obj(buffer).unwrap().data,Data::ByteBuf{bytes,..}if bytes==&[0])
        );
        let decoded = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, buffer],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.class_of(decoded), Some(CONTENT));
        assert_eq!(ctx.rt.get_field(decoded, "fluidStack"), empty_stack(ctx.rt));
    }

    #[test]
    fn mandatory_fluid_wire_rejects_empty_and_optional_decoder_accepts_negative_amounts() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let mandatory = get_static_field(
            &mut rt,
            FLUID_STACK,
            "STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let optional = get_static_field(
            &mut rt,
            FLUID_STACK,
            "OPTIONAL_STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let empty = empty_stack(&mut rt);
        let out = buffer(&mut rt, vec![]);
        let input = buffer(&mut rt, vec![0]);
        let negative = buffer(&mut rt, vec![255, 255, 255, 255, 15]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
            &[mandatory, out, empty],
        );
        assert_eq!(
            exception_class(ctx.rt),
            Some("io/netty/handler/codec/EncoderException")
        );
        assert!(
            matches!(&ctx.rt.get_obj(out).unwrap().data, Data::ByteBuf { bytes, .. } if bytes.is_empty())
        );
        ctx.rt.pending_exception = None;
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[mandatory, input],
        );
        assert_eq!(
            exception_class(ctx.rt),
            Some("io/netty/handler/codec/DecoderException")
        );
        ctx.rt.pending_exception = None;
        let decoded = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[optional, negative],
        );
        assert_eq!(decoded, empty);
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn nonempty_wire_and_component_patches_never_disappear_into_empty_content() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let codec = get_static_field(
            &mut rt,
            CONTENT,
            "STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let input = buffer(&mut rt, vec![1]);
        let truncated = buffer(&mut rt, vec![128]);
        let fluid = rt.new_instance("test/Fluid", 0);
        let stack = rt.new_instance(FLUID_STACK, 0);
        initialize_stack(&mut rt, stack, fluid, 1000);
        let content = wrap_content(&mut rt, stack);
        let out = buffer(&mut rt, vec![]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, input],
        );
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/UnsupportedOperationException")
        );
        ctx.rt.pending_exception = None;
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, truncated],
        );
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/IndexOutOfBoundsException")
        );
        ctx.rt.pending_exception = None;
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
            &[codec, out, content],
        );
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/UnsupportedOperationException")
        );
        assert!(
            matches!(&ctx.rt.get_obj(out).unwrap().data, Data::ByteBuf { bytes, .. } if bytes == &[232,7])
        );
        ctx.rt.pending_exception = None;
        assert!(decode_json(
            &mut ctx,
            &serde_json::json!({"id":"water","amount":1000,"components":{"test:component":1}}),
            true,
            true
        )
        .is_err());
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn fluid_amount_is_required_positive_and_empty_json_has_exact_optional_rules() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for bad in [
            serde_json::json!({}),
            serde_json::json!({"id":"water"}),
            serde_json::json!({"id":"water","amount":0}),
            serde_json::json!({"id":"water","amount":-3}),
            serde_json::json!({"id":"minecraft:empty","amount":10}),
            serde_json::json!({"id":"WATER","amount":1}),
        ] {
            assert!(decode_json(&mut ctx, &bad, false, false).is_err(), "{bad}");
            assert!(
                ctx.rt.pending_exception.is_none(),
                "Invalid input should be a data error"
            );
        }
        let empty = empty_stack(ctx.rt);
        assert!(encode_json(&mut ctx, empty, false, false).is_err());
        assert_eq!(
            encode_json(&mut ctx, empty, true, false).unwrap(),
            serde_json::json!({})
        );
        assert_eq!(
            decode_json(&mut ctx, &serde_json::json!({}), true, false).unwrap(),
            empty
        );
    }
    #[test]
    fn canonical_empty_cannot_be_mutated_nonempty_and_copy_of_snapshots_nonempty_amounts() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let empty = empty_stack(&mut rt);
        let fluid = rt.new_instance("test/Fluid", 0);
        let stack = rt.new_instance(FLUID_STACK, 0);
        initialize_stack(&mut rt, stack, fluid, 500);
        call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(FLUID_STACK, "setAmount", "(I)V"),
            false,
            &[empty, V::I(500)],
        )
        .unwrap();
        assert_eq!(amount(&rt, empty), 0);
        let copy = call(&mut rt, &linked, &mut host, &MemberRef::new(CONTENT,"copyOf","(Lnet/neoforged/neoforge/fluids/FluidStack;)Lnet/neoforged/neoforge/fluids/SimpleFluidContent;"),true,&[stack]).unwrap();
        let copy_stack = rt.get_field(copy, "fluidStack");
        rt.put_field(stack, "amount", V::I(10));
        assert_eq!(amount(&rt, copy_stack), 500);
        assert_eq!(rt.get_field(copy_stack, "fluid"), fluid);
        assert_ne!(
            rt.get_field(stack, "components"),
            rt.get_field(copy_stack, "components")
        );
        let canonical = call(&mut rt, &linked, &mut host, &MemberRef::new(CONTENT,"copyOf","(Lnet/neoforged/neoforge/fluids/FluidStack;)Lnet/neoforged/neoforge/fluids/SimpleFluidContent;"),true,&[empty]).unwrap();
        assert_eq!(canonical, empty_content(&mut rt));
    }
    #[test]
    fn object_typed_content_equality_and_hash_compare_real_amount_fluid_and_components() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let fluid = rt.new_instance("test/Fluid", 0);
        let left_stack = rt.new_instance(FLUID_STACK, 0);
        initialize_stack(&mut rt, left_stack, fluid, 500);
        let right_stack = rt.new_instance(FLUID_STACK, 0);
        initialize_stack(&mut rt, right_stack, fluid, 500);
        let left = wrap_content(&mut rt, left_stack);
        let right = wrap_content(&mut rt, right_stack);
        let empty = empty_content(&mut rt);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let equals = MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z");
        let hash = MemberRef::new("java/lang/Object", "hashCode", "()I");
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &equals, &[left, right]),
            V::TRUE
        );
        let lhs = invoke_member(&mut ctx, op::INVOKEVIRTUAL, &hash, &[left]);
        let rhs = invoke_member(&mut ctx, op::INVOKEVIRTUAL, &hash, &[right]);
        assert_eq!(
            lhs, rhs,
            "Distinct copied component maps must have equal value hashes"
        );
        assert!(ctx.rt.pending_exception.is_none());
        ctx.rt.put_field(right_stack, "amount", V::I(501));
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &equals, &[left, right]),
            V::FALSE
        );
        assert_ne!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &hash, &[right]),
            lhs
        );
        let decoded = decode_json(&mut ctx, &serde_json::json!({}), true, true).unwrap();
        assert_eq!(
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &equals, &[empty, decoded]),
            V::TRUE
        );
        let bad = ctx.rt.new_instance("test/UnknownComponents", 0);
        ctx.rt.put_field(right_stack, "components", bad);
        ctx.rt.put_field(right_stack, "amount", V::I(500));
        invoke_member(&mut ctx, op::INVOKEVIRTUAL, &equals, &[left, right]);
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/UnsupportedOperationException")
        );
        ctx.rt.pending_exception = None;
        invoke_member(&mut ctx, op::INVOKEVIRTUAL, &hash, &[right]);
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn json_roundtrip_resolves_live_registered_fluids_and_preserves_amount_without_fallbacks() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = FluidHost::default();
        let registry = fluid_registry(&mut rt);
        let fluid = rt.new_instance("test/Fluid", 0);
        let location = rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "test:syrup",
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new("net/minecraft/core/Registry","register","(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Object;)Ljava/lang/Object;"),&[registry,location,fluid]);
        assert!(ctx.rt.pending_exception.is_none());
        let input = serde_json::json!({"id":"test:syrup","amount":1000});
        let content = decode_json(&mut ctx, &input, true, true).unwrap();
        let stack = ctx.rt.get_field(content, "fluidStack");
        assert_eq!(ctx.rt.get_field(stack, "fluid"), fluid);
        assert_eq!(ctx.rt.get_field(stack, "amount"), V::I(1000));
        assert_eq!(encode_json(&mut ctx, content, true, true).unwrap(), input);
        assert!(decode_json(
            &mut ctx,
            &serde_json::json!({"id":"test:unknown","amount":1000}),
            true,
            true
        )
        .is_err());
        assert_eq!(
            exception_class(ctx.rt),
            Some("java/lang/UnsupportedOperationException")
        );
    }
}

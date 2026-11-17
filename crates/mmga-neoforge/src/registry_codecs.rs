//! Live Registry.byNameCodec name/value mapping from Minecraft 1.21.1 `jz`.
//! The mapping captures the registry handle, not an entry snapshot, so entries
//! registered later are visible. Missing custom entries are DataResult errors;
//! absent definitions in incomplete built-in registries are explicit unsupported.
use mmga_classfile::MemberRef;
use mmga_runtime::{
    invoke_member, read_static_field, ClosureData, ClosureTarget, Data, ExecCtx, Rt, V,
};
const REGISTRY: &str = "net/minecraft/core/Registry";
const CODEC: &str = "com/mojang/serialization/Codec";
const ADAPTER: &str = "mmga/internal/RegistryNameCodec";
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static
        && matches!(
            member.owner.as_str(),
            REGISTRY | "net/minecraft/core/DefaultedRegistry" | "net/minecraft/core/MappedRegistry"
        )
        && matches!(member.name.as_str(), "byNameCodec" | "holderByNameCodec")
        && member.desc == "()Lcom/mojang/serialization/Codec;"
    {
        let registry = *args.first()?;
        let ("registry", _, _, _) = ctx.rt.handle_of(registry)? else {
            return None;
        };
        let location = read_static_field(
            ctx,
            "net/minecraft/resources/ResourceLocation",
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let holder = member.name == "holderByNameCodec";
        let decoder = callback(
            ctx.rt,
            if holder { "decodeHolder" } else { "decode" },
            vec![registry],
        );
        let encoder = callback(
            ctx.rt,
            if holder { "encodeHolder" } else { "encode" },
            vec![registry],
        );
        return Some(invoke_member(ctx,mmga_classfile::op::INVOKEINTERFACE,&MemberRef::new(CODEC,"flatXmap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;"),&[location,decoder,encoder]));
    }
    if !is_static || member.owner != ADAPTER {
        return None;
    }
    if member.name == "errorMessage" && member.desc == "(Ljava/lang/String;)Ljava/lang/Object;" {
        return Some(*args.first()?);
    }
    if member.desc != "(Lnet/minecraft/core/Registry;Ljava/lang/Object;)Ljava/lang/Object;" {
        return None;
    }
    let registry = *args.first()?;
    let value = *args.get(1)?;
    let ("registry", _, registry_id, _) = ctx.rt.handle_of(registry)? else {
        return None;
    };
    let registry_id = registry_id.to_owned();
    if matches!(member.name.as_str(), "decodeHolder" | "decode") {
        let Some(holder) = get_holder(ctx, registry, value) else {
            return Some(V::NULL);
        };
        if holder.is_null() {
            let location = ctx.rt.handle_of(value)?.2.to_owned();
            if (registry_id.starts_with("minecraft:") || registry_id.starts_with("neoforge:"))
                && (location.starts_with("minecraft:") || location.starts_with("neoforge:"))
            {
                return Some(ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    &format!(
                        "Cannot determine missing holder in incomplete registry {registry_id}"
                    ),
                ));
            }
            return Some(error(ctx, &format!("Unknown registry key in ResourceKey[minecraft:root / {registry_id}]: {location}")));
        }
        let verified = verify_reference(ctx, registry, &registry_id, holder);
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        match verified {
            Ok(_) => {
                let decoded = if member.name == "decodeHolder" {
                    holder
                } else {
                    invoke_member(
                        ctx,
                        mmga_classfile::op::INVOKEINTERFACE,
                        &MemberRef::new(
                            "net/minecraft/core/Holder",
                            "value",
                            "()Ljava/lang/Object;",
                        ),
                        &[holder],
                    )
                };
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                return Some(reference_result(ctx, registry, holder, decoded));
            }
            Err(message) => return Some(error(ctx, &message)),
        }
    }
    if matches!(member.name.as_str(), "encodeHolder" | "encode") {
        let reference = if member.name == "encodeHolder" {
            value
        } else {
            invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new(
                    REGISTRY,
                    "wrapAsHolder",
                    "(Ljava/lang/Object;)Lnet/minecraft/core/Holder;",
                ),
                &[registry, value],
            )
        };
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        if ctx.rt.class_of(reference) != Some("net/minecraft/core/Holder$Reference") {
            return Some(error(
                ctx,
                &format!("Cannot serialize direct holder in registry {registry_id}"),
            ));
        }
        let verified = verify_reference(ctx, registry, &registry_id, reference);
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        return Some(match verified {
            Ok(location) => reference_result(ctx, registry, reference, location),
            Err(message) => error(ctx, &message),
        });
    }

    None
}
fn get_holder(ctx: &mut ExecCtx<'_>, registry: V, location: V) -> Option<V> {
    let optional = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new(
            REGISTRY,
            "getHolder",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/Optional;",
        ),
        &[registry, location],
    );
    if ctx.rt.pending_exception.is_some() {
        return None;
    }
    match ctx.rt.get_obj(optional).map(|o| &o.data) {
        Some(Data::Optional(value)) => Some(value.unwrap_or(V::NULL)),
        _ => {
            ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "Registry.getHolder must return Optional",
            );
            None
        }
    }
}
fn verify_reference(
    ctx: &mut ExecCtx<'_>,
    registry: V,
    registry_id: &str,
    holder: V,
) -> Result<V, String> {
    if ctx.rt.class_of(holder) != Some("net/minecraft/core/Holder$Reference") {
        return Err("Registry holder codec requires a real reference holder".into());
    }
    let key = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEVIRTUAL,
        &MemberRef::new(
            "net/minecraft/core/Holder$Reference",
            "key",
            "()Lnet/minecraft/resources/ResourceKey;",
        ),
        &[holder],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Holder key is not implemented".into());
    }
    let (owner, location) = crate::minecraft::resource_key_parts(ctx.rt, key)
        .ok_or("Reference key must be ResourceKey")?;
    if owner != registry_id {
        return Err(format!(
            "Reference belongs to registry {owner}, not {registry_id}"
        ));
    }
    let location = ctx.rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        location,
    );
    let canonical =
        get_holder(ctx, registry, location).ok_or("Registry holder lookup is not implemented")?;
    if canonical != holder {
        return Err(format!(
            "Reference is not the registered holder in registry {registry_id}"
        ));
    }
    let bound = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new("net/minecraft/core/Holder", "isBound", "()Z"),
        &[holder],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Reference binding query failed".into());
    }
    if bound != V::TRUE {
        return Err("Unbound registry reference".into());
    }
    let value = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new("net/minecraft/core/Holder", "value", "()Ljava/lang/Object;"),
        &[holder],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Reference value query failed".into());
    }
    let registered = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new(
            REGISTRY,
            "get",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
        ),
        &[registry, location],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Registered value query failed".into());
    }
    if value.is_null() || registered != value {
        return Err("Reference is not bound to its registered value".into());
    }
    Ok(location)
}
fn reference_result(ctx: &mut ExecCtx<'_>, registry: V, holder: V, value: V) -> V {
    let key = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEVIRTUAL,
        &MemberRef::new(
            "net/minecraft/core/Holder$Reference",
            "key",
            "()Lnet/minecraft/resources/ResourceKey;",
        ),
        &[holder],
    );
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let info = invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new(
            REGISTRY,
            "registrationInfo",
            "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
        ),
        &[registry, key],
    );
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let lifecycle = match ctx.rt.get_obj(info).map(|o| &o.data) {
        Some(Data::Optional(None)) => invoke_member(
            ctx,
            mmga_classfile::op::INVOKESTATIC,
            &MemberRef::new(
                "com/mojang/serialization/Lifecycle",
                "experimental",
                "()Lcom/mojang/serialization/Lifecycle;",
            ),
            &[],
        ),
        Some(Data::Optional(Some(info))) => {
            let info = *info;
            invoke_member(
                ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/minecraft/core/RegistrationInfo",
                    "lifecycle",
                    "()Lcom/mojang/serialization/Lifecycle;",
                ),
                &[info],
            )
        }
        _ => {
            return ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "Registry.registrationInfo must return Optional",
            )
        }
    };
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let result = success(ctx, value);
    invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new(
            "com/mojang/serialization/DataResult",
            "setLifecycle",
            "(Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;",
        ),
        &[result, lifecycle],
    )
}
fn callback(rt: &mut Rt, name: &str, captures: Vec<V>) -> V {
    rt.alloc(
        "java/util/function/Function",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Function".into(),
            sam_name: "apply".into(),
            sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            instantiated_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            captures,
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(
                    ADAPTER,
                    name,
                    "(Lnet/minecraft/core/Registry;Ljava/lang/Object;)Ljava/lang/Object;",
                ),
            },
        })),
    )
}
fn success(ctx: &mut ExecCtx<'_>, value: V) -> V {
    invoke_member(
        ctx,
        mmga_classfile::op::INVOKESTATIC,
        &MemberRef::new(
            "com/mojang/serialization/DataResult",
            "success",
            "(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",
        ),
        &[value],
    )
}
fn error(ctx: &mut ExecCtx<'_>, message: &str) -> V {
    let message = ctx.rt.new_str(message);
    let supplier = ctx.rt.alloc(
        "java/util/function/Supplier",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Supplier".into(),
            sam_name: "get".into(),
            sam_desc: "()Ljava/lang/Object;".into(),
            instantiated_desc: "()Ljava/lang/String;".into(),
            captures: vec![message],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(
                    ADAPTER,
                    "errorMessage",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                ),
            },
        })),
    );
    invoke_member(
        ctx,
        mmga_classfile::op::INVOKESTATIC,
        &MemberRef::new(
            "com/mojang/serialization/DataResult",
            "error",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/DataResult;",
        ),
        &[supplier],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryStore;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, HostApi, LinkedSpace, Rt};
    #[derive(Default)]
    struct CodecHost(RegistryStore);
    impl HostApi for CodecHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            crate::dfu::get_static_field(rt, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
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
    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn json(ctx: &mut ExecCtx<'_>, text: &str) -> V {
        let text = ctx.rt.intern_str(text);
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "com/google/gson/JsonParser",
                "parseString",
                "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ),
            &[text],
        )
    }

    #[test]
    fn holder_name_codec_retains_registry_and_does_not_invent_missing_references() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = CodecHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry = ctx.rt.new_pinned_handle(
            "net/minecraft/core/DefaultedRegistry",
            "registry",
            0,
            "test:types",
        );
        let codec = invoke(
            &mut ctx,
            "net/minecraft/core/DefaultedRegistry",
            "holderByNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        assert_eq!(ctx.rt.class_of(codec), Some(CODEC));
        let graph_args = ctx.rt.get_field(codec, "$dfu_args");
        let values = match &ctx.rt.get_obj(graph_args).unwrap().data {
            Data::Array(v) => v.clone(),
            _ => panic!("expected retained graph"),
        };
        assert_eq!(values.len(), 3);
        for callback in &values[1..] {
            let Data::Closure(callback) = &ctx.rt.get_obj(*callback).unwrap().data else {
                panic!("expected retained callback")
            };
            assert_eq!(callback.captures, vec![registry]);
        }
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let input = json(&mut ctx, "\"test:registered\"");
        let result = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isError",
                "()Z",
                &[result]
            ),
            V::TRUE
        );
        let builtin = ctx
            .rt
            .new_pinned_handle(REGISTRY, "registry", 0, "minecraft:item");
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "holderByNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[builtin],
        );
        let input = json(&mut ctx, "\"minecraft:unmodeled\"");
        invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, input]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }

    #[test]
    fn holder_name_codec_rejects_direct_holders_and_checks_binding_before_encoding() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = CodecHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry = ctx
            .rt
            .new_pinned_handle(REGISTRY, "registry", 0, "test:types");
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "holderByNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let direct = ctx.rt.new_instance("net/minecraft/core/Holder$Direct", 0);
        let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, direct]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isError",
                "()Z",
                &[encoded]
            ),
            V::TRUE
        );
        let unresolved = ctx
            .rt
            .new_instance("net/minecraft/core/Holder$Reference", 0);
        invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, unresolved]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        // A field-shaped reference alone must not be enough to encode a key.
        let key = crate::minecraft::new_resource_key(ctx.rt, "test:types", "test:registered");
        let value = ctx.rt.new_instance("test/RegisteredValue", 0);
        ctx.rt.put_field(unresolved, "$key", key);
        ctx.rt.put_field(unresolved, "$value", value);
        invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", &[codec, ops, unresolved]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }

    #[test]
    fn registry_codec_observes_late_entries_and_returns_real_values() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = CodecHost::default();
        host.0
            .entries
            .insert("test:types".into(), Default::default());
        let registry = rt.new_pinned_handle(REGISTRY, "registry", 0, "test:types");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "byNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let location = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "test:registered",
        );
        let value = ctx.rt.new_instance("test/RegisteredType", 0);
        invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(REGISTRY,"register","(Lnet/minecraft/core/Registry;Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Object;)Ljava/lang/Object;"),&[registry,location,value]);
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let input = json(&mut ctx, "\"test:registered\"");
        let decoded=invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,input]);
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "getOrThrow",
                "()Ljava/lang/Object;",
                &[decoded]
            ),
            value
        );
        let encoded=invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,value]);
        let encoded = invoke(
            &mut ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            &[encoded],
        );
        let rendered = invoke(
            &mut ctx,
            "com/google/gson/JsonElement",
            "toString",
            "()Ljava/lang/String;",
            &[encoded],
        );
        assert_eq!(ctx.rt.str_of(rendered), Some("\"test:registered\""));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn unknown_custom_names_are_errors_and_unmodeled_builtin_names_are_not_missing() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = CodecHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let registry = ctx
            .rt
            .new_pinned_handle(REGISTRY, "registry", 0, "test:types");
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "byNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[registry],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let input = json(&mut ctx, "\"test:missing\"");
        let decoded=invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,input]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isError",
                "()Z",
                &[decoded]
            ),
            V::TRUE
        );
        let value = ctx.rt.new_instance("test/Unregistered", 0);
        let encoded=invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,value]);
        assert_eq!(
            invoke(
                &mut ctx,
                "com/mojang/serialization/DataResult",
                "isError",
                "()Z",
                &[encoded]
            ),
            V::TRUE
        );
        let builtin = ctx
            .rt
            .new_pinned_handle(REGISTRY, "registry", 0, "minecraft:block");
        let codec = invoke(
            &mut ctx,
            REGISTRY,
            "byNameCodec",
            "()Lcom/mojang/serialization/Codec;",
            &[builtin],
        );
        let stone = json(&mut ctx, "\"minecraft:stone\"");
        invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,stone]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

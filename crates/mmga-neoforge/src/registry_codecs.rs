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
        && member.name == "byNameCodec"
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
        let decoder = callback(ctx.rt, "decode", vec![registry]);
        let encoder = callback(ctx.rt, "encode", vec![registry]);
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
    if member.name == "decode" {
        let ("resource_location", _, location, _) = ctx.rt.handle_of(value)? else {
            return None;
        };
        let location = location.to_owned();
        let decoded = invoke_member(
            ctx,
            mmga_classfile::op::INVOKEINTERFACE,
            &MemberRef::new(
                REGISTRY,
                "get",
                "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
            ),
            &[registry, value],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        if decoded.is_null() {
            if registry_id.starts_with("minecraft:") || registry_id.starts_with("neoforge:") {
                return Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException",&format!("MMGA cannot determine missing entry {location} in incomplete built-in registry {registry_id}")));
            }
            return Some(error(ctx,&format!("Unknown registry key in ResourceKey[minecraft:root / {registry_id}]: {location}")));
        }
        // A DefaultedRegistry may return its fallback for an absent name; the
        // official reference-holder codec must still reject that absent name.
        let actual = invoke_member(
            ctx,
            mmga_classfile::op::INVOKEINTERFACE,
            &MemberRef::new(
                REGISTRY,
                "getKey",
                "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;",
            ),
            &[registry, decoded],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        if !ctx.rt.val_eq(actual, value) {
            return Some(error(ctx,&format!("Unknown registry key in ResourceKey[minecraft:root / {registry_id}]: {location}")));
        }
        return Some(success(ctx, decoded));
    }
    if member.name == "encode" {
        let name = invoke_member(
            ctx,
            mmga_classfile::op::INVOKEINTERFACE,
            &MemberRef::new(
                REGISTRY,
                "getKey",
                "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;",
            ),
            &[registry, value],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        if name.is_null() {
            return Some(error(
                ctx,
                &format!("Unregistered value in registry {registry_id}"),
            ));
        }
        return Some(success(ctx, name));
    }
    None
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

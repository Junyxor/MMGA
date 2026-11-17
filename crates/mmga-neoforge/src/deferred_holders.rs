//! NeoForge DeferredHolder key access before and after registration.
//!
//! Semantics checked against NeoForge's official 1.21.1 DeferredHolder.java:
//! getKey/getId do not require a bound value; value/get distinguish an absent
//! registry from an unbound target. Metadata does not fabricate a target value.

use crate::RegistryStore;
use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const DEFERRED: &str = "net/neoforged/neoforge/registries/DeferredHolder";

pub fn call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if is_static {
        if member.owner != DEFERRED || name != "create" {
            return None;
        }
        let result = match desc {
            "(Lnet/minecraft/resources/ResourceKey;)Lnet/neoforged/neoforge/registries/DeferredHolder;" => {
                let key = *args.first()?;
                if key.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "DeferredHolder key")); }
                let (registry, location) = crate::minecraft::resource_key_parts(rt, key)?;
                (registry, location, Some(key))
            }
            "(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/neoforged/neoforge/registries/DeferredHolder;" => {
                if args.first()?.is_null() || args.get(1)?.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "DeferredHolder registry/key")); }
                let (_,registry) = crate::minecraft::resource_key_parts(rt,args[0])?;
                (registry, location(rt,args[1])?, None)
            }
            "(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceLocation;)Lnet/neoforged/neoforge/registries/DeferredHolder;" => {
                if args.first()?.is_null() || args.get(1)?.is_null() { return Some(rt.throw_exception("java/lang/NullPointerException", "DeferredHolder registry/key")); }
                (location(rt,args[0])?,location(rt,args[1])?,None)
            }
            _ => return None,
        };
        let holder = registries.bind_holder(rt, &result.0, &result.1, DEFERRED);
        let holder_id = rt.handle_of(holder)?.1;
        let key = result
            .2
            .unwrap_or_else(|| intern_key(rt, &result.0, &result.1));
        rt.put_static("mmga/deferred_holder_keys", &holder_id.to_string(), key);
        return Some(holder);
    }
    let recv = *args.first()?;
    if member.owner == DEFERRED
        && name == "<init>"
        && desc == "(Lnet/minecraft/resources/ResourceKey;)V"
    {
        let key = *args.get(1)?;
        if key.is_null() {
            return Some(
                rt.throw_exception("java/lang/NullPointerException", "DeferredHolder key"),
            );
        }
        let (registry, location) = crate::minecraft::resource_key_parts(rt, key)?;
        let handle = registries.bind_holder(rt, &registry, &location, DEFERRED);
        let holder_id = rt.handle_of(handle)?.1;
        rt.put_static("mmga/deferred_holder_keys", &holder_id.to_string(), key);
        rt.put_field(recv, "key", key);
        rt.put_field(recv, "$deferredHandle", handle);
        return Some(V::Void);
    }
    let handle = if rt
        .handle_of(recv)
        .is_some_and(|(kind, ..)| kind == "deferred_holder")
    {
        recv
    } else {
        rt.get_field(recv, "$deferredHandle")
    };
    let ("deferred_holder", id, _, _) = rt.handle_of(handle)? else {
        return None;
    };
    match (name, desc) {
        ("getKey", "()Lnet/minecraft/resources/ResourceKey;") => {
            Some(holder_key(rt, registries, id))
        }
        ("getId", "()Lnet/minecraft/resources/ResourceLocation;") => {
            let (_, location, _) = registries.holders.get(&id)?.clone();
            Some(intern_location(rt, &location))
        }
        ("get" | "value", "()Ljava/lang/Object;") => Some(registries.require_holder_value(rt, id)),
        ("isBound", "()Z") => Some(V::bool(registries.holder_value(id).is_some())),
        ("asOptional", "()Ljava/util/Optional;") => Some(rt.alloc(
            "java/util/Optional",
            0,
            Data::Optional(registries.holder_value(id)),
        )),
        ("unwrapKey", "()Ljava/util/Optional;") => {
            let key = holder_key(rt, registries, id);
            Some(rt.alloc("java/util/Optional", 0, Data::Optional(Some(key))))
        }
        ("is", "(Lnet/minecraft/resources/ResourceKey;)Z") => {
            Some(V::bool(holder_key(rt, registries, id) == *args.get(1)?))
        }
        ("is", "(Lnet/minecraft/resources/ResourceLocation;)Z") => {
            let value = *args.get(1)?;
            if value.is_null() {
                return Some(
                    rt.throw_exception("java/lang/NullPointerException", "DeferredHolder.is(null)"),
                );
            }
            let expected = registries.holders.get(&id)?.1.clone();
            Some(V::bool(
                location(rt, value).as_deref() == Some(expected.as_str()),
            ))
        }
        ("toString", "()Ljava/lang/String;") => {
            let (registry, location, _) = registries.holders.get(&id)?;
            let text = format!("DeferredHolder{{ResourceKey[{registry} / {location}]}}");
            Some(rt.new_str(text))
        }
        ("hashCode", "()I") => Some(V::I(holder_key(rt, registries, id).r() as i32)),
        ("equals", "(Ljava/lang/Object;)Z") => {
            let other = *args.get(1)?;
            if recv == other {
                return Some(V::TRUE);
            }
            let other_handle = if rt
                .handle_of(other)
                .is_some_and(|(kind, ..)| kind == "deferred_holder")
            {
                other
            } else {
                rt.get_field(other, "$deferredHandle")
            };
            let other_id = rt
                .handle_of(other_handle)
                .filter(|(kind, ..)| *kind == "deferred_holder")
                .map(|(_, id, ..)| id);
            let other_key = if let Some(other_id) = other_id {
                holder_key(rt, registries, other_id)
            } else if rt.class_of(other) == Some("net/minecraft/core/Holder$Reference") {
                rt.get_field(other, "$key")
            } else {
                V::NULL
            };
            Some(V::bool(
                !other_key.is_null() && holder_key(rt, registries, id) == other_key,
            ))
        }
        _ => None,
    }
}

fn location(rt: &Rt, value: V) -> Option<String> {
    let ("resource_location", _, tag, _) = rt.handle_of(value)? else {
        return None;
    };
    Some(tag.to_owned())
}
fn intern_location(rt: &mut Rt, location: &str) -> V {
    if let Some(value) = rt.get_static("mmga/deferred_holder_locations", location) {
        return value;
    }
    let value = rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        location,
    );
    rt.put_static("mmga/deferred_holder_locations", location, value);
    value
}
fn intern_key(rt: &mut Rt, registry: &str, location: &str) -> V {
    let tag = format!("{registry}|{location}");
    if let Some(value) = rt.get_static("mmga/deferred_resource_keys", &tag) {
        return value;
    }
    let value = crate::minecraft::new_resource_key(rt, registry, location);
    rt.put_static("mmga/deferred_resource_keys", &tag, value);
    value
}
fn holder_key(rt: &mut Rt, registries: &RegistryStore, id: u64) -> V {
    if let Some(value) = rt.get_static("mmga/deferred_holder_keys", &id.to_string()) {
        return value;
    }
    let Some((registry, location, _)) = registries.holders.get(&id) else {
        return rt.throw_exception(
            "java/lang/IllegalStateException",
            "DeferredHolder has no resource key",
        );
    };
    let value = intern_key(rt, registry, location);
    rt.put_static("mmga/deferred_holder_keys", &id.to_string(), value);
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        minecraft::{new_resource_key, resource_key_parts},
        ContentProperties,
    };
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, Data, ExecCtx, HostApi, LinkedSpace};
    const HOLDER: &str = "net/minecraft/core/Holder";
    #[derive(Default)]
    struct HolderHost(RegistryStore);
    impl HostApi for HolderHost {
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
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, &mut self.0, member, is_static, args)
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

    #[test]
    fn key_and_id_are_available_and_stable_before_binding() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = HolderHost::default();
        let holder = host
            .0
            .bind_holder(&mut rt, "minecraft:item", "example:unbound", DEFERRED);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let key = invoke(
            &mut ctx,
            DEFERRED,
            "getKey",
            "()Lnet/minecraft/resources/ResourceKey;",
            &[holder],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            resource_key_parts(ctx.rt, key),
            Some(("minecraft:item".into(), "example:unbound".into()))
        );
        assert_eq!(
            invoke(
                &mut ctx,
                HOLDER,
                "getKey",
                "()Lnet/minecraft/resources/ResourceKey;",
                &[holder]
            ),
            key
        );
        let id = invoke(
            &mut ctx,
            DEFERRED,
            "getId",
            "()Lnet/minecraft/resources/ResourceLocation;",
            &[holder],
        );
        assert_eq!(ctx.rt.handle_of(id).unwrap().2, "example:unbound");
        assert_eq!(
            invoke(
                &mut ctx,
                DEFERRED,
                "getId",
                "()Lnet/minecraft/resources/ResourceLocation;",
                &[holder]
            ),
            id
        );
        assert_eq!(
            invoke(&mut ctx, HOLDER, "isBound", "()Z", &[holder]),
            V::FALSE
        );
        let optional = invoke(
            &mut ctx,
            HOLDER,
            "unwrapKey",
            "()Ljava/util/Optional;",
            &[holder],
        );
        assert!(
            matches!(ctx.rt.get_obj(optional).map(|o|&o.data),Some(Data::Optional(Some(value))) if *value==key)
        );
        for owner in [HOLDER, DEFERRED] {
            invoke(&mut ctx, owner, "value", "()Ljava/lang/Object;", &[holder]);
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("NullPointerException"));
        }
    }

    #[test]
    fn late_registration_preserves_key_and_returns_actual_value() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = HolderHost::default();
        let holder = host.0.bind_holder(
            &mut rt,
            "example:missing_registry",
            "example:entry",
            DEFERRED,
        );
        let key = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            let key = invoke(
                &mut ctx,
                DEFERRED,
                "getKey",
                "()Lnet/minecraft/resources/ResourceKey;",
                &[holder],
            );
            invoke(&mut ctx, DEFERRED, "get", "()Ljava/lang/Object;", &[holder]);
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("IllegalStateException"));
            key
        };
        let value = rt.new_instance("example/RegisteredValue", 0);
        host.0.register_direct(
            &mut rt,
            "example:missing_registry",
            "example:entry",
            "example",
            value,
            ContentProperties::default(),
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(
            invoke(
                &mut ctx,
                DEFERRED,
                "getKey",
                "()Lnet/minecraft/resources/ResourceKey;",
                &[holder]
            ),
            key
        );
        assert_eq!(
            invoke(&mut ctx, HOLDER, "isBound", "()Z", &[holder]),
            V::TRUE
        );
        assert_eq!(
            invoke(&mut ctx, DEFERRED, "get", "()Ljava/lang/Object;", &[holder]),
            value
        );
        assert_eq!(
            invoke(&mut ctx, HOLDER, "value", "()Ljava/lang/Object;", &[holder]),
            value
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn resource_key_create_keeps_the_supplied_key_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = HolderHost::default();
        let key = new_resource_key(&mut rt, "minecraft:item", "example:reference");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let holder=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(DEFERRED,"create","(Lnet/minecraft/resources/ResourceKey;)Lnet/neoforged/neoforge/registries/DeferredHolder;"),&[key]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                DEFERRED,
                "getKey",
                "()Lnet/minecraft/resources/ResourceKey;",
                &[holder]
            ),
            key
        );
        assert_eq!(
            invoke(
                &mut ctx,
                DEFERRED,
                "is",
                "(Lnet/minecraft/resources/ResourceKey;)Z",
                &[holder, key]
            ),
            V::TRUE
        );
    }

    #[test]
    fn registrate_subclass_constructor_preserves_fields_and_reference_binding() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = HolderHost::default();
        let key = new_resource_key(&mut rt, "minecraft:item", "example:registrate");
        let subclass = rt.new_instance("com/tterrag/registrate/util/entry/RegistryEntry", 0);
        let owner = rt.new_instance("example/RegistrateOwner", 0);
        rt.put_field(subclass, "owner", owner);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                DEFERRED,
                "<init>",
                "(Lnet/minecraft/resources/ResourceKey;)V",
            ),
            &[subclass, key],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(subclass, "owner"), owner);
        assert_eq!(
            invoke(
                &mut ctx,
                DEFERRED,
                "getKey",
                "()Lnet/minecraft/resources/ResourceKey;",
                &[subclass]
            ),
            key
        );
        assert_eq!(
            invoke(&mut ctx, DEFERRED, "isBound", "()Z", &[subclass]),
            V::FALSE
        );
        invoke(
            &mut ctx,
            DEFERRED,
            "get",
            "()Ljava/lang/Object;",
            &[subclass],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        let wrong = call(
            ctx.rt,
            &mut RegistryStore::default(),
            &MemberRef::new(DEFERRED, "getKey", "()Ljava/lang/Object;"),
            false,
            &[subclass],
        );
        assert!(wrong.is_none());
    }
}

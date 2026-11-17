//! DeferredRegister subclasses keep their Java fields and virtual createHolder override.
//!
//! The backing registration record remains in RegistryStore; suppliers are retained
//! until registration. No entry values or getEntries contents are fabricated.

use crate::{FmlLoader, PendingDeferredItem};
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, LinkedSpace, Rt, V};

const REGISTER: &str = "net/neoforged/neoforge/registries/DeferredRegister";
const HOLDER: &str = "net/neoforged/neoforge/registries/DeferredHolder";
const ADAPTER: &str = "mmga/internal/DeferredRegisterFactory";
const CREATE_HOLDER:&str="(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/neoforged/neoforge/registries/DeferredHolder;";

pub fn handle(rt: &Rt, receiver: V) -> Option<V> {
    if rt
        .handle_of(receiver)
        .is_some_and(|(kind, ..)| kind == "deferred_register")
    {
        Some(receiver)
    } else {
        let handle = rt.get_field(receiver, "$deferredRegister");
        rt.handle_of(handle)
            .filter(|(kind, ..)| *kind == "deferred_register")
            .map(|_| handle)
    }
}
pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    loader: &mut FmlLoader,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner==ADAPTER&&is_static&&name=="create"&&desc=="(Ljava/util/function/Function;Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;"{
        let mut ctx=ExecCtx{rt,linked,host:loader,depth:0};
        return Some(invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new("java/util/function/Function","apply","(Ljava/lang/Object;)Ljava/lang/Object;"),args));
    }
    if owner == ADAPTER && is_static && name == "fromSupplier"
        && desc == "(Ljava/util/function/Supplier;Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;"
    {
        let mut ctx = ExecCtx { rt, linked, host: loader, depth: 0 };
        return Some(invoke_member(&mut ctx, op::INVOKEINTERFACE,
            &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"), &[*args.first()?]));
    }
    if is_static || !linked.is_subtype(owner, REGISTER, loader) {
        return None;
    }
    let recv = *args.first()?;
    if owner == REGISTER
        && name == "<init>"
        && desc == "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/String;)V"
    {
        let registry = *args.get(1)?;
        let namespace = *args.get(2)?;
        if registry.is_null() || namespace.is_null() {
            return Some(rt.throw_exception(
                "java/lang/NullPointerException",
                "DeferredRegister registryKey/namespace",
            ));
        }
        let (_, registry_id) = crate::minecraft::resource_key_parts(rt, registry)?;
        let namespace_text = rt.str_of(namespace)?.to_owned();
        let id = loader
            .state
            .registries
            .create_deferred_register(&registry_id, &namespace_text);
        let handle = rt.new_pinned_handle(REGISTER, "deferred_register", id, &registry_id);
        rt.put_field(recv, "registryKey", registry);
        rt.put_field(recv, "namespace", namespace);
        rt.put_field(recv, "$deferredRegister", handle);
        return Some(V::Void);
    }
    if name == "createHolder" && desc == CREATE_HOLDER {
        let registry = *args.get(1)?;
        let location = *args.get(2)?;
        return crate::deferred_holders::call(
            rt,
            &mut loader.state.registries,
            &MemberRef::new(HOLDER, "create", CREATE_HOLDER),
            true,
            &[registry, location],
        );
    }
    let native = handle(rt, recv)?;
    let id = rt.handle_of(native)?.1;
    let register = loader.state.registries.deferred_registers.get(&id)?.clone();
    if name == "addAlias" && desc == "(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceLocation;)V" {
        // DeferredRegister keeps aliases until its actual registry event. Null
        // aliases require the original HashMap/BaseMappedRegistry failure path,
        // which this string-backed subset deliberately leaves unsupported.
        let from = *args.get(1)?;
        let to = *args.get(2)?;
        if loader.state.registries.deferred_registration_closed(id) {
            return Some(rt.throw_exception("java/lang/IllegalStateException",
                "Cannot add aliases to DeferredRegister after RegisterEvent has been fired."));
        }
        let ("resource_location", _, from, _) = rt.handle_of(from)? else { return None; };
        let from = from.to_owned();
        let ("resource_location", _, to, _) = rt.handle_of(to)? else { return None; };
        let to = to.to_owned();
        return Some(match loader.state.registries.add_deferred_alias(id, &from, &to) {
            Ok(()) => V::Void,
            Err(message) => rt.throw_exception("java/lang/IllegalStateException", &message),
        });
    }
    if name == "getNamespace" && desc == "()Ljava/lang/String;" {
        return Some(rt.intern_str(&register.namespace));
    }
    if name == "getRegistryKey" && desc == "()Lnet/minecraft/resources/ResourceKey;" {
        return Some(crate::minecraft::new_resource_key(
            rt,
            "minecraft:root",
            &register.registry_key,
        ));
    }
    if name == "getRegistryName" && desc == "()Lnet/minecraft/resources/ResourceLocation;" {
        return Some(location(rt, &register.registry_key));
    }
    if name != "register" {
        return None;
    }
    if desc == "(Lnet/neoforged/bus/api/IEventBus;)V" {
        let bus = *args.get(1)?;
        if register.bound_mod_bus.is_some() {
            return Some(rt.throw_exception(
                "java/lang/IllegalStateException",
                "Cannot register DeferredRegister to more than one event bus.",
            ));
        }
        if bus.is_null() {
            return Some(rt.throw_exception(
                "java/lang/NullPointerException",
                "DeferredRegister event bus",
            ));
        }
        let Some(("mod_event_bus", _, modid, _)) = rt.handle_of(bus) else {
            return None;
        };
        let modid = modid.to_owned();
        loader.state.registries.bind_mod_bus(id, &modid);
        return Some(V::Void);
    }
    let is_function=match desc{
        "(Ljava/lang/String;Ljava/util/function/Function;)Lnet/neoforged/neoforge/registries/DeferredHolder;"=>true,
        "(Ljava/lang/String;Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/registries/DeferredHolder;"=>false,
        _=>return None,
    };
    // Existing native factory handles keep their specialized DeferredItem/Block
    // paths. This adapter owns Java subclass instances that need virtual hooks.
    if native == recv {
        return None;
    }
    if !is_function {
        // The Supplier overload delegates virtually to register(Function).
        let function = rt.alloc("java/util/function/Function", 0, Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Function".into(), sam_name: "apply".into(),
            sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            instantiated_desc: "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;".into(),
            captures: vec![*args.get(2)?], target: ClosureTarget::MethodHandle { handle_kind: 6,
                member: MemberRef::new(ADAPTER, "fromSupplier", "(Ljava/util/function/Supplier;Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;") },
        })));
        let mut ctx = ExecCtx {
            rt,
            linked,
            host: loader,
            depth: 0,
        };
        return Some(invoke_member(&mut ctx, op::INVOKEVIRTUAL,
            &MemberRef::new(REGISTER, "register", "(Ljava/lang/String;Ljava/util/function/Function;)Lnet/neoforged/neoforge/registries/DeferredHolder;"),
            &[recv, *args.get(1)?, function]));
    }
    if loader.state.registries.deferred_registration_closed(id) {
        return Some(rt.throw_exception(
            "java/lang/IllegalStateException",
            "Cannot register new entries to DeferredRegister after RegisterEvent has been fired.",
        ));
    }
    let entry_name = *args.get(1)?;
    let factory = *args.get(2)?;
    if entry_name.is_null() || (is_function && factory.is_null()) {
        return Some(rt.throw_exception(
            "java/lang/NullPointerException",
            "DeferredRegister name/function",
        ));
    }
    let entry_name = rt.str_of(entry_name)?.to_owned();
    if !register
        .namespace
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        || !entry_name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
    {
        return Some(rt.throw_exception(
            "net/minecraft/ResourceLocationException",
            "Invalid deferred registration id",
        ));
    }
    let full_id = format!("{}:{entry_name}", register.namespace);
    let key = location(rt, &full_id);
    let registry = crate::minecraft::new_resource_key(rt, "minecraft:root", &register.registry_key);
    let holder = {
        let mut ctx = ExecCtx {
            rt,
            linked,
            host: loader,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(REGISTER, "createHolder", CREATE_HOLDER),
            &[recv, registry, key],
        )
    };
    if rt.pending_exception.is_some() {
        return Some(V::NULL);
    }
    if loader
        .state
        .registries
        .deferred_registers
        .get(&id)?
        .entries
        .iter()
        .any(|entry| entry.full_id == full_id)
    {
        return Some(rt.throw_exception(
            "java/lang/IllegalArgumentException",
            &format!("Duplicate registration {entry_name}"),
        ));
    }
    if !holder.is_null() && !linked.is_subtype(rt.class_of(holder).unwrap_or(""), HOLDER, loader) {
        return Some(rt.throw_exception(
            "java/lang/ClassCastException",
            "createHolder did not return DeferredHolder",
        ));
    }
    let supplier = rt.alloc("java/util/function/Supplier",0,Data::Closure(Box::new(ClosureData{interface_class:"java/util/function/Supplier".into(),sam_name:"get".into(),sam_desc:"()Ljava/lang/Object;".into(),instantiated_desc:"()Ljava/lang/Object;".into(),captures:vec![factory,key],target:ClosureTarget::MethodHandle{handle_kind:6,member:MemberRef::new(ADAPTER,"create","(Ljava/util/function/Function;Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;")}})));
    rt.pin(supplier);
    rt.pin(holder);
    loader
        .state
        .registries
        .deferred_registers
        .get_mut(&id)?
        .entries
        .push(PendingDeferredItem {
            name: entry_name,
            full_id,
            supplier,
            holder_handle: holder,
        });
    Some(holder)
}

fn location(rt: &mut Rt, id: &str) -> V {
    rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{minecraft::new_resource_key, PreparedLoad};
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, ExecCtx, NativeClassDef, NativeMethodFn, NativeModuleRegistry,
    };
    fn create_holder(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let registry = args[1];
        let location = args[2];
        let calls = ctx.rt.get_field(args[0], "holderCalls").i();
        ctx.rt.put_field(args[0], "holderCalls", V::I(calls + 1));
        let key=invoke_member(ctx,op::INVOKESTATIC,&MemberRef::new("net/minecraft/resources/ResourceKey","create","(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/minecraft/resources/ResourceKey;"),&[registry,location]);
        let holder = ctx.rt.new_instance("test/SpecialHolder", 0);
        ctx.rt.put_field(holder, "owner", args[0]);
        invoke_member(
            ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(HOLDER, "<init>", "(Lnet/minecraft/resources/ResourceKey;)V"),
            &[holder, key],
        );
        holder
    }
    fn make_value(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let counter = args[0];
        let calls = ctx.rt.get_field(counter, "calls").i();
        ctx.rt.put_field(counter, "calls", V::I(calls + 1));
        let value = ctx.rt.new_instance("test/RegisteredValue", 0);
        ctx.rt.put_field(value, "id", args[1]);
        value
    }
    static METHODS:&[(&str,NativeMethodFn,bool,bool)]=&[("createHolder.(Lnet/minecraft/resources/ResourceKey;Lnet/minecraft/resources/ResourceLocation;)Lnet/neoforged/neoforge/registries/DeferredHolder;",create_holder,false,false)];
    static FACTORY: &[(&str, NativeMethodFn, bool, bool)] = &[(
        "apply.(Ljava/lang/Object;)Ljava/lang/Object;",
        make_value,
        false,
        false,
    )];
    static CLASSES: &[NativeClassDef] = &[
        NativeClassDef {
            name: "test/SpecialRegister",
            superclass: Some(REGISTER),
            interfaces: &[],
            is_interface: false,
            methods: METHODS,
        },
        NativeClassDef {
            name: "test/SpecialHolder",
            superclass: Some(HOLDER),
            interfaces: &[],
            is_interface: false,
            methods: &[],
        },
        NativeClassDef {
            name: "test/ValueFactory",
            superclass: Some("java/lang/Object"),
            interfaces: &["java/util/function/Function"],
            is_interface: false,
            methods: FACTORY,
        },
    ];
    #[test]
    fn inherited_aliases_remain_deferred_and_close_with_the_registry_event() {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        prepared
            .linked
            .register_native_module(NativeModuleRegistry {
                classes: CLASSES,
                lambdas: &[],
            });
        let mut rt = Rt::new();
        let receiver = rt.new_instance("test/SpecialRegister", 0);
        let registry = new_resource_key(&mut rt, "minecraft:root", "example:component");
        let namespace = rt.intern_str("example");
        let from = location(&mut rt, "example:legacy");
        let to = location(&mut rt, "example:current");
        let value = rt.new_instance("test/RegisteredValue", 0);
        prepared.loader.state.registries.register_direct(
            &mut rt,
            "example:component",
            "example:current",
            "example",
            value,
            crate::ContentProperties::default(),
        );
        let alias = MemberRef::new("test/SpecialRegister", "addAlias",
            "(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/resources/ResourceLocation;)V");
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            invoke_member(
                &mut ctx,
                op::INVOKESPECIAL,
                &MemberRef::new(
                    REGISTER,
                    "<init>",
                    "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/String;)V",
                ),
                &[receiver, registry, namespace],
            );
            invoke_member(&mut ctx, op::INVOKEVIRTUAL, &alias, &[receiver, from, to]);
            assert!(
                ctx.rt.pending_exception.is_none(),
                "{:?}",
                ctx.rt.pending_exception
            );
        }
        let store = &mut prepared.loader.state.registries;
        assert!(store
            .lookup("example:component", "example:legacy")
            .is_none());
        store.begin_deferred_registration("example:component");
        store.apply_deferred_aliases("example:component").unwrap();
        assert_eq!(
            store
                .lookup("example:component", "example:legacy")
                .unwrap()
                .value,
            value
        );
        assert!(!store.contains_entry("example:component", "example:legacy"));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke_member(&mut ctx, op::INVOKEVIRTUAL, &alias, &[receiver, from, to]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalStateException")
        );
    }

    #[test]
    fn subclass_constructor_and_registration_keep_fields_override_and_lazy_factory() {
        let mut prepared = PreparedLoad::from_jars(vec![], true).unwrap();
        prepared
            .linked
            .register_native_module(NativeModuleRegistry {
                classes: CLASSES,
                lambdas: &[],
            });
        let mut rt = Rt::new();
        let receiver = rt.new_instance("test/SpecialRegister", 0);
        let marker = rt.new_instance("test/Marker", 0);
        rt.put_field(receiver, "marker", marker);
        let registry = new_resource_key(&mut rt, "minecraft:root", "minecraft:item");
        let namespace = rt.intern_str("example");
        let name = rt.intern_str("machine");
        let factory = rt.new_instance("test/ValueFactory", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(
                REGISTER,
                "<init>",
                "(Lnet/minecraft/resources/ResourceKey;Ljava/lang/String;)V",
            ),
            &[receiver, registry, namespace],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(receiver, "marker"), marker);
        assert_eq!(ctx.rt.class_of(receiver), Some("test/SpecialRegister"));
        let holder=invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new(REGISTER,"register","(Ljava/lang/String;Ljava/util/function/Function;)Lnet/neoforged/neoforge/registries/DeferredHolder;"),&[receiver,name,factory]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.class_of(holder), Some("test/SpecialHolder"));
        assert_eq!(ctx.rt.get_field(holder, "owner"), receiver);
        assert_eq!(ctx.rt.get_field(receiver, "holderCalls"), V::I(1));
        assert_eq!(ctx.rt.get_field(factory, "calls").i(), 0);
        let bus = ctx.rt.new_pinned_handle(
            "net/neoforged/bus/api/IEventBus",
            "mod_event_bus",
            1,
            "example",
        );
        invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(REGISTER, "register", "(Lnet/neoforged/bus/api/IEventBus;)V"),
            &[receiver, bus],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let dr = handle(ctx.rt, receiver).unwrap();
        let id = ctx.rt.handle_of(dr).unwrap().1;
        drop(ctx);
        assert_eq!(
            prepared.loader.state.registries.deferred_registers[&id]
                .bound_mod_bus
                .as_deref(),
            Some("example")
        );
        let pending = prepared.loader.state.registries.pending_suppliers();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].full_id, "example:machine");
        let supplier = pending[0].supplier;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let value = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
            &[supplier],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(factory, "calls"), V::I(1));
        assert_eq!(
            ctx.rt.handle_of(ctx.rt.get_field(value, "id")).unwrap().2,
            "example:machine"
        );
        drop(ctx);
        prepared.loader.state.registries.register_direct(
            &mut rt,
            "minecraft:item",
            "example:machine",
            "example",
            value,
            crate::ContentProperties::default(),
        );
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        assert_eq!(
            invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(HOLDER, "get", "()Ljava/lang/Object;"),
                &[holder]
            ),
            value
        );
        let duplicate=invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new(REGISTER,"register","(Ljava/lang/String;Ljava/util/function/Function;)Lnet/neoforged/neoforge/registries/DeferredHolder;"),&[receiver,name,factory]);
        assert!(duplicate.is_null());
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalArgumentException")
        );
        assert_eq!(ctx.rt.get_field(factory, "calls"), V::I(1));
    }
}

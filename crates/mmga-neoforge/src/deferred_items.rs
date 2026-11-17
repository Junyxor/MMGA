//! DeferredRegister.Items adapters retain the factory and the same properties
//! object until the registry phase. They do not construct an item early.
use crate::RegistryStore;
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace, Rt, V,
};

const ITEMS: &str = "net/neoforged/neoforge/registries/DeferredRegister$Items";
const HOLDER: &str = "net/neoforged/neoforge/registries/DeferredItem";
const ADAPTER: &str = "mmga/internal/DeferredItemFactory";

pub fn register(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static || member.owner != ITEMS || member.name != "registerItem"
        || member.desc != format!("(Ljava/lang/String;Ljava/util/function/Function;Lnet/minecraft/world/item/Item$Properties;)L{HOLDER};") {
        return None;
    }
    let (kind, id, _, _) = rt.handle_of(args[0])?;
    if kind != "deferred_register" {
        return None;
    }
    let id = id;
    if args[1].is_null() {
        return Some(rt.throw_exception("java/lang/NullPointerException", "Deferred item name"));
    }
    let entry_name = rt.str_of(args[1])?.to_owned();
    let Some(register) = registries.deferred_registers.get(&id) else {
        return None;
    };
    if register.registry_key != "minecraft:item" {
        return None;
    }
    if registries.frozen {
        return Some(rt.throw_exception(
            "java/lang/IllegalStateException",
            "Cannot register items after the register event",
        ));
    }
    if register.entries.iter().any(|e| e.name == entry_name) {
        return Some(rt.throw_exception(
            "java/lang/IllegalArgumentException",
            &format!("Duplicate registration {entry_name}"),
        ));
    }
    let namespace = register.namespace.clone();
    if !namespace
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        || !entry_name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
    {
        return Some(rt.throw_exception(
            "net/minecraft/ResourceLocationException",
            "Invalid item registration location",
        ));
    }
    let supplier = rt.alloc("java/util/function/Supplier", 0, Data::Closure(Box::new(ClosureData {
        interface_class: "java/util/function/Supplier".into(),
        sam_name: "get".into(),
        sam_desc: "()Ljava/lang/Object;".into(),
        instantiated_desc: "()Lnet/minecraft/world/item/Item;".into(),
        captures: vec![args[2], args[3]],
        target: ClosureTarget::MethodHandle { handle_kind: 6, member: MemberRef::new(ADAPTER, "create", "(Ljava/util/function/Function;Lnet/minecraft/world/item/Item$Properties;)Lnet/minecraft/world/item/Item;") },
    })));
    registries.enqueue_deferred_entry(rt, id, HOLDER, &entry_name, supplier)
}

pub fn call_factory(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if !is_static || member.owner != ADAPTER || member.name != "create" || member.desc != "(Ljava/util/function/Function;Lnet/minecraft/world/item/Item$Properties;)Lnet/minecraft/world/item/Item;" { return None; }
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    let value = invoke_member(
        &mut ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            "java/util/function/Function",
            "apply",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
        ),
        &[args[0], args[1]],
    );
    if ctx.rt.pending_exception.is_none() && value.is_null() {
        return Some(ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "Item factory returned null during registration",
        ));
    }
    if ctx.rt.pending_exception.is_none() {
        let class = ctx.rt.class_of(value).unwrap_or("");
        if !ctx
            .linked
            .is_subtype(class, "net/minecraft/world/item/Item", ctx.host)
        {
            return Some(ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "Item factory did not return Item",
            ));
        }
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{NativeClassDef, NativeModuleRegistry};

    fn factory(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let previous = ctx.rt.get_field(args[0], "calls").i();
        ctx.rt.put_field(args[0], "calls", V::I(previous + 1));
        let item = ctx.rt.new_instance("test/DeferredItem", 0);
        ctx.rt.put_field(item, "properties", args[1]);
        item
    }

    #[test]
    fn item_factory_runs_at_registration_with_original_properties() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        prepared
            .linked
            .register_native_module(NativeModuleRegistry {
                classes: &[
                    NativeClassDef {
                        name: "test/ItemFactory",
                        superclass: Some("java/lang/Object"),
                        interfaces: &["java/util/function/Function"],
                        is_interface: false,
                        methods: &[(
                            "apply.(Ljava/lang/Object;)Ljava/lang/Object;",
                            factory,
                            false,
                            false,
                        )],
                    },
                    NativeClassDef {
                        name: "test/DeferredItem",
                        superclass: Some("net/minecraft/world/item/Item"),
                        interfaces: &[],
                        is_interface: false,
                        methods: &[],
                    },
                ],
                lambdas: &[],
            });
        let mut rt = Rt::new();
        let factory = rt.new_instance("test/ItemFactory", 0);
        let props = rt.new_instance("net/minecraft/world/item/Item$Properties", 0);
        let namespace = rt.intern_str("test");
        let name = rt.intern_str("book");
        let holder;
        {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &prepared.linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            let register = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    "net/neoforged/neoforge/registries/DeferredRegister",
                    "createItems",
                    &format!("(Ljava/lang/String;)L{ITEMS};"),
                ),
                &[namespace],
            );
            holder = invoke_member(&mut ctx, op::INVOKEVIRTUAL, &MemberRef::new(ITEMS, "registerItem", &format!("(Ljava/lang/String;Ljava/util/function/Function;Lnet/minecraft/world/item/Item$Properties;)L{HOLDER};")), &[register, name, factory, props]);
            assert!(
                ctx.rt.pending_exception.is_none(),
                "{:?}",
                ctx.rt.pending_exception
            );
            assert_eq!(ctx.rt.get_field(factory, "calls").i(), 0);
        }
        rt.put_field(props, "maxStackSize", V::I(7));
        prepared.bootstrap(&mut rt).unwrap();
        assert_eq!(rt.get_field(factory, "calls").i(), 1);
        let id = rt.handle_of(holder).unwrap().1;
        let actual = prepared.loader.state.registries.holder_value(id).unwrap();
        assert_eq!(rt.get_field(actual, "properties"), props);
        assert_eq!(rt.get_field(props, "maxStackSize").i(), 7);
        assert_eq!(
            prepared
                .loader
                .state
                .registries
                .lookup("item", "test:book")
                .unwrap()
                .value,
            actual
        );
    }

    #[test]
    fn null_factory_fails_only_when_deferred_supplier_is_run() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let dr = prepared
            .loader
            .state
            .registries
            .create_deferred_register("item", "test");
        let register = rt.new_pinned_handle(ITEMS, "deferred_register", dr, "minecraft:item");
        let name = rt.intern_str("invalid");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke_member(&mut ctx, op::INVOKEVIRTUAL, &MemberRef::new(ITEMS, "registerItem", &format!("(Ljava/lang/String;Ljava/util/function/Function;Lnet/minecraft/world/item/Item$Properties;)L{HOLDER};")), &[register, name, V::NULL, V::NULL]);
        assert!(ctx.rt.pending_exception.is_none());
        let error = prepared.bootstrap(&mut rt).unwrap_err();
        assert!(error.contains("NullPointerException"), "{error}");
        assert!(prepared
            .loader
            .state
            .registries
            .lookup("item", "test:invalid")
            .is_none());
    }
}

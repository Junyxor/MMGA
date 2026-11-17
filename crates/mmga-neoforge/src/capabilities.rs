//! NeoForge capability identity and type metadata. Providers/world queries are
//! separate operations and remain unsupported until they can actually execute.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, Rt, V};

const BASE: &str = "net/neoforged/neoforge/capabilities/BaseCapability";
const BLOCK: &str = "net/neoforged/neoforge/capabilities/BlockCapability";
const ENTITY: &str = "net/neoforged/neoforge/capabilities/EntityCapability";
const ITEM: &str = "net/neoforged/neoforge/capabilities/ItemCapability";

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sup == BASE && matches!(sub, BLOCK | ENTITY | ITEM)
}

fn class_tag(rt: &Rt, value: V) -> Option<&str> {
    rt.handle_of(value)
        .filter(|(kind, _, _, _)| *kind == "class")
        .map(|(_, _, tag, _)| tag)
}

fn class(rt: &mut Rt, name: &str) -> V {
    rt.new_pinned_handle("java/lang/Class", "class", 0, name)
}

fn create(rt: &mut Rt, owner: &str, name: V, typ: V, context: V) -> V {
    if name.is_null() || typ.is_null() || context.is_null() {
        return rt.throw_exception(
            "java/lang/NullPointerException",
            "Capability name, type and context must not be null",
        );
    }
    let Some(("resource_location", _, identifier, _)) = rt.handle_of(name) else {
        return rt.throw_exception(
            "java/lang/ClassCastException",
            "Capability name must be ResourceLocation",
        );
    };
    let identifier = identifier.to_owned();
    let (Some(typ_name), Some(context_name)) = (class_tag(rt, typ), class_tag(rt, context)) else {
        return rt.throw_exception(
            "java/lang/ClassCastException",
            "Capability types must be Class values",
        );
    };
    let cache_key = format!("$capability:{identifier}");
    if let Some(existing) = rt.get_static(owner, &cache_key) {
        let existing_type = class_tag(rt, rt.get_field(existing, "typeClass"));
        let existing_context = class_tag(rt, rt.get_field(existing, "contextClass"));
        // The VM's Class handles may be separate allocations for the same class;
        // their class-space names identify the Java Class, not heap reference IDs.
        if existing_type != Some(typ_name) || existing_context != Some(context_name) {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                &format!(
                    "Capability {identifier} already exists with a different type or context class"
                ),
            );
        }
        return existing;
    }
    let capability = rt.new_instance(owner, 0);
    rt.put_field(capability, "name", name);
    rt.put_field(capability, "typeClass", typ);
    rt.put_field(capability, "contextClass", context);
    rt.put_static(owner, &cache_key, capability);
    let known = match rt.get_static(owner, "$capabilities") {
        Some(list) => list,
        None => {
            let list = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
            rt.put_static(owner, "$capabilities", list);
            list
        }
    };
    if let Some(Data::List(values)) = rt.get_obj_mut(known).map(|obj| &mut obj.data) {
        values.push(capability);
    }
    capability
}

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    let (path, queried_type) = match owner {
        "net/neoforged/neoforge/capabilities/Capabilities$EnergyStorage" => {
            ("energy", "net/neoforged/neoforge/energy/IEnergyStorage")
        }
        "net/neoforged/neoforge/capabilities/Capabilities$FluidHandler" => (
            "fluid_handler",
            "net/neoforged/neoforge/fluids/capability/IFluidHandler",
        ),
        "net/neoforged/neoforge/capabilities/Capabilities$ItemHandler" => {
            ("item_handler", "net/neoforged/neoforge/items/IItemHandler")
        }
        _ => return None,
    };
    let (kind, context) = match name {
        "BLOCK" => (BLOCK, "net/minecraft/core/Direction"),
        "ENTITY" => (
            ENTITY,
            if path == "item_handler" {
                "void"
            } else {
                "net/minecraft/core/Direction"
            },
        ),
        "ENTITY_AUTOMATION" if path == "item_handler" => (ENTITY, "net/minecraft/core/Direction"),
        "ITEM" => (ITEM, "void"),
        _ => return None,
    };
    if desc != format!("L{kind};") {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let id = format!(
        "neoforge:{}",
        if name == "ENTITY_AUTOMATION" {
            "item_handler_automation"
        } else {
            path
        }
    );
    let location = rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        id,
    );
    let queried_type = if kind == ITEM && path == "fluid_handler" {
        "net/neoforged/neoforge/fluids/capability/IFluidHandlerItem"
    } else {
        queried_type
    };
    let typ = class(rt, queried_type);
    let context = class(rt, context);
    let capability = create(rt, kind, location, typ, context);
    if rt.pending_exception.is_none() {
        rt.put_static(owner, name, capability);
    }
    Some(capability)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if is_static && matches!(owner, BLOCK | ENTITY | ITEM) {
        let constructor = format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;Ljava/lang/Class;)L{owner};");
        let simple =
            format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{owner};");
        if name == "create" && desc == constructor {
            return Some(create(rt, owner, args[0], args[1], args[2]));
        }
        if desc == simple && (name == "createVoid" || name == "createSided" && owner != ITEM) {
            let context = class(
                rt,
                if name == "createVoid" {
                    "void"
                } else {
                    "net/minecraft/core/Direction"
                },
            );
            return Some(create(rt, owner, args[0], args[1], context));
        }
        if name == "getAll" && desc == "()Ljava/util/List;" {
            let known = rt.get_static(owner, "$capabilities").unwrap_or(V::NULL);
            let values = match rt.get_obj(known).map(|obj| &obj.data) {
                Some(Data::List(xs)) => xs.clone(),
                _ => Vec::new(),
            };
            return Some(rt.alloc(
                "java/util/ImmutableCollections$ListN",
                0,
                Data::List(values),
            ));
        }
    }
    if !is_static && matches!(owner, BASE | BLOCK | ENTITY | ITEM) {
        match (name, desc) {
            ("name", "()Lnet/minecraft/resources/ResourceLocation;")
            | ("typeClass" | "contextClass", "()Ljava/lang/Class;") => {
                return Some(rt.get_field(args[0], name))
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreparedLoad;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx};

    fn class(rt: &mut Rt, name: &str) -> V {
        rt.new_pinned_handle("java/lang/Class", "class", 0, name)
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

    #[test]
    fn balm_factories_preserve_identity_metadata_and_type_conflicts() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let location = rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "balm:container",
        );
        let container = class(&mut rt, "net/minecraft/world/Container");
        let direction = class(&mut rt, "net/minecraft/core/Direction");
        let wrong = class(&mut rt, "java/lang/String");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let sided = invoke(
            &mut ctx,
            BLOCK,
            "createSided",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{BLOCK};"),
            true,
            &[location, container],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        let same = invoke(&mut ctx, BLOCK, "create", &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;Ljava/lang/Class;)L{BLOCK};"), true, &[location, container, direction]);
        assert_eq!(sided, same);
        assert_eq!(
            invoke(
                &mut ctx,
                BASE,
                "name",
                "()Lnet/minecraft/resources/ResourceLocation;",
                false,
                &[sided]
            ),
            location
        );
        assert_eq!(
            invoke(
                &mut ctx,
                BASE,
                "typeClass",
                "()Ljava/lang/Class;",
                false,
                &[sided]
            ),
            container
        );
        let context = invoke(
            &mut ctx,
            BASE,
            "contextClass",
            "()Ljava/lang/Class;",
            false,
            &[sided],
        );
        assert_eq!(
            ctx.rt.handle_of(context).unwrap().2,
            "net/minecraft/core/Direction"
        );
        invoke(
            &mut ctx,
            BLOCK,
            "createSided",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{BLOCK};"),
            true,
            &[location, wrong],
        );
        let exception = ctx.rt.take_exception().expect("type conflict");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("IllegalStateException"));
        invoke(
            &mut ctx,
            BLOCK,
            "createVoid",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{BLOCK};"),
            true,
            &[location, container],
        );
        let exception = ctx.rt.take_exception().expect("context conflict");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("IllegalStateException"));
        let item = invoke(
            &mut ctx,
            ITEM,
            "createVoid",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{ITEM};"),
            true,
            &[location, container],
        );
        assert_ne!(
            item, sided,
            "separate capability kinds have separate registries"
        );
        let context = invoke(
            &mut ctx,
            BASE,
            "contextClass",
            "()Ljava/lang/Class;",
            false,
            &[item],
        );
        assert_eq!(ctx.rt.handle_of(context).unwrap().2, "void");
    }

    #[test]
    fn builtin_capabilities_share_factories_and_immutable_snapshots() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let owner = "net/neoforged/neoforge/capabilities/Capabilities$EnergyStorage";
        let builtin = read_static_field(&mut ctx, owner, "BLOCK", &format!("L{BLOCK};"));
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        let location = invoke(
            &mut ctx,
            BASE,
            "name",
            "()Lnet/minecraft/resources/ResourceLocation;",
            false,
            &[builtin],
        );
        assert_eq!(ctx.rt.handle_of(location).unwrap().2, "neoforge:energy");
        let typ = invoke(
            &mut ctx,
            BASE,
            "typeClass",
            "()Ljava/lang/Class;",
            false,
            &[builtin],
        );
        let same = invoke(
            &mut ctx,
            BLOCK,
            "createSided",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{BLOCK};"),
            true,
            &[location, typ],
        );
        assert_eq!(builtin, same);
        let list = invoke(&mut ctx, BLOCK, "getAll", "()Ljava/util/List;", true, &[]);
        let before = match &ctx.rt.get_obj(list).unwrap().data {
            Data::List(xs) => xs.len(),
            _ => panic!(),
        };
        assert_eq!(before, 1);
        invoke(&mut ctx, "java/util/List", "clear", "()V", false, &[list]);
        let exception = ctx.rt.take_exception().expect("snapshot immutable");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("UnsupportedOperationException"));
        let location2 = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "test:another",
        );
        invoke(
            &mut ctx,
            BLOCK,
            "createSided",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{BLOCK};"),
            true,
            &[location2, typ],
        );
        assert_eq!(
            match &ctx.rt.get_obj(list).unwrap().data {
                Data::List(xs) => xs.len(),
                _ => panic!(),
            },
            before
        );
        invoke(
            &mut ctx,
            BLOCK,
            "createSided",
            &format!("(Lnet/minecraft/resources/ResourceLocation;Ljava/lang/Class;)L{BLOCK};"),
            true,
            &[V::NULL, typ],
        );
        let exception = ctx.rt.take_exception().expect("null name");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("NullPointerException"));
        assert!(super::call(
            ctx.rt,
            &MemberRef::new(BLOCK, "getCapability", "()Ljava/lang/Object;"),
            false,
            &[builtin]
        )
        .is_none());
    }
}

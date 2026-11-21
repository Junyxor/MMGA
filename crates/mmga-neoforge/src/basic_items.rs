//! Exact base Item / BlockItem construction used by the native content bridge.
//! Custom subclasses and unimplemented component behavior are not substituted.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, read_static_field, Data, ExecCtx, LinkedSpace, NativeClassDef,
    NativeModuleRegistry, Rt, V,
};

pub const ITEM: &str = "net/minecraft/world/item/Item";
pub const BLOCK_ITEM: &str = "net/minecraft/world/item/BlockItem";
const PROPERTIES: &str = "net/minecraft/world/item/Item$Properties";
const BLOCK: &str = "net/minecraft/world/level/block/Block";

pub fn register_types(linked: &mut LinkedSpace) {
    linked.register_native_module(NativeModuleRegistry {
        classes: &[
            NativeClassDef {
                name: ITEM,
                superclass: Some("java/lang/Object"),
                interfaces: &[
                    "net/minecraft/world/level/ItemLike",
                    "net/minecraft/world/flag/FeatureElement",
                ],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: BLOCK_ITEM,
                superclass: Some(ITEM),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
        ],
        lambdas: &[],
    });
}

fn field(rt: &Rt, value: V, key: &str) -> Option<V> {
    match rt.get_obj(value).map(|object| &object.data) {
        Some(Data::Fields(fields)) => fields.get(key).copied(),
        _ => None,
    }
}

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if owner == PROPERTIES {
        if name == "<init>" && desc == "()V" {
            ctx.rt.put_field(recv, "maxStackSize", V::I(64));
            return Some(V::Void);
        }
        if name == "stacksTo" && desc == format!("(I)L{PROPERTIES};") {
            ctx.rt.put_field(recv, "maxStackSize", args[1]);
            return Some(recv);
        }
        if name == "durability" && desc == format!("(I)L{PROPERTIES};") {
            ctx.rt.put_field(recv, "maxDamage", args[1]);
            ctx.rt.put_field(recv, "maxStackSize", V::I(1));
            ctx.rt.put_field(recv, "damage", V::I(0));
            return Some(recv);
        }
        if name == "rarity" && desc == format!("(Lnet/minecraft/world/item/Rarity;)L{PROPERTIES};")
        {
            ctx.rt.put_field(recv, "rarity", args[1]);
            return Some(recv);
        }
        if name == "fireResistant" && desc == format!("()L{PROPERTIES};") {
            ctx.rt.put_field(recv, "fireResistant", V::TRUE);
            return Some(recv);
        }
    }
    if owner == BLOCK_ITEM && name == "<init>" && desc == format!("(L{BLOCK};L{PROPERTIES};)V") {
        let value = invoke_member(
            ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(ITEM, "<init>", &format!("(L{PROPERTIES};)V")),
            &[recv, args[2]],
        );
        if ctx.rt.pending_exception.is_none() {
            ctx.rt.put_field(recv, "block", args[1]);
        }
        return Some(value);
    }
    if owner == ITEM && name == "<init>" && desc == format!("(L{PROPERTIES};)V") {
        let props = args[1];
        if props.is_null() {
            return Some(
                ctx.rt
                    .throw_exception("java/lang/NullPointerException", "Item properties"),
            );
        }
        let max_stack = field(ctx.rt, props, "maxStackSize").unwrap_or(V::I(64));
        if field(ctx.rt, props, "damage").is_some() && max_stack.i() > 1 {
            return Some(ctx.rt.throw_exception(
                "java/lang/IllegalStateException",
                "Item cannot have both durability and be stackable",
            ));
        }
        ctx.rt.put_field(recv, "properties", props);
        ctx.rt.put_field(recv, "maxStackSize", max_stack);
        for name in ["maxDamage", "damage", "rarity", "fireResistant"] {
            if let Some(value) = field(ctx.rt, props, name) {
                ctx.rt.put_field(recv, name, value);
            }
        }
        let registry = read_static_field(
            ctx,
            "net/minecraft/core/registries/BuiltInRegistries",
            "ITEM",
            "Lnet/minecraft/core/DefaultedRegistry;",
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let holder = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "net/minecraft/core/Registry",
                "createIntrusiveHolder",
                "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;",
            ),
            &[registry, recv],
        );
        if ctx.rt.pending_exception.is_none() {
            ctx.rt.put_field(recv, "builtInRegistryHolder", holder);
        }
        return Some(if ctx.rt.pending_exception.is_some() {
            V::NULL
        } else {
            V::Void
        });
    }
    if !matches!(
        owner,
        ITEM | BLOCK_ITEM | "net/minecraft/world/level/ItemLike"
    ) {
        return None;
    }
    let class = ctx.rt.class_of(recv)?.to_owned();
    if !ctx.linked.is_subtype(&class, ITEM, ctx.host) {
        return None;
    }
    match (name, desc) {
        ("asItem", "()Lnet/minecraft/world/item/Item;") => Some(recv),
        ("getBlock", "()Lnet/minecraft/world/level/block/Block;")
            if ctx.rt.class_of(recv) == Some(BLOCK_ITEM) =>
        {
            Some(ctx.rt.get_field(recv, "block"))
        }
        ("getDefaultMaxStackSize", "()I") => field(ctx.rt, recv, "maxStackSize"),
        ("builtInRegistryHolder", "()Lnet/minecraft/core/Holder$Reference;") => {
            field(ctx.rt, recv, "builtInRegistryHolder")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreparedLoad;

    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            if name == "<init>" {
                op::INVOKESPECIAL
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    #[test]
    fn base_items_snapshot_properties_and_block_items_keep_the_original_block() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let block = rt.new_instance(BLOCK, 0);
        let props = rt.new_instance(PROPERTIES, 0);
        let first = rt.new_instance(BLOCK_ITEM, 0);
        let second = rt.new_instance(ITEM, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke(&mut ctx, PROPERTIES, "<init>", "()V", &[props]);
        invoke(
            &mut ctx,
            PROPERTIES,
            "stacksTo",
            &format!("(I)L{PROPERTIES};"),
            &[props, V::I(16)],
        );
        invoke(
            &mut ctx,
            BLOCK_ITEM,
            "<init>",
            &format!("(L{BLOCK};L{PROPERTIES};)V"),
            &[first, block, props],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        invoke(
            &mut ctx,
            PROPERTIES,
            "stacksTo",
            &format!("(I)L{PROPERTIES};"),
            &[props, V::I(7)],
        );
        invoke(
            &mut ctx,
            ITEM,
            "<init>",
            &format!("(L{PROPERTIES};)V"),
            &[second, props],
        );
        assert_eq!(
            invoke(&mut ctx, ITEM, "getDefaultMaxStackSize", "()I", &[first]),
            V::I(16)
        );
        assert_eq!(
            invoke(&mut ctx, ITEM, "getDefaultMaxStackSize", "()I", &[second]),
            V::I(7)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                BLOCK_ITEM,
                "getBlock",
                &format!("()L{BLOCK};"),
                &[first]
            ),
            block
        );
        assert_ne!(ctx.rt.get_field(first, "builtInRegistryHolder"), V::NULL);
        assert!(ctx.linked.is_subtype(BLOCK_ITEM, ITEM, ctx.host));
    }

    #[test]
    fn invalid_item_signatures_cannot_fall_back_to_legacy_success() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let props = rt.new_instance(PROPERTIES, 0);
        let item = rt.new_instance(ITEM, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke(&mut ctx, ITEM, "<init>", "()V", &[item]);
        let exception = ctx.rt.take_exception().expect("unsupported constructor");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("UnsupportedOperationException"));
        invoke(&mut ctx, PROPERTIES, "<init>", "()V", &[props]);
        invoke(
            &mut ctx,
            PROPERTIES,
            "stacksTo",
            &format!("(J)L{PROPERTIES};"),
            &[props, V::J(12)],
        );
        let exception = ctx.rt.take_exception().expect("unsupported overload");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("UnsupportedOperationException"));
        assert_eq!(ctx.rt.get_field(props, "maxStackSize"), V::I(64));
    }

    #[test]
    fn durable_item_validation_matches_vanilla_and_does_not_invent_a_fallback() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let props = rt.new_instance(PROPERTIES, 0);
        let item = rt.new_instance(ITEM, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        invoke(&mut ctx, PROPERTIES, "<init>", "()V", &[props]);
        invoke(
            &mut ctx,
            PROPERTIES,
            "durability",
            &format!("(I)L{PROPERTIES};"),
            &[props, V::I(200)],
        );
        invoke(
            &mut ctx,
            PROPERTIES,
            "stacksTo",
            &format!("(I)L{PROPERTIES};"),
            &[props, V::I(2)],
        );
        invoke(
            &mut ctx,
            ITEM,
            "<init>",
            &format!("(L{PROPERTIES};)V"),
            &[item, props],
        );
        let exception = ctx
            .rt
            .take_exception()
            .expect("durability plus stack size must fail");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("Item cannot have both durability and be stackable"));
    }
}

//! Minecraft 1.21.1 InteractionHand enum metadata.
//!
//! Official client mappings and `bqq.class` bytecode define MAIN_HAND at ordinal 0 and
//! OFF_HAND at ordinal 1. This module models the enum values and reflection metadata only.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt, V};

const CLASS: &str = "net/minecraft/world/InteractionHand";
const DESC: &str = "Lnet/minecraft/world/InteractionHand;";
const VALUES_DESC: &str = "()[Lnet/minecraft/world/InteractionHand;";
const VALUE_OF_DESC: &str = "(Ljava/lang/String;)Lnet/minecraft/world/InteractionHand;";
const NAMES: [&str; 2] = ["MAIN_HAND", "OFF_HAND"];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != CLASS || desc != DESC {
        return None;
    }
    constant(rt, name)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if member.owner != CLASS {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str(), is_static) {
        ("values", VALUES_DESC, true) => {
            let values = NAMES
                .iter()
                .map(|name| constant(rt, name).expect("listed enum constant"))
                .collect();
            Some(rt.alloc(
                "[Lnet/minecraft/world/InteractionHand;",
                0,
                Data::Array(values),
            ))
        }
        ("valueOf", VALUE_OF_DESC, true) => {
            let input = *args.first()?;
            if input.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let name = rt.str_of(input)?.to_owned();
            Some(constant(rt, &name).unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!("No enum constant net.minecraft.world.InteractionHand.{name}"),
                )
            }))
        }
        _ => None,
    }
}

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    sub_class == CLASS
        && matches!(
            super_class,
            "java/lang/Enum" | "java/lang/Comparable" | "java/io/Serializable"
        )
}

pub fn register_types(linked: &mut LinkedSpace) {
    linked.register_native_module(NativeModuleRegistry {
        classes: &[NativeClassDef {
            name: CLASS,
            superclass: Some("java/lang/Enum"),
            interfaces: &[],
            is_interface: false,
            methods: &[],
        }],
        lambdas: &[],
    });
}

fn constant(rt: &mut Rt, name: &str) -> Option<V> {
    let ordinal = NAMES.iter().position(|&candidate| candidate == name)?;
    if let Some(value) = rt.get_static(CLASS, name) {
        return Some(value);
    }
    let value = rt.new_instance(CLASS, 0);
    let name = rt.intern_str(name);
    rt.put_field(value, "$name", name);
    rt.put_field(value, "$ordinal", V::I(ordinal as i32));
    rt.put_static(CLASS, NAMES[ordinal], value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi};

    struct Host;

    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            get_static_field(rt, owner, name, desc)
        }

        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(rt, member, is_static, args)
        }

        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            is_subtype(sub_class, super_class)
        }
    }

    fn invoke(ctx: &mut ExecCtx<'_>, member: &str, desc: &str, is_static: bool, args: &[V]) -> V {
        invoke_member(
            ctx,
            if is_static {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(CLASS, member, desc),
            args,
        )
    }

    #[test]
    fn interaction_hand_values_and_class_enum_constants_match_official_order() {
        let mut linked = LinkedSpace::new();
        register_types(&mut linked);
        let mut rt = Rt::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };

        let enum_class = ctx
            .rt
            .new_pinned_handle("java/lang/Class", "class", 0, CLASS);
        let constants = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "java/lang/Class",
                "getEnumConstants",
                "()[Ljava/lang/Object;",
            ),
            &[enum_class],
        );
        let values = match &ctx.rt.get_obj(constants).unwrap().data {
            Data::Array(values) => values.clone(),
            other => panic!("expected enum constant array, got {other:?}"),
        };
        assert_eq!(values.len(), NAMES.len());

        let returned = invoke(&mut ctx, "values", VALUES_DESC, true, &[]);
        assert_ne!(constants, returned);
        assert!(
            matches!(&ctx.rt.get_obj(returned).unwrap().data, Data::Array(next) if next == &values)
        );
        for (ordinal, name) in NAMES.iter().enumerate() {
            let value = get_static_field(ctx.rt, CLASS, name, DESC).unwrap();
            assert_eq!(value, values[ordinal]);
            assert_eq!(ctx.rt.str_of(ctx.rt.get_field(value, "$name")), Some(*name));
            assert_eq!(ctx.rt.get_field(value, "$ordinal"), V::I(ordinal as i32));
            let label = ctx.rt.intern_str(name);
            assert_eq!(
                invoke(&mut ctx, "valueOf", VALUE_OF_DESC, true, &[label]),
                value
            );
        }
        assert!(linked.is_subtype(CLASS, "java/lang/Enum", ctx.host));
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn interaction_hand_value_of_rejects_null_and_unknown_names() {
        let mut rt = Rt::new();
        for input in [V::NULL, rt.intern_str("main_hand")] {
            call(
                &mut rt,
                &MemberRef::new(CLASS, "valueOf", VALUE_OF_DESC),
                true,
                &[input],
            );
            let exception = rt.take_exception().unwrap();
            let expected = if input.is_null() {
                "java/lang/NullPointerException"
            } else {
                "java/lang/IllegalArgumentException"
            };
            assert_eq!(rt.class_of(exception), Some(expected));
        }
        assert!(get_static_field(&mut rt, CLASS, "MAIN_HAND", DESC).is_some());
        assert!(get_static_field(&mut rt, CLASS, "MAINHAND", DESC).is_none());
    }
}

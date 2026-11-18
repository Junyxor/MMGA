//! Minecraft 1.21.1 item rarity enum metadata.
//!
//! The official enum has four constants in declaration order. This models only their
//! identity, id, serialized name, color, and enum reflection methods.

use mmga_classfile::MemberRef;
use mmga_runtime::{
    Data, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt, V,
};

const CLASS: &str = "net/minecraft/world/item/Rarity";
const DESC: &str = "Lnet/minecraft/world/item/Rarity;";
const VALUES_DESC: &str = "()[Lnet/minecraft/world/item/Rarity;";
const VALUE_OF_DESC: &str = "(Ljava/lang/String;)Lnet/minecraft/world/item/Rarity;";
const NAMES: [&str; 4] = ["COMMON", "UNCOMMON", "RARE", "EPIC"];
const VALUES: [(&str, &str, &str); 4] = [
    ("COMMON", "common", "WHITE"),
    ("UNCOMMON", "uncommon", "YELLOW"),
    ("RARE", "rare", "AQUA"),
    ("EPIC", "epic", "LIGHT_PURPLE"),
];

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
            Some(rt.alloc("[Lnet/minecraft/world/item/Rarity;", 0, Data::Array(values)))
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
                    &format!("No enum constant net.minecraft.world.item.Rarity.{name}"),
                )
            }))
        }
        ("getSerializedName", "()Ljava/lang/String;", false) => {
            Some(rt.get_field(*args.first()?, "name"))
        }
        ("color", "()Lnet/minecraft/ChatFormatting;", false) => {
            Some(rt.get_field(*args.first()?, "color"))
        }
        _ => None,
    }
}

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    sub_class == CLASS
        && matches!(
            super_class,
            "java/lang/Enum"
                | "java/lang/Comparable"
                | "java/io/Serializable"
                | "net/minecraft/util/StringRepresentable"
        )
}

pub fn register_types(linked: &mut LinkedSpace) {
    linked.register_native_module(NativeModuleRegistry {
        classes: &[NativeClassDef {
            name: CLASS,
            superclass: Some("java/lang/Enum"),
            interfaces: &["net/minecraft/util/StringRepresentable"],
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
    let (_, serialized, color_name) = VALUES[ordinal];
    let color_owner = "net/minecraft/ChatFormatting";
    let color = rt
        .get_static(color_owner, color_name)
        .or_else(|| crate::minecraft::get_static_field(rt, color_owner, color_name))?;
    rt.put_static(color_owner, color_name, color);
    let value = rt.new_instance(CLASS, 0);
    let enum_name = rt.intern_str(name);
    let serialized = rt.intern_str(serialized);
    for (field, field_value) in [
        ("$name", enum_name),
        ("$ordinal", V::I(ordinal as i32)),
        ("id", V::I(ordinal as i32)),
        ("name", serialized),
        ("color", color),
    ] {
        rt.put_field(value, field, field_value);
    }
    rt.put_static(CLASS, name, value);
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
                .or_else(|| crate::minecraft::get_static_field(rt, owner, name))
        }

        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(rt, member, is_static, args).or_else(|| {
                crate::minecraft::call_chat(
                    rt,
                    &std::collections::BTreeMap::new(),
                    member,
                    is_static,
                    args,
                )
            })
        }

        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            is_subtype(sub_class, super_class)
        }
    }

    #[test]
    fn rarity_values_and_enum_reflection_preserve_official_metadata() {
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
        let class = ctx
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
            &[class],
        );
        let values = match &ctx.rt.get_obj(constants).unwrap().data {
            Data::Array(values) => values.clone(),
            other => panic!("expected enum constants, got {other:?}"),
        };
        assert_eq!(values.len(), VALUES.len());
        for (ordinal, (name, serialized, color_name)) in VALUES.iter().enumerate() {
            let value = get_static_field(ctx.rt, CLASS, name, DESC).unwrap();
            assert_eq!(value, values[ordinal]);
            assert_eq!(ctx.rt.get_field(value, "$ordinal"), V::I(ordinal as i32));
            assert_eq!(ctx.rt.get_field(value, "id"), V::I(ordinal as i32));
            assert_eq!(ctx.rt.str_of(ctx.rt.get_field(value, "$name")), Some(*name));
            assert_eq!(
                ctx.rt.str_of(ctx.rt.get_field(value, "name")),
                Some(*serialized)
            );
            let color = call(
                ctx.rt,
                &MemberRef::new(CLASS, "color", "()Lnet/minecraft/ChatFormatting;"),
                false,
                &[value],
            )
            .unwrap();
            assert_eq!(
                ctx.rt.str_of(ctx.rt.get_field(color, "$name")),
                Some(*color_name)
            );
            let name_result = call(
                ctx.rt,
                &MemberRef::new(CLASS, "getSerializedName", "()Ljava/lang/String;"),
                false,
                &[value],
            )
            .unwrap();
            assert_eq!(ctx.rt.str_of(name_result), Some(*serialized));
            let label = ctx.rt.intern_str(name);
            assert_eq!(
                call(
                    ctx.rt,
                    &MemberRef::new(CLASS, "valueOf", VALUE_OF_DESC),
                    true,
                    &[label]
                ),
                Some(value)
            );
        }
        assert!(linked.is_subtype(CLASS, "net/minecraft/util/StringRepresentable", ctx.host));
        assert!(ctx.rt.pending_exception.is_none());
    }
}

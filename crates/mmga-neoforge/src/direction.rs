//! The canonical Minecraft 1.21.1 Direction enum used by mod initialization.
//!
//! Checked against Mojang's official `Direction` (`ji`) bytecode and mappings:
//! client SHA-1 30c73b1c5da787909b2f73340419fdf13b9def88, mappings SHA-1
//! 2244b6f072256667bcd9a73df124d6c58de77992. Geometry, axis operations, codecs,
//! and rotation methods are not implemented by this enum-only module.

use mmga_classfile::MemberRef;
use mmga_runtime::{Data, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt, V};

const DIRECTION: &str = "net/minecraft/core/Direction";
const DIRECTION_DESC: &str = "Lnet/minecraft/core/Direction;";
const VALUES_DESC: &str = "()[Lnet/minecraft/core/Direction;";
const NAMES: [&str; 6] = ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"];
const HORIZONTAL_NAMES: [&str; 4] = ["SOUTH", "WEST", "NORTH", "EAST"];
const AXIS: &str = "net/minecraft/core/Direction$Axis";
const AXIS_DIRECTION: &str = "net/minecraft/core/Direction$AxisDirection";
const AXIS_NAMES: [&str; 3] = ["X", "Y", "Z"];
const AXIS_CLASSES: [&str; 3] = [
    "net/minecraft/core/Direction$Axis$1",
    "net/minecraft/core/Direction$Axis$2",
    "net/minecraft/core/Direction$Axis$3",
];
const AXIS_DIRECTION_NAMES: [&str; 2] = ["POSITIVE", "NEGATIVE"];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if (owner == AXIS && desc == "Lnet/minecraft/core/Direction$Axis;")
        || (owner == AXIS_DIRECTION && desc == "Lnet/minecraft/core/Direction$AxisDirection;")
    {
        return nested_constant(rt, owner, name);
    }
    if owner != DIRECTION || desc != DIRECTION_DESC {
        return None;
    }
    constant(rt, name)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if let Some(value) = nested_call(rt, member, is_static, args) {
        return Some(value);
    }
    if member.owner != DIRECTION {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str(), is_static) {
        ("from2DDataValue", "(I)Lnet/minecraft/core/Direction;", true) => {
            // ji uses Mth.abs(input % BY_2D_DATA.length), preserving Java
            // negative remainders (it is not floorMod or abs before modulo).
            let index = (args.first()?.i() % 4).abs() as usize;
            constant(rt, HORIZONTAL_NAMES[index])
        }
        ("from3DDataValue", "(I)Lnet/minecraft/core/Direction;", true) => {
            let index = (args.first()?.i() % 6).abs() as usize;
            constant(rt, NAMES[index])
        }
        ("get3DDataValue" | "get2DDataValue", "()I", false) => {
            let field = if member.name == "get3DDataValue" {
                "data3d"
            } else {
                "data2d"
            };
            let value = rt.get_field(*args.first()?, field);
            Some(if value.is_null() {
                rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Direction data-index metadata is unavailable",
                )
            } else {
                value
            })
        }
        ("values", VALUES_DESC, true) => {
            let values = NAMES
                .into_iter()
                .map(|name| constant(rt, name).unwrap())
                .collect();
            Some(rt.alloc("[Lnet/minecraft/core/Direction;", 0, Data::Array(values)))
        }
        ("valueOf", "(Ljava/lang/String;)Lnet/minecraft/core/Direction;", true) => {
            let value = *args.first()?;
            if value.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let name = rt.str_of(value)?.to_owned();
            Some(constant(rt, &name).unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!("No enum constant net.minecraft.core.Direction.{name}"),
                )
            }))
        }
        ("getName" | "getSerializedName" | "toString", "()Ljava/lang/String;", false) => {
            Some(rt.get_field(*args.first()?, "name"))
        }
        _ => None,
    }
}

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    let axis = sub == AXIS || AXIS_CLASSES.contains(&sub);
    (AXIS_CLASSES.contains(&sub) && sup == AXIS)
        || (matches!(sub, DIRECTION | AXIS_DIRECTION) || axis)
            && matches!(
                sup,
                "java/lang/Enum" | "java/lang/Comparable" | "java/io/Serializable"
            )
        || (sub == DIRECTION || axis) && sup == "net/minecraft/util/StringRepresentable"
        || axis && sup == "java/util/function/Predicate"
}

/// Actual superclass/interface metadata from ji$a and its three constant bodies.
/// Empty method tables let their modeled methods continue through the host API.
pub fn register_types(linked: &mut LinkedSpace) {
    linked.register_native_module(NativeModuleRegistry {
        classes: &[
            NativeClassDef {
                name: AXIS,
                superclass: Some("java/lang/Enum"),
                interfaces: &[
                    "net/minecraft/util/StringRepresentable",
                    "java/util/function/Predicate",
                ],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: AXIS_CLASSES[0],
                superclass: Some(AXIS),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: AXIS_CLASSES[1],
                superclass: Some(AXIS),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: AXIS_CLASSES[2],
                superclass: Some(AXIS),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: AXIS_DIRECTION,
                superclass: Some("java/lang/Enum"),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
        ],
        lambdas: &[],
    });
}

fn nested_call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static && matches!(owner, AXIS | AXIS_DIRECTION) {
        let (names, array_desc, value_desc): (&[&str], &str, &str) = if owner == AXIS {
            (
                &AXIS_NAMES,
                "()[Lnet/minecraft/core/Direction$Axis;",
                "(Ljava/lang/String;)Lnet/minecraft/core/Direction$Axis;",
            )
        } else {
            (
                &AXIS_DIRECTION_NAMES,
                "()[Lnet/minecraft/core/Direction$AxisDirection;",
                "(Ljava/lang/String;)Lnet/minecraft/core/Direction$AxisDirection;",
            )
        };
        if name == "values" && desc == array_desc {
            let values = names
                .iter()
                .map(|name| nested_constant(rt, owner, name).unwrap())
                .collect();
            return Some(rt.alloc(&array_desc[2..], 0, Data::Array(values)));
        }
        if name == "valueOf" && desc == value_desc {
            let input = *args.first()?;
            if input.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "Name is null"));
            }
            let label = rt.str_of(input)?.to_owned();
            return Some(nested_constant(rt, owner, &label).unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!(
                        "No enum constant {}.{label}",
                        owner.replace('/', ".").replace('$', ".")
                    ),
                )
            }));
        }
        return None;
    }
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    let class = rt.class_of(receiver)?;
    if AXIS_CLASSES.contains(&class)
        && matches!(owner, AXIS | "net/minecraft/util/StringRepresentable")
        || AXIS_CLASSES.contains(&owner) && AXIS_CLASSES.contains(&class)
    {
        if matches!(name, "getName" | "getSerializedName" | "toString")
            && desc == "()Ljava/lang/String;"
        {
            return Some(rt.get_field(receiver, "name"));
        }
    }
    if owner == AXIS_DIRECTION && class == AXIS_DIRECTION {
        return match (name, desc) {
            ("getName" | "toString", "()Ljava/lang/String;") => {
                Some(rt.get_field(receiver, "name"))
            }
            ("getStep", "()I") => Some(rt.get_field(receiver, "step")),
            _ => None,
        };
    }
    None
}
fn nested_constant(rt: &mut Rt, owner: &str, name: &str) -> Option<V> {
    let index = if owner == AXIS {
        AXIS_NAMES.iter().position(|candidate| *candidate == name)?
    } else {
        AXIS_DIRECTION_NAMES
            .iter()
            .position(|candidate| *candidate == name)?
    };
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = rt.new_instance(
        if owner == AXIS {
            AXIS_CLASSES[index]
        } else {
            AXIS_DIRECTION
        },
        0,
    );
    let label = rt.intern_str(name);
    let printed = rt.intern_str(if owner == AXIS {
        ["x", "y", "z"][index]
    } else {
        ["Towards positive", "Towards negative"][index]
    });
    rt.put_field(value, "$name", label);
    rt.put_field(value, "$ordinal", V::I(index as i32));
    rt.put_field(value, "name", printed);
    if owner == AXIS_DIRECTION {
        rt.put_field(value, "step", V::I(if index == 0 { 1 } else { -1 }));
    }
    rt.put_static(owner, name, value);
    Some(value)
}

fn constant(rt: &mut Rt, name: &str) -> Option<V> {
    let ordinal = NAMES.iter().position(|&candidate| candidate == name)?;
    if let Some(value) = rt.get_static(DIRECTION, name) {
        return Some(value);
    }
    let value = rt.new_instance(DIRECTION, 0);
    let enum_name = rt.intern_str(name);
    let serialized_name = rt.intern_str(&name.to_ascii_lowercase());
    rt.put_field(value, "$name", enum_name);
    rt.put_field(value, "$ordinal", V::I(ordinal as i32));
    rt.put_field(value, "data3d", V::I(ordinal as i32));
    let data2d = HORIZONTAL_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|index| index as i32)
        .unwrap_or(-1);
    rt.put_field(value, "data2d", V::I(data2d));
    rt.put_field(value, "name", serialized_name);
    rt.put_static(DIRECTION, name, value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, Data, ExecCtx, HostApi, LinkedSpace};

    struct DirectionHost;
    impl HostApi for DirectionHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup)
        }
    }

    fn values(ctx: &mut ExecCtx<'_>) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(DIRECTION, "values", VALUES_DESC),
            &[],
        )
    }

    fn items(rt: &Rt, value: V) -> Vec<V> {
        match &rt.get_obj(value).unwrap().data {
            Data::Array(values) => values.clone(),
            _ => panic!("expected Direction[]"),
        }
    }

    #[test]
    fn create_horizontal_lookups_keep_vanilla_data_ids_and_java_signed_modulus() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (input, name) in [
            (0, "SOUTH"),
            (1, "WEST"),
            (2, "NORTH"),
            (3, "EAST"),
            (4, "SOUTH"),
            (-1, "WEST"),
            (-2, "NORTH"),
            (-3, "EAST"),
            (-4, "SOUTH"),
            (i32::MIN, "SOUTH"),
            (i32::MAX, "EAST"),
        ] {
            let value = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    DIRECTION,
                    "from2DDataValue",
                    "(I)Lnet/minecraft/core/Direction;",
                ),
                &[V::I(input)],
            );
            assert!(
                ctx.rt.pending_exception.is_none(),
                "from2DDataValue({input}) unavailable"
            );
            assert_eq!(
                value,
                read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC)
            );
        }
        for (input, name) in [
            (0, "DOWN"),
            (1, "UP"),
            (5, "EAST"),
            (6, "DOWN"),
            (-1, "UP"),
            (-5, "EAST"),
            (i32::MIN, "NORTH"),
            (i32::MAX, "UP"),
        ] {
            let value = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    DIRECTION,
                    "from3DDataValue",
                    "(I)Lnet/minecraft/core/Direction;",
                ),
                &[V::I(input)],
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(
                value,
                read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC)
            );
        }
        for (index, (name, id2d)) in [
            ("DOWN", -1),
            ("UP", -1),
            ("NORTH", 2),
            ("SOUTH", 0),
            ("WEST", 1),
            ("EAST", 3),
        ]
        .into_iter()
        .enumerate()
        {
            let value = read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC);
            assert_eq!(
                invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(DIRECTION, "get3DDataValue", "()I"),
                    &[value]
                ),
                V::I(index as i32)
            );
            assert_eq!(
                invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(DIRECTION, "get2DDataValue", "()I"),
                    &[value]
                ),
                V::I(id2d)
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn iterate_axes_keep_real_subclass_identity_enum_names_and_fresh_arrays() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (owner, names) in [
            ("net/minecraft/core/Direction$Axis", vec!["X", "Y", "Z"]),
            (
                "net/minecraft/core/Direction$AxisDirection",
                vec!["POSITIVE", "NEGATIVE"],
            ),
        ] {
            let desc = format!("()[L{owner};");
            let first = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(owner, "values", &desc),
                &[],
            );
            assert!(
                ctx.rt.pending_exception.is_none(),
                "missing real {owner}.values"
            );
            let canonical = items(ctx.rt, first);
            assert_eq!(canonical.len(), names.len());
            for (index, name) in names.into_iter().enumerate() {
                let constant = read_static_field(&mut ctx, owner, name, &format!("L{owner};"));
                assert_eq!(constant, canonical[index]);
                assert_eq!(
                    invoke_member(
                        &mut ctx,
                        op::INVOKEVIRTUAL,
                        &MemberRef::new(owner, "ordinal", "()I"),
                        &[constant]
                    ),
                    V::I(index as i32)
                );
                let label = ctx.rt.intern_str(name);
                assert_eq!(
                    invoke_member(
                        &mut ctx,
                        op::INVOKESTATIC,
                        &MemberRef::new(
                            owner,
                            "valueOf",
                            &format!("(Ljava/lang/String;)L{owner};")
                        ),
                        &[label]
                    ),
                    constant
                );
                if owner.ends_with("$Axis") {
                    assert_eq!(
                        ctx.rt.class_of(constant),
                        Some(format!("{owner}${}", index + 1).as_str())
                    );
                    assert!(ctx.linked.is_subtype(
                        ctx.rt.class_of(constant).unwrap(),
                        owner,
                        ctx.host
                    ));
                    let label = invoke_member(
                        &mut ctx,
                        op::INVOKEVIRTUAL,
                        &MemberRef::new(owner, "getSerializedName", "()Ljava/lang/String;"),
                        &[constant],
                    );
                    assert_eq!(
                        ctx.rt.str_of(label),
                        Some(name.to_ascii_lowercase().as_str())
                    );
                } else {
                    let label = invoke_member(
                        &mut ctx,
                        op::INVOKEVIRTUAL,
                        &MemberRef::new(owner, "toString", "()Ljava/lang/String;"),
                        &[constant],
                    );
                    assert_eq!(
                        ctx.rt.str_of(label),
                        Some(if index == 0 {
                            "Towards positive"
                        } else {
                            "Towards negative"
                        })
                    );
                }
            }
            if let Data::Array(values) = &mut ctx.rt.get_obj_mut(first).unwrap().data {
                values[0] = V::NULL;
            }
            let second = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(owner, "values", &desc),
                &[],
            );
            assert_eq!(items(ctx.rt, second), canonical);
            let bad = ctx.rt.intern_str("missing");
            invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(owner, "valueOf", &format!("(Ljava/lang/String;)L{owner};")),
                &[bad],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/IllegalArgumentException")
            );
            invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(owner, "valueOf", &format!("(Ljava/lang/String;)L{owner};")),
                &[V::NULL],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/NullPointerException")
            );
        }
    }

    #[test]
    fn axis_constant_bodies_use_the_declaring_enum_for_enum_sets() {
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        register_types(&mut linked);
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let class = ctx
            .rt
            .new_pinned_handle("java/lang/Class", "class", 0, AXIS);
        let set = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "java/util/EnumSet",
                "noneOf",
                "(Ljava/lang/Class;)Ljava/util/EnumSet;",
            ),
            &[class],
        );
        assert!(ctx.rt.pending_exception.is_none());
        for name in AXIS_NAMES {
            let value =
                read_static_field(&mut ctx, AXIS, name, "Lnet/minecraft/core/Direction$Axis;");
            assert_eq!(
                invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Set", "add", "(Ljava/lang/Object;)Z"),
                    &[set, value]
                ),
                V::TRUE
            );
        }
        assert_eq!(
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/util/Set", "size", "()I"),
                &[set]
            ),
            V::I(3)
        );
        let body_class = ctx
            .rt
            .new_pinned_handle("java/lang/Class", "class", 0, AXIS_CLASSES[0]);
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "java/util/EnumSet",
                "noneOf",
                "(Ljava/lang/Class;)Ljava/util/EnumSet;",
            ),
            &[body_class],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
    }

    #[test]
    fn values_preserve_mekanism_enum_order_and_getstatic_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        // EnumUtils obtains values() before reading four horizontal GETSTATICs.
        let first = values(&mut ctx);
        assert!(ctx.rt.pending_exception.is_none());
        let canonical = items(ctx.rt, first);
        assert_eq!(canonical.len(), 6);
        for (ordinal, (name, value)) in NAMES.into_iter().zip(&canonical).enumerate() {
            assert_eq!(
                *value,
                read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC)
            );
            assert_eq!(
                *value,
                read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC)
            );
            assert_eq!(ctx.rt.str_of(ctx.rt.get_field(*value, "$name")), Some(name));
            assert_eq!(ctx.rt.get_field(*value, "$ordinal"), V::I(ordinal as i32));
            let string = ctx.rt.intern_str(name);
            let from_name = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    DIRECTION,
                    "valueOf",
                    "(Ljava/lang/String;)Lnet/minecraft/core/Direction;",
                ),
                &[string],
            );
            assert_eq!(*value, from_name);
            for name_method in ["getName", "getSerializedName", "toString"] {
                let label = invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new(DIRECTION, name_method, "()Ljava/lang/String;"),
                    &[*value],
                );
                assert_eq!(
                    ctx.rt.str_of(label),
                    Some(name.to_ascii_lowercase().as_str())
                );
            }
        }
        if let Data::Array(array) = &mut ctx.rt.get_obj_mut(first).unwrap().data {
            array[0] = V::NULL;
        }
        let second = values(&mut ctx);
        assert_ne!(first, second);
        assert_eq!(items(ctx.rt, second), canonical);
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn invalid_enum_names_and_unknown_signatures_fail_explicitly() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = DirectionHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for name in ["NORTHEAST", "north", ""] {
            let string = ctx.rt.intern_str(name);
            invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    DIRECTION,
                    "valueOf",
                    "(Ljava/lang/String;)Lnet/minecraft/core/Direction;",
                ),
                &[string],
            );
            let error = ctx.rt.take_exception().expect("unknown enum must fail");
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("IllegalArgumentException"));
        }
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                DIRECTION,
                "valueOf",
                "(Ljava/lang/String;)Lnet/minecraft/core/Direction;",
            ),
            &[V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        assert!(get_static_field(ctx.rt, DIRECTION, "NORTHEAST", DIRECTION_DESC).is_none());
        assert!(get_static_field(ctx.rt, DIRECTION, "NORTH", "I").is_none());
        assert!(call(
            ctx.rt,
            &MemberRef::new(DIRECTION, "values", "()Ljava/lang/Object;"),
            true,
            &[]
        )
        .is_none());
    }
}

//! Minecraft 1.21.1 Rotation enum and its value-only transforms.
//!
//! Checked against the operator's Mojmap 1.21.1 JAR. CODEC, random selection,
//! shuffled lists and OctahedralGroup are deliberately outside this module.
use mmga_classfile::MemberRef;
use mmga_runtime::{Data, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt, V};

const CLASS: &str = "net/minecraft/world/level/block/Rotation";
const DESC: &str = "Lnet/minecraft/world/level/block/Rotation;";
const DIRECTION: &str = "net/minecraft/core/Direction";
const DIRECTION_DESC: &str = "Lnet/minecraft/core/Direction;";
const NAMES: [&str; 4] = [
    "NONE",
    "CLOCKWISE_90",
    "CLOCKWISE_180",
    "COUNTERCLOCKWISE_90",
];
const IDS: [&str; 4] = ["none", "clockwise_90", "180", "counterclockwise_90"];
// Clockwise around Y, starting at north; vertical directions are unchanged.
const HORIZONTAL: [&str; 4] = ["NORTH", "EAST", "SOUTH", "WEST"];

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

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == CLASS
        && matches!(
            sup,
            "java/lang/Enum"
                | "java/lang/Comparable"
                | "java/io/Serializable"
                | "net/minecraft/util/StringRepresentable"
        )
}

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != CLASS || desc != DESC {
        return None;
    }
    constant(rt, NAMES.iter().position(|candidate| *candidate == name)?)
}

fn constant(rt: &mut Rt, ordinal: usize) -> Option<V> {
    let name = *NAMES.get(ordinal)?;
    if let Some(value) = rt.get_static(CLASS, name) {
        return Some(value);
    }
    let value = rt.new_instance(CLASS, 0);
    let name_value = rt.intern_str(name);
    let id = rt.intern_str(IDS[ordinal]);
    rt.put_field(value, "$name", name_value);
    rt.put_field(value, "$ordinal", V::I(ordinal as i32));
    rt.put_field(value, "id", id);
    rt.put_static(CLASS, name, value);
    Some(value)
}

fn ordinal(rt: &Rt, value: V) -> Option<usize> {
    if rt.class_of(value) != Some(CLASS) {
        return None;
    }
    let V::I(index) = rt.get_field(value, "$ordinal") else {
        return None;
    };
    let index = usize::try_from(index).ok()?;
    let name = NAMES.get(index)?;
    (rt.get_static(CLASS, name) == Some(value)).then_some(index)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if member.owner != CLASS {
        return None;
    }
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    if is_static {
        return match (name, desc) {
            ("values", "()[Lnet/minecraft/world/level/block/Rotation;") => {
                let values = (0..NAMES.len())
                    .map(|index| constant(rt, index).unwrap())
                    .collect();
                Some(rt.alloc(
                    "[Lnet/minecraft/world/level/block/Rotation;",
                    0,
                    Data::Array(values),
                ))
            }
            ("valueOf", "(Ljava/lang/String;)Lnet/minecraft/world/level/block/Rotation;") => {
                let value = *args.first()?;
                if value.is_null() {
                    return Some(
                        rt.throw_exception("java/lang/NullPointerException", "Name is null"),
                    );
                }
                let input = rt.str_of(value)?.to_owned();
                Some(
                    match NAMES.iter().position(|candidate| *candidate == input) {
                        Some(index) => constant(rt, index).unwrap(),
                        None => rt.throw_exception(
                            "java/lang/IllegalArgumentException",
                            &format!(
                                "No enum constant net.minecraft.world.level.block.Rotation.{input}"
                            ),
                        ),
                    },
                )
            }
            _ => None,
        };
    }
    let receiver = *args.first()?;
    let index = ordinal(rt, receiver)?;
    match (name, desc) {
        ("getSerializedName", "()Ljava/lang/String;") => Some(rt.get_field(receiver, "id")),
        ("getRotated", "(Lnet/minecraft/world/level/block/Rotation;)Lnet/minecraft/world/level/block/Rotation;") => {
            let other = *args.get(1)?;
            if other.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "rotation is null"));
            }
            let other = ordinal(rt, other)?;
            constant(rt, (index + other) % 4)
        }
        ("rotate", "(Lnet/minecraft/core/Direction;)Lnet/minecraft/core/Direction;") => {
            let direction = *args.get(1)?;
            if direction.is_null() {
                return Some(rt.throw_exception("java/lang/NullPointerException", "direction is null"));
            }
            if rt.class_of(direction) != Some(DIRECTION) {
                return None;
            }
            let dir_name = rt.str_of(rt.get_field(direction, "$name"))?;
            if rt.get_static(DIRECTION, dir_name) != Some(direction) {
                return None;
            }
            if matches!(dir_name, "UP" | "DOWN") {
                return Some(direction);
            }
            let rotated = (HORIZONTAL.iter().position(|candidate| *candidate == dir_name)? + index) % 4;
            crate::direction::get_static_field(rt, DIRECTION, HORIZONTAL[rotated], DIRECTION_DESC)
        }
        ("rotate", "(II)I") => {
            let (V::I(value), V::I(size)) = (*args.get(1)?, *args.get(2)?) else {
                return None;
            };
            // NONE does not divide or reduce the input. Other cases preserve
            // Java int overflow, truncation and remainder, including negatives.
            if index == 0 {
                return Some(V::I(value));
            }
            if size == 0 {
                return Some(rt.throw_exception("java/lang/ArithmeticException", "/ by zero"));
            }
            let offset = match index {
                1 => size / 4,
                2 => size / 2,
                3 => size.wrapping_mul(3) / 4,
                _ => unreachable!(),
            };
            let sum = value.wrapping_add(offset);
            Some(V::I(if size == -1 { 0 } else { sum % size }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, read_static_field, ExecCtx, HostApi};

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
                .or_else(|| crate::direction::get_static_field(rt, owner, name, desc))
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
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            is_subtype(sub, sup) || crate::direction::is_subtype(sub, sup)
        }
    }
    fn method(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(CLASS, name, desc),
            args,
        )
    }

    #[test]
    fn enum_composition_serialization_and_horizontal_turns_match_1_21_1() {
        let (mut rt, mut linked, mut host) = (Rt::new(), LinkedSpace::new(), Host);
        register_types(&mut linked);
        crate::direction::register_types(&mut linked);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let values: Vec<_> = NAMES
            .iter()
            .map(|name| read_static_field(&mut ctx, CLASS, name, DESC))
            .collect();
        let expected_composition = [[0, 1, 2, 3], [1, 2, 3, 0], [2, 3, 0, 1], [3, 0, 1, 2]];
        let directions = ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"];
        let turns = [
            ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"],
            ["DOWN", "UP", "EAST", "WEST", "NORTH", "SOUTH"],
            ["DOWN", "UP", "SOUTH", "NORTH", "EAST", "WEST"],
            ["DOWN", "UP", "WEST", "EAST", "SOUTH", "NORTH"],
        ];
        for (index, &value) in values.iter().enumerate() {
            let id = method(
                &mut ctx,
                "getSerializedName",
                "()Ljava/lang/String;",
                &[value],
            );
            assert_eq!(ctx.rt.str_of(id), Some(IDS[index]));
            for (other, &second) in values.iter().enumerate() {
                assert_eq!(method(&mut ctx, "getRotated", "(Lnet/minecraft/world/level/block/Rotation;)Lnet/minecraft/world/level/block/Rotation;", &[value, second]), values[expected_composition[index][other]]);
            }
            for (d, name) in directions.iter().enumerate() {
                let original = read_static_field(&mut ctx, DIRECTION, name, DIRECTION_DESC);
                let expected =
                    read_static_field(&mut ctx, DIRECTION, turns[index][d], DIRECTION_DESC);
                assert_eq!(
                    method(
                        &mut ctx,
                        "rotate",
                        "(Lnet/minecraft/core/Direction;)Lnet/minecraft/core/Direction;",
                        &[value, original]
                    ),
                    expected
                );
            }
        }
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
    }

    #[test]
    fn integer_turns_keep_java_overflow_negative_remainder_and_zero_divisor() {
        let mut rt = Rt::new();
        let rotation = constant(&mut rt, 1).unwrap();
        let none = constant(&mut rt, 0).unwrap();
        let ccw = constant(&mut rt, 3).unwrap();
        let member = MemberRef::new(CLASS, "rotate", "(II)I");
        for (value, input, size, expected) in [
            (rotation, -7, 16, -3),
            (rotation, i32::MAX, 16, -13),
            (ccw, -7, 16, 5),
            (ccw, 5, i32::MAX, 536870916),
            (none, 99, 0, 99),
            (rotation, i32::MIN, -1, 0),
        ] {
            assert_eq!(
                call(&mut rt, &member, false, &[value, V::I(input), V::I(size)]),
                Some(V::I(expected))
            );
        }
        call(&mut rt, &member, false, &[rotation, V::I(1), V::I(0)]);
        let exception = rt.take_exception().unwrap();
        assert!(rt
            .exception_summary(exception)
            .contains("ArithmeticException"));
        assert!(call(
            &mut rt,
            &MemberRef::new(CLASS, "rotation", "()Lcom/mojang/math/OctahedralGroup;"),
            false,
            &[rotation]
        )
        .is_none());
    }

    #[test]
    #[ignore = "needs MMGA_SHAPE_ORACLE_JSON from the actual Minecraft 1.21.1 JAR"]
    fn installed_1_21_1_rotation_oracle() {
        let path = std::env::var_os("MMGA_SHAPE_ORACLE_JSON").expect("MMGA_SHAPE_ORACLE_JSON");
        let oracle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(oracle["minecraft"], "1.21.1");
        let mut rt = Rt::new();
        for item in oracle["rotations"].as_array().unwrap() {
            let rotation =
                get_static_field(&mut rt, CLASS, item["name"].as_str().unwrap(), DESC).unwrap();
            let id = call(
                &mut rt,
                &MemberRef::new(CLASS, "getSerializedName", "()Ljava/lang/String;"),
                false,
                &[rotation],
            )
            .unwrap();
            assert_eq!(rt.str_of(id), item["id"].as_str());
            for (index, expected) in item["composition"].as_array().unwrap().iter().enumerate() {
                let other = constant(&mut rt, index).unwrap();
                let result = call(&mut rt, &MemberRef::new(CLASS, "getRotated", "(Lnet/minecraft/world/level/block/Rotation;)Lnet/minecraft/world/level/block/Rotation;"), false, &[rotation, other]).unwrap();
                assert_eq!(rt.str_of(rt.get_field(result, "$name")), expected.as_str());
            }
            for (index, expected) in item["directions"].as_array().unwrap().iter().enumerate() {
                let direction = crate::direction::get_static_field(
                    &mut rt,
                    DIRECTION,
                    ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"][index],
                    DIRECTION_DESC,
                )
                .unwrap();
                let result = call(
                    &mut rt,
                    &MemberRef::new(
                        CLASS,
                        "rotate",
                        "(Lnet/minecraft/core/Direction;)Lnet/minecraft/core/Direction;",
                    ),
                    false,
                    &[rotation, direction],
                )
                .unwrap();
                assert_eq!(rt.str_of(rt.get_field(result, "$name")), expected.as_str());
            }
            for case in item["integers"].as_array().unwrap() {
                let value = case["value"].as_i64().unwrap() as i32;
                let size = case["size"].as_i64().unwrap() as i32;
                let result = call(
                    &mut rt,
                    &MemberRef::new(CLASS, "rotate", "(II)I"),
                    false,
                    &[rotation, V::I(value), V::I(size)],
                )
                .unwrap();
                if let Some(expected) = case["result"].as_i64() {
                    assert!(rt.pending_exception.is_none());
                    assert_eq!(
                        result,
                        V::I(expected as i32),
                        "{item_name}: {case}",
                        item_name = item["name"]
                    );
                } else {
                    let error = rt.take_exception().unwrap();
                    assert!(rt
                        .exception_summary(error)
                        .contains(case["error"].as_str().unwrap()));
                }
            }
        }
    }
}

//! Value-only Direction transforms, checked against Minecraft 1.21.1.
//! Canonical enum construction stays in `direction`; entity/world queries and
//! vector-returning methods remain outside this module.
use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};

const CLASS: &str = "net/minecraft/core/Direction";
const DESC: &str = "Lnet/minecraft/core/Direction;";
const AXIS: &str = "net/minecraft/core/Direction$Axis";
const AXIS_DESC: &str = "Lnet/minecraft/core/Direction$Axis;";
const NAMES: [&str; 6] = ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"];
const AXES: [&str; 6] = ["Y", "Y", "Z", "Z", "X", "X"];
const STEPS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];
// Turn around positive X, Y or Z as Direction's actual public axis overloads.
const CLOCKWISE: [[usize; 6]; 3] = [[3, 2, 0, 1, 4, 5], [0, 1, 5, 4, 2, 3], [4, 5, 2, 3, 1, 0]];

fn index(rt: &Rt, value: V) -> Option<usize> {
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

fn direction(rt: &mut Rt, index: usize) -> Option<V> {
    crate::direction::get_static_field(rt, CLASS, NAMES[index], DESC)
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static || member.owner != CLASS {
        return None;
    }
    let receiver = *args.first()?;
    let index = index(rt, receiver)?;
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    match (name, desc) {
        ("getOpposite", "()Lnet/minecraft/core/Direction;") => direction(rt, index ^ 1),
        ("getAxis", "()Lnet/minecraft/core/Direction$Axis;") => {
            crate::direction::get_static_field(rt, AXIS, AXES[index], AXIS_DESC)
        }
        ("getAxisDirection", "()Lnet/minecraft/core/Direction$AxisDirection;") => {
            crate::direction::get_static_field(
                rt,
                "net/minecraft/core/Direction$AxisDirection",
                if index % 2 == 0 {
                    "NEGATIVE"
                } else {
                    "POSITIVE"
                },
                "Lnet/minecraft/core/Direction$AxisDirection;",
            )
        }
        ("getStepX" | "getStepY" | "getStepZ", "()I") => Some(V::I(
            STEPS[index][match name {
                "getStepX" => 0,
                "getStepY" => 1,
                _ => 2,
            }],
        )),
        ("getClockWise" | "getCounterClockWise", "()Lnet/minecraft/core/Direction;") => {
            if index < 2 {
                let kind = if name == "getClockWise" {
                    "Y-rotated"
                } else {
                    "CCW"
                };
                return Some(rt.throw_exception(
                    "java/lang/IllegalStateException",
                    &format!(
                        "Unable to get {kind} facing of {}",
                        NAMES[index].to_ascii_lowercase()
                    ),
                ));
            }
            let turns = if name == "getClockWise" { 1 } else { 3 };
            let mut rotated = index;
            for _ in 0..turns {
                rotated = CLOCKWISE[1][rotated];
            }
            direction(rt, rotated)
        }
        (
            "getClockWise" | "getCounterClockWise",
            "(Lnet/minecraft/core/Direction$Axis;)Lnet/minecraft/core/Direction;",
        ) => {
            let value = *args.get(1)?;
            if value.is_null() {
                return Some(
                    rt.throw_exception("java/lang/NullPointerException", "rotation axis is null"),
                );
            }
            let axis_name = rt.str_of(rt.get_field(value, "$name"))?;
            let axis = ["X", "Y", "Z"].iter().position(|name| *name == axis_name)?;
            if rt.get_static(AXIS, axis_name) != Some(value) {
                return None;
            }
            let turns = if name == "getClockWise" { 1 } else { 3 };
            let mut rotated = index;
            for _ in 0..turns {
                rotated = CLOCKWISE[axis][rotated];
            }
            direction(rt, rotated)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(rt: &mut Rt, receiver: V, name: &str, desc: &str, rest: &[V]) -> V {
        let mut args = vec![receiver];
        args.extend(rest);
        call(rt, &MemberRef::new(CLASS, name, desc), false, &args).unwrap()
    }

    #[test]
    fn opposite_turns_and_parallel_axis_behavior_keep_canonical_directions() {
        let mut rt = Rt::new();
        let values: Vec<_> = (0..6)
            .map(|index| direction(&mut rt, index).unwrap())
            .collect();
        let expected_opposite = [1, 0, 3, 2, 5, 4];
        for index in 0..6 {
            assert_eq!(
                invoke(
                    &mut rt,
                    values[index],
                    "getOpposite",
                    "()Lnet/minecraft/core/Direction;",
                    &[]
                ),
                values[expected_opposite[index]]
            );
            for axis_name in ["X", "Y", "Z"] {
                let axis = crate::direction::get_static_field(&mut rt, AXIS, axis_name, AXIS_DESC)
                    .unwrap();
                let rotated = invoke(
                    &mut rt,
                    values[index],
                    "getClockWise",
                    "(Lnet/minecraft/core/Direction$Axis;)Lnet/minecraft/core/Direction;",
                    &[axis],
                );
                let original = invoke(
                    &mut rt,
                    rotated,
                    "getCounterClockWise",
                    "(Lnet/minecraft/core/Direction$Axis;)Lnet/minecraft/core/Direction;",
                    &[axis],
                );
                assert_eq!(original, values[index]);
                if AXES[index] == axis_name {
                    assert_eq!(rotated, values[index]);
                }
            }
        }
        invoke(
            &mut rt,
            values[0],
            "getClockWise",
            "()Lnet/minecraft/core/Direction;",
            &[],
        );
        let error = rt.take_exception().unwrap();
        assert!(rt
            .exception_summary(error)
            .contains("IllegalStateException"));
    }

    #[test]
    #[ignore = "needs MMGA_SHAPE_ORACLE_JSON from the actual Minecraft 1.21.1 JAR"]
    fn installed_1_21_1_direction_oracle() {
        let path = std::env::var_os("MMGA_SHAPE_ORACLE_JSON").expect("MMGA_SHAPE_ORACLE_JSON");
        let oracle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(oracle["minecraft"], "1.21.1");
        let mut rt = Rt::new();
        for item in oracle["directions"].as_array().unwrap() {
            let original = crate::direction::get_static_field(
                &mut rt,
                CLASS,
                item["name"].as_str().unwrap(),
                DESC,
            )
            .unwrap();
            for (method, descriptor, key) in [
                (
                    "getOpposite",
                    "()Lnet/minecraft/core/Direction;",
                    "opposite",
                ),
                ("getAxis", "()Lnet/minecraft/core/Direction$Axis;", "axis"),
                (
                    "getAxisDirection",
                    "()Lnet/minecraft/core/Direction$AxisDirection;",
                    "axis_direction",
                ),
            ] {
                let result = invoke(&mut rt, original, method, descriptor, &[]);
                assert_eq!(rt.str_of(rt.get_field(result, "$name")), item[key].as_str());
            }
            for (index, method) in ["getStepX", "getStepY", "getStepZ"].iter().enumerate() {
                assert_eq!(
                    invoke(&mut rt, original, method, "()I", &[]),
                    V::I(item["steps"][index].as_i64().unwrap() as i32)
                );
            }
            for (method, key, error_key, axis_key) in [
                (
                    "getClockWise",
                    "clockwise",
                    "clockwise_error",
                    "axis_clockwise",
                ),
                (
                    "getCounterClockWise",
                    "counterclockwise",
                    "counterclockwise_error",
                    "axis_counterclockwise",
                ),
            ] {
                let result = invoke(
                    &mut rt,
                    original,
                    method,
                    "()Lnet/minecraft/core/Direction;",
                    &[],
                );
                if let Some(expected) = item[key].as_str() {
                    assert!(rt.pending_exception.is_none());
                    assert_eq!(rt.str_of(rt.get_field(result, "$name")), Some(expected));
                } else {
                    let error = rt.take_exception().unwrap();
                    let expected = item[error_key]
                        .as_str()
                        .unwrap()
                        .replace("IllegalStateException: ", "");
                    assert!(rt.exception_summary(error).contains(&expected));
                }
                for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
                    let axis =
                        crate::direction::get_static_field(&mut rt, AXIS, name, AXIS_DESC).unwrap();
                    let result = invoke(
                        &mut rt,
                        original,
                        method,
                        "(Lnet/minecraft/core/Direction$Axis;)Lnet/minecraft/core/Direction;",
                        &[axis],
                    );
                    assert_eq!(
                        rt.str_of(rt.get_field(result, "$name")),
                        item[axis_key][index].as_str()
                    );
                }
            }
        }
    }
}

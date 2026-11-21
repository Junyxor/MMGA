//! Java boxed-number Comparable bridges use exact wrapper types, signed integer
//! ordering, and Float/Double.compare's canonical NaN and signed-zero rules.
use crate::{ExecCtx, V};
use mmga_classfile::MemberRef;

fn float_compare(a: f32, b: f32) -> i32 {
    if a < b {
        return -1;
    }
    if a > b {
        return 1;
    }
    let a = if a.is_nan() { 0x7fc0_0000 } else { a.to_bits() } as i32;
    let b = if b.is_nan() { 0x7fc0_0000 } else { b.to_bits() } as i32;
    a.cmp(&b) as i32
}
fn double_compare(a: f64, b: f64) -> i32 {
    if a < b {
        return -1;
    }
    if a > b {
        return 1;
    }
    let a = if a.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        a.to_bits()
    } as i64;
    let b = if b.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        b.to_bits()
    } as i64;
    a.cmp(&b) as i32
}
fn compare(class: &str, a: V, b: V) -> Option<i32> {
    match (class, a, b) {
        ("java/lang/Integer", V::I(a), V::I(b)) => Some(a.cmp(&b) as i32),
        ("java/lang/Byte", V::I(a), V::I(b))
            if i8::try_from(a).is_ok() && i8::try_from(b).is_ok() =>
        {
            Some(a - b)
        }
        ("java/lang/Short", V::I(a), V::I(b))
            if i16::try_from(a).is_ok() && i16::try_from(b).is_ok() =>
        {
            Some(a - b)
        }
        ("java/lang/Long", V::J(a), V::J(b)) => Some(a.cmp(&b) as i32),
        ("java/lang/Float", V::F(a), V::F(b)) => Some(float_compare(a, b)),
        ("java/lang/Double", V::D(a), V::D(b)) => Some(double_compare(a, b)),
        _ => None,
    }
}
fn descriptor(class: &str) -> Option<&'static str> {
    match class {
        "java/lang/Integer" => Some("(II)I"),
        "java/lang/Long" => Some("(JJ)I"),
        "java/lang/Byte" => Some("(BB)I"),
        "java/lang/Short" => Some("(SS)I"),
        "java/lang/Float" => Some("(FF)I"),
        "java/lang/Double" => Some("(DD)I"),
        _ => None,
    }
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static
        && member.name == "compare"
        && Some(member.desc.as_str()) == descriptor(&member.owner)
    {
        return Some(
            match compare(&member.owner, *args.first()?, *args.get(1)?) {
                Some(value) => V::I(value),
                None => ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "Incorrect primitive comparison operands",
                ),
            },
        );
    }
    if is_static || member.name != "compareTo" {
        return None;
    }
    let left = *args.first()?;
    let class = ctx.rt.class_of(left)?.to_owned();
    descriptor(&class)?;
    if member.owner != class && member.owner != "java/lang/Comparable" {
        return None;
    }
    if member.desc != "(Ljava/lang/Object;)I" && member.desc != format!("(L{class};)I") {
        return None;
    }
    let right = *args.get(1)?;
    if right.is_null() {
        return Some(
            ctx.rt
                .throw_exception("java/lang/NullPointerException", "Numeric compareTo(null)"),
        );
    }
    if ctx.rt.class_of(right) != Some(class.as_str()) {
        return Some(ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "Numeric Comparable requires the exact same wrapper type",
        ));
    }
    Some(
        match compare(
            &class,
            ctx.rt.get_field(left, "$value"),
            ctx.rt.get_field(right, "$value"),
        ) {
            Some(value) => V::I(value),
            None => ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Numeric wrapper payload metadata is unavailable",
            ),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{box_primitive, invoke_member, LinkedSpace, NullHost, Rt};
    use mmga_classfile::op;
    #[test]
    fn boxed_comparable_uses_exact_types_and_java_float_total_order() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (class, left, right, expected) in [
            ("java/lang/Integer", V::I(i32::MIN), V::I(i32::MAX), -1),
            ("java/lang/Long", V::J(i64::MAX - 1), V::J(i64::MAX), -1),
            ("java/lang/Byte", V::I(127), V::I(-128), 255),
            ("java/lang/Short", V::I(32767), V::I(-32768), 65535),
            ("java/lang/Float", V::F(-0.0), V::F(0.0), -1),
            (
                "java/lang/Float",
                V::F(f32::NAN),
                V::F(f32::from_bits(0xffc0_1234)),
                0,
            ),
            ("java/lang/Double", V::D(f64::NAN), V::D(f64::INFINITY), 1),
        ] {
            let left = box_primitive(ctx.rt, class, left);
            let right = box_primitive(ctx.rt, class, right);
            let value = invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/lang/Comparable", "compareTo", "(Ljava/lang/Object;)I"),
                &[left, right],
            );
            assert!(
                ctx.rt.pending_exception.is_none(),
                "{:?}",
                ctx.rt.pending_exception
            );
            assert_eq!(value, V::I(expected));
        }
        let integer = box_primitive(ctx.rt, "java/lang/Integer", V::I(2));
        let long = box_primitive(ctx.rt, "java/lang/Long", V::J(2));
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/lang/Comparable", "compareTo", "(Ljava/lang/Object;)I"),
            &[integer, long],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("ClassCastException"));
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/lang/Comparable", "compareTo", "(Ljava/lang/Object;)I"),
            &[integer, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
    }
}

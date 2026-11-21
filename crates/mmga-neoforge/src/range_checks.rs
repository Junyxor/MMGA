//! Codec.checkRange from the actual DFU 8.0.16 Codec.java source.
//! Comparisons execute Comparable.compareTo, and errors retain a lazy message
//! supplier with no partial value. Unsupported comparisons remain exceptions.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, V};
const CODEC: &str = "com/mojang/serialization/Codec";
const RESULT: &str = "com/mojang/serialization/DataResult";
const ADAPTER: &str = "mmga/internal/RangeChecker";

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if is_static
        && member.owner == CODEC
        && member.name == "checkRange"
        && member.desc == "(Ljava/lang/Number;Ljava/lang/Number;)Ljava/util/function/Function;"
    {
        return Some(ctx.rt.alloc("java/util/function/Function", 0, Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Function".into(), sam_name: "apply".into(), sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(), instantiated_desc: "(Ljava/lang/Number;)Lcom/mojang/serialization/DataResult;".into(), captures: args.to_vec(),
            target: ClosureTarget::MethodHandle { handle_kind: 6, member: MemberRef::new(ADAPTER, "apply", "(Ljava/lang/Number;Ljava/lang/Number;Ljava/lang/Object;)Ljava/lang/Object;") },
        }))));
    }
    if !is_static || member.owner != ADAPTER {
        return None;
    }
    if member.name == "apply"
        && member.desc
            == "(Ljava/lang/Number;Ljava/lang/Number;Ljava/lang/Object;)Ljava/lang/Object;"
    {
        let min = *args.first()?;
        let max = *args.get(1)?;
        let value = *args.get(2)?;
        if let Some(class) = ctx.rt.class_of(value) {
            if !ctx.linked.is_subtype(class, "java/lang/Number", ctx.host)
                || !ctx
                    .linked
                    .is_subtype(class, "java/lang/Comparable", ctx.host)
            {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "checkRange requires Number implementing Comparable",
                ));
            }
        } else if !value.is_null() {
            return Some(
                ctx.rt
                    .throw_exception("java/lang/ClassCastException", "checkRange requires Number"),
            );
        }
        let low = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/lang/Comparable", "compareTo", "(Ljava/lang/Object;)I"),
            &[value, min],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        if !matches!(low, V::I(_)) {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Comparable.compareTo returned an unmodeled primitive value",
            ));
        }
        let accepted = if low.i() >= 0 {
            let high = invoke_member(
                ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new("java/lang/Comparable", "compareTo", "(Ljava/lang/Object;)I"),
                &[value, max],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if !matches!(high, V::I(_)) {
                return Some(ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Comparable.compareTo returned an unmodeled primitive value",
                ));
            }
            high.i() <= 0
        } else {
            false
        };
        if accepted {
            return Some(invoke_member(
                ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    RESULT,
                    "success",
                    "(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",
                ),
                &[value],
            ));
        }
        let supplier = ctx.rt.alloc("java/util/function/Supplier", 0, Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Supplier".into(), sam_name: "get".into(), sam_desc: "()Ljava/lang/Object;".into(), instantiated_desc: "()Ljava/lang/String;".into(), captures: vec![value, min, max],
            target: ClosureTarget::MethodHandle { handle_kind: 6, member: MemberRef::new(ADAPTER, "message", "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;") },
        })));
        return Some(invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                RESULT,
                "error",
                "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/DataResult;",
            ),
            &[supplier],
        ));
    }
    if member.name == "message"
        && member.desc
            == "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"
    {
        let Some(value) = text(ctx, *args.first()?) else {
            return Some(V::NULL);
        };
        let Some(min) = text(ctx, *args.get(1)?) else {
            return Some(V::NULL);
        };
        let Some(max) = text(ctx, *args.get(2)?) else {
            return Some(V::NULL);
        };
        return Some(
            ctx.rt
                .new_str(format!("Value {value} outside of range [{min}:{max}]")),
        );
    }
    None
}
fn text(ctx: &mut ExecCtx<'_>, value: V) -> Option<String> {
    if value.is_null() {
        return Some("null".into());
    }
    let result = invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new("java/lang/Object", "toString", "()Ljava/lang/String;"),
        &[value],
    );
    if ctx.rt.pending_exception.is_some() {
        return None;
    }
    if result.is_null() {
        return Some("null".into());
    }
    match ctx.rt.str_of(result) {
        Some(value) => Some(value.into()),
        None => {
            ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "toString must return String",
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{
        box_primitive, HostApi, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt,
    };
    struct RangeHost;
    impl HostApi for RangeHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            crate::dfu::get_static_field(rt, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(
                &mut ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                },
                member,
                is_static,
                args,
            )
            .or_else(|| crate::dfu::call(rt, linked, self, member, is_static, args))
        }
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        static_call: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if static_call {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn checker(ctx: &mut ExecCtx<'_>, min: V, max: V) -> V {
        invoke(
            ctx,
            CODEC,
            "checkRange",
            "(Ljava/lang/Number;Ljava/lang/Number;)Ljava/util/function/Function;",
            true,
            &[min, max],
        )
    }
    fn apply(ctx: &mut ExecCtx<'_>, checker: V, value: V) -> V {
        invoke(
            ctx,
            "java/util/function/Function",
            "apply",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
            &[checker, value],
        )
    }
    fn successful(ctx: &mut ExecCtx<'_>, result: V) -> bool {
        invoke(ctx, RESULT, "isSuccess", "()Z", false, &[result]) == V::TRUE
    }
    fn message(ctx: &mut ExecCtx<'_>, result: V) -> String {
        let optional = invoke(
            ctx,
            RESULT,
            "error",
            "()Ljava/util/Optional;",
            false,
            &[result],
        );
        let error = invoke(
            ctx,
            "java/util/Optional",
            "get",
            "()Ljava/lang/Object;",
            false,
            &[optional],
        );
        let text = invoke(
            ctx,
            "com/mojang/serialization/DataResult$Error",
            "message",
            "()Ljava/lang/String;",
            false,
            &[error],
        );
        ctx.rt.str_of(text).unwrap().into()
    }
    #[test]
    fn integer_and_long_bounds_use_exact_order_and_return_original_values_without_error_partials() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = RangeHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (class, low, high, bad) in [
            ("java/lang/Integer", V::I(-2), V::I(2), V::I(3)),
            (
                "java/lang/Long",
                V::J(i64::MAX - 1),
                V::J(i64::MAX),
                V::J(i64::MAX - 2),
            ),
        ] {
            let min = box_primitive(ctx.rt, class, low);
            let max = box_primitive(ctx.rt, class, high);
            let check = checker(&mut ctx, min, max);
            assert!(ctx.rt.pending_exception.is_none());
            for value in [min, max] {
                let result = apply(&mut ctx, check, value);
                assert!(successful(&mut ctx, result));
                assert_eq!(
                    invoke(
                        &mut ctx,
                        RESULT,
                        "getOrThrow",
                        "()Ljava/lang/Object;",
                        false,
                        &[result]
                    ),
                    value
                );
            }
            let bad = box_primitive(ctx.rt, class, bad);
            let result = apply(&mut ctx, check, bad);
            assert!(!successful(&mut ctx, result));
            assert!(ctx.rt.get_field(result, "$partial").is_null());
            assert!(message(&mut ctx, result).contains("outside of range"));
            assert!(ctx.rt.pending_exception.is_none());
        }
    }
    #[test]
    fn float_nan_and_signed_zero_follow_comparable_order_instead_of_ordinary_float_relations() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = RangeHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (class, zero, negzero, nan, other_nan, infinity) in [
            (
                "java/lang/Float",
                V::F(0.0),
                V::F(-0.0),
                V::F(f32::NAN),
                V::F(f32::from_bits(0xffc0_1234)),
                V::F(f32::INFINITY),
            ),
            (
                "java/lang/Double",
                V::D(0.0),
                V::D(-0.0),
                V::D(f64::NAN),
                V::D(f64::from_bits(0xfff8_0000_0000_1234)),
                V::D(f64::INFINITY),
            ),
        ] {
            let zero = box_primitive(ctx.rt, class, zero);
            let negzero = box_primitive(ctx.rt, class, negzero);
            let nan = box_primitive(ctx.rt, class, nan);
            let other_nan = box_primitive(ctx.rt, class, other_nan);
            let infinity = box_primitive(ctx.rt, class, infinity);
            let zero_range = checker(&mut ctx, zero, zero);
            let result = apply(&mut ctx, zero_range, negzero);
            assert!(!successful(&mut ctx, result));
            assert!(message(&mut ctx, result).contains("-0.0"));
            let result = apply(&mut ctx, zero_range, nan);
            assert!(!successful(&mut ctx, result));
            let nan_range = checker(&mut ctx, nan, nan);
            let result = apply(&mut ctx, nan_range, other_nan);
            assert!(successful(&mut ctx, result));
            let result = apply(&mut ctx, nan_range, infinity);
            assert!(!successful(&mut ctx, result));
            let all_nonnegative = checker(&mut ctx, zero, nan);
            let result = apply(&mut ctx, all_nonnegative, infinity);
            assert!(successful(&mut ctx, result));
        }
    }
    #[test]
    fn null_and_mixed_wrapper_bounds_are_deferred_and_second_comparison_is_short_circuited() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = RangeHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let low = box_primitive(ctx.rt, "java/lang/Integer", V::I(2));
        let below = box_primitive(ctx.rt, "java/lang/Integer", V::I(1));
        let check = checker(&mut ctx, low, V::NULL);
        assert!(ctx.rt.pending_exception.is_none());
        let result = apply(&mut ctx, check, below);
        assert!(!successful(&mut ctx, result));
        assert_eq!(
            message(&mut ctx, result),
            "Value 1 outside of range [2:null]"
        );
        apply(&mut ctx, check, low);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        let long = box_primitive(ctx.rt, "java/lang/Long", V::J(3));
        let check = checker(&mut ctx, low, long);
        assert!(ctx.rt.pending_exception.is_none());
        apply(&mut ctx, check, low);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("ClassCastException"));
    }
    fn custom_compare(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "calls").i() + 1;
        ctx.rt.put_field(args[0], "calls", V::I(calls));
        if ctx.rt.get_field(args[0], "throw") == V::TRUE {
            return ctx
                .rt
                .throw_exception("test/ComparisonFailure", "original compareTo exception");
        }
        V::I(
            ctx.rt
                .get_field(args[0], "number")
                .i()
                .cmp(&ctx.rt.get_field(args[1], "number").i()) as i32,
        )
    }
    fn custom_text(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt
            .new_str(format!("N{}", ctx.rt.get_field(args[0], "number").i()))
    }
    #[test]
    fn custom_comparable_and_lazy_message_use_original_overrides_and_preserve_exceptions() {
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        let mut host = RangeHost;
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/Number",
                superclass: Some("java/lang/Number"),
                interfaces: &["java/lang/Comparable"],
                is_interface: false,
                methods: &[
                    (
                        "compareTo.(Ljava/lang/Object;)I",
                        custom_compare,
                        false,
                        false,
                    ),
                    ("toString.()Ljava/lang/String;", custom_text, false, false),
                ],
            }],
            lambdas: &[],
        });
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let low = ctx.rt.new_instance("test/Number", 0);
        ctx.rt.put_field(low, "number", V::I(2));
        let high = ctx.rt.new_instance("test/Number", 0);
        ctx.rt.put_field(high, "number", V::I(5));
        let value = ctx.rt.new_instance("test/Number", 0);
        ctx.rt.put_field(value, "number", V::I(1));
        let check = checker(&mut ctx, low, high);
        assert_eq!(ctx.rt.get_field(value, "calls").i(), 0);
        let result = apply(&mut ctx, check, value);
        assert_eq!(ctx.rt.get_field(value, "calls"), V::I(1));
        ctx.rt.put_field(value, "number", V::I(9));
        ctx.rt.put_field(low, "number", V::I(4));
        assert_eq!(
            message(&mut ctx, result),
            "Value N9 outside of range [N4:N5]"
        );
        ctx.rt.put_field(value, "throw", V::TRUE);
        apply(&mut ctx, check, value);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("test.ComparisonFailure"));
    }
}

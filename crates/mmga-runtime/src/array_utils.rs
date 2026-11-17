//! Commons Lang 3.14.0 object-array search, used by Create CopperBlockSet.
//! MC1.21.1's official manifest selects this version. Source apache/commons-lang
//! rel/commons-lang-3.14.0 ArrayUtils.java blob c5460e9c222361a74548eabc3f93017008b38f74.
//! Search executes wanted.equals(element), preserving nulls and callback failures.

use crate::{invoke_member, Data, ExecCtx, V};
use mmga_classfile::{op, parse_field_descriptor, DescriptorType, MemberRef};
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static || member.owner != "org/apache/commons/lang3/ArrayUtils" {
        return None;
    }
    let contains = match (member.name.as_str(), member.desc.as_str()) {
        ("contains", "([Ljava/lang/Object;Ljava/lang/Object;)Z") => true,
        (
            "indexOf",
            "([Ljava/lang/Object;Ljava/lang/Object;)I"
            | "([Ljava/lang/Object;Ljava/lang/Object;I)I",
        ) => false,
        _ => return None,
    };
    let array = *args.first()?;
    let wanted = *args.get(1)?;
    if array.is_null() {
        return Some(if contains { V::FALSE } else { V::I(-1) });
    }
    let object = ctx.rt.get_obj(array)?;
    let reference_array = matches!(parse_field_descriptor(&object.class),Ok(DescriptorType::Array(component)) if matches!(*component,DescriptorType::Object(_)|DescriptorType::Array(_)));
    let length = match &object.data {
        Data::Array(values) if reference_array => values.len(),
        _ => {
            return Some(ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "ArrayUtils object-array search requires Object[]",
            ))
        }
    };
    let start = args.get(2).map_or(0, |value| value.i().max(0) as usize);
    for index in start..length {
        // Read each element after the previous equals callback: user equality
        // may mutate the supplied array, which Java's loop observes directly.
        let value = match ctx.rt.get_obj(array).map(|object| &object.data) {
            Some(Data::Array(values)) => values[index],
            _ => {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "ArrayUtils array representation changed",
                ))
            }
        };
        let equal = if wanted.is_null() {
            value.is_null()
        } else {
            let equal = invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                &[wanted, value],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            equal.i() != 0
        };
        if equal {
            return Some(if contains {
                V::TRUE
            } else {
                V::I(index as i32)
            });
        }
    }
    Some(if contains { V::FALSE } else { V::I(-1) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost, Rt};
    use mmga_classfile::op;
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new("org/apache/commons/lang3/ArrayUtils", name, desc),
            args,
        )
    }
    #[test]
    fn contains_and_index_of_preserve_null_array_null_values_and_start_boundaries() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = ctx.rt.new_str("first");
        let last = ctx.rt.new_str("last");
        let array = ctx.rt.alloc(
            "[Ljava/lang/Object;",
            0,
            Data::Array(vec![first, V::NULL, last]),
        );
        let equivalent = ctx.rt.new_str("last");
        assert_eq!(
            invoke(
                &mut ctx,
                "contains",
                "([Ljava/lang/Object;Ljava/lang/Object;)Z",
                &[array, equivalent]
            ),
            V::TRUE
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                "contains",
                "([Ljava/lang/Object;Ljava/lang/Object;)Z",
                &[V::NULL, last]
            ),
            V::FALSE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "contains",
                "([Ljava/lang/Object;Ljava/lang/Object;)Z",
                &[array, V::NULL]
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "indexOf",
                "([Ljava/lang/Object;Ljava/lang/Object;I)I",
                &[array, first, V::I(-99)]
            ),
            V::I(0)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "indexOf",
                "([Ljava/lang/Object;Ljava/lang/Object;I)I",
                &[array, first, V::I(1)]
            ),
            V::I(-1)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "indexOf",
                "([Ljava/lang/Object;Ljava/lang/Object;I)I",
                &[array, V::NULL, V::I(0)]
            ),
            V::I(1)
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "indexOf",
                "([Ljava/lang/Object;Ljava/lang/Object;I)I",
                &[array, last, V::I(3)]
            ),
            V::I(-1)
        );
        let not_array = ctx
            .rt
            .alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
        invoke(
            &mut ctx,
            "contains",
            "([Ljava/lang/Object;Ljava/lang/Object;)Z",
            &[not_array, last],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
    }
    fn equals(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "calls").i();
        ctx.rt.put_field(args[0], "calls", V::I(calls + 1));
        if ctx.rt.get_field(args[0], "fail").i() != 0 {
            return ctx
                .rt
                .throw_exception("test/SearchFailure", "equality callback");
        }
        let rewrite = ctx.rt.get_field(args[0], "rewrite");
        if !rewrite.is_null() {
            let updated = ctx.rt.intern_str("match");
            if let Some(Data::Array(values)) =
                ctx.rt.get_obj_mut(rewrite).map(|object| &mut object.data)
            {
                values[1] = updated;
            }
        }
        V::bool(ctx.rt.str_of(args[1]) == Some("match"))
    }
    #[test]
    fn search_runs_real_wanted_equality_and_stops_at_first_match_or_exception() {
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        let mut host = NullHost;
        linked.register_native_module(NativeModuleRegistry {
            classes: &[NativeClassDef {
                name: "test/Search",
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[("equals.(Ljava/lang/Object;)Z", equals, false, false)],
            }],
            lambdas: &[],
        });
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let match_value = ctx.rt.intern_str("match");
        let later = ctx.rt.intern_str("later");
        let array = ctx.rt.alloc(
            "[Ljava/lang/Object;",
            0,
            Data::Array(vec![V::NULL, match_value, later]),
        );
        let search = ctx.rt.new_instance("test/Search", 0);
        assert_eq!(
            invoke(
                &mut ctx,
                "contains",
                "([Ljava/lang/Object;Ljava/lang/Object;)Z",
                &[array, search]
            ),
            V::TRUE
        );
        assert_eq!(ctx.rt.get_field(search, "calls"), V::I(2));
        ctx.rt.put_field(search, "fail", V::TRUE);
        invoke(
            &mut ctx,
            "contains",
            "([Ljava/lang/Object;Ljava/lang/Object;)Z",
            &[array, search],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("test/SearchFailure"));
        assert_eq!(ctx.rt.get_field(search, "calls"), V::I(3));
        ctx.rt.put_field(search, "fail", V::FALSE);
        ctx.rt.put_field(search, "calls", V::I(0));
        let live_array = ctx
            .rt
            .alloc("[Ljava/lang/Object;", 0, Data::Array(vec![V::NULL, later]));
        ctx.rt.put_field(search, "rewrite", live_array);
        assert_eq!(
            invoke(
                &mut ctx,
                "contains",
                "([Ljava/lang/Object;Ljava/lang/Object;)Z",
                &[live_array, search]
            ),
            V::TRUE
        );
        assert_eq!(ctx.rt.get_field(search, "calls"), V::I(2));
    }
}

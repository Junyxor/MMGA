//! NeoForge BaseFlowingFluid.Properties metadata, verified at source commit
//! 89e887b367ab0c11cb58ce8500920efadc17b803 / BaseFlowingFluid.java blob
//! bfe69f61299ed9de2556be92ff2848307dcc044c. Properties retain configuration
//! only. Fluid construction fails explicitly until the real superclass hooks,
//! intrusive registry holder, and complete state definition are implemented.

use mmga_classfile::MemberRef;
use mmga_runtime::{Rt, V};
const PROPERTIES: &str = "net/neoforged/neoforge/fluids/BaseFlowingFluid$Properties";
const BASE_FLOWING: &str = "net/neoforged/neoforge/fluids/BaseFlowingFluid";
const SOURCE: &str = "net/neoforged/neoforge/fluids/BaseFlowingFluid$Source";
const FLOWING: &str = "net/neoforged/neoforge/fluids/BaseFlowingFluid$Flowing";
const FLUID: &str = "net/minecraft/world/level/material/Fluid";
const STATE: &str = "net/minecraft/world/level/material/FluidState";

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    match sub {
        SOURCE | FLOWING => matches!(
            sup,
            BASE_FLOWING
                | "net/minecraft/world/level/material/FlowingFluid"
                | FLUID
                | "java/lang/Object"
        ),
        BASE_FLOWING => matches!(
            sup,
            "net/minecraft/world/level/material/FlowingFluid" | FLUID | "java/lang/Object"
        ),
        STATE if sup == "net/minecraft/world/level/block/state/StateHolder" => true,
        _ => false,
    }
}

pub fn call(rt: &mut Rt, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static
        && member.name == "<init>"
        && member.desc == format!("(L{PROPERTIES};)V")
        && matches!(member.owner.as_str(), BASE_FLOWING | SOURCE | FLOWING)
    {
        // Fluid.<init> creates its intrusive holder and invokes the subclass's
        // createFluidStateDefinition before BaseFlowingFluid reads Properties.
        // None of that behavior can be replaced by copying Properties fields or
        // inventing a shared Source/Flowing state graph, even for null Properties.
        if rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        return Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "BaseFlowingFluid construction requires an intrusive Fluid registry holder and executable subclass state-definition hooks; MMGA does not implement that constructor",
        ));
    }
    if is_static || member.owner != PROPERTIES {
        return None;
    }
    let receiver = *args.first()?;
    if member.name=="<init>"&&member.desc=="(Ljava/util/function/Supplier;Ljava/util/function/Supplier;Ljava/util/function/Supplier;)V"{
        for(field,value)in[("fluidType",*args.get(1)?),("still",*args.get(2)?),("flowing",*args.get(3)?),("bucket",V::NULL),("block",V::NULL),("slopeFindDistance",V::I(4)),("levelDecreasePerBlock",V::I(1)),("explosionResistance",V::F(1.0)),("tickRate",V::I(5))]{rt.put_field(receiver,field,value);}
        return Some(V::Void);
    }
    let setter=matches!((member.name.as_str(),member.desc.as_str()),
        ("bucket"|"block","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;")
        |("slopeFindDistance"|"levelDecreasePerBlock"|"tickRate","(I)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;")
        |("explosionResistance","(F)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;"));
    if setter {
        rt.put_field(receiver, &member.name, *args.get(1)?);
        return Some(receiver);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace};
    struct Host;
    impl HostApi for Host {
        fn get_static_field(&mut self, _: &mut Rt, _: &str, _: &str, _: &str) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            _: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, member, is_static, args)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            if name == "<init>" {
                op::INVOKESPECIAL
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(PROPERTIES, name, desc),
            args,
        )
    }
    #[test]
    fn constructor_keeps_lazy_suppliers_and_exact_defaults() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let properties = ctx.rt.new_instance(PROPERTIES, 0);
        let fluid_type = ctx.rt.new_instance("test/FluidTypeSupplier", 0);
        let still = ctx.rt.new_instance("test/StillSupplier", 0);
        let flowing = ctx.rt.new_instance("test/FlowingSupplier", 0);
        invoke(&mut ctx,"<init>","(Ljava/util/function/Supplier;Ljava/util/function/Supplier;Ljava/util/function/Supplier;)V",&[properties,fluid_type,still,flowing]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "actual BaseFlowingFluid properties ctor missing"
        );
        for (field, value) in [
            ("fluidType", fluid_type),
            ("still", still),
            ("flowing", flowing),
            ("bucket", V::NULL),
            ("block", V::NULL),
            ("slopeFindDistance", V::I(4)),
            ("levelDecreasePerBlock", V::I(1)),
            ("explosionResistance", V::F(1.0)),
            ("tickRate", V::I(5)),
        ] {
            assert_eq!(ctx.rt.get_field(properties, field), value);
        }
        for supplier in [fluid_type, still, flowing] {
            assert_eq!(ctx.rt.get_field(supplier, "calls").i(), 0);
        }
    }
    #[test]
    fn property_setters_preserve_references_and_values_without_invented_validation() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let properties = ctx.rt.new_instance(PROPERTIES, 0);
        invoke(&mut ctx,"<init>","(Ljava/util/function/Supplier;Ljava/util/function/Supplier;Ljava/util/function/Supplier;)V",&[properties,V::NULL,V::NULL,V::NULL]);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "source constructor preserves null suppliers until use"
        );
        let supplier = ctx.rt.new_instance("test/BucketSupplier", 0);
        for (name,desc,value) in [
            ("bucket","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",supplier),
            ("block","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",V::NULL),
            ("tickRate","(I)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",V::I(-7)),
            ("slopeFindDistance","(I)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",V::I(0)),
            ("levelDecreasePerBlock","(I)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",V::I(-1)),
            ("explosionResistance","(F)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",V::F(-3.5)),
        ]{
            assert_eq!(invoke(&mut ctx,name,desc,&[properties,value]),properties);
            assert_eq!(ctx.rt.get_field(properties,name),value);
        }
        assert!(ctx.rt.pending_exception.is_none());
        assert!(call(
            ctx.rt,
            &MemberRef::new(
                PROPERTIES,
                "tickRate",
                "(D)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;"
            ),
            false,
            &[properties, V::D(1.0)]
        )
        .is_none());
    }

    #[test]
    fn fluid_constructor_rejects_before_copying_properties_or_fabricating_state() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let properties = ctx.rt.new_instance(PROPERTIES, 0);
        let fluid_type = ctx.rt.new_instance("test/FluidTypeSupplier", 0);
        let still = ctx.rt.new_instance("test/StillSupplier", 0);
        let flowing = ctx.rt.new_instance("test/FlowingSupplier", 0);
        invoke(&mut ctx,"<init>","(Ljava/util/function/Supplier;Ljava/util/function/Supplier;Ljava/util/function/Supplier;)V",&[properties,fluid_type,still,flowing]);
        let bucket = ctx.rt.new_instance("test/BucketSupplier", 0);
        let block = ctx.rt.new_instance("test/BlockSupplier", 0);
        invoke(&mut ctx,"bucket","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",&[properties,bucket]);
        invoke(&mut ctx,"block","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",&[properties,block]);
        invoke(
            &mut ctx,
            "tickRate",
            "(I)Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;",
            &[properties, V::I(12)],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let original_properties = match &ctx.rt.get_obj(properties).unwrap().data {
            mmga_runtime::Data::Fields(fields) => fields.clone(),
            _ => panic!("Properties fields"),
        };
        for class in [BASE_FLOWING, SOURCE, FLOWING] {
            let fluid = ctx.rt.new_instance(class, 0);
            let marker = ctx.rt.intern_str("unmodified subclass marker");
            ctx.rt.put_field(fluid, "marker", marker);
            let before = match &ctx.rt.get_obj(fluid).unwrap().data {
                mmga_runtime::Data::Fields(fields) => fields.clone(),
                _ => panic!("fluid fields"),
            };
            let result = invoke_member(
                &mut ctx,
                op::INVOKESPECIAL,
                &MemberRef::new(class, "<init>", &format!("(L{PROPERTIES};)V")),
                &[fluid, properties],
            );
            assert_eq!(result, V::NULL);
            let exception = ctx
                .rt
                .take_exception()
                .expect("fluid constructor must fail explicitly");
            assert_eq!(
                ctx.rt.class_of(exception),
                Some("java/lang/UnsupportedOperationException")
            );
            assert!(ctx
                .rt
                .exception_summary(exception)
                .contains("intrusive Fluid registry holder"));
            let after = match &ctx.rt.get_obj(fluid).unwrap().data {
                mmga_runtime::Data::Fields(fields) => fields,
                _ => panic!("fluid fields"),
            };
            assert_eq!(
                after, &before,
                "rejected {class} constructor must not initialize partial state"
            );
        }
        let after_properties = match &ctx.rt.get_obj(properties).unwrap().data {
            mmga_runtime::Data::Fields(fields) => fields,
            _ => panic!("Properties fields"),
        };
        assert_eq!(
            after_properties, &original_properties,
            "rejected construction does not consume or alter Properties"
        );
        assert!(is_subtype(SOURCE, BASE_FLOWING));
        assert!(is_subtype(
            SOURCE,
            "net/minecraft/world/level/material/Fluid"
        ));
        assert!(!is_subtype(SOURCE, "net/minecraft/world/level/block/Block"));
        for supplier in [fluid_type, still, flowing, bucket, block] {
            assert_eq!(
                ctx.rt.get_field(supplier, "calls").i(),
                0,
                "rejected construction must not evaluate suppliers"
            );
        }
    }

    #[test]
    fn fluid_constructor_null_properties_still_reports_the_missing_superclass_boundary() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for class in [BASE_FLOWING, SOURCE, FLOWING] {
            let fluid = ctx.rt.new_instance(class, 0);
            let result = invoke_member(
                &mut ctx,
                op::INVOKESPECIAL,
                &MemberRef::new(class, "<init>", &format!("(L{PROPERTIES};)V")),
                &[fluid, V::NULL],
            );
            assert_eq!(result, V::NULL);
            let exception = ctx
                .rt
                .take_exception()
                .expect("unknown superclass cannot be skipped for null Properties");
            assert_eq!(
                ctx.rt.class_of(exception),
                Some("java/lang/UnsupportedOperationException")
            );
            assert!(ctx.rt.get_field(fluid, "stateDefinition").is_null());
            assert!(ctx.rt.get_field(fluid, "defaultFluidState").is_null());
            assert!(ctx.rt.get_field(fluid, "builtInRegistryHolder").is_null());
        }
    }

    #[test]
    fn fluid_and_fluid_state_operations_no_longer_return_fabricated_values() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let fluid = ctx.rt.new_instance(SOURCE, 0);
        let supplier = ctx.rt.new_instance("test/BucketSupplier", 0);
        ctx.rt.put_field(fluid, "bucket", supplier);
        let state = ctx.rt.new_instance(STATE, 0);
        ctx.rt.put_field(state, "owner", fluid);
        ctx.rt.put_field(state, "level", V::I(8));
        for (owner, name, desc, values) in [
            (
                FLUID,
                "getStateDefinition",
                "()Lnet/minecraft/world/level/block/state/StateDefinition;",
                vec![fluid],
            ),
            (
                FLUID,
                "defaultFluidState",
                "()Lnet/minecraft/world/level/material/FluidState;",
                vec![fluid],
            ),
            (
                FLUID,
                "getBucket",
                "()Lnet/minecraft/world/item/Item;",
                vec![fluid],
            ),
            (FLUID, "getExplosionResistance", "()F", vec![fluid]),
            (FLUID, "isEmpty", "()Z", vec![fluid]),
            (
                STATE,
                "getType",
                "()Lnet/minecraft/world/level/material/Fluid;",
                vec![state],
            ),
            (STATE, "getAmount", "()I", vec![state]),
            (STATE, "isSource", "()Z", vec![state]),
            (STATE, "isEmpty", "()Z", vec![state]),
            (STATE, "getOwnHeight", "()F", vec![state]),
            (
                STATE,
                "is",
                "(Lnet/minecraft/world/level/material/Fluid;)Z",
                vec![state, fluid],
            ),
        ] {
            let member = MemberRef::new(owner, name, desc);
            assert!(
                call(ctx.rt, &member, false, &values).is_none(),
                "{owner}.{name} is not implemented by Properties metadata"
            );
            let result = invoke_member(&mut ctx, op::INVOKEVIRTUAL, &member, &values);
            assert_eq!(result, V::NULL);
            let exception = ctx
                .rt
                .take_exception()
                .expect("runtime must expose the unsupported operation");
            assert_eq!(
                ctx.rt.class_of(exception),
                Some("java/lang/UnsupportedOperationException")
            );
        }
    }

    #[test]
    fn fluid_constructor_preserves_an_existing_exception_and_rejects_only_the_exact_signature() {
        let mut rt = Rt::new();
        let fluid = rt.new_instance(SOURCE, 0);
        rt.throw_exception("test/OriginalFailure", "original callback failure");
        let before = rt.pending_exception;
        assert_eq!(
            call(
                &mut rt,
                &MemberRef::new(SOURCE, "<init>", &format!("(L{PROPERTIES};)V")),
                false,
                &[fluid, V::NULL]
            ),
            Some(V::NULL)
        );
        assert_eq!(rt.pending_exception, before);
        rt.take_exception();
        for (owner, desc, is_static) in [
            (SOURCE, "()V", false),
            (SOURCE, "(Ljava/lang/Object;)V", false),
            (PROPERTIES, "()V", false),
            (
                SOURCE,
                "(Lnet/neoforged/neoforge/fluids/BaseFlowingFluid$Properties;)V",
                true,
            ),
        ] {
            assert!(call(
                &mut rt,
                &MemberRef::new(owner, "<init>", desc),
                is_static,
                &[fluid, V::NULL]
            )
            .is_none());
            assert!(rt.pending_exception.is_none());
        }
    }
}

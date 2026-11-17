//! Conditional loot-function value and codec composition from Minecraft 1.21.1.
//!
//! Official `ete.commonFields` uses LootItemCondition.DIRECT_CODEC, an optional
//! conditions list and the actual predicates getter. Applying a function tests
//! retained predicates in order before invoking its subclass run method. World
//! loot generation and registry condition decoding remain explicitly unsupported.
use mmga_classfile::MemberRef;
use mmga_runtime::{invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, Rt, V};
const BASE: &str = "net/minecraft/world/level/storage/loot/functions/LootItemConditionalFunction";
const CONDITION: &str = "net/minecraft/world/level/storage/loot/predicates/LootItemCondition";
const FUNCTION_TYPE: &str = "net/minecraft/world/level/storage/loot/functions/LootItemFunctionType";
const INSTANCE: &str = "com/mojang/serialization/codecs/RecordCodecBuilder$Instance";
const ADAPTER: &str = "mmga/internal/ConditionalLootFunction";
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != CONDITION || name != "DIRECT_CODEC" || desc != "Lcom/mojang/serialization/Codec;" {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let codec = crate::dfu::unsupported_codec(
        rt,
        "LootItemCondition.DIRECT_CODEC registry condition dispatch and inline all_of",
    );
    rt.put_static(owner, name, codec);
    Some(codec)
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if is_static
        && member.owner == ADAPTER
        && name == "conditions"
        && desc == "(Ljava/lang/Object;)Ljava/lang/Object;"
    {
        let value = *args.first()?;
        if value.is_null() {
            return Some(ctx.rt.throw_exception(
                "java/lang/NullPointerException",
                "conditional loot function getter",
            ));
        }
        return Some(ctx.rt.get_field(value, "predicates"));
    }
    if is_static
        && member.owner == ADAPTER
        && name == "test"
        && desc == "([Ljava/util/function/Predicate;Ljava/lang/Object;)Z"
    {
        let values = match ctx.rt.get_obj(*args.first()?).map(|o| &o.data) {
            Some(Data::Array(v)) => v.clone(),
            _ => return None,
        };
        for value in values {
            let tested = invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new(
                    "java/util/function/Predicate",
                    "test",
                    "(Ljava/lang/Object;)Z",
                ),
                &[value, *args.get(1)?],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            if tested.i() == 0 {
                return Some(V::FALSE);
            }
        }
        return Some(V::TRUE);
    }
    let is_conditional =
        member.owner == BASE || ctx.linked.is_subtype(&member.owner, BASE, ctx.host);
    if is_static&&is_conditional&&name=="commonFields"&&desc=="(Lcom/mojang/serialization/codecs/RecordCodecBuilder$Instance;)Lcom/mojang/datafixers/Products$P1;"{
        let direct=get_static_field(ctx.rt,CONDITION,"DIRECT_CODEC","Lcom/mojang/serialization/Codec;")?;
        let list=invoke_member(ctx,mmga_classfile::op::INVOKEINTERFACE,&MemberRef::new("com/mojang/serialization/Codec","listOf","()Lcom/mojang/serialization/Codec;"),&[direct]);
        if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}
        let empty=ctx.rt.alloc("java/util/ImmutableCollections$ListN",0,Data::List(Vec::new()));let field=ctx.rt.intern_str("conditions");
        let map=invoke_member(ctx,mmga_classfile::op::INVOKEINTERFACE,&MemberRef::new("com/mojang/serialization/Codec","optionalFieldOf","(Ljava/lang/String;Ljava/lang/Object;)Lcom/mojang/serialization/MapCodec;"),&[list,field,empty]);
        if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}
        let getter=ctx.rt.alloc("java/util/function/Function",0,Data::Closure(Box::new(ClosureData{
            interface_class:"java/util/function/Function".into(),sam_name:"apply".into(),sam_desc:"(Ljava/lang/Object;)Ljava/lang/Object;".into(),instantiated_desc:"(Ljava/lang/Object;)Ljava/lang/Object;".into(),captures:vec![],
            target:ClosureTarget::MethodHandle{handle_kind:6,member:MemberRef::new(ADAPTER,"conditions","(Ljava/lang/Object;)Ljava/lang/Object;")},
        })));
        let builder=invoke_member(ctx,mmga_classfile::op::INVOKEVIRTUAL,&MemberRef::new("com/mojang/serialization/MapCodec","forGetter","(Ljava/util/function/Function;)Lcom/mojang/serialization/codecs/RecordCodecBuilder;"),&[map,getter]);
        if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}
        return Some(invoke_member(ctx,mmga_classfile::op::INVOKEVIRTUAL,&MemberRef::new(INSTANCE,"group","(Lcom/mojang/datafixers/kinds/App;)Lcom/mojang/datafixers/Products$P1;"),&[*args.first()?,builder]));
    }
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    if member.owner == FUNCTION_TYPE {
        return match (name, desc) {
            ("<init>", "(Lcom/mojang/serialization/MapCodec;)V") => {
                ctx.rt.put_field(recv, "codec", *args.get(1)?);
                Some(V::Void)
            }
            ("codec", "()Lcom/mojang/serialization/MapCodec;") => {
                Some(ctx.rt.get_field(recv, "codec"))
            }
            _ => None,
        };
    }
    if !is_conditional {
        return None;
    }
    if name == "<init>" && desc == "(Ljava/util/List;)V" && member.owner == BASE {
        let predicates = *args.get(1)?;
        ctx.rt.put_field(recv, "predicates", predicates);
        let count = invoke_member(
            ctx,
            mmga_classfile::op::INVOKEINTERFACE,
            &MemberRef::new("java/util/List", "size", "()I"),
            &[predicates],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let mut values = Vec::new();
        for index in 0..count.i() {
            let value = invoke_member(
                ctx,
                mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new("java/util/List", "get", "(I)Ljava/lang/Object;"),
                &[predicates, V::I(index)],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            values.push(value);
        }
        // Util.allOf captures each predicate (or an array for >2), not the list.
        let composite = if values.len() == 1 {
            values[0]
        } else {
            if values.len() == 2 && values.iter().any(|v| v.is_null()) {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "null condition in Predicate.and",
                ));
            }
            let values = ctx
                .rt
                .alloc("[Ljava/util/function/Predicate;", 0, Data::Array(values));
            ctx.rt.alloc(
                "java/util/function/Predicate",
                0,
                Data::Closure(Box::new(ClosureData {
                    interface_class: "java/util/function/Predicate".into(),
                    sam_name: "test".into(),
                    sam_desc: "(Ljava/lang/Object;)Z".into(),
                    instantiated_desc: "(Ljava/lang/Object;)Z".into(),
                    captures: vec![values],
                    target: ClosureTarget::MethodHandle {
                        handle_kind: 6,
                        member: MemberRef::new(
                            ADAPTER,
                            "test",
                            "([Ljava/util/function/Predicate;Ljava/lang/Object;)Z",
                        ),
                    },
                })),
            )
        };
        ctx.rt.put_field(recv, "compositePredicates", composite);
        return Some(V::Void);
    }
    if name=="apply"&&matches!(desc,"(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/storage/loot/LootContext;)Lnet/minecraft/world/item/ItemStack;"|"(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"){
        let predicate=ctx.rt.get_field(recv,"compositePredicates");
        let passes=invoke_member(ctx,mmga_classfile::op::INVOKEINTERFACE,&MemberRef::new("java/util/function/Predicate","test","(Ljava/lang/Object;)Z"),&[predicate,*args.get(2)?]);
        if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}
        if passes.i()==0{return Some(args[1]);}
        return Some(invoke_member(ctx,mmga_classfile::op::INVOKEVIRTUAL,&MemberRef::new(BASE,"run","(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/storage/loot/LootContext;)Lnet/minecraft/world/item/ItemStack;"),args));
    }
    None
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == BASE
        && matches!(
            sup,
            "net/minecraft/world/level/storage/loot/functions/LootItemFunction"
                | "java/util/function/BiFunction"
                | "net/minecraft/world/level/storage/loot/LootContextUser"
        ))
        || (sub == CONDITION
            && matches!(
                sup,
                "java/util/function/Predicate"
                    | "net/minecraft/world/level/storage/loot/LootContextUser"
            ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, read_static_field, Data, HostApi, LinkedSpace, NativeClassDef,
        NativeModuleRegistry,
    };
    struct LootHost;
    impl HostApi for LootHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, owner, name, desc)
                .or_else(|| crate::dfu::get_static_field(rt, owner, name, desc))
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
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            super::is_subtype(sub, sup) || crate::dfu::is_subtype(sub, sup)
        }
    }
    fn run(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(args[0], "ran", V::TRUE);
        args[1]
    }
    fn predicate(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_field(args[0], "calls").i() + 1;
        ctx.rt.put_field(args[0], "calls", V::I(count));
        if ctx.rt.get_field(args[0], "throw") == V::TRUE {
            return ctx
                .rt
                .throw_exception("example/LootConditionFailure", "condition");
        }
        ctx.rt.get_field(args[0], "result")
    }
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry{classes:&[
        NativeClassDef{name:"test/LootFunction",superclass:Some(BASE),interfaces:&[],is_interface:false,methods:&[("run.(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/storage/loot/LootContext;)Lnet/minecraft/world/item/ItemStack;",run,false,false)]},
        NativeClassDef{name:"test/LootCondition",superclass:Some("java/lang/Object"),interfaces:&["java/util/function/Predicate"],is_interface:false,methods:&[("test.(Ljava/lang/Object;)Z",predicate,false,false)]},
    ],lambdas:&[]});
        linked
    }
    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    #[test]
    fn conditions_snapshot_short_circuits_and_runs_real_subclass_only_on_success() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = LootHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let a = ctx.rt.new_instance("test/LootCondition", 0);
        ctx.rt.put_field(a, "result", V::FALSE);
        let b = ctx.rt.new_instance("test/LootCondition", 0);
        ctx.rt.put_field(b, "result", V::TRUE);
        let list = ctx
            .rt
            .alloc("java/util/ArrayList", 0, Data::List(vec![a, b]));
        let function = ctx.rt.new_instance("test/LootFunction", 0);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(BASE, "<init>", "(Ljava/util/List;)V"),
            &[function, list],
        );
        assert!(ctx.rt.pending_exception.is_none());
        if let Data::List(items) = &mut ctx.rt.get_obj_mut(list).unwrap().data {
            items.clear();
        }
        let stack = ctx.rt.new_instance("net/minecraft/world/item/ItemStack", 0);
        let context = ctx
            .rt
            .new_instance("net/minecraft/world/level/storage/loot/LootContext", 0);
        let desc="(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/storage/loot/LootContext;)Lnet/minecraft/world/item/ItemStack;";
        assert_eq!(
            invoke(&mut ctx, BASE, "apply", desc, &[function, stack, context]),
            stack
        );
        assert_eq!(ctx.rt.get_field(a, "calls"), V::I(1));
        assert_eq!(ctx.rt.get_field(b, "calls"), V::NULL);
        assert_eq!(ctx.rt.get_field(function, "ran"), V::NULL);
        ctx.rt.put_field(a, "result", V::TRUE);
        invoke(&mut ctx, BASE, "apply", desc, &[function, stack, context]);
        assert_eq!(ctx.rt.get_field(function, "ran"), V::TRUE);
        assert_eq!(ctx.rt.get_field(b, "calls"), V::I(1));
        ctx.rt.put_field(a, "throw", V::TRUE);
        invoke(&mut ctx, BASE, "apply", desc, &[function, stack, context]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("LootConditionFailure"));
        assert_eq!(ctx.rt.get_field(b, "calls"), V::I(1));
    }
    #[test]
    fn inherited_common_fields_uses_real_default_and_rejects_unknown_conditions() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = LootHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let instance = ctx.rt.new_instance(INSTANCE, 0);
        let group=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new("test/LootFunction","commonFields","(Lcom/mojang/serialization/codecs/RecordCodecBuilder$Instance;)Lcom/mojang/datafixers/Products$P1;"),&[instance]);
        assert!(ctx.rt.pending_exception.is_none());
        let fields = ctx.rt.get_field(group, "$group_fields");
        let builder = match &ctx.rt.get_obj(fields).unwrap().data {
            Data::Array(v) => v[0],
            _ => panic!(),
        };
        let map = ctx.rt.get_field(builder, "$builder_codec");
        let codec = invoke(
            &mut ctx,
            "com/mojang/serialization/MapCodec",
            "codec",
            "()Lcom/mojang/serialization/Codec;",
            &[map],
        );
        let ops = read_static_field(
            &mut ctx,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        );
        let empty = ctx.rt.new_instance("com/google/gson/JsonObject", 0);
        invoke(
            &mut ctx,
            "com/google/gson/JsonObject",
            "<init>",
            "()V",
            &[empty],
        );
        let result=invoke(&mut ctx,"com/mojang/serialization/Codec","parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,empty]);
        let conditions = invoke(
            &mut ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            &[result],
        );
        assert!(
            matches!(ctx.rt.get_obj(conditions).map(|o|&o.data),Some(Data::List(v)) if v.is_empty())
        );
        assert!(ctx.rt.pending_exception.is_none());
        let encoded=invoke(&mut ctx,"com/mojang/serialization/Codec","encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,conditions]);
        let encoded = invoke(
            &mut ctx,
            "com/mojang/serialization/DataResult",
            "getOrThrow",
            "()Ljava/lang/Object;",
            &[encoded],
        );
        let rendered = invoke(
            &mut ctx,
            "com/google/gson/JsonElement",
            "toString",
            "()Ljava/lang/String;",
            &[encoded],
        );
        assert_eq!(
            ctx.rt.str_of(rendered),
            Some("{}"),
            "absent default conditions remain omitted"
        );
        let function = ctx.rt.new_instance("test/LootFunction", 0);
        ctx.rt.put_field(function, "predicates", conditions);
        let getter = ctx.rt.get_field(builder, "$builder_getter");
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/function/Function",
                "apply",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[getter, function]
            ),
            conditions
        );
        let nonempty = ctx
            .rt
            .alloc("java/util/ArrayList", 0, Data::List(vec![function]));
        invoke(&mut ctx,"com/mojang/serialization/Codec","encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[codec,ops,nonempty]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        let direct = read_static_field(
            &mut ctx,
            CONDITION,
            "DIRECT_CODEC",
            "Lcom/mojang/serialization/Codec;",
        );
        invoke(&mut ctx,"com/mojang/serialization/Codec","parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",&[direct,ops,empty]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

//! Custom EntitySelectorOptions registrations retain their original callback and description
//! references. Minecraft 1.21.1 class hk/hk$b is the semantic reference.
//! The vanilla selector option catalog and entity selection remain explicitly unavailable.
//! Custom lookup is executable when its actual canUse callback accepts the parser;
//! denied lookups and suggestions still require unimplemented parser/error context.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, Rt, V};

const OPTIONS: &str = "net/minecraft/commands/arguments/selector/options/EntitySelectorOptions";
const OPTION: &str =
    "net/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Option";
const MODIFIER: &str =
    "net/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier";
const PARSER: &str = "net/minecraft/commands/arguments/selector/EntitySelectorParser";
const STORAGE: &str = "mmga/internal/CustomEntitySelectorOptions";

/// Only custom registrations seen by this runtime are available. None does not assert
/// that a name is absent from Minecraft's full vanilla option catalog.
pub fn custom_option(rt: &Rt, name: V) -> Option<V> {
    let map = rt.get_static(STORAGE, "entries")?;
    let Data::Map(entries) = &rt.get_obj(map)?.data else {
        return None;
    };
    entries
        .iter()
        .find(|(key, _)| rt.val_eq(*key, name))
        .map(|(_, value)| *value)
}
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == OPTIONS && name == "OPTIONS" && desc == "Ljava/util/Map;" {
        return Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "The complete vanilla EntitySelectorOptions map is not materialized",
        ));
    }
    None
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == OPTION && sup == "java/lang/Record"
}
fn entries(rt: &mut Rt) -> V {
    if let Some(map) = rt.get_static(STORAGE, "entries") {
        return map;
    }
    let map = rt.alloc("java/util/HashMap", 0, Data::Map(vec![]));
    rt.put_static(STORAGE, "entries", map);
    map
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static && owner == "net/minecraft/commands/arguments/EntityArgument"
        && matches!(name,"getPlayers" | "getEntities")
        && desc == "(Lcom/mojang/brigadier/context/CommandContext;Ljava/lang/String;)Ljava/util/Collection;" {
        return Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException",
            "EntityArgument entity/player selection requires an actual world and EntitySelector execution"));
    }
    let receiver_class = args.first().and_then(|recv| ctx.rt.class_of(*recv));
    if !is_static
        && receiver_class == Some(OPTION)
        && (owner == OPTION || owner == "java/lang/Object")
        && name != "<init>"
    {
        let recv = *args.first()?;
        return match (name,desc) {
            ("modifier","()Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;") => Some(ctx.rt.get_field(recv,"modifier")),
            ("canUse","()Ljava/util/function/Predicate;") => Some(ctx.rt.get_field(recv,"canUse")),
            ("description","()Lnet/minecraft/network/chat/Component;") => Some(ctx.rt.get_field(recv,"description")),
            ("equals","(Ljava/lang/Object;)Z") => Some(option_equals(ctx,recv,args[1])),
            ("hashCode","()I") => Some(option_hash(ctx,recv)),
            ("toString","()Ljava/lang/String;") => Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException",
                "Selector Option record formatting requires callback and component text formatting")),
            _=>None,
        };
    }
    if !is_static && owner == OPTION && name == "<init>"
        && desc == "(Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;Ljava/util/function/Predicate;Lnet/minecraft/network/chat/Component;)V" {
        for (field,value) in [("modifier",args[1]),("canUse",args[2]),("description",args[3])] { ctx.rt.put_field(args[0],field,value); }
        return Some(V::Void);
    }
    if owner != OPTIONS || !is_static {
        return None;
    }
    match (name,desc) {
        ("register","(Ljava/lang/String;Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;Ljava/util/function/Predicate;Lnet/minecraft/network/chat/Component;)V") => {
            let key = *args.first()?;
            if !key.is_null() && ctx.rt.str_of(key).is_none() {
                return Some(ctx.rt.throw_exception("java/lang/ClassCastException","Selector option name must be a String"));
            }
            let option = ctx.rt.new_instance(OPTION,0);
            ctx.rt.put_field(option,"modifier",args[1]);
            ctx.rt.put_field(option,"canUse",args[2]);
            ctx.rt.put_field(option,"description",args[3]);
            let map = entries(ctx.rt);
            let Data::Map(values) = &ctx.rt.get_obj(map)?.data else { return None; };
            let existing = values.iter().position(|(name,_)|ctx.rt.val_eq(*name,key));
            if let Some(Data::Map(values)) = ctx.rt.get_obj_mut(map).map(|o|&mut o.data) {
                if let Some(index) = existing { values[index].1=option; } else { values.push((key,option)); }
            }
            Some(V::Void)
        }
        ("get","(Lnet/minecraft/commands/arguments/selector/EntitySelectorParser;Ljava/lang/String;I)Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;") => {
            let Some(option) = custom_option(ctx.rt,args[1]) else {
                return Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException",
                    "A missing custom selector registration does not prove absence from the unavailable vanilla option catalog"));
            };
            let predicate = ctx.rt.get_field(option,"canUse");
            let result = invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new("java/util/function/Predicate","test","(Ljava/lang/Object;)Z"),&[predicate,args[0]]);
            if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
            if result.i()!=0 { Some(ctx.rt.get_field(option,"modifier")) }
            else { Some(inapplicable(ctx,args[0],args[1])) }
        }
        ("bootStrap","()V") | ("suggestNames","(Lnet/minecraft/commands/arguments/selector/EntitySelectorParser;Lcom/mojang/brigadier/suggestion/SuggestionsBuilder;)V") =>
            Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException","The complete vanilla selector option catalog is not implemented")),
        _=>None,
    }
}
fn option_equals(ctx: &mut ExecCtx<'_>, left: V, right: V) -> V {
    if left == right {
        return V::TRUE;
    }
    if ctx.rt.class_of(right) != Some(OPTION) {
        return V::FALSE;
    }
    for field in ["modifier", "canUse", "description"] {
        let a = ctx.rt.get_field(left, field);
        let b = ctx.rt.get_field(right, field);
        if a == b {
            continue;
        }
        if a.is_null() {
            return V::FALSE;
        }
        let equal = invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
            &[a, b],
        );
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        if equal.i() == 0 {
            return V::FALSE;
        }
    }
    V::TRUE
}
fn option_hash(ctx: &mut ExecCtx<'_>, option: V) -> V {
    let mut hash = 0_i32;
    for field in ["modifier", "canUse", "description"] {
        let value = ctx.rt.get_field(option, field);
        if field == "description"
            && !value.is_null()
            && ctx.rt.class_of(value).is_some_and(|class| {
                ctx.linked
                    .resolve_method(class, "hashCode", "()I")
                    .is_none()
            })
        {
            // Native Component equality exists, but its matching hashCode does not yet.
            // Falling back to Object identity would violate the record hash contract.
            return ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Selector Option hashing requires Component.hashCode",
            );
        }
        let next = if value.is_null() {
            0
        } else {
            invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Object", "hashCode", "()I"),
                &[value],
            )
            .i()
        };
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        hash = hash.wrapping_mul(31).wrapping_add(next);
    }
    V::I(hash)
}
fn inapplicable(ctx: &mut ExecCtx<'_>, _parser: V, _name: V) -> V {
    // The official denied path throws ERROR_INAPPLICABLE_OPTION.createWithContext.
    // The reader, canonical error type, and CommandSyntaxException context are not
    // implemented here, so no incomplete exception or dummy reader is fabricated.
    ctx.rt.throw_exception("java/lang/UnsupportedOperationException",
        "Denied selector options require the actual parser reader and CommandSyntaxException context")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, ClosureData, ClosureTarget, Data, HostApi, LinkedSpace, NativeModuleRegistry,
    };
    struct SelectorHost;
    impl HostApi for SelectorHost {
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
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            call(
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
        }
    }
    fn modifier(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let observed = args[0];
        let parser = args[1];
        let count = ctx.rt.get_field(observed, "modifier_calls").i();
        ctx.rt
            .put_field(observed, "modifier_calls", V::I(count + 1));
        ctx.rt.put_field(observed, "parser", parser);
        V::Void
    }
    fn can_use(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let observed = args[0];
        let parser = args[1];
        let count = ctx.rt.get_field(observed, "predicate_calls").i();
        ctx.rt
            .put_field(observed, "predicate_calls", V::I(count + 1));
        ctx.rt.put_field(observed, "parser", parser);
        V::TRUE
    }
    fn failing_predicate(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("test/CanUseFailure", "actual predicate failed")
    }
    fn deny(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        can_use(ctx, args);
        V::FALSE
    }
    fn closure(rt: &mut Rt, interface: &str, name: &str, desc: &str, observed: V, id: u32) -> V {
        rt.alloc(
            interface,
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: interface.into(),
                sam_name: name.into(),
                sam_desc: desc.into(),
                instantiated_desc: if name == "handle" {
                    format!("(L{PARSER};)V")
                } else {
                    format!("(L{PARSER};)Z")
                },
                captures: vec![observed],
                target: ClosureTarget::NativeLambda(id),
            })),
        )
    }
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[],
            lambdas: &[
                (301, modifier),
                (302, can_use),
                (303, failing_predicate),
                (304, deny),
            ],
        });
        linked
    }
    fn register(ctx: &mut ExecCtx<'_>, name: V, modifier: V, predicate: V, description: V) -> V {
        invoke_member(ctx,op::INVOKESTATIC,&MemberRef::new(OPTIONS,"register","(Ljava/lang/String;Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;Ljava/util/function/Predicate;Lnet/minecraft/network/chat/Component;)V"),&[name,modifier,predicate,description])
    }
    #[test]
    fn registration_is_lazy_and_replacement_retains_original_option_values_and_identity() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = SelectorHost;
        let observed = rt.new_instance("test/Observer", 0);
        let modifier = closure(
            &mut rt,
            MODIFIER,
            "handle",
            &format!("(L{PARSER};)V"),
            observed,
            301,
        );
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            observed,
            302,
        );
        let description = rt.new_instance("net/minecraft/network/chat/MutableComponent", 0);
        let name = rt.new_str("curios");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(
            register(&mut ctx, name, modifier, predicate, description),
            V::Void
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(observed, "modifier_calls").i(), 0);
        assert_eq!(ctx.rt.get_field(observed, "predicate_calls").i(), 0);
        let value = custom_option(ctx.rt, name).unwrap();
        assert_eq!(ctx.rt.class_of(value), Some(OPTION));
        for (field, expected) in [
            ("modifier", modifier),
            ("canUse", predicate),
            ("description", description),
        ] {
            assert_eq!(ctx.rt.get_field(value, field), expected);
        }
        let equal_name = ctx.rt.new_str("curios");
        assert_eq!(custom_option(ctx.rt, equal_name), Some(value));
        ctx.rt.put_field(description, "changed", V::TRUE);
        assert_eq!(
            ctx.rt
                .get_field(ctx.rt.get_field(value, "description"), "changed"),
            V::TRUE
        );
        register(&mut ctx, equal_name, V::NULL, V::NULL, V::NULL);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "Official register does not reject null record fields"
        );
        let replaced = custom_option(ctx.rt, name).unwrap();
        assert_ne!(value, replaced);
        assert_eq!(ctx.rt.get_field(replaced, "modifier"), V::NULL);
        assert_eq!(
            ctx.rt.get_field(value, "modifier"),
            modifier,
            "Previously returned record remains independent"
        );
        let other = Rt::new();
        assert!(
            custom_option(&other, name).is_none(),
            "Registration state belongs to the runtime"
        );
    }
    #[test]
    fn custom_get_runs_actual_can_use_then_returns_original_modifier_without_parsing_entities() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = SelectorHost;
        let observed = rt.new_instance("test/Observer", 0);
        let parser = rt.new_instance(PARSER, 0);
        let modifier = closure(
            &mut rt,
            MODIFIER,
            "handle",
            &format!("(L{PARSER};)V"),
            observed,
            301,
        );
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            observed,
            302,
        );
        let name = rt.new_str("custom");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        register(&mut ctx, name, modifier, predicate, V::NULL);
        let resolved = invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(OPTIONS,"get","(Lnet/minecraft/commands/arguments/selector/EntitySelectorParser;Ljava/lang/String;I)Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;"),&[parser,name,V::I(2)]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(resolved, modifier);
        assert_eq!(ctx.rt.get_field(observed, "predicate_calls"), V::I(1));
        assert_eq!(ctx.rt.get_field(observed, "modifier_calls").i(), 0);
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(MODIFIER, "handle", &format!("(L{PARSER};)V")),
            &[resolved, parser],
        );
        assert_eq!(ctx.rt.get_field(observed, "modifier_calls"), V::I(1));
        assert_eq!(ctx.rt.get_field(observed, "parser"), parser);
    }
    #[test]
    fn unknown_vanilla_catalog_and_failing_predicates_never_turn_into_empty_results() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = SelectorHost;
        let observed = rt.new_instance("test/Observer", 0);
        let parser = rt.new_instance(PARSER, 0);
        let failure = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            observed,
            303,
        );
        let name = rt.new_str("custom");
        let vanilla = rt.new_str("distance");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        register(&mut ctx, name, V::NULL, failure, V::NULL);
        invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(OPTIONS,"get","(Lnet/minecraft/commands/arguments/selector/EntitySelectorParser;Ljava/lang/String;I)Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;"),&[parser,name,V::I(0)]);
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(exception), Some("test/CanUseFailure"));
        invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(OPTIONS,"get","(Lnet/minecraft/commands/arguments/selector/EntitySelectorParser;Ljava/lang/String;I)Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;"),&[parser,vanilla,V::I(0)]);
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(exception),
            Some("java/lang/UnsupportedOperationException")
        );
        assert!(ctx.rt.exception_summary(exception).contains("vanilla"));
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(OPTIONS, "bootStrap", "()V"),
            &[],
        );
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(exception),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn null_key_is_retained_but_denied_parser_context_and_world_queries_are_explicit() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = SelectorHost;
        let observed = rt.new_instance("test/Observer", 0);
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            observed,
            304,
        );
        let parser = rt.new_instance(PARSER, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        register(&mut ctx, V::NULL, V::NULL, predicate, V::NULL);
        assert!(ctx.rt.pending_exception.is_none());
        assert!(custom_option(ctx.rt, V::NULL).is_some());
        invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(OPTIONS,"get","(Lnet/minecraft/commands/arguments/selector/EntitySelectorParser;Ljava/lang/String;I)Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;"),&[parser,V::NULL,V::I(9)]);
        let exception = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(exception),
            Some("java/lang/UnsupportedOperationException")
        );
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("CommandSyntaxException context"));
        assert_eq!(ctx.rt.get_field(observed, "predicate_calls"), V::I(1));
        for method in ["getPlayers", "getEntities"] {
            invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new("net/minecraft/commands/arguments/EntityArgument",method,"(Lcom/mojang/brigadier/context/CommandContext;Ljava/lang/String;)Ljava/util/Collection;"),&[V::NULL,V::NULL]);
            let exception = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(exception),
                Some("java/lang/UnsupportedOperationException")
            );
            assert!(ctx.rt.exception_summary(exception).contains("actual world"));
        }
    }
    #[test]
    fn direct_option_record_constructor_preserves_fields_and_nullable_components() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = SelectorHost;
        let observer = rt.new_instance("test/Observer", 0);
        let modifier = closure(
            &mut rt,
            MODIFIER,
            "handle",
            &format!("(L{PARSER};)V"),
            observer,
            301,
        );
        let predicate = closure(
            &mut rt,
            "java/util/function/Predicate",
            "test",
            "(Ljava/lang/Object;)Z",
            observer,
            302,
        );
        let option = rt.new_instance(OPTION, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let ctor = MemberRef::new(OPTION,"<init>","(Lnet/minecraft/commands/arguments/selector/options/EntitySelectorOptions$Modifier;Ljava/util/function/Predicate;Lnet/minecraft/network/chat/Component;)V");
        assert_eq!(
            invoke_member(
                &mut ctx,
                op::INVOKESPECIAL,
                &ctor,
                &[option, modifier, predicate, V::NULL]
            ),
            V::Void
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(option, "modifier"), modifier);
        assert_eq!(ctx.rt.get_field(option, "canUse"), predicate);
        assert_eq!(ctx.rt.get_field(option, "description"), V::NULL);
    }
}

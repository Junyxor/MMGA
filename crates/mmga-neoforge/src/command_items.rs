//! Item command argument construction and read-only command registry lookup.
//! Semantic reference: official Minecraft 1.21.1 hc/he/ep/jo$a/akp bytecode.
//! Argument construction performs the real provider calls. Item/NBT parsing and
//! complete vanilla registry/tag enumeration are deliberately not approximated.

use crate::RegistryStore;
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{alloc_new_object, invoke_member, read_static_field, Data, ExecCtx, Rt, V};

const ARGUMENT: &str = "net/minecraft/commands/arguments/item/ItemArgument";
const PARSER: &str = "net/minecraft/commands/arguments/item/ItemParser";
const CONTEXT: &str = "net/minecraft/commands/CommandBuildContext";
const PROVIDER: &str = "net/minecraft/core/HolderLookup$Provider";
const LOOKUP: &str = "mmga/internal/CommandRegistryLookup";
const REGISTRY_OPS: &str = "net/minecraft/resources/RegistryOps";
const NBT_OPS: &str = "net/minecraft/nbt/NbtOps";
const ADAPTER: &str = "net/minecraft/resources/RegistryOps$HolderLookupAdapter";
const CONTEXT_CACHE: &str = "mmga/internal/CommandLookupCache";

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != NBT_OPS || name != "INSTANCE" || desc != "Lnet/minecraft/nbt/NbtOps;" {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = rt.new_instance(NBT_OPS, 0);
    rt.put_static(owner, name, value);
    Some(value)
}
pub fn call_context(
    rt: &mut Rt,
    registries: &RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    let is_context = matches!(rt.handle_of(receiver), Some(("cmd_build_ctx", ..)));
    if is_context
        && name == "lookup"
        && desc == "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;"
    {
        let key = *args.get(1)?;
        let (parent, registry) = crate::minecraft::resource_key_parts(rt, key)?;
        let available = parent == "minecraft:root"
            && (registries.entries.contains_key(&registry)
                || registries.completed_registries.contains(&registry)
                || registries
                    .reference_states
                    .get(&registry)
                    .is_some_and(|state| state.config.is_some()));
        if !available {
            return Some(rt.alloc("java/util/Optional", 0, Data::Optional(None)));
        }
        let cache_key = format!("{}:{registry}", receiver.r());
        let lookup = if let Some(value) = rt.get_static(CONTEXT_CACHE, &cache_key) {
            value
        } else {
            let value = rt.new_instance(LOOKUP, 0);
            let registry_name = rt.intern_str(&registry);
            rt.put_field(value, "registry", registry_name);
            rt.put_field(value, "key", key);
            rt.put_field(value, "provider", receiver);
            rt.put_static(CONTEXT_CACHE, &cache_key, value);
            value
        };
        return Some(rt.alloc("java/util/Optional", 0, Data::Optional(Some(lookup))));
    }
    if rt.class_of(receiver) != Some(LOOKUP) {
        return None;
    }
    let registry = rt.str_of(rt.get_field(receiver, "registry"))?.to_owned();
    match (name, desc) {
        ("key", "()Lnet/minecraft/resources/ResourceKey;") => Some(rt.get_field(receiver, "key")),
        ("registryLifecycle", "()Lcom/mojang/serialization/Lifecycle;") => {
            let lifecycle = registries
                .reference_states
                .get(&registry)
                .and_then(|state| state.config.as_ref())
                .map(|config| config.lifecycle);
            Some(lifecycle.unwrap_or_else(|| {
                rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Registry lifecycle metadata is not available for command lookup",
                )
            }))
        }
        ("get", "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;") => {
            let key = *args.get(1)?;
            let (owner, location) = crate::minecraft::resource_key_parts(rt, key)?;
            if owner != registry {
                return Some(rt.alloc("java/util/Optional", 0, Data::Optional(None)));
            }
            if let Some(reference) = registries.canonical_reference(&registry, &location) {
                return Some(rt.alloc("java/util/Optional", 0, Data::Optional(Some(reference))));
            }
            if let Some(entry) = registries.lookup(&registry, &location) {
                if !entry.value.is_null() {
                    return Some(rt.throw_exception("java/lang/UnsupportedOperationException",&format!("Registered element {location} has no canonical Holder.Reference for command lookup")));
                }
                return Some(rt.throw_exception("java/lang/UnsupportedOperationException",&format!("Builtin element {location} is not materialized in partial command lookup {registry}")));
            }
            Some(rt.alloc("java/util/Optional", 0, Data::Optional(None)))
        }
        ("get", "(Lnet/minecraft/tags/TagKey;)Ljava/util/Optional;")
        | ("listTags", "()Ljava/util/stream/Stream;")
        | ("listTagIds", "()Ljava/util/stream/Stream;") => Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Command lookup tag membership is not loaded",
        )),
        ("listElements", "()Ljava/util/stream/Stream;")
        | ("listElementIds", "()Ljava/util/stream/Stream;") => Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Complete command registry enumeration is not materialized",
        )),
        _ => None,
    }
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if !is_static && name=="lookupOrThrow" && desc=="(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/core/HolderLookup$RegistryLookup;"
        && ctx.linked.is_subtype(ctx.rt.class_of(*args.first()?)?,PROVIDER,ctx.host){
        let optional=invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(PROVIDER,"lookup","(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;"),args);
        if ctx.rt.pending_exception.is_some(){return Some(V::NULL);}
        return Some(match ctx.rt.get_obj(optional).map(|o|&o.data){
            Some(Data::Optional(Some(value)))=>*value,
            Some(Data::Optional(None))=>ctx.rt.throw_exception("java/lang/IllegalStateException","Requested command registry is not present"),
            _=>ctx.rt.throw_exception("java/lang/ClassCastException","HolderLookup.Provider.lookup must return Optional"),
        });
    }
    if !is_static
        && name == "createSerializationContext"
        && desc == "(Lcom/mojang/serialization/DynamicOps;)Lnet/minecraft/resources/RegistryOps;"
        && ctx
            .linked
            .is_subtype(ctx.rt.class_of(*args.first()?)?, PROVIDER, ctx.host)
    {
        return Some(registry_ops(ctx.rt, args[1], args[0], true));
    }
    if is_static && owner == REGISTRY_OPS && name == "create" {
        return match desc{
            "(Lcom/mojang/serialization/DynamicOps;Lnet/minecraft/core/HolderLookup$Provider;)Lnet/minecraft/resources/RegistryOps;"=>Some(registry_ops(ctx.rt,args[0],args[1],true)),
            "(Lcom/mojang/serialization/DynamicOps;Lnet/minecraft/resources/RegistryOps$RegistryInfoLookup;)Lnet/minecraft/resources/RegistryOps;"=>Some(registry_ops(ctx.rt,args[0],args[1],false)),
            _=>None,
        };
    }
    let receiver = args.first().copied().unwrap_or(V::NULL);
    let class = ctx.rt.class_of(receiver).map(str::to_owned);
    if !is_static && class.as_deref() == Some(REGISTRY_OPS) {
        return match (name,desc){
            ("withParent","(Lcom/mojang/serialization/DynamicOps;)Lnet/minecraft/resources/RegistryOps;")=>{
                if ctx.rt.get_field(receiver,"delegate")==args[1]{Some(receiver)}else{let provider=ctx.rt.get_field(receiver,"lookupProvider");Some(registry_ops(ctx.rt,args[1],provider,false))}
            }
            ("equals","(Ljava/lang/Object;)Z")=>Some(ops_equals(ctx,receiver,args[1])),
            ("hashCode","()I")=>Some(ops_hash(ctx,receiver)),
            _=>Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException","RegistryOps execution requires the actual NBT/DynamicOps and registry info services")),
        };
    }
    if !is_static && class.as_deref() == Some(ADAPTER) {
        return match (name, desc) {
            ("equals", "(Ljava/lang/Object;)Z") => {
                if receiver == args[1] {
                    Some(V::TRUE)
                } else if ctx.rt.class_of(args[1]) != Some(ADAPTER) {
                    Some(V::FALSE)
                } else {
                    let a = ctx.rt.get_field(receiver, "lookupProvider");
                    let b = ctx.rt.get_field(args[1], "lookupProvider");
                    Some(object_call(ctx, a, "equals", "(Ljava/lang/Object;)Z", &[b]))
                }
            }
            ("hashCode", "()I") => {
                let provider = ctx.rt.get_field(receiver, "lookupProvider");
                Some(object_call(ctx, provider, "hashCode", "()I", &[]))
            }
            _ => Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "RegistryInfo lookup execution is not implemented",
            )),
        };
    }
    if !is_static && class.as_deref() == Some(NBT_OPS) {
        return match (name, desc) {
            ("equals", "(Ljava/lang/Object;)Z") => Some(V::bool(receiver == args[1])),
            ("hashCode", "()I") => Some(V::I(receiver.r() as i32)),
            _ => Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "NbtOps execution is not implemented",
            )),
        };
    }
    if is_static && owner==ARGUMENT && name=="item" && desc=="(Lnet/minecraft/commands/CommandBuildContext;)Lnet/minecraft/commands/arguments/item/ItemArgument;"{
        let value=alloc_new_object(ctx,ARGUMENT);
        let constructed=construct_argument(ctx,value,args[0]);
        return Some(if ctx.rt.pending_exception.is_some(){constructed}else{value});
    }
    if !is_static
        && owner == ARGUMENT
        && name == "<init>"
        && desc == "(Lnet/minecraft/commands/CommandBuildContext;)V"
    {
        return Some(construct_argument(ctx, receiver, args[1]));
    }
    if !is_static
        && owner == PARSER
        && name == "<init>"
        && desc == "(Lnet/minecraft/core/HolderLookup$Provider;)V"
    {
        return Some(construct_parser(ctx, receiver, args[1]));
    }
    if !is_static
        && matches!(class.as_deref(), Some(ARGUMENT | PARSER))
        && matches!(
            name,
            "parse" | "fillSuggestions" | "listSuggestions" | "getExamples"
        )
    {
        return Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException","Item command parsing/suggestions require full item component prototypes and NBT/Brigadier execution"));
    }
    None
}
fn construct_argument(ctx: &mut ExecCtx<'_>, argument: V, provider: V) -> V {
    let parser = alloc_new_object(ctx, PARSER);
    construct_parser(ctx, parser, provider);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    ctx.rt.put_field(argument, "parser", parser);
    V::Void
}
fn construct_parser(ctx: &mut ExecCtx<'_>, parser: V, provider: V) -> V {
    let registry = read_static_field(
        ctx,
        "net/minecraft/core/registries/Registries",
        "ITEM",
        "Lnet/minecraft/resources/ResourceKey;",
    );
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let items=invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(PROVIDER,"lookupOrThrow","(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/core/HolderLookup$RegistryLookup;"),&[provider,registry]);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    ctx.rt.put_field(parser, "items", items);
    let ops = read_static_field(ctx, NBT_OPS, "INSTANCE", "Lnet/minecraft/nbt/NbtOps;");
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let registry_ops = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(
            PROVIDER,
            "createSerializationContext",
            "(Lcom/mojang/serialization/DynamicOps;)Lnet/minecraft/resources/RegistryOps;",
        ),
        &[provider, ops],
    );
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    ctx.rt.put_field(parser, "registryOps", registry_ops);
    V::Void
}
fn registry_ops(rt: &mut Rt, delegate: V, provider: V, adapt: bool) -> V {
    let provider = if adapt {
        let value = rt.new_instance(ADAPTER, 0);
        let cache = rt.alloc("java/util/HashMap", 0, Data::Map(vec![]));
        rt.put_field(value, "lookupProvider", provider);
        rt.put_field(value, "lookups", cache);
        value
    } else {
        provider
    };
    let value = rt.new_instance(REGISTRY_OPS, 0);
    rt.put_field(value, "delegate", delegate);
    rt.put_field(value, "lookupProvider", provider);
    value
}
fn object_call(ctx: &mut ExecCtx<'_>, value: V, name: &str, desc: &str, args: &[V]) -> V {
    let class = ctx
        .rt
        .class_of(value)
        .unwrap_or("java/lang/Object")
        .to_owned();
    let mut all = vec![value];
    all.extend_from_slice(args);
    invoke_member(
        ctx,
        op::INVOKEVIRTUAL,
        &MemberRef::new(class, name, desc),
        &all,
    )
}
fn ops_equals(ctx: &mut ExecCtx<'_>, left: V, right: V) -> V {
    if left == right {
        return V::TRUE;
    }
    if ctx.rt.class_of(right) != ctx.rt.class_of(left) {
        return V::FALSE;
    }
    for field in ["delegate", "lookupProvider"] {
        let a = ctx.rt.get_field(left, field);
        let b = ctx.rt.get_field(right, field);
        let equal = object_call(ctx, a, "equals", "(Ljava/lang/Object;)Z", &[b]);
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
        if equal.i() == 0 {
            return V::FALSE;
        }
    }
    V::TRUE
}
fn ops_hash(ctx: &mut ExecCtx<'_>, value: V) -> V {
    let delegate = ctx.rt.get_field(value, "delegate");
    let provider = ctx.rt.get_field(value, "lookupProvider");
    let a = object_call(ctx, delegate, "hashCode", "()I", &[]).i();
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let b = object_call(ctx, provider, "hashCode", "()I", &[]).i();
    V::I(a.wrapping_mul(31).wrapping_add(b))
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == CONTEXT && sup == PROVIDER)
        || (sub == ARGUMENT && sup == "com/mojang/brigadier/arguments/ArgumentType")
        || (sub == LOOKUP
            && matches!(
                sup,
                "net/minecraft/core/HolderLookup$RegistryLookup"
                    | "net/minecraft/core/HolderLookup"
                    | "net/minecraft/core/HolderGetter"
                    | "net/minecraft/core/HolderOwner"
            ))
        || (sub == REGISTRY_OPS
            && matches!(
                sup,
                "com/mojang/serialization/DynamicOps" | "net/minecraft/resources/DelegatingOps"
            ))
        || (sub == NBT_OPS && sup == "com/mojang/serialization/DynamicOps")
        || (sub == ADAPTER && sup == "net/minecraft/resources/RegistryOps$RegistryInfoLookup")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentProperties, RegistryStore};
    use mmga_classfile::{op, MemberRef};
    use mmga_runtime::{
        invoke_member, read_static_field, Data, HostApi, LinkedSpace, NativeClassDef,
        NativeModuleRegistry,
    };
    struct ItemHost(RegistryStore);
    impl HostApi for ItemHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            get_static_field(rt, owner, name, desc).or_else(|| {
                if owner == "net/minecraft/core/registries/Registries"
                    && name == "ITEM"
                    && desc == "Lnet/minecraft/resources/ResourceKey;"
                {
                    Some(crate::minecraft::new_resource_key(
                        rt,
                        "minecraft:root",
                        "minecraft:item",
                    ))
                } else {
                    None
                }
            })
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            if let Some(value) = call_context(rt, &self.0, member, is_static, args) {
                return Some(value);
            }
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
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            is_subtype(sub, sup)
        }
    }
    fn provider_lookup(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let expected = ctx.rt.get_field(args[0], "expectedKey");
        assert_eq!(args[1], expected);
        let n = ctx.rt.get_field(args[0], "lookupCalls").i();
        ctx.rt.put_field(args[0], "lookupCalls", V::I(n + 1));
        let value = ctx.rt.get_field(args[0], "lookup");
        ctx.rt.alloc(
            "java/util/Optional",
            0,
            Data::Optional((!value.is_null()).then_some(value)),
        )
    }
    fn provider_ops(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let n = ctx.rt.get_field(args[0], "opsCalls").i();
        ctx.rt.put_field(args[0], "opsCalls", V::I(n + 1));
        ctx.rt.put_field(args[0], "observedOps", args[1]);
        ctx.rt.get_field(args[0], "resultOps")
    }
    fn throwing_lookup(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        ctx.rt
            .throw_exception("test/CommandLookupFailure", "lookup failed")
    }
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry{classes:&[
            NativeClassDef{name:"test/CommandContext",superclass:Some("java/lang/Object"),interfaces:&[CONTEXT],is_interface:false,methods:&[
                ("lookup.(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",provider_lookup,false,false),
                ("createSerializationContext.(Lcom/mojang/serialization/DynamicOps;)Lnet/minecraft/resources/RegistryOps;",provider_ops,false,false),
            ]},
            NativeClassDef{name:"test/ThrowingCommandContext",superclass:Some("java/lang/Object"),interfaces:&[CONTEXT],is_interface:false,methods:&[
                ("lookup.(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",throwing_lookup,false,false),
            ]},
        ],lambdas:&[]});
        linked
    }
    #[test]
    fn item_argument_constructor_invokes_provider_lookup_and_serialization_and_keeps_exact_results()
    {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = ItemHost(RegistryStore::new());
        let key = crate::minecraft::new_resource_key(&mut rt, "minecraft:root", "minecraft:item");
        let context = rt.new_instance("test/CommandContext", 0);
        let lookup = rt.new_instance(LOOKUP, 0);
        let ops = rt.new_instance(REGISTRY_OPS, 0);
        rt.put_field(context, "expectedKey", key);
        rt.put_field(context, "lookup", lookup);
        rt.put_field(context, "resultOps", ops);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let argument=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(ARGUMENT,"item","(Lnet/minecraft/commands/CommandBuildContext;)Lnet/minecraft/commands/arguments/item/ItemArgument;"),&[context]);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.class_of(argument), Some(ARGUMENT));
        assert_eq!(ctx.rt.get_field(context, "lookupCalls"), V::I(1));
        assert_eq!(ctx.rt.get_field(context, "opsCalls"), V::I(1));
        let parser = ctx.rt.get_field(argument, "parser");
        assert_eq!(ctx.rt.class_of(parser), Some(PARSER));
        assert_eq!(ctx.rt.get_field(parser, "items"), lookup);
        assert_eq!(ctx.rt.get_field(parser, "registryOps"), ops);
        let nbt = read_static_field(&mut ctx, NBT_OPS, "INSTANCE", "Lnet/minecraft/nbt/NbtOps;");
        assert_eq!(ctx.rt.get_field(context, "observedOps"), nbt);
        let reader = ctx.rt.new_instance("com/mojang/brigadier/StringReader", 0);
        invoke_member(&mut ctx,op::INVOKEVIRTUAL,&MemberRef::new(ARGUMENT,"parse","(Lcom/mojang/brigadier/StringReader;)Lnet/minecraft/commands/arguments/item/ItemInput;"),&[argument,reader]);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
    #[test]
    fn missing_registry_null_context_and_original_lookup_failure_never_construct_fake_parser() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = ItemHost(RegistryStore::new());
        let key = crate::minecraft::new_resource_key(&mut rt, "minecraft:root", "minecraft:item");
        let missing = rt.new_instance("test/CommandContext", 0);
        rt.put_field(missing, "expectedKey", key);
        let throwing = rt.new_instance("test/ThrowingCommandContext", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (context, expected) in [
            (missing, "java/lang/IllegalStateException"),
            (V::NULL, "java/lang/NullPointerException"),
            (throwing, "test/CommandLookupFailure"),
        ] {
            invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(ARGUMENT,"item","(Lnet/minecraft/commands/CommandBuildContext;)Lnet/minecraft/commands/arguments/item/ItemArgument;"),&[context]);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(ctx.rt.class_of(error), Some(expected));
        }
        assert_eq!(ctx.rt.get_field(missing, "opsCalls").i(), 0);
    }
    #[test]
    fn native_command_context_lookup_reads_real_canonical_references_after_freeze() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut registries = RegistryStore::new();
        let item = rt.new_instance("net/minecraft/world/item/Item", 0);
        registries.register_direct(
            &mut rt,
            "minecraft:item",
            "test:actual",
            "test",
            item,
            ContentProperties {
                kind: "item".into(),
                ..Default::default()
            },
        );
        let reference = registries
            .canonical_reference("minecraft:item", "test:actual")
            .unwrap();
        registries.freeze_checked(&mut rt);
        let mut host = ItemHost(registries);
        let original = host.0.entries.len();
        let count = host.0.entries["minecraft:item"].len();
        let context = rt.new_pinned_handle(CONTEXT, "cmd_build_ctx", 0, "");
        let registry_key =
            crate::minecraft::new_resource_key(&mut rt, "minecraft:root", "minecraft:item");
        let item_key = crate::minecraft::new_resource_key(&mut rt, "minecraft:item", "test:actual");
        let vanilla_key =
            crate::minecraft::new_resource_key(&mut rt, "minecraft:item", "minecraft:apple");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let lookup=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(PROVIDER,"lookupOrThrow","(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/core/HolderLookup$RegistryLookup;"),&[context,registry_key]);
        assert!(ctx.rt.pending_exception.is_none());
        let result = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "net/minecraft/core/HolderGetter",
                "get",
                "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
            ),
            &[lookup, item_key],
        );
        assert!(
            matches!(&ctx.rt.get_obj(result).unwrap().data,Data::Optional(Some(value)) if *value==reference)
        );
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "net/minecraft/core/HolderGetter",
                "get",
                "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
            ),
            &[lookup, vanilla_key],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        assert_eq!(host.0.entries.len(), original);
        assert_eq!(host.0.entries["minecraft:item"].len(), count);
        assert!(host.0.frozen);
    }
    #[test]
    fn owner_only_prereference_does_not_claim_an_available_empty_registry() {
        let mut rt = Rt::new();
        let mut registries = RegistryStore::new();
        let linked = linked();
        let mut host = ItemHost(RegistryStore::new());
        // A cached owner/referenceState can exist for a negative intrusive-holders probe,
        // even when this registry has no builder/config or item entries.
        registries.reference_owner(&mut rt, "test:incomplete");
        assert!(registries.reference_states.contains_key("test:incomplete"));
        host.0 = registries;
        let context = rt.new_pinned_handle(CONTEXT, "cmd_build_ctx", 0, "");
        let key = crate::minecraft::new_resource_key(&mut rt, "minecraft:root", "test:incomplete");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let optional = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                PROVIDER,
                "lookup",
                "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
            ),
            &[context, key],
        );
        assert!(matches!(
            &ctx.rt.get_obj(optional).unwrap().data,
            Data::Optional(None)
        ));
    }
    #[test]
    fn directly_listed_value_without_canonical_holder_is_an_explicit_integrity_error() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut registries = RegistryStore::new();
        let value = rt.new_instance("test/Item", 0);
        registries.register_direct(
            &mut rt,
            "minecraft:item",
            "test:loose",
            "test",
            value,
            ContentProperties {
                kind: "item".into(),
                ..Default::default()
            },
        );
        registries
            .reference_states
            .get_mut("minecraft:item")
            .unwrap()
            .by_key
            .remove("test:loose");
        assert!(registries
            .canonical_reference("minecraft:item", "test:loose")
            .is_none());
        let context = rt.new_pinned_handle(CONTEXT, "cmd_build_ctx", 0, "");
        let registry =
            crate::minecraft::new_resource_key(&mut rt, "minecraft:root", "minecraft:item");
        let mut host = ItemHost(registries);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let lookup=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new(PROVIDER,"lookupOrThrow","(Lnet/minecraft/resources/ResourceKey;)Lnet/minecraft/core/HolderLookup$RegistryLookup;"),&[context,registry]);
        let item_key = crate::minecraft::new_resource_key(ctx.rt, "minecraft:item", "test:loose");
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "net/minecraft/core/HolderGetter",
                "get",
                "(Lnet/minecraft/resources/ResourceKey;)Ljava/util/Optional;",
            ),
            &[lookup, item_key],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("no canonical Holder.Reference"));
    }
    #[test]
    fn native_item_argument_keeps_readonly_lookup_nbt_delegate_and_provider_adapter() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = ItemHost(RegistryStore::new());
        let context = rt.new_pinned_handle(CONTEXT, "cmd_build_ctx", 0, "");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let argument=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(ARGUMENT,"item","(Lnet/minecraft/commands/CommandBuildContext;)Lnet/minecraft/commands/arguments/item/ItemArgument;"),&[context]);
        assert!(ctx.rt.pending_exception.is_none());
        let parser = ctx.rt.get_field(argument, "parser");
        let lookup = ctx.rt.get_field(parser, "items");
        let ops = ctx.rt.get_field(parser, "registryOps");
        assert_eq!(ctx.rt.class_of(lookup), Some(LOOKUP));
        assert_eq!(ctx.rt.class_of(ops), Some(REGISTRY_OPS));
        let nbt = read_static_field(&mut ctx, NBT_OPS, "INSTANCE", "Lnet/minecraft/nbt/NbtOps;");
        assert_eq!(ctx.rt.get_field(ops, "delegate"), nbt);
        let adapter = ctx.rt.get_field(ops, "lookupProvider");
        assert_eq!(ctx.rt.class_of(adapter), Some(ADAPTER));
        assert_eq!(ctx.rt.get_field(adapter, "lookupProvider"), context);
        assert_eq!(
            invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    REGISTRY_OPS,
                    "withParent",
                    "(Lcom/mojang/serialization/DynamicOps;)Lnet/minecraft/resources/RegistryOps;"
                ),
                &[ops, nbt]
            ),
            ops
        );
        let json = ctx.rt.new_instance("com/mojang/serialization/JsonOps", 0);
        let other = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                REGISTRY_OPS,
                "withParent",
                "(Lcom/mojang/serialization/DynamicOps;)Lnet/minecraft/resources/RegistryOps;",
            ),
            &[ops, json],
        );
        assert_ne!(other, ops);
        assert_eq!(ctx.rt.get_field(other, "delegate"), json);
        assert_eq!(ctx.rt.get_field(other, "lookupProvider"), adapter);
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "com/mojang/serialization/DynamicOps",
                "empty",
                "()Ljava/lang/Object;",
            ),
            &[nbt],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }
}

//! Registered vanilla block-entity type definitions used by Create display sources.
//!
//! Minecraft 1.21.1 `BlockEntityType` = `dqj`; verified factory method handles
//! are CommandBlockEntity (`dqr`), EnchantingTableBlockEntity (`drc`),
//! SignBlockEntity (`drs`) and LecternBlockEntity (`drk`). This
//! module retains those constructors but does not fabricate an entity instance:
//! invoking an unimplemented constructor fails at that exact method boundary.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{
    invoke_closure, ClosureData, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace, Rt, V,
};
const TYPE: &str = "net/minecraft/world/level/block/entity/BlockEntityType";
const SUPPLIER: &str = "net/minecraft/world/level/block/entity/BlockEntityType$BlockEntitySupplier";
const STATE: &str = "net/minecraft/world/level/block/state/BlockState";
const REGISTRY: &str = "minecraft:block_entity_type";
struct Definition {
    field: &'static str,
    id: &'static str,
    entity: &'static str,
    blocks: &'static [&'static str],
}
const DEFINITIONS: [Definition; 4] = [
    Definition {
        field: "COMMAND_BLOCK",
        id: "minecraft:command_block",
        entity: "net/minecraft/world/level/block/entity/CommandBlockEntity",
        blocks: &[
            "COMMAND_BLOCK",
            "CHAIN_COMMAND_BLOCK",
            "REPEATING_COMMAND_BLOCK",
        ],
    },
    Definition {
        field: "ENCHANTING_TABLE",
        id: "minecraft:enchanting_table",
        entity: "net/minecraft/world/level/block/entity/EnchantingTableBlockEntity",
        blocks: &["ENCHANTING_TABLE"],
    },
    Definition {
        field: "SIGN",
        id: "minecraft:sign",
        entity: "net/minecraft/world/level/block/entity/SignBlockEntity",
        blocks: &[
            "OAK_SIGN",
            "SPRUCE_SIGN",
            "BIRCH_SIGN",
            "ACACIA_SIGN",
            "CHERRY_SIGN",
            "JUNGLE_SIGN",
            "DARK_OAK_SIGN",
            "OAK_WALL_SIGN",
            "SPRUCE_WALL_SIGN",
            "BIRCH_WALL_SIGN",
            "ACACIA_WALL_SIGN",
            "CHERRY_WALL_SIGN",
            "JUNGLE_WALL_SIGN",
            "DARK_OAK_WALL_SIGN",
            "CRIMSON_SIGN",
            "CRIMSON_WALL_SIGN",
            "WARPED_SIGN",
            "WARPED_WALL_SIGN",
            "MANGROVE_SIGN",
            "MANGROVE_WALL_SIGN",
            "BAMBOO_SIGN",
            "BAMBOO_WALL_SIGN",
        ],
    },
    Definition {
        field: "LECTERN",
        id: "minecraft:lectern",
        entity: "net/minecraft/world/level/block/entity/LecternBlockEntity",
        blocks: &["LECTERN"],
    },
];
pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if owner != TYPE || desc != "Lnet/minecraft/world/level/block/entity/BlockEntityType;" {
        return None;
    }
    let definition = DEFINITIONS
        .iter()
        .find(|definition| definition.field == name)?;
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let value = rt.new_instance(TYPE, 0);
    let factory=rt.alloc(SUPPLIER,0,Data::Closure(Box::new(ClosureData{
        interface_class:SUPPLIER.into(),sam_name:"create".into(),sam_desc:"(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/entity/BlockEntity;".into(),
        instantiated_desc:format!("(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)L{};",definition.entity),captures:vec![],
        target:ClosureTarget::MethodHandle{handle_kind:8,member:MemberRef::new(definition.entity,"<init>","(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)V")},
    })));
    let id = rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        definition.id,
    );
    let field = rt.intern_str(definition.field);
    rt.put_field(value, "factory", factory);
    rt.put_field(value, "$definition", field);
    rt.put_field(value, "$id", id);
    registries.register_direct(
        rt,
        REGISTRY,
        definition.id,
        "minecraft",
        value,
        ContentProperties {
            kind: "block_entity_type".into(),
            template: definition.id.into(),
            ..ContentProperties::default()
        },
    );
    if rt.pending_exception.is_some() { return Some(V::NULL); }
    let holder = registries.canonical_reference(REGISTRY, definition.id)?;
    rt.put_field(value, "builtInRegistryHolder", holder);
    rt.put_static(owner, name, value);
    Some(value)
}
pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if !is_static
        && matches!(
            member.owner.as_str(),
            "net/minecraft/core/Registry"
                | "net/minecraft/core/DefaultedRegistry"
                | "net/minecraft/core/IdMap"
        )
    {
        if rt
            .handle_of(*args.first()?)
            .is_some_and(|(kind, _, tag, _)| kind == "registry" && tag == REGISTRY)
        {
            if matches!(
                name,
                "size"
                    | "stream"
                    | "iterator"
                    | "keySet"
                    | "entrySet"
                    | "holders"
                    | "getId"
                    | "byId"
                    | "byIdOrThrow"
            ) {
                return Some(rt.throw_exception("java/lang/UnsupportedOperationException","MMGA does not implement complete vanilla block entity registry enumeration or numeric IDs"));
            }
            if matches!(
                name,
                "get" | "getOptional" | "containsKey" | "getHolder" | "getHolderOrThrow"
            ) {
                let id = rt
                    .handle_of(*args.get(1)?)
                    .and_then(|(kind, _, tag, _)| match kind {
                        "resource_location" => Some(tag.to_owned()),
                        "resource_key" => tag.split_once('|').map(|(_, id)| id.to_owned()),
                        _ => None,
                    });
                if let Some(id) = id {
                    if let Some(definition) =
                        DEFINITIONS.iter().find(|definition| definition.id == id)
                    {
                        host.get_static_field(
                            rt,
                            TYPE,
                            definition.field,
                            "Lnet/minecraft/world/level/block/entity/BlockEntityType;",
                        );
                    } else if id.starts_with("minecraft:") {
                        return Some(rt.throw_exception(
                            "java/lang/UnsupportedOperationException",
                            &format!("MMGA does not implement vanilla block entity type {id}"),
                        ));
                    }
                }
            }
        }
    }
    if member.owner != TYPE {
        return None;
    }
    if is_static {
        if name!="getKey"||desc!="(Lnet/minecraft/world/level/block/entity/BlockEntityType;)Lnet/minecraft/resources/ResourceLocation;"{return None;}
        let value = *args.first()?;
        return Some(if value.is_null() {
            V::NULL
        } else if !rt.get_field(value, "$id").is_null() {
            rt.get_field(value, "$id")
        } else {
            rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "BlockEntityType.getKey for an unmodeled registry value",
            )
        });
    }
    let recv = *args.first()?;
    let definition_name = rt.str_of(rt.get_field(recv, "$definition"))?;
    let definition = DEFINITIONS
        .iter()
        .find(|definition| definition.field == definition_name)?;
    match(name,desc){
        ("builtInRegistryHolder","()Lnet/minecraft/core/Holder$Reference;")=>Some(rt.get_field(recv,"builtInRegistryHolder")),
        ("create","(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/entity/BlockEntity;")=>{
            let factory=rt.get_field(recv,"factory");let mut ctx=ExecCtx{rt,linked,host,depth:0};Some(invoke_closure(&mut ctx,factory,&[*args.get(1)?,*args.get(2)?]))
        }
        ("isValid","(Lnet/minecraft/world/level/block/state/BlockState;)Z")=>{
            let state=*args.get(1)?;
            if state.is_null(){return Some(rt.throw_exception("java/lang/NullPointerException","BlockEntityType.isValid(null)"));}
            let block=rt.get_field(state,"owner");
            if rt.class_of(state)!=Some(STATE)||block.is_null(){return Some(rt.throw_exception("java/lang/UnsupportedOperationException","BlockEntityType.isValid needs a modeled block state"));}
            // Compare canonical block object identity, never just a forged string ID.
            if definition.blocks.iter().any(|name|rt.get_static("net/minecraft/world/level/block/Blocks",name)==Some(block)){return Some(V::TRUE);}
            let known=rt.statics.iter().any(|(key,value)|key.starts_with("net/minecraft/world/level/block/Blocks.")&&*value==block);
            if !known{return Some(rt.throw_exception("java/lang/UnsupportedOperationException","BlockEntityType.isValid cannot identify this block"));}
            Some(V::FALSE)
        }
        _=>None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, read_static_field, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace,
    };
    #[derive(Default)]
    struct TypeHost(RegistryStore);
    impl HostApi for TypeHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, &mut self.0, owner, name, desc)
                .or_else(|| crate::blocks::get_static_field(rt, &mut self.0, owner, name, desc))
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            let result = super::call(rt, linked, self, member, is_static, args);
            result
                .or_else(|| crate::blocks::call(rt, &mut self.0, member, is_static, args))
                .or_else(|| {
                    crate::minecraft::call_keys_authlib_and_libs(
                        rt,
                        &mut self.0,
                        member,
                        is_static,
                        args,
                    )
                })
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(TYPE, name, desc),
            args,
        )
    }

    #[test]
    fn type_constants_preserve_registry_keys_and_exact_constructor_targets() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TypeHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, id, factory) in [
            (
                "COMMAND_BLOCK",
                "minecraft:command_block",
                "net/minecraft/world/level/block/entity/CommandBlockEntity",
            ),
            (
                "ENCHANTING_TABLE",
                "minecraft:enchanting_table",
                "net/minecraft/world/level/block/entity/EnchantingTableBlockEntity",
            ),
            (
                "SIGN",
                "minecraft:sign",
                "net/minecraft/world/level/block/entity/SignBlockEntity",
            ),
            (
                "LECTERN",
                "minecraft:lectern",
                "net/minecraft/world/level/block/entity/LecternBlockEntity",
            ),
        ] {
            let ty = read_static_field(
                &mut ctx,
                TYPE,
                name,
                "Lnet/minecraft/world/level/block/entity/BlockEntityType;",
            );
            assert!(ctx.rt.pending_exception.is_none());
            assert_eq!(
                ty,
                read_static_field(
                    &mut ctx,
                    TYPE,
                    name,
                    "Lnet/minecraft/world/level/block/entity/BlockEntityType;"
                )
            );
            let key=invoke_member(&mut ctx,op::INVOKESTATIC,&MemberRef::new(TYPE,"getKey","(Lnet/minecraft/world/level/block/entity/BlockEntityType;)Lnet/minecraft/resources/ResourceLocation;"),&[ty]);
            assert_eq!(ctx.rt.handle_of(key).unwrap().2, id);
            let holder = invoke(
                &mut ctx,
                "builtInRegistryHolder",
                "()Lnet/minecraft/core/Holder$Reference;",
                &[ty],
            );
            assert_eq!(ctx.rt.get_field(holder, "$value"), ty);
            let factory_value = ctx.rt.get_field(ty, "factory");
            let Some(Data::Closure(closure)) = ctx.rt.get_obj(factory_value).map(|o| &o.data)
            else {
                panic!("real factory closure")
            };
            assert_eq!(closure.interface_class, SUPPLIER);
            assert!(
                matches!(&closure.target,ClosureTarget::MethodHandle{handle_kind:8,member}if member.owner==factory&&member.name=="<init>"&&member.desc=="(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)V")
            );
        }
    }
    #[test]
    fn known_invalid_states_return_false_but_unknown_states_and_entity_factories_fail() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = TypeHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let ty = read_static_field(
            &mut ctx,
            TYPE,
            "COMMAND_BLOCK",
            "Lnet/minecraft/world/level/block/entity/BlockEntityType;",
        );
        let target = read_static_field(
            &mut ctx,
            "net/minecraft/world/level/block/Blocks",
            "TARGET",
            "Lnet/minecraft/world/level/block/Block;",
        );
        let state = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "net/minecraft/world/level/block/Block",
                "defaultBlockState",
                "()Lnet/minecraft/world/level/block/state/BlockState;",
            ),
            &[target],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(
            invoke(
                &mut ctx,
                "isValid",
                "(Lnet/minecraft/world/level/block/state/BlockState;)Z",
                &[ty, state]
            ),
            V::FALSE
        );
        let unknown = ctx.rt.new_instance(STATE, 0);
        invoke(
            &mut ctx,
            "isValid",
            "(Lnet/minecraft/world/level/block/state/BlockState;)Z",
            &[ty, unknown],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        let pos = ctx.rt.new_instance("net/minecraft/core/BlockPos", 0);
        invoke(&mut ctx,"create","(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/entity/BlockEntity;",&[ty,pos,state]);
        let error = ctx.rt.take_exception().unwrap();
        let text = ctx.rt.exception_summary(error);
        assert!(
            text.contains("UnsupportedOperationException")
                && text.contains("CommandBlockEntity.<init>")
        );
        read_static_field(
            &mut ctx,
            TYPE,
            "NOT_REAL",
            "Lnet/minecraft/world/level/block/entity/BlockEntityType;",
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

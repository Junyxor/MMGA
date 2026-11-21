//! NeoForge attachment type builders and serializer/copy callbacks.
//!
//! Based on official NeoForge 1.21.1 AttachmentType.java. Persistence and entity
//! attachment storage are not simulated here: serializer calls execute the mod's
//! original INBTSerializable methods with the supplied holder, provider and tag.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace, Rt, V};

const TYPE: &str = "net/neoforged/neoforge/attachment/AttachmentType";
const BUILDER: &str = "net/neoforged/neoforge/attachment/AttachmentType$Builder";
const SERIALIZER: &str = "net/neoforged/neoforge/attachment/IAttachmentSerializer";
const COPY: &str = "net/neoforged/neoforge/attachment/IAttachmentCopyHandler";
const NBT: &str = "net/neoforged/neoforge/common/util/INBTSerializable";
const SUPPLIER_ADAPTER: &str = "mmga/internal/AttachmentSupplier";
const SERIALIZER_ADAPTER: &str = "mmga/internal/SerializableAttachment";
const COPY_ADAPTER: &str = "mmga/internal/AttachmentDefaultCopy";
const READ:&str="(Lnet/neoforged/neoforge/attachment/IAttachmentHolder;Lnet/minecraft/nbt/Tag;Lnet/minecraft/core/HolderLookup$Provider;)Ljava/lang/Object;";
const WRITE: &str =
    "(Ljava/lang/Object;Lnet/minecraft/core/HolderLookup$Provider;)Lnet/minecraft/nbt/Tag;";
const COPY_DESC:&str="(Ljava/lang/Object;Lnet/neoforged/neoforge/attachment/IAttachmentHolder;Lnet/minecraft/core/HolderLookup$Provider;)Ljava/lang/Object;";

pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    if owner == TYPE
        && is_static
        && matches!(name, "builder" | "serializable")
        && matches!(desc,
        "(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;"|
        "(Ljava/util/function/Function;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;")
    {
        let factory = *args.first()?;
        let factory = if desc.contains("Supplier;") {
            let adapter = ctx.rt.new_instance(SUPPLIER_ADAPTER, 0);
            ctx.rt.put_field(adapter, "$supplier", factory);
            adapter
        } else {
            factory
        };
        let builder = ctx.rt.new_instance(BUILDER, 0);
        ctx.rt.put_field(builder, "defaultValueSupplier", factory);
        ctx.rt.put_field(builder, "copyOnDeath", V::FALSE);
        if name == "serializable" {
            let serializer = ctx.rt.new_instance(SERIALIZER_ADAPTER, 0);
            ctx.rt.put_field(serializer, "$factory", factory);
            ctx.rt.put_field(builder, "serializer", serializer);
        }
        return Some(builder);
    }
    if owner == BUILDER && !is_static {
        let recv = *args.first()?;
        return match(name,desc){
            ("serialize","(Lnet/neoforged/neoforge/attachment/IAttachmentSerializer;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;")=>{
                let serializer=*args.get(1)?;
                if serializer.is_null(){return Some(ctx.rt.throw_exception("java/lang/NullPointerException","serializer"));}
                if !ctx.rt.get_field(recv,"serializer").is_null(){return Some(ctx.rt.throw_exception("java/lang/IllegalStateException","Serializer already set"));}
                ctx.rt.put_field(recv,"serializer",serializer);Some(recv)
            }
            ("copyOnDeath","()Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;")=>{
                if ctx.rt.get_field(recv,"serializer").is_null(){return Some(ctx.rt.throw_exception("java/lang/IllegalStateException","copyOnDeath requires a serializer"));}
                ctx.rt.put_field(recv,"copyOnDeath",V::TRUE);Some(recv)
            }
            ("copyHandler","(Lnet/neoforged/neoforge/attachment/IAttachmentCopyHandler;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;")=>{
                let cloner=*args.get(1)?;
                if cloner.is_null(){return Some(ctx.rt.throw_exception("java/lang/NullPointerException","cloner"));}
                if ctx.rt.get_field(recv,"serializer").is_null(){return Some(ctx.rt.throw_exception("java/lang/IllegalStateException","copyHandler requires a serializer"));}
                ctx.rt.put_field(recv,"copyHandler",cloner);Some(recv)
            }
            ("sync","(Lnet/neoforged/neoforge/attachment/AttachmentSyncHandler;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;")=>{
                let handler=*args.get(1)?;
                if handler.is_null(){return Some(ctx.rt.throw_exception("java/lang/NullPointerException","syncHandler"));}
                ctx.rt.put_field(recv,"syncHandler",handler);Some(recv)
            }
            ("build","()Lnet/neoforged/neoforge/attachment/AttachmentType;")=>{
                let ty=ctx.rt.new_instance(TYPE,0);
                for field in ["defaultValueSupplier","serializer","copyOnDeath","copyHandler","syncHandler"]{
                    ctx.rt.put_field(ty,field,ctx.rt.get_field(recv,field));
                }
                if ctx.rt.get_field(ty,"copyHandler").is_null(){
                    let handler=ctx.rt.new_instance(COPY_ADAPTER,0);ctx.rt.put_field(handler,"$serializer",ctx.rt.get_field(ty,"serializer"));ctx.rt.put_field(ty,"copyHandler",handler);
                }
                Some(ty)
            }
            _=>None,
        };
    }
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    if ctx.rt.class_of(recv) == Some(SUPPLIER_ADAPTER)
        && matches!(owner, SUPPLIER_ADAPTER | "java/util/function/Function")
        && name == "apply"
        && desc == "(Ljava/lang/Object;)Ljava/lang/Object;"
    {
        let supplier = ctx.rt.get_field(recv, "$supplier");
        return Some(invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/function/Supplier", "get", "()Ljava/lang/Object;"),
            &[supplier],
        ));
    }
    if ctx.rt.class_of(recv) == Some(SERIALIZER_ADAPTER)
        && matches!(owner, SERIALIZER | SERIALIZER_ADAPTER)
    {
        return match (name, desc) {
            ("read", READ) => {
                let factory = ctx.rt.get_field(recv, "$factory");
                let value = invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Function",
                        "apply",
                        "(Ljava/lang/Object;)Ljava/lang/Object;",
                    ),
                    &[factory, *args.get(1)?],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                if !value.is_null()
                    && !ctx
                        .linked
                        .is_subtype(ctx.rt.class_of(value).unwrap_or(""), NBT, ctx.host)
                {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "serializable factory did not return INBTSerializable",
                    ));
                }
                invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        NBT,
                        "deserializeNBT",
                        "(Lnet/minecraft/core/HolderLookup$Provider;Lnet/minecraft/nbt/Tag;)V",
                    ),
                    &[value, *args.get(3)?, *args.get(2)?],
                );
                Some(if ctx.rt.pending_exception.is_some() {
                    V::NULL
                } else {
                    value
                })
            }
            ("write", WRITE) => {
                let value = *args.get(1)?;
                if !value.is_null()
                    && !ctx
                        .linked
                        .is_subtype(ctx.rt.class_of(value).unwrap_or(""), NBT, ctx.host)
                {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "attachment does not implement INBTSerializable",
                    ));
                }
                Some(invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        NBT,
                        "serializeNBT",
                        "(Lnet/minecraft/core/HolderLookup$Provider;)Lnet/minecraft/nbt/Tag;",
                    ),
                    &[value, *args.get(2)?],
                ))
            }
            _ => None,
        };
    }
    if ctx.rt.class_of(recv) == Some(COPY_ADAPTER)
        && matches!(owner, COPY | COPY_ADAPTER)
        && name == "copy"
        && desc == COPY_DESC
    {
        let serializer = ctx.rt.get_field(recv, "$serializer");
        if serializer.is_null() {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Cannot copy non-serializable attachments",
            ));
        }
        let tag = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(SERIALIZER, "write", WRITE),
            &[serializer, *args.get(1)?, *args.get(3)?],
        );
        if ctx.rt.pending_exception.is_some() || tag.is_null() {
            return Some(V::NULL);
        }
        return Some(invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(SERIALIZER, "read", READ),
            &[serializer, *args.get(2)?, tag, *args.get(3)?],
        ));
    }
    None
}
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (sub_class == SUPPLIER_ADAPTER && super_class == "java/util/function/Function")
        || (sub_class == SERIALIZER_ADAPTER && super_class == SERIALIZER)
        || (sub_class == COPY_ADAPTER && super_class == COPY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, NativeClassDef, NativeLambdaFn,
        NativeMethodFn, NativeModuleRegistry,
    };
    struct Host;
    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            _rt: &mut Rt,
            _owner: &str,
            _name: &str,
            _desc: &str,
        ) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, linked, self, member, is_static, args)
        }
        fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
            super::is_subtype(sub_class, super_class)
        }
    }
    fn make(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let state = args[0];
        let count = ctx.rt.get_field(state, "calls").i();
        ctx.rt.put_field(state, "calls", V::I(count + 1));
        let value = ctx.rt.new_instance("example/Attachment", 0);
        ctx.rt
            .put_field(value, "holder", args.get(1).copied().unwrap_or(V::NULL));
        value
    }
    fn deserialize(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(args[0], "provider", args[1]);
        ctx.rt.put_field(args[0], "tag", args[2]);
        V::Void
    }
    fn serialize(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(args[0], "provider", args[1]);
        ctx.rt.get_field(args[0], "tag")
    }
    static METHODS: &[(&str, NativeMethodFn, bool, bool)] = &[
        (
            "deserializeNBT.(Lnet/minecraft/core/HolderLookup$Provider;Lnet/minecraft/nbt/Tag;)V",
            deserialize,
            false,
            false,
        ),
        (
            "serializeNBT.(Lnet/minecraft/core/HolderLookup$Provider;)Lnet/minecraft/nbt/Tag;",
            serialize,
            false,
            false,
        ),
    ];
    static CLASSES: &[NativeClassDef] = &[NativeClassDef {
        name: "example/Attachment",
        superclass: Some("java/lang/Object"),
        interfaces: &[NBT],
        is_interface: false,
        methods: METHODS,
    }];
    static LAMBDAS: &[(u32, NativeLambdaFn)] = &[(1, make)];
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: CLASSES,
            lambdas: LAMBDAS,
        });
        linked
    }
    fn factory(rt: &mut Rt, state: V, function: bool) -> V {
        let (class, name, desc) = if function {
            (
                "java/util/function/Function",
                "apply",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
            )
        } else {
            ("java/util/function/Supplier", "get", "()Ljava/lang/Object;")
        };
        rt.alloc(
            class,
            0,
            Data::Closure(Box::new(ClosureData {
                interface_class: class.into(),
                sam_name: name.into(),
                sam_desc: desc.into(),
                instantiated_desc: desc.into(),
                captures: vec![state],
                target: ClosureTarget::NativeLambda(1),
            })),
        )
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        is_static: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if is_static {
                op::INVOKESTATIC
            } else {
                op::INVOKEINTERFACE
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn build(ctx: &mut ExecCtx<'_>, builder: V) -> V {
        invoke(
            ctx,
            BUILDER,
            "build",
            "()Lnet/neoforged/neoforge/attachment/AttachmentType;",
            false,
            &[builder],
        )
    }

    #[test]
    fn serializable_supplier_is_lazy_and_build_snapshots_copy_flags() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let state = rt.new_instance("example/State", 0);
        let supplier = factory(&mut rt, state, false);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let builder=invoke(&mut ctx,TYPE,"serializable","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;",true,&[supplier]);
        let first = build(&mut ctx, builder);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(state, "calls").i(), 0);
        assert_eq!(ctx.rt.get_field(first, "copyOnDeath"), V::FALSE);
        assert_eq!(
            invoke(
                &mut ctx,
                BUILDER,
                "copyOnDeath",
                "()Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;",
                false,
                &[builder]
            ),
            builder
        );
        let second = build(&mut ctx, builder);
        assert_eq!(ctx.rt.get_field(first, "copyOnDeath"), V::FALSE);
        assert_eq!(ctx.rt.get_field(second, "copyOnDeath"), V::TRUE);
        let function = ctx.rt.get_field(first, "defaultValueSupplier");
        let value = invoke(
            &mut ctx,
            "java/util/function/Function",
            "apply",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            false,
            &[function, V::NULL],
        );
        assert_eq!(ctx.rt.class_of(value), Some("example/Attachment"));
        assert_eq!(ctx.rt.get_field(state, "calls"), V::I(1));
    }
    #[test]
    fn nbt_serializer_and_default_copy_execute_original_callbacks() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let state = rt.new_instance("example/State", 0);
        let function = factory(&mut rt, state, true);
        let holder = rt.new_instance("example/Holder", 0);
        let provider = rt.new_instance("example/Lookup", 0);
        let tag = rt.new_instance("net/minecraft/nbt/CompoundTag", 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let builder=invoke(&mut ctx,TYPE,"serializable","(Ljava/util/function/Function;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;",true,&[function]);
        let ty = build(&mut ctx, builder);
        let serializer = ctx.rt.get_field(ty, "serializer");
        let value = invoke(
            &mut ctx,
            SERIALIZER,
            "read",
            READ,
            false,
            &[serializer, holder, tag, provider],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(ctx.rt.get_field(value, "holder"), holder);
        assert_eq!(ctx.rt.get_field(value, "provider"), provider);
        assert_eq!(ctx.rt.get_field(value, "tag"), tag);
        assert_eq!(
            invoke(
                &mut ctx,
                SERIALIZER,
                "write",
                WRITE,
                false,
                &[serializer, value, provider]
            ),
            tag
        );
        let target = ctx.rt.new_instance("example/Holder", 0);
        let copier = ctx.rt.get_field(ty, "copyHandler");
        let copy = invoke(
            &mut ctx,
            COPY,
            "copy",
            COPY_DESC,
            false,
            &[copier, value, target, provider],
        );
        assert_ne!(copy, value);
        assert_eq!(ctx.rt.get_field(copy, "holder"), target);
        assert_eq!(ctx.rt.get_field(copy, "tag"), tag);
        assert_eq!(ctx.rt.get_field(state, "calls"), V::I(2));
        ctx.rt.put_field(value, "tag", V::NULL);
        assert_eq!(
            invoke(
                &mut ctx,
                COPY,
                "copy",
                COPY_DESC,
                false,
                &[copier, value, target, provider]
            ),
            V::NULL
        );
        assert_eq!(ctx.rt.get_field(state, "calls"), V::I(2));
    }
    #[test]
    fn nonserializable_copy_and_duplicate_serializers_throw() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = Host;
        let state = rt.new_instance("example/State", 0);
        let supplier = factory(&mut rt, state, false);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let builder=invoke(&mut ctx,TYPE,"builder","(Ljava/util/function/Supplier;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;",true,&[supplier]);
        invoke(
            &mut ctx,
            BUILDER,
            "copyOnDeath",
            "()Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;",
            false,
            &[builder],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalStateException")
        );
        let ty = build(&mut ctx, builder);
        let copier = ctx.rt.get_field(ty, "copyHandler");
        invoke(
            &mut ctx,
            COPY,
            "copy",
            COPY_DESC,
            false,
            &[copier, V::NULL, V::NULL, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
        let serializer = ctx.rt.new_instance("example/Serializer", 0);
        let desc="(Lnet/neoforged/neoforge/attachment/IAttachmentSerializer;)Lnet/neoforged/neoforge/attachment/AttachmentType$Builder;";
        assert_eq!(
            invoke(
                &mut ctx,
                BUILDER,
                "serialize",
                desc,
                false,
                &[builder, serializer]
            ),
            builder
        );
        invoke(
            &mut ctx,
            BUILDER,
            "serialize",
            desc,
            false,
            &[builder, serializer],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalStateException")
        );
    }
}

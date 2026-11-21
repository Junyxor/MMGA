//! NeoForge enum StreamCodec delegates FriendlyByteBuf's VarInt enum encoding.
//!
//! Checked against official NeoForge 1.21.1 source blob
//! b752d7552c07fbcfac7d9e94f5c1c7e070e06fee and Minecraft `vw.readEnum/writeEnum`.
//! Construction retains the class without initializing it; decoding obtains its
//! constants before reading the ordinal, and invalid ordinals throw.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace, Rt, V,
};

const API: &str = "net/neoforged/neoforge/network/codec/NeoForgeStreamCodecs";
const CALLBACK: &str = "mmga/internal/EnumStreamCodec";
const STREAM: &str = "net/minecraft/network/codec/StreamCodec";

pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static
        && member.owner == API
        && member.name == "enumCodec"
        && member.desc == "(Ljava/lang/Class;)Lnet/minecraft/network/codec/StreamCodec;"
    {
        let class = *args.first()?;
        let encoder = callback(
            rt,
            "encode",
            "(Ljava/lang/Object;Ljava/lang/Object;)V",
            vec![],
        );
        let decoder = callback(
            rt,
            "decode",
            "(Ljava/lang/Class;Ljava/lang/Object;)Ljava/lang/Object;",
            vec![class],
        );
        return crate::codecs::call(rt,linked,host,&MemberRef::new(STREAM,"of","(Lnet/minecraft/network/codec/StreamEncoder;Lnet/minecraft/network/codec/StreamDecoder;)Lnet/minecraft/network/codec/StreamCodec;"),true,&[encoder,decoder]);
    }
    if !is_static || member.owner != CALLBACK {
        return None;
    }
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    match (member.name.as_str(), member.desc.as_str()) {
        ("encode", "(Ljava/lang/Object;Ljava/lang/Object;)V") => {
            let buffer = args[0];
            let value = args[1];
            if !value.is_null()
                && !ctx
                    .rt
                    .class_of(value)
                    .is_some_and(|class| ctx.linked.is_subtype(class, "java/lang/Enum", ctx.host))
            {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/ClassCastException", "enumCodec requires Enum"),
                );
            }
            let ordinal = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Enum", "ordinal", "()I"),
                &[value],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let primitive = crate::codecs::get_static_field(
                ctx.rt,
                "net/minecraft/network/codec/ByteBufCodecs",
                "VAR_INT",
                "Lnet/minecraft/network/codec/StreamCodec;",
            )
            .unwrap();
            let ordinal = mmga_runtime::box_primitive(ctx.rt, "java/lang/Integer", ordinal);
            Some(invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
                &[primitive, buffer, ordinal],
            ))
        }
        ("decode", "(Ljava/lang/Class;Ljava/lang/Object;)Ljava/lang/Object;") => {
            let class = args[0];
            let buffer = args[1];
            if class.is_null() {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/NullPointerException", "enumClass is null"),
                );
            }
            let Some(("class", _, name, _)) = ctx.rt.handle_of(class) else {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/ClassCastException", "enumClass is not Class"),
                );
            };
            let name = name.to_owned();
            let constants = enum_constants(&mut ctx, &name);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            // Java evaluates getEnumConstants before readVarInt, but indexes after it.
            let primitive = crate::codecs::get_static_field(
                ctx.rt,
                "net/minecraft/network/codec/ByteBufCodecs",
                "VAR_INT",
                "Lnet/minecraft/network/codec/StreamCodec;",
            )
            .unwrap();
            let ordinal = invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
                &[primitive, buffer],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let ordinal = ctx.rt.get_field(ordinal, "$value").i();
            let Some(constants) = constants else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "Class.getEnumConstants returned null",
                ));
            };
            Some(constants.get(ordinal as usize).copied().unwrap_or_else(|| {
                ctx.rt.throw_exception(
                    "java/lang/ArrayIndexOutOfBoundsException",
                    &format!(
                        "Index {ordinal} out of bounds for enum length {}",
                        constants.len()
                    ),
                )
            }))
        }
        _ => None,
    }
}

fn callback(rt: &mut Rt, name: &str, desc: &str, captures: Vec<V>) -> V {
    let (interface, sam_desc) = if name == "encode" {
        (
            "net/minecraft/network/codec/StreamEncoder",
            "(Ljava/lang/Object;Ljava/lang/Object;)V",
        )
    } else {
        (
            "net/minecraft/network/codec/StreamDecoder",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
        )
    };
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: name.into(),
            sam_desc: sam_desc.into(),
            instantiated_desc: sam_desc.into(),
            captures,
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(CALLBACK, name, desc),
            },
        })),
    )
}

fn enum_constants(ctx: &mut ExecCtx<'_>, class: &str) -> Option<Vec<V>> {
    let key = "$enum_constants";
    if let Some(value) = ctx.rt.get_static(class, key) {
        if let Some(Data::Array(values)) = ctx.rt.get_obj(value).map(|o| &o.data) {
            return Some(values.clone());
        }
    }
    let is_enum = ctx
        .linked
        .bytecode_classes
        .get(class)
        .map(|cf| cf.access & 0x4000 != 0)
        .unwrap_or_else(|| ctx.linked.is_subtype(class, "java/lang/Enum", ctx.host));
    if !is_enum {
        return None;
    }
    let descriptor = format!("()[L{class};");
    let values = invoke_member(
        ctx,
        op::INVOKESTATIC,
        &MemberRef::new(class, "values", &descriptor),
        &[],
    );
    if ctx.rt.pending_exception.is_some() {
        return None;
    }
    let Some(Data::Array(values)) = ctx.rt.get_obj(values).map(|o| &o.data) else {
        ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "Enum values returned non-array",
        );
        return None;
    };
    let values = values.clone();
    let cached = ctx
        .rt
        .alloc(format!("[L{class};"), 0, Data::Array(values.clone()));
    ctx.rt.put_static(class, key, cached);
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{HostApi, NativeClassDef, NativeModuleRegistry};
    struct EnumHost;
    impl HostApi for EnumHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            crate::codecs::get_static_field(rt, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            if let Some(v) = call(rt, linked, self, member, is_static, args) {
                return Some(v);
            }
            crate::codecs::call(rt, linked, self, member, is_static, args)
        }
    }
    fn constants(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        let first = ctx.rt.get_static("test/SortBy", "FIRST").unwrap();
        let second = ctx.rt.get_static("test/SortBy", "SECOND").unwrap();
        ctx.rt
            .alloc("[Ltest/SortBy;", 0, Data::Array(vec![first, second]))
    }
    static CLASSES: &[NativeClassDef] = &[NativeClassDef {
        name: "test/SortBy",
        superclass: Some("java/lang/Enum"),
        interfaces: &[],
        is_interface: false,
        methods: &[("values.()[Ltest/SortBy;", constants, true, false)],
    }];
    #[test]
    fn enum_codec_uses_real_constants_and_varint_ordinals_and_rejects_invalid_ids() {
        let mut rt = Rt::new();
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: CLASSES,
            lambdas: &[],
        });
        let mut host = EnumHost;
        let mut values = Vec::new();
        for (i, name) in ["FIRST", "SECOND"].into_iter().enumerate() {
            let value = rt.new_instance("test/SortBy", 0);
            let name_v = rt.intern_str(name);
            rt.put_field(value, "$name", name_v);
            rt.put_field(value, "$ordinal", V::I(i as i32));
            rt.put_static("test/SortBy", name, value);
            values.push(value);
        }
        let class = rt.new_pinned_handle("java/lang/Class", "class", 0, "test/SortBy");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                API,
                "enumCodec",
                "(Ljava/lang/Class;)Lnet/minecraft/network/codec/StreamCodec;",
            ),
            &[class],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(!ctx.rt.initialized_classes.contains("test/SortBy"));
        let buf = ctx.rt.alloc(
            "net/minecraft/network/FriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![],
                reader_idx: 0,
            },
        );
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
            &[codec, buf, values[1]],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(matches!(&ctx.rt.get_obj(buf).unwrap().data,Data::ByteBuf{bytes,..}if bytes==&[1]));
        let decoded = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, buf],
        );
        assert_eq!(decoded, values[1]);
        assert!(ctx.rt.pending_exception.is_none());
        let invalid = ctx.rt.alloc(
            "net/minecraft/network/FriendlyByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![2],
                reader_idx: 0,
            },
        );
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
            &[codec, invalid],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("ArrayIndexOutOfBoundsException"));
    }

    #[test]
    fn null_and_non_enum_class_are_not_replaced_with_a_default_constant() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = EnumHost;
        let class = rt.new_pinned_handle("java/lang/Class", "class", 0, "java/lang/String");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for class in [V::NULL, class] {
            let codec = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    API,
                    "enumCodec",
                    "(Ljava/lang/Class;)Lnet/minecraft/network/codec/StreamCodec;",
                ),
                &[class],
            );
            assert!(ctx.rt.pending_exception.is_none());
            let buffer = ctx.rt.alloc(
                "net/minecraft/network/FriendlyByteBuf",
                0,
                Data::ByteBuf {
                    bytes: vec![0],
                    reader_idx: 0,
                },
            );
            invoke_member(
                &mut ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
                &[codec, buffer],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert!(ctx
                .rt
                .exception_summary(error)
                .contains("NullPointerException"));
            let Data::ByteBuf { reader_idx, .. } = &ctx.rt.get_obj(buffer).unwrap().data else {
                unreachable!()
            };
            assert_eq!(*reader_idx, if class.is_null() { 0 } else { 1 });
        }
    }
}

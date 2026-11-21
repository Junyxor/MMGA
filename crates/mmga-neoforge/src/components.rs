//! Data component type builder/value semantics from Minecraft 1.21.1.
//!
//! Checked against Mojang client classes `kp`, `kp$a`, `kp$a$a`, and `axu$2`.
//! Persistent codecs are retained as actual objects, not executed as fake DFU.

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, ExecCtx, HostApi, LinkedSpace, Rt, V};

const TYPE: &str = "net/minecraft/core/component/DataComponentType";
const BUILDER: &str = "net/minecraft/core/component/DataComponentType$Builder";
const SIMPLE: &str = "net/minecraft/core/component/DataComponentType$Builder$SimpleType";
const CACHED_CODEC: &str = "net/minecraft/util/EncoderCache$2";

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    (sub_class == SIMPLE && super_class == TYPE)
        || (sub_class == CACHED_CODEC
            && matches!(
                super_class,
                "com/mojang/serialization/Codec"
                    | "com/mojang/serialization/Encoder"
                    | "com/mojang/serialization/Decoder"
            ))
}

pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if owner == TYPE
        && is_static
        && name == "builder"
        && desc == "()Lnet/minecraft/core/component/DataComponentType$Builder;"
    {
        let builder = rt.new_instance(BUILDER, 0);
        init_builder(rt, builder);
        return Some(builder);
    }
    if owner == BUILDER && !is_static {
        let recv = *args.first()?;
        return match (name, desc) {
            ("<init>", "()V") => {
                init_builder(rt, recv);
                Some(V::Void)
            }
            ("persistent", "(Lcom/mojang/serialization/Codec;)Lnet/minecraft/core/component/DataComponentType$Builder;") => {
                rt.put_field(recv, "codec", *args.get(1)?);
                Some(recv)
            }
            ("networkSynchronized", "(Lnet/minecraft/network/codec/StreamCodec;)Lnet/minecraft/core/component/DataComponentType$Builder;") => {
                rt.put_field(recv, "streamCodec", *args.get(1)?);
                Some(recv)
            }
            ("cacheEncoding", "()Lnet/minecraft/core/component/DataComponentType$Builder;") => {
                rt.put_field(recv, "cacheEncoding", V::TRUE);
                Some(recv)
            }
            ("build", "()Lnet/minecraft/core/component/DataComponentType;") => {
                let original_codec = rt.get_field(recv, "codec");
                let mut stream = rt.get_field(recv, "streamCodec");
                if stream.is_null() {
                    if original_codec.is_null() {
                        return Some(rt.throw_exception("java/lang/NullPointerException", "Missing Codec for component"));
                    }
                    // Vanilla chooses the network fallback before wrapping the
                    // persistent codec in its encoding cache.
                    stream = crate::codecs::call(
                        rt, linked, host,
                        &MemberRef::new(
                            "net/minecraft/network/codec/ByteBufCodecs",
                            "fromCodecWithRegistries",
                            "(Lcom/mojang/serialization/Codec;)Lnet/minecraft/network/codec/StreamCodec;",
                        ),
                        true, &[original_codec],
                    )?;
                    if rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                let codec = if rt.get_field(recv, "cacheEncoding") == V::TRUE && !original_codec.is_null() {
                    let wrapped = rt.new_instance(CACHED_CODEC, 0);
                    rt.put_field(wrapped, "$delegate_codec", original_codec);
                    wrapped
                } else {
                    original_codec
                };
                let component = rt.new_instance(SIMPLE, 0);
                rt.put_field(component, "codec", codec);
                rt.put_field(component, "streamCodec", stream);
                Some(component)
            }
            _ => None,
        };
    }
    if !is_static && matches!(owner, TYPE | SIMPLE) {
        let recv = *args.first()?;
        return match (name, desc) {
            ("codec", "()Lcom/mojang/serialization/Codec;") => Some(rt.get_field(recv, "codec")),
            ("streamCodec", "()Lnet/minecraft/network/codec/StreamCodec;") => {
                Some(rt.get_field(recv, "streamCodec"))
            }
            ("isTransient", "()Z") => Some(V::bool(rt.get_field(recv, "codec").is_null())),
            ("codecOrThrow", "()Lcom/mojang/serialization/Codec;") => {
                let codec = rt.get_field(recv, "codec");
                Some(if codec.is_null() {
                    let label = rt.value_to_string(recv);
                    rt.throw_exception(
                        "java/lang/IllegalStateException",
                        &format!("{label} is not a persistent component"),
                    )
                } else {
                    codec
                })
            }
            (
                "<init>",
                "(Lcom/mojang/serialization/Codec;Lnet/minecraft/network/codec/StreamCodec;)V",
            ) if owner == SIMPLE => {
                rt.put_field(recv, "codec", *args.get(1)?);
                rt.put_field(recv, "streamCodec", *args.get(2)?);
                Some(V::Void)
            }
            _ => None,
        };
    }
    if !is_static
        && matches!(
            owner,
            CACHED_CODEC
                | "com/mojang/serialization/Codec"
                | "com/mojang/serialization/Encoder"
                | "com/mojang/serialization/Decoder"
        )
    {
        let recv = *args.first()?;
        if rt.class_of(recv) != Some(CACHED_CODEC) {
            return None;
        }
        return match (name, desc) {
            ("decode", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;") => {
                let delegate = rt.get_field(recv, "$delegate_codec");
                let mut ctx = ExecCtx { rt, linked, host, depth: 0 };
                Some(invoke_member(
                    &mut ctx, op::INVOKEINTERFACE,
                    &MemberRef::new("com/mojang/serialization/Codec", "decode", desc),
                    &[delegate, *args.get(1)?, *args.get(2)?],
                ))
            }
            ("encode", "(Ljava/lang/Object;Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;")
            | ("encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;") => {
                Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "MMGA does not implement EncoderCache persistent encoding (DFU result caching and NBT copy)",
                ))
            }
            _ => None,
        };
    }
    None
}

fn init_builder(rt: &mut Rt, builder: V) {
    rt.put_field(builder, "codec", V::NULL);
    rt.put_field(builder, "streamCodec", V::NULL);
    rt.put_field(builder, "cacheEncoding", V::FALSE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{Data, NullHost};

    fn invoke(rt: &mut Rt, owner: &str, name: &str, desc: &str, is_static: bool, args: &[V]) -> V {
        call(
            rt,
            &LinkedSpace::new(),
            &mut NullHost,
            &MemberRef::new(owner, name, desc),
            is_static,
            args,
        )
        .expect("implemented component API")
    }

    fn builder(rt: &mut Rt) -> V {
        invoke(
            rt,
            TYPE,
            "builder",
            "()Lnet/minecraft/core/component/DataComponentType$Builder;",
            true,
            &[],
        )
    }

    fn build(rt: &mut Rt, builder: V) -> V {
        invoke(
            rt,
            BUILDER,
            "build",
            "()Lnet/minecraft/core/component/DataComponentType;",
            false,
            &[builder],
        )
    }

    fn persistent(rt: &mut Rt, builder: V, codec: V) {
        assert_eq!(invoke(rt, BUILDER, "persistent", "(Lcom/mojang/serialization/Codec;)Lnet/minecraft/core/component/DataComponentType$Builder;", false, &[builder, codec]), builder);
    }

    fn network(rt: &mut Rt, builder: V, codec: V) {
        assert_eq!(invoke(rt, BUILDER, "networkSynchronized", "(Lnet/minecraft/network/codec/StreamCodec;)Lnet/minecraft/core/component/DataComponentType$Builder;", false, &[builder, codec]), builder);
    }

    #[test]
    fn empty_builder_throws_the_official_missing_codec_exception() {
        let mut rt = Rt::new();
        let b = builder(&mut rt);
        build(&mut rt, b);
        let error = rt.take_exception().expect("missing codec must fail");
        assert_eq!(rt.class_of(error), Some("java/lang/NullPointerException"));
        assert!(rt
            .exception_summary(error)
            .contains("Missing Codec for component"));
    }

    #[test]
    fn network_only_type_is_transient_and_codec_or_throw_rejects_it() {
        let mut rt = Rt::new();
        let b = builder(&mut rt);
        let stream = crate::codecs::get_static_field(
            &mut rt,
            "net/minecraft/network/codec/ByteBufCodecs",
            "INT",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        network(&mut rt, b, stream);
        let ty = build(&mut rt, b);
        assert_eq!(
            invoke(&mut rt, TYPE, "isTransient", "()Z", false, &[ty]),
            V::TRUE
        );
        assert_eq!(
            invoke(
                &mut rt,
                TYPE,
                "codec",
                "()Lcom/mojang/serialization/Codec;",
                false,
                &[ty]
            ),
            V::NULL
        );
        assert_eq!(
            invoke(
                &mut rt,
                TYPE,
                "streamCodec",
                "()Lnet/minecraft/network/codec/StreamCodec;",
                false,
                &[ty]
            ),
            stream
        );
        invoke(
            &mut rt,
            TYPE,
            "codecOrThrow",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[ty],
        );
        let error = rt
            .take_exception()
            .expect("transient has no persistent codec");
        assert_eq!(rt.class_of(error), Some("java/lang/IllegalStateException"));
    }

    #[test]
    fn build_snapshots_codecs_and_derives_network_from_original_persistent_codec() {
        let mut rt = Rt::new();
        let b = rt.new_instance(BUILDER, 0);
        invoke(&mut rt, BUILDER, "<init>", "()V", false, &[b]);
        let codec = rt.new_instance("test/PersistentCodec", 0);
        persistent(&mut rt, b, codec);
        let ty = build(&mut rt, b);
        let stream = invoke(
            &mut rt,
            TYPE,
            "streamCodec",
            "()Lnet/minecraft/network/codec/StreamCodec;",
            false,
            &[ty],
        );
        assert_eq!(rt.str_of(rt.get_field(stream, "$codec_kind")), Some("net/minecraft/network/codec/ByteBufCodecs.fromCodecWithRegistries(Lcom/mojang/serialization/Codec;)Lnet/minecraft/network/codec/StreamCodec;"));
        assert!(
            matches!(rt.get_obj(rt.get_field(stream, "$codec_args")).map(|o| &o.data), Some(Data::Array(args)) if args == &[codec])
        );
        persistent(&mut rt, b, V::NULL);
        assert_eq!(
            invoke(
                &mut rt,
                TYPE,
                "codec",
                "()Lcom/mojang/serialization/Codec;",
                false,
                &[ty]
            ),
            codec
        );
        assert_eq!(
            invoke(&mut rt, TYPE, "isTransient", "()Z", false, &[ty]),
            V::FALSE
        );
        assert_eq!(rt.class_of(ty), Some(SIMPLE));
        assert!(is_subtype(SIMPLE, TYPE));
    }

    #[test]
    fn cache_encoding_retains_delegate_and_never_fabricates_encoded_data() {
        let mut rt = Rt::new();
        let b = builder(&mut rt);
        let original = rt.new_instance("test/PersistentCodec", 0);
        persistent(&mut rt, b, original);
        assert_eq!(
            invoke(
                &mut rt,
                BUILDER,
                "cacheEncoding",
                "()Lnet/minecraft/core/component/DataComponentType$Builder;",
                false,
                &[b]
            ),
            b
        );
        let ty = build(&mut rt, b);
        let wrapped = invoke(
            &mut rt,
            TYPE,
            "codec",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[ty],
        );
        assert_ne!(wrapped, original);
        assert_eq!(rt.class_of(wrapped), Some(CACHED_CODEC));
        assert_eq!(rt.get_field(wrapped, "$delegate_codec"), original);
        invoke(&mut rt, "com/mojang/serialization/Codec", "encode", "(Ljava/lang/Object;Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[wrapped, V::NULL, V::NULL, V::NULL]);
        let error = rt
            .take_exception()
            .expect("cached encoding is not implemented");
        assert_eq!(
            rt.class_of(error),
            Some("java/lang/UnsupportedOperationException")
        );
    }

    #[test]
    fn unrelated_signatures_remain_unimplemented() {
        let mut rt = Rt::new();
        assert_eq!(
            call(
                &mut rt,
                &LinkedSpace::new(),
                &mut NullHost,
                &MemberRef::new(BUILDER, "build", "()V"),
                false,
                &[V::NULL]
            ),
            None
        );
        assert_eq!(
            call(
                &mut rt,
                &LinkedSpace::new(),
                &mut NullHost,
                &MemberRef::new(TYPE, "getValue", "()Ljava/lang/Object;"),
                false,
                &[V::NULL]
            ),
            None
        );
    }
}

use super::*;
use crate::PreparedLoad;
use mmga_runtime::{
    box_primitive, unbox_primitive, ClosureData, ClosureTarget, NativeLambdaFn,
    NativeModuleRegistry,
};

const CHANNEL_DESC: &str = "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload$Type;Lnet/minecraft/network/codec/StreamCodec;Lnet/neoforged/neoforge/network/handling/IPayloadHandler;)Lnet/neoforged/neoforge/network/registration/PayloadRegistrar;";
const HANDLE_DESC: &str = "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;Lnet/neoforged/neoforge/network/handling/IPayloadContext;)V";

fn invoke(
    prepared: &mut PreparedLoad,
    rt: &mut Rt,
    owner: &str,
    name: &str,
    desc: &str,
    args: &[V],
) -> V {
    invoke_member(
        &mut ExecCtx {
            rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        },
        op::INVOKEVIRTUAL,
        &MemberRef::new(owner, name, desc),
        args,
    )
}

fn registrar(prepared: &mut PreparedLoad, rt: &mut Rt, version: &str) -> V {
    let event = rt.new_instance(EVENT, 0);
    let version = rt.new_str(version);
    invoke(
        prepared,
        rt,
        EVENT,
        "registrar",
        &format!("(Ljava/lang/String;)L{REGISTRAR};"),
        &[event, version],
    )
}

fn payload_type(rt: &mut Rt, id: &str) -> V {
    let value = rt.new_instance(
        "net/minecraft/network/protocol/common/custom/CustomPacketPayload$Type",
        0,
    );
    let location = rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        id,
    );
    rt.put_field(value, "id", location);
    value
}

fn codec(rt: &mut Rt, name: &str) -> V {
    crate::codecs::get_static_field(
        rt,
        "net/minecraft/network/codec/ByteBufCodecs",
        name,
        &format!("L{STREAM};"),
    )
    .unwrap()
}

fn exception(rt: &mut Rt) -> String {
    let error = rt.take_exception().expect("expected a guest exception");
    rt.exception_summary(error)
}

fn closure(rt: &mut Rt, id: u32, captures: Vec<V>, handler: bool) -> V {
    let (interface, name, desc) = if handler {
        (HANDLER, "handle", HANDLE_DESC)
    } else {
        (
            "java/util/function/Function",
            "apply",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
        )
    };
    rt.alloc(
        interface,
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: interface.into(),
            sam_name: name.into(),
            sam_desc: desc.into(),
            instantiated_desc: desc.into(),
            captures,
            target: ClosureTarget::NativeLambda(id),
        })),
    )
}

fn forward(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
    let add = ctx.rt.get_field(args[0], "offset").i();
    let value = unbox_primitive(ctx.rt, args[1]).i();
    box_primitive(ctx.rt, "java/lang/Integer", V::I(value + add))
}

fn backward(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
    let subtract = ctx.rt.get_field(args[0], "offset").i();
    let value = unbox_primitive(ctx.rt, args[1]).i();
    box_primitive(ctx.rt, "java/lang/Integer", V::I(value - subtract))
}

fn payload_value(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
    ctx.rt.get_field(args[0], "value")
}

fn payload_constructor(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
    let value = ctx.rt.new_instance("example/Packet", 0);
    ctx.rt.put_field(value, "value", args[0]);
    value
}

fn handler(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
    // The context is supplied by the test host, never fabricated by the codec.
    ctx.rt.put_field(args[0], "handled", args[1]);
    ctx.rt.put_field(args[0], "context", args[2]);
    V::Void
}

fn failure(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
    ctx.rt
        .throw_exception("example/HandlerFailure", "guest callback failed")
}

static LAMBDAS: &[(u32, NativeLambdaFn)] = &[
    (1, forward),
    (2, backward),
    (3, payload_value),
    (4, payload_constructor),
    (5, handler),
    (6, failure),
];

#[test]
fn registrar_modifiers_are_immutable_and_keep_protocol_flow_and_operands() {
    let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
    let mut rt = Rt::new();
    let base = registrar(&mut prepared, &mut rt, "  1.2\u{2003}");
    let optional = invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "optional",
        &format!("()L{REGISTRAR};"),
        &[base],
    );
    let network = rt.new_pinned_handle(THREAD, "handler_thread", 0, "NETWORK");
    let direct = invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "executesOn",
        &format!("(L{THREAD};)L{REGISTRAR};"),
        &[optional, network],
    );
    let version = rt.new_str("\u{1c}2.0\u{3000}");
    let versioned = invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "versioned",
        &format!("(Ljava/lang/String;)L{REGISTRAR};"),
        &[direct, version],
    );
    assert_ne!(base, optional);
    assert_ne!(optional, direct);
    assert_ne!(direct, versioned);
    let varint = codec(&mut rt, "VAR_INT");
    let callback = rt.new_instance(HANDLER, 0);
    for (index, (name, protocol, flow)) in [
        ("playToServer", PayloadProtocol::Play, PayloadFlow::ToServer),
        ("playToClient", PayloadProtocol::Play, PayloadFlow::ToClient),
        (
            "playBidirectional",
            PayloadProtocol::Play,
            PayloadFlow::Bidirectional,
        ),
        (
            "configurationToServer",
            PayloadProtocol::Configuration,
            PayloadFlow::ToServer,
        ),
        (
            "configurationToClient",
            PayloadProtocol::Configuration,
            PayloadFlow::ToClient,
        ),
        (
            "configurationBidirectional",
            PayloadProtocol::Configuration,
            PayloadFlow::Bidirectional,
        ),
        (
            "commonToServer",
            PayloadProtocol::Common,
            PayloadFlow::ToServer,
        ),
        (
            "commonToClient",
            PayloadProtocol::Common,
            PayloadFlow::ToClient,
        ),
        (
            "commonBidirectional",
            PayloadProtocol::Common,
            PayloadFlow::Bidirectional,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let ty = payload_type(&mut rt, &format!("fixture:channel{index}"));
        let registrar = if index == 0 { base } else { versioned };
        assert_eq!(
            invoke(
                &mut prepared,
                &mut rt,
                REGISTRAR,
                name,
                CHANNEL_DESC,
                &[registrar, ty, varint, callback]
            ),
            registrar
        );
        assert!(rt.pending_exception.is_none(), "{:?}", rt.pending_exception);
        let channel = prepared.loader.state.network.channels.last().unwrap();
        assert_eq!((channel.protocol, channel.flow), (protocol, flow));
        assert_eq!((channel.codec, channel.handler_closure), (varint, callback));
        assert_eq!(
            channel.protocol_version,
            if index == 0 { "1.2" } else { "2.0" }
        );
        assert_eq!(channel.optional, index != 0);
        assert_eq!(
            channel.handler_thread,
            if index == 0 {
                PayloadHandlerThread::Main
            } else {
                PayloadHandlerThread::Network
            }
        );
    }
    assert!(prepared.loader.state.network.sent_packets.is_empty());
}

#[test]
fn retained_codec_closures_and_handler_survive_collection_and_execute_real_bytes() {
    let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
    prepared
        .linked
        .register_native_module(NativeModuleRegistry {
            classes: &[],
            lambdas: LAMBDAS,
        });
    let mut rt = Rt::new();
    let reg = registrar(&mut prepared, &mut rt, "1");
    let capture = rt.new_instance("example/CallbackState", 0);
    rt.put_field(capture, "offset", V::I(10));
    let varint = codec(&mut rt, "VAR_INT");
    let forward = closure(&mut rt, 1, vec![capture], false);
    let backward = closure(&mut rt, 2, vec![capture], false);
    let mapped = invoke(
        &mut prepared,
        &mut rt,
        STREAM,
        "map",
        &format!("(Ljava/util/function/Function;Ljava/util/function/Function;)L{STREAM};"),
        &[varint, forward, backward],
    );
    let getter = closure(&mut rt, 3, vec![], false);
    let constructor = closure(&mut rt, 4, vec![], false);
    let composite = crate::codecs::call(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &MemberRef::new(
            STREAM,
            "composite",
            &format!(
                "(L{STREAM};Ljava/util/function/Function;Ljava/util/function/Function;)L{STREAM};"
            ),
        ),
        true,
        &[mapped, getter, constructor],
    )
    .unwrap();
    let callback = closure(&mut rt, 5, vec![capture], true);
    let ty = payload_type(&mut rt, "fixture:mapped");
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "playToServer",
        CHANNEL_DESC,
        &[reg, ty, composite, callback],
    );
    assert!(rt.pending_exception.is_none(), "{:?}", rt.pending_exception);
    let channel = prepared.loader.state.network.channels[0].clone();
    rt.collect_full(&[]);
    let payload = rt.new_instance("example/Packet", 0);
    let integer = box_primitive(&mut rt, "java/lang/Integer", V::I(310));
    rt.put_field(payload, "value", integer);
    let bytes = encode(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        payload,
    )
    .unwrap();
    // 310 minus the captured 10 = 300, encoded as the 1.21.1 VarInt AC 02.
    assert_eq!(bytes, [0xac, 0x02]);
    let decoded = decode_exact(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        &bytes,
    )
    .unwrap();
    assert_ne!(decoded, payload);
    assert_eq!(
        unbox_primitive(&rt, rt.get_field(decoded, "value")),
        V::I(310)
    );
    let context = rt.new_instance("example/HostPayloadContext", 0);
    handle(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        decoded,
        context,
    )
    .unwrap();
    assert_eq!(rt.get_field(capture, "handled"), decoded);
    assert_eq!(rt.get_field(capture, "context"), context);
    assert!(prepared.loader.state.network.sent_packets.is_empty());
}

#[test]
fn decode_failure_trailing_bytes_and_guest_handler_exception_remain_errors() {
    let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
    prepared
        .linked
        .register_native_module(NativeModuleRegistry {
            classes: &[],
            lambdas: LAMBDAS,
        });
    let mut rt = Rt::new();
    let reg = registrar(&mut prepared, &mut rt, "1");
    let primitive = codec(&mut rt, "VAR_INT");
    let callback = closure(&mut rt, 6, vec![], true);
    let ty = payload_type(&mut rt, "fixture:failures");
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "configurationToServer",
        CHANNEL_DESC,
        &[reg, ty, primitive, callback],
    );
    let mut channel = prepared.loader.state.network.channels[0].clone();
    assert!(decode_exact(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        &[1, 0]
    )
    .unwrap_err()
    .contains("unread bytes"));
    assert!(decode_exact(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        &[0x80]
    )
    .unwrap_err()
    .contains("beyond available bytes"));
    rt.take_exception().unwrap();
    let payload = rt.new_instance("example/Packet", 0);
    assert!(handle(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        payload,
        V::NULL
    )
    .unwrap_err()
    .contains("host context"));
    let context = rt.new_instance("example/HostPayloadContext", 0);
    assert!(handle(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        payload,
        context
    )
    .unwrap_err()
    .contains("guest callback failed"));
    assert!(rt.pending_exception.is_some());
    rt.take_exception();
    channel.codec = codec(&mut rt, "COMPOUND_TAG");
    assert!(encode(
        &mut rt,
        &prepared.linked,
        &mut prepared.loader,
        &channel,
        payload
    )
    .unwrap_err()
    .contains("does not implement StreamCodec encode"));
    assert!(prepared.loader.state.network.sent_packets.is_empty());
}

#[test]
fn invalid_versions_reserved_namespaces_and_protocol_duplicates_do_not_register() {
    let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
    let mut rt = Rt::new();
    let primitive = codec(&mut rt, "VAR_INT");
    let callback = rt.new_instance(HANDLER, 0);
    let reg = registrar(&mut prepared, &mut rt, "\u{2003}\u{1c}");
    // Blank is accepted by the registrar and refused only by registration.
    assert!(rt.pending_exception.is_none());
    let ty = payload_type(&mut rt, "fixture:duplicate");
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "playToServer",
        CHANNEL_DESC,
        &[reg, ty, primitive, callback],
    );
    assert!(exception(&mut rt).contains("blank"));
    assert!(prepared.loader.state.network.channels.is_empty());
    let reg = registrar(&mut prepared, &mut rt, "\u{a0}");
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "playToServer",
        CHANNEL_DESC,
        &[reg, ty, primitive, callback],
    );
    assert!(rt.pending_exception.is_none());
    assert_eq!(
        prepared.loader.state.network.channels[0].protocol_version,
        "\u{a0}"
    );
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "configurationToServer",
        CHANNEL_DESC,
        &[reg, ty, primitive, callback],
    );
    assert!(rt.pending_exception.is_none());
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "playToClient",
        CHANNEL_DESC,
        &[reg, ty, primitive, callback],
    );
    assert!(exception(&mut rt).contains("already registered"));
    assert_eq!(prepared.loader.state.network.channels.len(), 2);
    let vanilla = payload_type(&mut rt, "minecraft:brand");
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "playToServer",
        CHANNEL_DESC,
        &[reg, vanilla, primitive, callback],
    );
    assert!(exception(&mut rt).contains("minecraft namespace"));
    assert_eq!(prepared.loader.state.network.channels.len(), 2);
    invoke(
        &mut prepared,
        &mut rt,
        REGISTRAR,
        "optional",
        &format!("(I)L{REGISTRAR};"),
        &[reg, V::I(0)],
    );
    assert!(exception(&mut rt).contains("does not implement"));
}

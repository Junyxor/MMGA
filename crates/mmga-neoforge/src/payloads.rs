//! Guest payload declarations and executable codecs, without a native transport.
//!
//! Registration keeps the guest codec and handler alive, including the mapper and
//! constructor closures captured by a composite codec. The host remains responsible
//! for negotiation, direction checks, scheduling and the actual IPayloadContext.
//! Successfully encoding a body does not mean it was sent or accepted by a peer.

use crate::subsystems::{
    NetworkStore, PayloadFlow, PayloadHandlerThread, PayloadProtocol, RegisteredPayloadChannel,
};
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{invoke_member, Data, ExecCtx, HostApi, LinkedSpace, Rt, V};

const EVENT: &str = "net/neoforged/neoforge/network/event/RegisterPayloadHandlersEvent";
const REGISTRAR: &str = "net/neoforged/neoforge/network/registration/PayloadRegistrar";
const THREAD: &str = "net/neoforged/neoforge/network/registration/HandlerThread";
const STREAM: &str = "net/minecraft/network/codec/StreamCodec";
const HANDLER: &str = "net/neoforged/neoforge/network/handling/IPayloadHandler";

/// Exact Java registrar dispatch. Add this before the legacy networking branch.
/// Returning a fresh view for each modifier preserves the original registrar.
pub fn call(
    rt: &mut Rt,
    network: &mut NetworkStore,
    owner_mod: Option<&str>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return None;
    }
    if member.owner == EVENT
        && member.name == "registrar"
        && member.desc == format!("(Ljava/lang/String;)L{REGISTRAR};")
    {
        let version = *args.get(1)?;
        let registrar = rt.new_instance(REGISTRAR, 0);
        initialize_registrar(rt, registrar, version, owner_mod);
        return Some(registrar);
    }
    if member.owner != REGISTRAR {
        return None;
    }
    let receiver = *args.first()?;
    if receiver.is_null() {
        return Some(rt.throw_exception(
            "java/lang/NullPointerException",
            "payload registrar is null",
        ));
    }
    match (member.name.as_str(), member.desc.as_str()) {
        ("<init>", "(Ljava/lang/String;)V") => {
            initialize_registrar(rt, receiver, *args.get(1)?, owner_mod);
            Some(V::Void)
        }
        ("versioned", descriptor) if descriptor == format!("(Ljava/lang/String;)L{REGISTRAR};") => {
            let version = *args.get(1)?;
            let next = clone_registrar(rt, receiver);
            rt.put_field(next, "$ver", version);
            Some(next)
        }
        ("optional", descriptor) if descriptor == format!("()L{REGISTRAR};") => {
            let next = clone_registrar(rt, receiver);
            rt.put_field(next, "$optional", V::TRUE);
            Some(next)
        }
        ("executesOn", descriptor) if descriptor == format!("(L{THREAD};)L{REGISTRAR};") => {
            let thread = *args.get(1)?;
            if handler_thread(rt, thread).is_none() {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "unknown payload handler thread",
                ));
            }
            let next = clone_registrar(rt, receiver);
            rt.put_field(next, "$thread", thread);
            Some(next)
        }
        (name, descriptor) => {
            let (protocol, flow, direction) = registration_kind(name)?;
            if descriptor != format!("(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload$Type;L{STREAM};L{HANDLER};)L{REGISTRAR};") {
                return None;
            }
            let payload_type = *args.get(1)?;
            let codec = *args.get(2)?;
            let handler = *args.get(3)?;
            if payload_type.is_null() || codec.is_null() || handler.is_null() {
                return Some(rt.throw_exception(
                    "java/lang/NullPointerException",
                    "payload type, codec and handler must be non-null",
                ));
            }
            let Some(payload_id) = location(rt, rt.get_field(payload_type, "id")) else {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "payload type has no resource identifier",
                ));
            };
            let version = rt.get_field(receiver, "$ver");
            if version.is_null() {
                return Some(
                    rt.throw_exception("java/lang/NullPointerException", "payload version is null"),
                );
            }
            let version = rt.str_of(version)?.trim_matches(java_whitespace).to_owned();
            if version.is_empty() {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "payload version is blank",
                ));
            }
            if payload_id.starts_with("minecraft:") {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "cannot register payload in the minecraft namespace",
                ));
            }
            if network.channels.iter().any(|channel| {
                channel.payload_id == payload_id
                    && (channel.protocol == protocol
                        || channel.protocol == PayloadProtocol::Common
                        || protocol == PayloadProtocol::Common)
            }) {
                return Some(rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    &format!("payload {payload_id} is already registered in this protocol"),
                ));
            }
            let channel = RegisteredPayloadChannel {
                mod_namespace: rt
                    .str_of(rt.get_field(receiver, "$ns"))
                    .unwrap_or("minecraft")
                    .to_owned(),
                protocol_version: version,
                payload_id,
                direction,
                codec,
                handler_closure: handler,
                protocol,
                flow,
                optional: rt.get_field(receiver, "$optional").i() != 0,
                handler_thread: handler_thread(rt, rt.get_field(receiver, "$thread"))?,
            };
            network.register_payload(rt, channel);
            Some(receiver)
        }
    }
}

fn initialize_registrar(rt: &mut Rt, registrar: V, version: V, owner_mod: Option<&str>) {
    let namespace = rt.new_str(owner_mod.unwrap_or("minecraft"));
    let thread = rt.new_pinned_handle(THREAD, "handler_thread", 0, "MAIN");
    rt.put_field(registrar, "$ns", namespace);
    rt.put_field(registrar, "$ver", version);
    rt.put_field(registrar, "$optional", V::FALSE);
    rt.put_field(registrar, "$thread", thread);
}

// String.strip/isBlank use Character.isWhitespace, which excludes NBSP, NEL,
// figure space and narrow NBSP and includes the four ASCII separator controls.
fn java_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{1680}'
        | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200a}' | '\u{2028}'..='\u{2029}'
        | '\u{205f}' | '\u{3000}')
}

fn clone_registrar(rt: &mut Rt, receiver: V) -> V {
    let next = rt.new_instance(REGISTRAR, 0);
    for field in ["$ns", "$ver", "$optional", "$thread"] {
        rt.put_field(next, field, rt.get_field(receiver, field));
    }
    next
}

fn handler_thread(rt: &Rt, value: V) -> Option<PayloadHandlerThread> {
    // The Java registrar only wraps MAIN; executesOn(null) retains the direct
    // handler just like NETWORK.
    if value.is_null() {
        return Some(PayloadHandlerThread::Network);
    }
    match rt.handle_of(value) {
        Some(("handler_thread", _, "MAIN", _)) => Some(PayloadHandlerThread::Main),
        Some(("handler_thread", _, "NETWORK", _)) => Some(PayloadHandlerThread::Network),
        _ => None,
    }
}

fn location(rt: &Rt, value: V) -> Option<String> {
    if let Some(("resource_location", _, id, _)) = rt.handle_of(value) {
        Some(id.to_owned())
    } else {
        rt.str_of(rt.get_field(value, "id")).map(str::to_owned)
    }
}

fn registration_kind(name: &str) -> Option<(PayloadProtocol, PayloadFlow, &'static str)> {
    Some(match name {
        "playToClient" => (
            PayloadProtocol::Play,
            PayloadFlow::ToClient,
            "play_to_client",
        ),
        "playToServer" => (
            PayloadProtocol::Play,
            PayloadFlow::ToServer,
            "play_to_server",
        ),
        "playBidirectional" => (
            PayloadProtocol::Play,
            PayloadFlow::Bidirectional,
            "bidirectional",
        ),
        "configurationToClient" => (
            PayloadProtocol::Configuration,
            PayloadFlow::ToClient,
            "configuration_to_client",
        ),
        "configurationToServer" => (
            PayloadProtocol::Configuration,
            PayloadFlow::ToServer,
            "configuration_to_server",
        ),
        "configurationBidirectional" => (
            PayloadProtocol::Configuration,
            PayloadFlow::Bidirectional,
            "configuration_bidirectional",
        ),
        "commonToClient" => (
            PayloadProtocol::Common,
            PayloadFlow::ToClient,
            "common_to_client",
        ),
        "commonToServer" => (
            PayloadProtocol::Common,
            PayloadFlow::ToServer,
            "common_to_server",
        ),
        "commonBidirectional" => (
            PayloadProtocol::Common,
            PayloadFlow::Bidirectional,
            "common_bidirectional",
        ),
        _ => return None,
    })
}

/// Encode an application body with the retained guest codec. No channel envelope
/// or arbitrary body cap is added; the eventual transport owns its framing limits.
pub fn encode(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    channel: &RegisteredPayloadChannel,
    payload: V,
) -> Result<Vec<u8>, String> {
    check_pending(rt, "before payload encoding")?;
    require_codec(rt, channel)?;
    let buffer = new_buffer(rt, channel, Vec::new());
    let mut context = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    invoke_member(
        &mut context,
        op::INVOKEINTERFACE,
        &MemberRef::new(STREAM, "encode", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
        &[channel.codec, buffer, payload],
    );
    check_pending(
        context.rt,
        &format!("encoding payload {}", channel.payload_id),
    )?;
    match &context
        .rt
        .get_obj(buffer)
        .ok_or("payload encoder lost its buffer")?
        .data
    {
        Data::ByteBuf { bytes, .. } => Ok(bytes.clone()),
        _ => Err("payload encoder replaced its byte buffer".into()),
    }
}

/// Decode one complete application body; trailing bytes are rejected rather than
/// concealed. The returned object is a VM reference owned by the calling VM thread.
pub fn decode_exact(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    channel: &RegisteredPayloadChannel,
    bytes: &[u8],
) -> Result<V, String> {
    check_pending(rt, "before payload decoding")?;
    require_codec(rt, channel)?;
    let buffer = new_buffer(rt, channel, bytes.to_vec());
    let mut context = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    let decoded = invoke_member(
        &mut context,
        op::INVOKEINTERFACE,
        &MemberRef::new(STREAM, "decode", "(Ljava/lang/Object;)Ljava/lang/Object;"),
        &[channel.codec, buffer],
    );
    check_pending(
        context.rt,
        &format!("decoding payload {}", channel.payload_id),
    )?;
    let Some(Data::ByteBuf { bytes, reader_idx }) =
        context.rt.get_obj(buffer).map(|object| &object.data)
    else {
        return Err("payload decoder lost its byte buffer".into());
    };
    if *reader_idx != bytes.len() {
        return Err(format!(
            "payload {} left {} unread bytes",
            channel.payload_id,
            bytes.len().saturating_sub(*reader_idx)
        ));
    }
    Ok(decoded)
}

fn new_buffer(rt: &mut Rt, channel: &RegisteredPayloadChannel, bytes: Vec<u8>) -> V {
    let class = if channel.protocol == PayloadProtocol::Play {
        "net/minecraft/network/RegistryFriendlyByteBuf"
    } else {
        "net/minecraft/network/FriendlyByteBuf"
    };
    rt.alloc(
        class,
        0,
        Data::ByteBuf {
            bytes,
            reader_idx: 0,
        },
    )
}

/// Invoke the retained guest handler. `context` must be supplied by the real host
/// and implement the correct player/executor/work semantics. This never invents
/// a player or treats a callback return as a delivery acknowledgement.
pub fn handle(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    channel: &RegisteredPayloadChannel,
    payload: V,
    context: V,
) -> Result<(), String> {
    check_pending(rt, "before payload handling")?;
    if channel.handler_closure.is_null() || context.is_null() {
        return Err(format!(
            "payload {} requires a handler and host context",
            channel.payload_id
        ));
    }
    let mut execution = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    invoke_member(&mut execution, op::INVOKEINTERFACE, &MemberRef::new(HANDLER, "handle", "(Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;Lnet/neoforged/neoforge/network/handling/IPayloadContext;)V"), &[channel.handler_closure, payload, context]);
    check_pending(
        execution.rt,
        &format!("handling payload {}", channel.payload_id),
    )
}

fn require_codec(rt: &Rt, channel: &RegisteredPayloadChannel) -> Result<(), String> {
    if channel.codec.is_null() || rt.get_obj(channel.codec).is_none() {
        Err(format!(
            "payload {} has no executable guest codec",
            channel.payload_id
        ))
    } else {
        Ok(())
    }
}

fn check_pending(rt: &Rt, operation: &str) -> Result<(), String> {
    match rt.pending_exception {
        Some(exception) => Err(format!(
            "{operation}: {}",
            rt.exception_summary(V::R(exception))
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "payloads_tests.rs"]
mod tests;

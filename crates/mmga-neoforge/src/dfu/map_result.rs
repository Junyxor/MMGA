//! MapCodec.ResultFunction executes at the decode/encode boundary.
//! JsonOps MapLike/RecordBuilder values expose real encoded entries and errors.
use super::*;
const MAP_LIKE: &str = "mmga/internal/JsonMapLike";
const BUILDER: &str = "mmga/internal/JsonRecordBuilder";
const RESULT_FUNCTION: &str = "com/mojang/serialization/MapCodec$ResultFunction";
const RECORD_BUILDER: &str = "com/mojang/serialization/RecordBuilder";

pub(super) fn is_subtype(sub: &str, sup: &str) -> bool {
    (sub == MAP_LIKE && sup == "com/mojang/serialization/MapLike")
        || (sub == BUILDER && sup == RECORD_BUILDER)
}
pub(super) fn call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    let class = ctx.rt.class_of(recv)?;
    let (name, desc) = (member.name.as_str(), member.desc.as_str());
    if class == MAP_LIKE {
        let payload = ctx.rt.get_field(recv, "$input");
        let object = json::to_value(ctx.rt, payload)?.as_object()?.clone();
        return match (name, desc) {
            (
                "get",
                "(Ljava/lang/String;)Ljava/lang/Object;" | "(Ljava/lang/Object;)Ljava/lang/Object;",
            ) => {
                let key = if desc.starts_with("(Ljava/lang/String;") {
                    ctx.rt.str_of(args[1])?.to_owned()
                } else {
                    match json::to_value(ctx.rt, args[1])? {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(v) => v.to_string(),
                        _ => {
                            return Some(ctx.rt.throw_exception(
                                "java/lang/UnsupportedOperationException",
                                "Json MapLike key must be primitive",
                            ))
                        }
                    }
                };
                Some(
                    object
                        .get(&key)
                        .map_or(V::NULL, |value| json::from_value(ctx.rt, value.clone())),
                )
            }
            ("entries", "()Ljava/util/stream/Stream;") => {
                let entries = object
                    .into_iter()
                    .map(|(key, value)| {
                        let key = json::from_value(ctx.rt, serde_json::Value::String(key));
                        let value = json::from_value(ctx.rt, value);
                        let pair = ctx.rt.new_instance("com/mojang/datafixers/util/Pair", 0);
                        ctx.rt.put_field(pair, "first", key);
                        ctx.rt.put_field(pair, "second", value);
                        pair
                    })
                    .collect();
                Some(
                    ctx.rt
                        .alloc("java/util/stream/Stream", 0, Data::Stream(entries)),
                )
            }
            _ => None,
        };
    }
    if class != BUILDER {
        return None;
    }
    match (name, desc) {
        ("ops", "()Lcom/mojang/serialization/DynamicOps;") => Some(ctx.rt.get_field(recv, "$ops")),
        (
            "add",
            "(Ljava/lang/String;Ljava/lang/Object;)Lcom/mojang/serialization/RecordBuilder;"
            | "(Ljava/lang/Object;Ljava/lang/Object;)Lcom/mojang/serialization/RecordBuilder;",
        ) => {
            let key = if desc.starts_with("(Ljava/lang/String;") {
                ctx.rt.str_of(args[1])?.to_owned()
            } else {
                match json::to_value(ctx.rt, args[1])? {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(v) => v.to_string(),
                    _ => {
                        return Some(ctx.rt.throw_exception(
                            "java/lang/UnsupportedOperationException",
                            "Json RecordBuilder key must be primitive",
                        ))
                    }
                }
            };
            let Some(value) = json::to_value(ctx.rt, args[2]) else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "Json RecordBuilder value must be JsonElement",
                ));
            };
            let mut payload = payload(ctx.rt, recv)?.as_object()?.clone();
            payload.insert(key, value);
            let object = json::from_value(ctx.rt, serde_json::Value::Object(payload));
            ctx.rt.put_field(recv, "$payload", object);
            Some(recv)
        }
        (
            "withErrorsFrom",
            "(Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/RecordBuilder;",
        ) => {
            if !matches!(ctx.rt.get_field(args[1], "$success"), V::TRUE | V::FALSE) {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "RecordBuilder error source must be an executable DataResult",
                ));
            }
            if ctx.rt.get_field(args[1], "$success") == V::FALSE {
                let message = result::message(ctx, args[1]);
                append_error(ctx.rt, recv, &message);
            }
            Some(recv)
        }
        ("build", "(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;") => {
            let built = finish(ctx.rt, recv);
            Some(match built {
                Ok(json) => match merge_prefix(ctx.rt, Some(args[1]), json) {
                    Ok(json) => {
                        let encoded = json::from_value(ctx.rt, json);
                        result::success(ctx.rt, encoded)
                    }
                    Err(error) => {
                        let partial = error.partial.map(|json| json::from_value(ctx.rt, json));
                        result::error(ctx.rt, &error.message, partial)
                    }
                },
                Err(error) => {
                    let partial = error
                        .partial
                        .and_then(|json| merge_prefix(ctx.rt, Some(args[1]), json).ok())
                        .map(|json| json::from_value(ctx.rt, json));
                    result::error(ctx.rt, &error.message, partial)
                }
            })
        }
        _ => None,
    }
}
fn append_error(rt: &mut Rt, builder: V, message: &str) {
    let old = rt.str_of(rt.get_field(builder, "$message")).unwrap_or("");
    let combined = if old.is_empty() {
        message.to_owned()
    } else {
        format!("{old}; {message}")
    };
    let text = rt.new_str(combined);
    rt.put_field(builder, "$message", text);
}
fn payload(rt: &Rt, builder: V) -> Option<serde_json::Value> {
    json::to_value(rt, rt.get_field(builder, "$payload"))
}
fn finish(rt: &Rt, builder: V) -> Result<serde_json::Value, EncodeError> {
    let value = payload(rt, builder).ok_or("Invalid Json RecordBuilder payload")?;
    let message = rt.str_of(rt.get_field(builder, "$message")).unwrap_or("");
    if message.is_empty() {
        Ok(value)
    } else {
        Err(EncodeError {
            message: message.into(),
            partial: Some(value),
        })
    }
}
pub(super) fn decode(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    input: &serde_json::Value,
    compressed: bool,
    depth: usize,
) -> Result<V, DecodeError> {
    if compressed {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Compressed MapCodec.mapResult requires a key compressor",
        );
        return Err("Unsupported compressed map result".into());
    }
    if !input.is_object() {
        return Err(format!("Not a map: {input}").into());
    }
    let decoded = decode_value(ctx, args[0], ops, input, compressed, depth + 1);
    if ctx.rt.pending_exception.is_some() {
        return Err("MapResult delegate failed".into());
    }
    let result = match decoded {
        Ok(value) => result::success(ctx.rt, value),
        Err(error) => result::error(ctx.rt, &error.message, error.partial),
    };
    let map = ctx.rt.new_instance(MAP_LIKE, 0);
    let original = json::from_value(ctx.rt, input.clone());
    ctx.rt.put_field(map, "$input", original);
    let transformed=invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(RESULT_FUNCTION,"apply","(Lcom/mojang/serialization/DynamicOps;Lcom/mojang/serialization/MapLike;Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/DataResult;"),&[args[1],ops,map,result]);
    if ctx.rt.pending_exception.is_some() {
        return Err("MapResult function failed".into());
    }
    if transformed.is_null() {
        ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "MapResult function returned null DataResult",
        );
        return Err("Null map result".into());
    }
    if !matches!(
        ctx.rt.get_field(transformed, "$success"),
        V::TRUE | V::FALSE
    ) {
        ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "MapResult function did not return DataResult",
        );
        return Err("Invalid map result".into());
    }
    flatten_decoded_result(ctx, transformed)
}
pub(super) fn encode(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    value: V,
    compressed: bool,
    depth: usize,
) -> Result<serde_json::Value, EncodeError> {
    if compressed {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Compressed MapCodec.mapResult requires a key compressor",
        );
        return Err("Unsupported compressed map result".into());
    }
    let encoded = encode_value(ctx, args[0], ops, value, compressed, depth + 1);
    if ctx.rt.pending_exception.is_some() {
        return Err("MapResult delegate failed".into());
    }
    let builder = ctx.rt.new_instance(BUILDER, 0);
    ctx.rt.put_field(builder, "$ops", ops);
    let payload = match encoded {
        Ok(value) => value,
        Err(error) => {
            append_error(ctx.rt, builder, &error.message);
            error.partial.unwrap_or_else(|| serde_json::json!({}))
        }
    };
    if !payload.is_object() {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MapResult child encoder requires an object RecordBuilder",
        );
        return Err("Unsupported map result builder".into());
    }
    let payload = json::from_value(ctx.rt, payload);
    ctx.rt.put_field(builder, "$payload", payload);
    let builder=invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(RESULT_FUNCTION,"coApply","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;Lcom/mojang/serialization/RecordBuilder;)Lcom/mojang/serialization/RecordBuilder;"),&[args[1],ops,value,builder]);
    if ctx.rt.pending_exception.is_some() {
        return Err("MapResult encoder function failed".into());
    }
    if ctx.rt.class_of(builder) != Some(BUILDER) {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MapResult custom RecordBuilder execution is not implemented",
        );
        return Err("Unknown map result builder".into());
    }
    finish(ctx.rt, builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{NativeClassDef, NativeModuleRegistry};
    fn apply(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(
            args[0],
            "decode_calls",
            V::I(ctx.rt.get_field(args[0], "decode_calls").i() + 1),
        );
        let key = ctx.rt.intern_str("n");
        let seen = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "com/mojang/serialization/MapLike",
                "get",
                "(Ljava/lang/String;)Ljava/lang/Object;",
            ),
            &[args[2], key],
        );
        ctx.rt.put_field(args[0], "seen", seen);
        args[3]
    }
    fn co_apply(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(
            args[0],
            "encode_calls",
            V::I(ctx.rt.get_field(args[0], "encode_calls").i() + 1),
        );
        let key = ctx.rt.intern_str("extra");
        let value = json::from_value(ctx.rt, serde_json::json!(true));
        invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                RECORD_BUILDER,
                "add",
                "(Ljava/lang/String;Ljava/lang/Object;)Lcom/mojang/serialization/RecordBuilder;",
            ),
            &[args[3], key, value],
        )
    }
    fn fixture() -> crate::PreparedLoad {
        let mut prepared = crate::PreparedLoad::from_jars(vec![], true).unwrap();
        prepared.linked.register_native_module(NativeModuleRegistry{classes:&[NativeClassDef{name:"test/ResultFunction",superclass:Some("java/lang/Object"),interfaces:&[RESULT_FUNCTION],is_interface:false,methods:&[("apply.(Lcom/mojang/serialization/DynamicOps;Lcom/mojang/serialization/MapLike;Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/DataResult;",apply,false,false),("coApply.(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;Lcom/mojang/serialization/RecordBuilder;)Lcom/mojang/serialization/RecordBuilder;",co_apply,false,false)]}],lambdas:&[]});
        prepared
    }
    #[test]
    fn result_function_runs_actual_decode_and_encoder_and_keeps_partial_errors() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let function = rt.new_instance("test/ResultFunction", 0);
        let int = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let field = field_codec(&mut rt, int, "n");
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let map=call_codec(&mut ctx,&MemberRef::new(MAP,"mapResult","(Lcom/mojang/serialization/MapCodec$ResultFunction;)Lcom/mojang/serialization/MapCodec;"),false,&[field,function]).unwrap();
        let value =
            decode_value(&mut ctx, map, ops, &serde_json::json!({"n":7}), false, 0).unwrap();
        assert_eq!(ctx.rt.get_field(value, "$value"), V::I(7));
        assert_eq!(
            json::to_value(ctx.rt, ctx.rt.get_field(function, "seen")),
            Some(serde_json::json!(7))
        );
        assert_eq!(
            encode_value(&mut ctx, map, ops, value, false, 0).unwrap(),
            serde_json::json!({"n":7,"extra":true})
        );
        assert_eq!(ctx.rt.get_field(function, "decode_calls"), V::I(1));
        assert_eq!(ctx.rt.get_field(function, "encode_calls"), V::I(1));
        assert!(decode_value(
            &mut ctx,
            map,
            ops,
            &serde_json::json!({"n":"bad"}),
            false,
            0
        )
        .is_err());
        assert_eq!(ctx.rt.get_field(function, "decode_calls"), V::I(2));
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn result_function_is_retained_and_unresolved_execution_never_succeeds() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let function = rt.new_instance("test/MissingResultFunction", 0);
        let int = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let field = field_codec(&mut rt, int, "n");
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let map=call_codec(&mut ctx,&MemberRef::new(MAP,"mapResult","(Lcom/mojang/serialization/MapCodec$ResultFunction;)Lcom/mojang/serialization/MapCodec;"),false,&[field,function]).unwrap();
        assert_eq!(graph_parts(ctx.rt, map).unwrap().1, vec![field, function]);
        assert!(decode_value(&mut ctx, map, ops, &serde_json::json!({"n":7}), false, 0).is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

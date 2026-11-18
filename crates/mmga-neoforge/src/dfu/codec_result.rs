//! Codec.ResultFunction receives actual decoded pairs and encoded prefix results.
use super::*;
const FUNCTION: &str = "com/mojang/serialization/Codec$ResultFunction";
pub(super) fn pair(rt: &mut Rt, value: V, remainder: serde_json::Value) -> V {
    let pair = rt.new_instance("com/mojang/datafixers/util/Pair", 0);
    let remainder = json::from_value(rt, remainder);
    rt.put_field(pair, "first", value);
    rt.put_field(pair, "second", remainder);
    pair
}
fn pair_parts(ctx: &mut ExecCtx<'_>, value: V) -> Result<(V, serde_json::Value), DecodeError> {
    if ctx.rt.class_of(value) != Some("com/mojang/datafixers/util/Pair") {
        ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "Codec ResultFunction decode must return Pair in DataResult",
        );
        return Err("Invalid codec result pair".into());
    }
    let remainder = json::to_value(ctx.rt, ctx.rt.get_field(value, "second"))
        .ok_or("Codec ResultFunction remainder must be JsonElement")?;
    Ok((ctx.rt.get_field(value, "first"), remainder))
}
pub(super) fn decode(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    input: &serde_json::Value,
    compressed: bool,
    depth: usize,
) -> Result<(V, serde_json::Value), DecodeError> {
    let transformed = decode_result(ctx, args, ops, input, compressed, depth, None)?;
    match flatten_decoded_result(ctx, transformed) {
        Ok(value) => pair_parts(ctx, value),
        Err(mut error) => {
            if let Some(value) = error.partial {
                let (value, remainder) = pair_parts(ctx, value)?;
                error.partial = Some(value);
                error.remainder = Some(remainder);
            }
            Err(error)
        }
    }
}
pub(super) fn decode_result(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    input: &serde_json::Value,
    compressed: bool,
    depth: usize,
    original: Option<V>,
) -> Result<V, DecodeError> {
    let decoded = decode_with_remainder(ctx, args[0], ops, input, compressed, depth + 1);
    if ctx.rt.pending_exception.is_some() {
        return Err("Codec ResultFunction delegate failed".into());
    }
    let result = match decoded {
        Ok((value, remainder)) => {
            let pair = pair(ctx.rt, value, remainder);
            result::success(ctx.rt, pair)
        }
        Err(error) => {
            let remainder = error
                .remainder
                .unwrap_or_else(|| decode_remainder(ctx.rt, args[0], input));
            let partial = error.partial.map(|value| pair(ctx.rt, value, remainder));
            result::error(ctx.rt, &error.message, partial)
        }
    };
    let input = original.unwrap_or_else(|| json::from_value(ctx.rt, input.clone()));
    let transformed=invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(FUNCTION,"apply","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/DataResult;"),&[args[1],ops,input,result]);
    if ctx.rt.pending_exception.is_some() {
        return Err("Codec ResultFunction callback failed".into());
    }
    if !matches!(
        ctx.rt.get_field(transformed, "$success"),
        V::TRUE | V::FALSE
    ) {
        ctx.rt.throw_exception(
            if transformed.is_null() {
                "java/lang/NullPointerException"
            } else {
                "java/lang/ClassCastException"
            },
            "Codec ResultFunction must return DataResult",
        );
        return Err("Invalid codec result".into());
    }
    Ok(transformed)
}
pub(super) fn encode(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    value: V,
    compressed: bool,
    depth: usize,
    prefix: Option<V>,
) -> Result<serde_json::Value, EncodeError> {
    let transformed = encode_result(ctx, args, ops, value, compressed, depth, prefix)?;
    match flatten_decoded_result(ctx, transformed) {
        Ok(value) => json::to_value(ctx.rt, value)
            .ok_or_else(|| "Codec ResultFunction encoder result must be JsonElement".into()),
        Err(error) => {
            let partial = error
                .partial
                .and_then(|value| json::to_value(ctx.rt, value));
            Err(EncodeError {
                message: error.message,
                partial,
            })
        }
    }
}
pub(super) fn encode_result(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    value: V,
    compressed: bool,
    depth: usize,
    prefix: Option<V>,
) -> Result<V, EncodeError> {
    let encoded = encode_value(ctx, args[0], ops, value, compressed, depth + 1);
    if ctx.rt.pending_exception.is_some() {
        return Err("Codec ResultFunction delegate failed".into());
    }
    let prefix = if is_empty_map_encoding(ctx.rt, args[0], value) {
        None
    } else {
        prefix
    };
    let encoded = match encoded {
        Ok(value) => merge_prefix(ctx.rt, prefix, value),
        Err(mut error) => {
            if let Some(value) = error.partial {
                match merge_prefix(ctx.rt, prefix, value) {
                    Ok(value) => error.partial = Some(value),
                    Err(next) => {
                        error.message.push_str("; ");
                        error.message.push_str(&next.message);
                        error.partial = next.partial;
                    }
                }
            }
            Err(error)
        }
    };
    let result = match encoded {
        Ok(value) => {
            let value = json::from_value(ctx.rt, value);
            result::success(ctx.rt, value)
        }
        Err(error) => {
            let partial = error.partial.map(|value| json::from_value(ctx.rt, value));
            result::error(ctx.rt, &error.message, partial)
        }
    };
    let transformed=invoke_member(ctx,op::INVOKEINTERFACE,&MemberRef::new(FUNCTION,"coApply","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/DataResult;"),&[args[1],ops,value,result]);
    if ctx.rt.pending_exception.is_some() {
        return Err("Codec ResultFunction callback failed".into());
    }
    if !matches!(
        ctx.rt.get_field(transformed, "$success"),
        V::TRUE | V::FALSE
    ) {
        ctx.rt.throw_exception(
            if transformed.is_null() {
                "java/lang/NullPointerException"
            } else {
                "java/lang/ClassCastException"
            },
            "Codec ResultFunction must return DataResult",
        );
        return Err("Invalid codec result".into());
    }
    Ok(transformed)
}
pub(super) fn parsed_result(ctx: &mut ExecCtx<'_>, result: V) -> Result<V, DecodeError> {
    let object = ctx
        .rt
        .get_obj(result)
        .ok_or("Invalid DataResult from codec")?
        .clone();
    let out = ctx.rt.alloc(object.class, 0, object.data);
    let field = if ctx.rt.get_field(result, "$success") == V::TRUE {
        "$value"
    } else {
        "$partial"
    };
    let pair = ctx.rt.get_field(result, field);
    if !pair.is_null() {
        if ctx.rt.class_of(pair) != Some("com/mojang/datafixers/util/Pair") {
            ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "Codec ResultFunction parse must map a Pair",
            );
            return Err("Invalid codec result pair".into());
        }
        ctx.rt
            .put_field(out, field, ctx.rt.get_field(pair, "first"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{NativeClassDef, NativeModuleRegistry};
    fn apply(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(args[0], "input", args[2]);
        ctx.rt.put_field(args[0], "result", args[3]);
        let stable = result::call(
            ctx,
            &MemberRef::new(
                "com/mojang/serialization/Lifecycle",
                "stable",
                "()Lcom/mojang/serialization/Lifecycle;",
            ),
            true,
            &[],
        )
        .unwrap();
        result::call(
            ctx,
            &MemberRef::new(
                RESULT,
                "setLifecycle",
                "(Lcom/mojang/serialization/Lifecycle;)Lcom/mojang/serialization/DataResult;",
            ),
            false,
            &[args[3], stable],
        )
        .unwrap()
    }
    fn co_apply(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.put_field(args[0], "encoded", args[3]);
        args[3]
    }
    fn fixture() -> crate::PreparedLoad {
        let mut prepared = crate::PreparedLoad::from_jars(vec![], true).unwrap();
        prepared.linked.register_native_module(NativeModuleRegistry{classes:&[NativeClassDef{name:"test/CodecResult",superclass:Some("java/lang/Object"),interfaces:&[FUNCTION],is_interface:false,methods:&[("apply.(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/DataResult;",apply,false,false),("coApply.(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;Lcom/mojang/serialization/DataResult;)Lcom/mojang/serialization/DataResult;",co_apply,false,false)]}],lambdas:&[]});
        prepared
    }
    #[test]
    fn result_function_sees_real_pairs_partial_error_and_encoded_prefix() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let callback = rt.new_instance("test/CodecResult", 0);
        let int = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let map = field_codec(&mut rt, int, "n");
        let base = codec_from_map(&mut rt, map);
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
        let codec = call_codec(
            &mut ctx,
            &MemberRef::new(
                CODEC,
                "mapResult",
                "(Lcom/mojang/serialization/Codec$ResultFunction;)Lcom/mojang/serialization/Codec;",
            ),
            false,
            &[base, callback],
        )
        .unwrap();
        let input = serde_json::json!({"n":7,"extra":"kept"});
        let (value, remainder) =
            decode_with_remainder(&mut ctx, codec, ops, &input, false, 0).unwrap();
        assert_eq!(ctx.rt.get_field(value, "$value"), V::I(7));
        assert_eq!(remainder, input);
        let pair = result::value(ctx.rt, ctx.rt.get_field(callback, "result")).unwrap();
        assert_eq!(ctx.rt.get_field(pair, "first"), value);
        let prefix = json::from_value(ctx.rt, serde_json::json!({"prefix":1}));
        let encoded=call_codec(&mut ctx,&MemberRef::new(CODEC,"encode","(Ljava/lang/Object;Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),false,&[codec,value,ops,prefix]).unwrap();
        let encoded = result::value(ctx.rt, encoded).unwrap();
        assert_eq!(
            json::to_value(ctx.rt, encoded),
            Some(serde_json::json!({"n":7,"prefix":1}))
        );
        let seen = result::value(ctx.rt, ctx.rt.get_field(callback, "encoded")).unwrap();
        assert_eq!(
            json::to_value(ctx.rt, seen),
            json::to_value(ctx.rt, encoded)
        );
        assert!(decode_with_remainder(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"n":"bad"}),
            false,
            0
        )
        .is_err());
        let result = ctx.rt.get_field(callback, "result");
        assert_eq!(ctx.rt.get_field(result, "$success"), V::FALSE);
        assert!(ctx.rt.pending_exception.is_none());
        let input = json::from_value(ctx.rt, serde_json::json!({"n":7}));
        let parsed=call_codec(&mut ctx,&MemberRef::new(CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),false,&[codec,ops,input]).unwrap();
        let life = ctx.rt.get_field(parsed, "$lifecycle");
        assert_eq!(ctx.rt.handle_of(life).unwrap().2, "stable");
        assert_eq!(ctx.rt.get_field(callback, "input"), input);
    }
    #[test]
    fn missing_result_function_execution_is_an_explicit_host_error() {
        let mut prepared = fixture();
        let mut rt = Rt::new();
        let callback = rt.new_instance("test/MissingResult", 0);
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
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
        let codec = call_codec(
            &mut ctx,
            &MemberRef::new(
                CODEC,
                "mapResult",
                "(Lcom/mojang/serialization/Codec$ResultFunction;)Lcom/mojang/serialization/Codec;",
            ),
            false,
            &[base, callback],
        )
        .unwrap();
        assert!(
            decode_with_remainder(&mut ctx, codec, ops, &serde_json::json!(7), false, 0).is_err()
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

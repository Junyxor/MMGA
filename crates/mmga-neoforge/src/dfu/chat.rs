//! ComponentSerialization's JSON value subset; unsupported content never becomes empty text.
use super::*;
use serde_json::{Map, Value as Json};

const COMPONENT: &str = "net/minecraft/network/chat/Component";
const MUTABLE: &str = "net/minecraft/network/chat/MutableComponent";
const SERIALIZER: &str = "net/minecraft/network/chat/ComponentSerialization";
const STYLE: &str = "net/minecraft/network/chat/Style";
const FLAGS: [(&str, &str); 5] = [
    ("bold", "Bold"),
    ("italic", "Italic"),
    ("underlined", "Underlined"),
    ("strikethrough", "Strikethrough"),
    ("obfuscated", "Obfuscated"),
];

pub(super) fn get_static(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == SERIALIZER
        && name == "TRUSTED_CONTEXT_FREE_STREAM_CODEC"
        && desc == "Lnet/minecraft/network/codec/StreamCodec;"
    {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let component = get_static(rt, owner, "CODEC", "Lcom/mojang/serialization/Codec;")?;
        let value = crate::codecs::call(
            rt,
            &LinkedSpace::new(),
            &mut mmga_runtime::NullHost,
            &MemberRef::new(
                "net/minecraft/network/codec/ByteBufCodecs",
                "fromCodecTrusted",
                "(Lcom/mojang/serialization/Codec;)Lnet/minecraft/network/codec/StreamCodec;",
            ),
            true,
            &[component],
        )?;
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if owner != SERIALIZER || name != "CODEC" || desc != "Lcom/mojang/serialization/Codec;" {
        return None;
    }
    if let Some(value) = rt.get_static(owner, name) {
        return Some(value);
    }
    let codec = graph(rt, CODEC, "component", &[]);
    rt.put_static(owner, name, codec);
    Some(codec)
}
fn literal(rt: &mut Rt, text: &str) -> V {
    let value = rt.new_str(text);
    crate::minecraft::call_chat(
        rt,
        &Default::default(),
        &MemberRef::new(
            COMPONENT,
            "literal",
            "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
        ),
        true,
        &[value],
    )
    .unwrap()
}
fn append(rt: &mut Rt, parent: V, child: V) {
    let list = rt.get_field(parent, "$siblings");
    if let Some(Data::List(values)) = rt.get_obj_mut(list).map(|o| &mut o.data) {
        values.push(child);
    }
}
fn unsupported_decode(ctx: &mut ExecCtx<'_>, feature: &str) -> DecodeError {
    let message = format!("MMGA ComponentSerialization does not implement {feature}");
    ctx.rt
        .throw_exception("java/lang/UnsupportedOperationException", &message);
    message.into()
}
pub(super) fn decode(
    ctx: &mut ExecCtx<'_>,
    ops: V,
    input: &Json,
    compressed: bool,
    depth: usize,
) -> Result<V, DecodeError> {
    if depth >= 128 {
        return Err("Component nesting exceeds128".into());
    }
    if ctx.rt.pending_exception.is_some() {
        return Err("Component callback failed".into());
    }
    if let Some(text) = input.as_str() {
        return Ok(literal(ctx.rt, text));
    }
    if compressed && input.is_number() {
        return Ok(literal(ctx.rt, &input.to_string()));
    }
    if let Some(values) = input.as_array() {
        if values.is_empty() {
            return Err("Component list cannot be empty".into());
        }
        let first = decode(ctx, ops, &values[0], compressed, depth + 1)?;
        for value in &values[1..] {
            let child = decode(ctx, ops, value, compressed, depth + 1)?;
            append(ctx.rt, first, child);
        }
        return Ok(first);
    }
    let object = input
        .as_object()
        .ok_or_else(|| format!("Not a component: {input}"))?;
    if compressed {
        return Err(unsupported_decode(ctx, "compressed component object maps"));
    }
    let explicit_type = object
        .get("type")
        .filter(|v| !v.is_null())
        .map(|v| v.as_str().ok_or("Component type must be a string"))
        .transpose()?;
    let selected = match explicit_type {
        Some("text") => "text",
        Some("translatable") => "translate",
        Some("score" | "selector" | "keybind" | "nbt") => {
            return Err(unsupported_decode(ctx, explicit_type.unwrap()))
        }
        Some(other) => return Err(format!("Unknown component type {other}").into()),
        None => {
            if object.get("text").is_some_and(Json::is_string) {
                "text"
            } else if object.get("translate").is_some_and(Json::is_string) {
                "translate"
            } else if ["score", "selector", "keybind", "nbt"]
                .iter()
                .any(|k| object.contains_key(*k))
            {
                return Err(unsupported_decode(
                    ctx,
                    "score/selector/keybind/NBT content",
                ));
            } else {
                return Err("No matching component content".into());
            }
        }
    };
    let component = if selected == "text" {
        let text = object
            .get("text")
            .and_then(Json::as_str)
            .ok_or("Component text must be a string")?;
        literal(ctx.rt, text)
    } else {
        let key = object
            .get("translate")
            .and_then(Json::as_str)
            .ok_or("Component translate must be a string")?;
        let key = ctx.rt.new_str(key);
        let fallback = object
            .get("fallback")
            .and_then(Json::as_str)
            .map(|s| ctx.rt.new_str(s))
            .unwrap_or(V::NULL);
        let mut values = Vec::new();
        if let Some(args) = object.get("with").filter(|v| !v.is_null()) {
            for arg in args.as_array().ok_or("Component with must be a list")? {
                let value = match arg {
                    Json::String(text) => ctx.rt.new_str(text),
                    Json::Bool(value) => {
                        box_primitive(ctx.rt, "java/lang/Boolean", V::bool(*value))
                    }
                    Json::Number(_) => json::primitive_read(ctx.rt, "NUMBER", arg, false)?,
                    Json::Object(_) | Json::Array(_) => decode(ctx, ops, arg, false, depth + 1)?,
                    Json::Null => return Err("Translation argument cannot be null".into()),
                };
                values.push(value);
            }
        }
        let args = ctx.rt.alloc("[Ljava/lang/Object;", 0, Data::Array(values));
        crate::minecraft::call_chat(ctx.rt,&Default::default(),&MemberRef::new(COMPONENT,"translatableWithFallback","(Ljava/lang/String;Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;"),true,&[key,fallback,args]).unwrap()
    };
    if let Some(extra) = object.get("extra").filter(|v| !v.is_null()) {
        let extra = extra.as_array().ok_or("Component extra must be a list")?;
        if extra.is_empty() {
            return Err("Component extra cannot be empty".into());
        }
        for value in extra {
            let child = decode(ctx, ops, value, false, depth + 1)?;
            append(ctx.rt, component, child);
        }
    }
    let style = ctx.rt.get_field(component, "$style");
    debug_assert_eq!(ctx.rt.class_of(style), Some(STYLE));
    for (key, field) in FLAGS {
        if let Some(value) = object.get(key).filter(|v| !v.is_null()) {
            let value = value
                .as_bool()
                .ok_or_else(|| format!("Style {key} must be boolean"))?;
            ctx.rt.put_field(style, field, V::bool(value));
        }
    }
    if let Some(color) = object.get("color").filter(|v| !v.is_null()) {
        let name = color.as_str().ok_or("Style color must be a string")?;
        let rgb = if let Some(hex) = name.strip_prefix('#') {
            let parsed =
                i32::from_str_radix(hex, 16).map_err(|_| format!("Invalid color {name}"))?;
            if !(0..=0xffffff).contains(&parsed) {
                return Err(format!("Invalid color {name}").into());
            }
            parsed
        } else {
            if name != name.to_ascii_lowercase() {
                return Err(format!("Invalid color {name}").into());
            }
            let format = crate::minecraft::get_static_field(
                ctx.rt,
                "net/minecraft/ChatFormatting",
                &name.to_ascii_uppercase(),
            )
            .ok_or_else(|| format!("Invalid color {name}"))?;
            let rgb = ctx.rt.get_field(format, "$rgb").i();
            if rgb < 0 {
                return Err(format!("Invalid color {name}").into());
            }
            let stored = ctx.rt.intern_str(name);
            ctx.rt.put_field(style, "$color_name", stored);
            ctx.rt.put_field(style, "$named_color_rgb", V::I(rgb));
            rgb
        };
        ctx.rt.put_field(style, "color", V::I(rgb));
    }
    for key in ["insertion"] {
        if let Some(value) = object.get(key).filter(|v| !v.is_null()) {
            let text = value
                .as_str()
                .ok_or_else(|| format!("Style {key} must be a string"))?;
            let stored = ctx.rt.new_str(text);
            ctx.rt.put_field(style, key, stored);
        }
    }
    if let Some(font) = object.get("font").filter(|v| !v.is_null()) {
        let codec = graph(ctx.rt, CODEC, "resource_location", &[]);
        let font = decode_value(ctx, codec, ops, font, false, depth + 1)?;
        ctx.rt.put_field(style, "font", font);
    }
    for key in ["clickEvent", "hoverEvent"] {
        if object.get(key).is_some_and(|v| !v.is_null()) {
            return Err(unsupported_decode(ctx, key));
        }
    }
    Ok(component)
}

pub(super) fn encode(
    ctx: &mut ExecCtx<'_>,
    ops: V,
    value: V,
    compressed: bool,
    depth: usize,
) -> Result<Json, EncodeError> {
    if depth >= 128 {
        return Err("Component nesting exceeds128".into());
    }
    if ctx.rt.class_of(value) != Some(MUTABLE) {
        return Err("Not a supported Component value".into());
    }
    let kind = ctx
        .rt
        .str_of(ctx.rt.get_field(value, "$kind"))
        .ok_or("Component has no content kind")?
        .to_owned();
    let style = ctx.rt.get_field(value, "$style");
    let mut output = Map::new();
    let rgb = ctx.rt.get_field(style, "color");
    if !rgb.is_null() && rgb.i() >= 0 {
        let color = ctx
            .rt
            .str_of(ctx.rt.get_field(style, "$color_name"))
            .filter(|_| ctx.rt.get_field(style, "$named_color_rgb") == rgb)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("#{:06X}", rgb.i() & 0xffffff));
        output.insert("color".into(), Json::String(color));
    }
    for (key, field) in FLAGS {
        let value = ctx.rt.get_field(style, field);
        if !value.is_null() {
            output.insert(key.into(), Json::Bool(value.i() != 0));
        }
    }
    for key in ["insertion"] {
        let value = ctx.rt.get_field(style, key);
        if !value.is_null() {
            output.insert(
                key.into(),
                Json::String(ctx.rt.str_of(value).ok_or("Invalid style string")?.into()),
            );
        }
    }
    let font = ctx.rt.get_field(style, "font");
    if !font.is_null() {
        let location = ctx
            .rt
            .handle_of(font)
            .filter(|(kind, ..)| *kind == "resource_location")
            .map(|(_, _, id, _)| id.to_owned())
            .ok_or("Invalid font resource location")?;
        output.insert("font".into(), Json::String(location));
    }
    for key in ["clickEvent", "hoverEvent"] {
        if !ctx.rt.get_field(style, key).is_null() {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("ComponentSerialization {key} not implemented"),
            );
            return Err("Unsupported style event".into());
        }
    }
    let siblings = array(ctx.rt, ctx.rt.get_field(value, "$siblings"))
        .ok_or("Component has no sibling list")?;
    if !siblings.is_empty() {
        let mut extra = Vec::new();
        for child in siblings {
            extra.push(encode(ctx, ops, child, compressed, depth + 1)?);
        }
        output.insert("extra".into(), Json::Array(extra));
    }
    match kind.as_str() {
        "literal" => {
            let text = ctx
                .rt
                .str_of(ctx.rt.get_field(value, "$text"))
                .ok_or("Literal component has no text")?
                .to_owned();
            if output.is_empty() {
                return Ok(Json::String(text));
            }
            output.insert("text".into(), Json::String(text));
        }
        "translatable" => {
            let key = ctx
                .rt
                .str_of(ctx.rt.get_field(value, "$key"))
                .ok_or("Translatable component has no key")?;
            output.insert("translate".into(), Json::String(key.into()));
            if let Some(fallback) = ctx.rt.str_of(ctx.rt.get_field(value, "$fallback")) {
                output.insert("fallback".into(), Json::String(fallback.into()));
            }
            let args = array(ctx.rt, ctx.rt.get_field(value, "$args"))
                .ok_or("Translatable component has no argument array")?;
            if !args.is_empty() {
                let mut values = Vec::new();
                for arg in args {
                    values.push(if ctx.rt.class_of(arg) == Some(MUTABLE) {
                        encode(ctx, ops, arg, compressed, depth + 1)?
                    } else if let Some(text) = ctx.rt.str_of(arg) {
                        Json::String(text.into())
                    } else if ctx.rt.class_of(arg) == Some("java/lang/Boolean") {
                        Json::Bool(ctx.rt.get_field(arg, "$value").i() != 0)
                    } else {
                        json::numeric_json(ctx.rt, arg).ok_or("Unsupported translation argument")?
                    });
                }
                output.insert("with".into(), Json::Array(values));
            }
        }
        _ => {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Component content {kind} is not implemented"),
            );
            return Err("Unsupported content".into());
        }
    }
    if compressed {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Compressed component object encoding not implemented",
        );
        return Err("Unsupported compressed component".into());
    }
    // Uncompressed legacy JSON omits the typed matcher discriminator.
    Ok(Json::Object(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::NullHost;
    #[test]
    fn translatable_style_arguments_and_siblings_roundtrip_without_flattening() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let ops = json::get_static(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let codec = get_static(
            &mut rt,
            SERIALIZER,
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        let ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let input = serde_json::json!({"translate":"screen.example","fallback":"%s x %s","with":["齿轮",7],"bold":false,"color":"red","extra":[{"text":"!","italic":true}]});
        let json_input = json::from_value(ctx.rt, input.clone());
        let parsed=super::super::call(ctx.rt,ctx.linked,ctx.host,&MemberRef::new(CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),false,&[codec,ops,json_input]).unwrap();
        let component = result::value(ctx.rt, parsed).expect("public Component codec parse");
        assert_eq!(
            crate::minecraft::render_component_text(ctx.rt, &Default::default(), component),
            "齿轮 x 7!"
        );
        let encoded=super::super::call(ctx.rt,ctx.linked,ctx.host,&MemberRef::new(CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),false,&[codec,ops,component]).unwrap();
        let encoded = json::to_value(
            ctx.rt,
            result::value(ctx.rt, encoded).expect("public Component codec encode"),
        )
        .unwrap();
        assert_eq!(encoded, input);
        assert!(ctx.rt.pending_exception.is_none());
    }
    #[test]
    fn plain_strings_collapse_but_invalid_or_unsupported_content_is_not_silently_replaced() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let ops = json::get_static(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let codec = get_static(
            &mut rt,
            SERIALIZER,
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let plain = decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"text":"hello"}),
            false,
            0,
        )
        .unwrap();
        assert_eq!(
            encode_value(&mut ctx, codec, ops, plain, false, 0).unwrap(),
            serde_json::json!("hello")
        );
        assert!(decode_value(&mut ctx, codec, ops, &serde_json::json!([]), false, 0).is_err());
        assert!(decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"text":"x","bold":"yes"}),
            false,
            0
        )
        .is_err());
        assert!(decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"text":"x","font":"Invalid:Font"}),
            false,
            0
        )
        .is_err());
        assert!(decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"text":"x","color":"RED"}),
            false,
            0
        )
        .is_err());
        assert!(decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"selector":"@a"}),
            false,
            0
        )
        .is_err());
        let exception = ctx
            .rt
            .take_exception()
            .expect("valid unsupported selector must throw");
        assert!(ctx
            .rt
            .exception_summary(exception)
            .contains("UnsupportedOperationException"));
    }

    #[test]
    fn style_copy_preserves_font_insertion_and_named_color_without_stale_rgb() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let ops = json::get_static(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let input = serde_json::json!({"text":"x","color":"red","font":"example:font","insertion":"copied"});
        let component = decode(&mut ctx, ops, &input, false, 0).unwrap();
        let copied = crate::minecraft::call_chat(
            ctx.rt,
            &Default::default(),
            &MemberRef::new(
                COMPONENT,
                "copy",
                "()Lnet/minecraft/network/chat/MutableComponent;",
            ),
            false,
            &[component],
        )
        .unwrap();
        assert_eq!(encode(&mut ctx, ops, copied, false, 0).unwrap(), input);
        crate::minecraft::call_chat(
            ctx.rt,
            &Default::default(),
            &MemberRef::new(
                MUTABLE,
                "withColor",
                "(I)Lnet/minecraft/network/chat/MutableComponent;",
            ),
            false,
            &[copied, V::I(0x123456)],
        )
        .unwrap();
        let encoded = encode(&mut ctx, ops, copied, false, 0).unwrap();
        assert_eq!(encoded["color"], serde_json::json!("#123456"));
        assert_eq!(encoded["font"], input["font"]);
        assert_eq!(encoded["insertion"], input["insertion"]);
    }

    #[test]
    fn trusted_context_free_stream_retains_component_codec_and_rejects_unimplemented_nbt() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let stream = get_static(
            &mut rt,
            SERIALIZER,
            "TRUSTED_CONTEXT_FREE_STREAM_CODEC",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )
        .unwrap();
        let component = get_static(
            &mut rt,
            SERIALIZER,
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        assert_eq!(
            array(&rt, rt.get_field(stream, "$codec_args")).unwrap(),
            vec![component]
        );
        let buffer = rt.alloc(
            "io/netty/buffer/ByteBuf",
            0,
            Data::ByteBuf {
                bytes: vec![],
                reader_idx: 0,
            },
        );
        let value = literal(&mut rt, "x");
        crate::codecs::call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(
                "net/minecraft/network/codec/StreamCodec",
                "encode",
                "(Ljava/lang/Object;Ljava/lang/Object;)V",
            ),
            false,
            &[stream, buffer, value],
        )
        .unwrap();
        let error = rt.take_exception().expect("NBT path not implemented");
        assert!(rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert!(
            matches!(&rt.get_obj(buffer).unwrap().data,Data::ByteBuf{bytes,..}if bytes.is_empty())
        );
    }
}

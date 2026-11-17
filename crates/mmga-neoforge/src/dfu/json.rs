use super::*;
use serde_json::Value as Json;

pub(super) fn get_static(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == JSON_OPS
        && matches!(name, "INSTANCE" | "COMPRESSED")
        && desc == "Lcom/mojang/serialization/JsonOps;"
    {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let value = rt.new_instance(JSON_OPS, 0);
        rt.put_field(value, "$compressed", V::bool(name == "COMPRESSED"));
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if owner == "com/google/gson/JsonNull"
        && name == "INSTANCE"
        && desc == "Lcom/google/gson/JsonNull;"
    {
        return Some(from_value(rt, Json::Null));
    }
    None
}

pub(super) fn to_value(rt: &Rt, value: V) -> Option<Json> {
    let raw = rt.get_field(value, "$json");
    match rt.class_of(value)? {
        "com/google/gson/JsonNull" => Some(Json::Null),
        "com/google/gson/JsonPrimitive" => serde_json::from_str(rt.str_of(raw)?).ok(),
        "com/google/gson/JsonArray" => {
            let values = array(rt, raw)?;
            values
                .into_iter()
                .map(|v| to_value(rt, v))
                .collect::<Option<Vec<_>>>()
                .map(Json::Array)
        }
        "com/google/gson/JsonObject" => {
            let Data::Map(entries) = &rt.get_obj(raw)?.data else {
                return None;
            };
            let mut object = serde_json::Map::new();
            for (key, value) in entries {
                object.insert(rt.str_of(*key)?.to_owned(), to_value(rt, *value)?);
            }
            Some(Json::Object(object))
        }
        _ => None,
    }
}

pub(super) fn from_value(rt: &mut Rt, value: Json) -> V {
    if value.is_null() {
        if let Some(value) = rt.get_static("com/google/gson/JsonNull", "INSTANCE") {
            return value;
        }
        let null = rt.new_instance("com/google/gson/JsonNull", 0);
        rt.put_static("com/google/gson/JsonNull", "INSTANCE", null);
        return null;
    }
    let (class, raw) = match value {
        Json::Array(values) => {
            let values = values.into_iter().map(|v| from_value(rt, v)).collect();
            (
                "com/google/gson/JsonArray",
                rt.alloc("java/util/ArrayList", 0, Data::List(values)),
            )
        }
        Json::Object(values) => {
            let values = values
                .into_iter()
                .map(|(k, v)| (rt.new_str(k), from_value(rt, v)))
                .collect();
            (
                "com/google/gson/JsonObject",
                rt.alloc("java/util/LinkedHashMap", 0, Data::Map(values)),
            )
        }
        value => (
            "com/google/gson/JsonPrimitive",
            rt.new_str(value.to_string()),
        ),
    };
    let object = rt.new_instance(class, 0);
    rt.put_field(object, "$json", raw);
    object
}

pub(super) fn compressed(rt: &Rt, ops: V) -> Option<bool> {
    (rt.class_of(ops) == Some(JSON_OPS)).then(|| rt.get_field(ops, "$compressed") == V::TRUE)
}

pub(super) fn call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == "com/google/gson/JsonParser"
        && is_static
        && name == "parseString"
        && desc == "(Ljava/lang/String;)Lcom/google/gson/JsonElement;"
    {
        let text = ctx.rt.str_of(*args.first()?)?.to_owned();
        return Some(match serde_json::from_str(&text) {
            Ok(value) => from_value(ctx.rt, value),
            Err(error) => ctx
                .rt
                .throw_exception("com/google/gson/JsonSyntaxException", &error.to_string()),
        });
    }
    if !is_static && matches!(owner, JSON_OPS | "com/mojang/serialization/DynamicOps") {
        let compressed = compressed(ctx.rt, *args.first()?)?;
        return match (name, desc) {
            ("compressMaps", "()Z") => Some(V::bool(compressed)),
            ("empty", "()Ljava/lang/Object;" | "()Lcom/google/gson/JsonElement;") => {
                Some(from_value(ctx.rt, Json::Null))
            }
            ("emptyMap", "()Ljava/lang/Object;") => Some(from_value(ctx.rt, serde_json::json!({}))),
            ("emptyList", "()Ljava/lang/Object;") => {
                Some(from_value(ctx.rt, serde_json::json!([])))
            }
            (
                "createString",
                "(Ljava/lang/String;)Ljava/lang/Object;"
                | "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ) => Some(from_value(
                ctx.rt,
                Json::String(ctx.rt.str_of(*args.get(1)?)?.to_owned()),
            )),
            ("createBoolean", "(Z)Ljava/lang/Object;" | "(Z)Lcom/google/gson/JsonElement;") => {
                Some(from_value(ctx.rt, Json::Bool(args.get(1)?.i() != 0)))
            }
            (
                "createByte" | "createShort" | "createInt",
                "(B)Ljava/lang/Object;" | "(S)Ljava/lang/Object;" | "(I)Ljava/lang/Object;",
            ) => Some(from_value(ctx.rt, Json::from(args.get(1)?.i()))),
            ("createLong", "(J)Ljava/lang/Object;") => {
                Some(from_value(ctx.rt, Json::from(args.get(1)?.j())))
            }
            ("createFloat", "(F)Ljava/lang/Object;") => {
                Some(from_value(ctx.rt, Json::from(args.get(1)?.f())))
            }
            ("createDouble", "(D)Ljava/lang/Object;") => {
                Some(from_value(ctx.rt, Json::from(args.get(1)?.d())))
            }
            (
                "getNumberValue" | "getStringValue" | "getBooleanValue",
                "(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"
                | "(Lcom/google/gson/JsonElement;)Lcom/mojang/serialization/DataResult;",
            ) => {
                let input = to_value(ctx.rt, *args.get(1)?)?;
                let decoded = primitive_read(
                    ctx.rt,
                    match name {
                        "getNumberValue" => "NUMBER",
                        "getStringValue" => "STRING",
                        _ => "BOOL",
                    },
                    &input,
                    compressed,
                );
                Some(match decoded {
                    Ok(value) => result::success(ctx.rt, value),
                    Err(message) => result::error(ctx.rt, &message, None),
                })
            }
            _ => None,
        };
    }
    if !is_static && owner.starts_with("com/google/gson/Json") {
        let recv = *args.first()?;
        if name == "<init>" {
            let value = match (owner, desc) {
                ("com/google/gson/JsonObject", "()V") => Json::Object(Default::default()),
                ("com/google/gson/JsonArray", "()V" | "(I)V") => Json::Array(Vec::new()),
                ("com/google/gson/JsonPrimitive", "(Ljava/lang/String;)V") => {
                    Json::String(ctx.rt.str_of(*args.get(1)?)?.to_owned())
                }
                ("com/google/gson/JsonPrimitive", "(Ljava/lang/Boolean;)V") => {
                    Json::Bool(ctx.rt.get_field(*args.get(1)?, "$value").i() != 0)
                }
                ("com/google/gson/JsonPrimitive", "(Ljava/lang/Number;)V") => {
                    numeric_json(ctx.rt, *args.get(1)?)?
                }
                _ => return None,
            };
            let source = from_value(ctx.rt, value);
            let raw = ctx.rt.get_field(source, "$json");
            ctx.rt.put_field(recv, "$json", raw);
            return Some(V::Void);
        }
        let value = to_value(ctx.rt, recv)?;
        return match (name, desc) {
            ("toString", "()Ljava/lang/String;") => Some(ctx.rt.new_str(value.to_string())),
            (
                "deepCopy",
                "()Lcom/google/gson/JsonElement;"
                | "()Lcom/google/gson/JsonObject;"
                | "()Lcom/google/gson/JsonArray;",
            ) => Some(from_value(ctx.rt, value)),
            ("isJsonNull", "()Z") => Some(V::bool(value.is_null())),
            ("isJsonArray", "()Z") => Some(V::bool(value.is_array())),
            ("isJsonObject", "()Z") => Some(V::bool(value.is_object())),
            ("isJsonPrimitive", "()Z") => Some(V::bool(
                !value.is_null() && !value.is_array() && !value.is_object(),
            )),
            ("isString", "()Z") => Some(V::bool(value.is_string())),
            ("isBoolean", "()Z") => Some(V::bool(value.is_boolean())),
            ("isNumber", "()Z") => Some(V::bool(value.is_number())),
            ("getAsString", "()Ljava/lang/String;")
                if value.is_string() || value.is_boolean() || value.is_number() =>
            {
                Some(
                    ctx.rt.new_str(
                        value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    ),
                )
            }
            ("getAsInt", "()I") => Some(V::I(
                value.as_i64().or_else(|| value.as_str()?.parse().ok())? as i32,
            )),
            ("getAsLong", "()J") => Some(V::J(
                value.as_i64().or_else(|| value.as_str()?.parse().ok())?,
            )),
            ("getAsBoolean", "()Z")
                if value.is_string() || value.is_boolean() || value.is_number() =>
            {
                Some(V::bool(value.as_bool().unwrap_or_else(|| {
                    value
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case("true"))
                })))
            }
            ("getAsJsonObject", "()Lcom/google/gson/JsonObject;") if value.is_object() => {
                Some(recv)
            }
            ("getAsJsonArray", "()Lcom/google/gson/JsonArray;") if value.is_array() => Some(recv),
            ("getAsJsonPrimitive", "()Lcom/google/gson/JsonPrimitive;")
                if value.is_string() || value.is_boolean() || value.is_number() =>
            {
                Some(recv)
            }
            ("size", "()I") => Some(V::I(match &value {
                Json::Array(v) => v.len(),
                Json::Object(v) => v.len(),
                _ => return None,
            } as i32)),
            ("get", "(Ljava/lang/String;)Lcom/google/gson/JsonElement;") => {
                let key = ctx.rt.str_of(*args.get(1)?)?;
                let raw = ctx.rt.get_field(recv, "$json");
                let Some(Data::Map(entries)) = ctx.rt.get_obj(raw).map(|o| &o.data) else {
                    return None;
                };
                Some(
                    entries
                        .iter()
                        .find(|(k, _)| ctx.rt.str_of(*k) == Some(key))
                        .map(|(_, v)| *v)
                        .unwrap_or(V::NULL),
                )
            }
            ("get", "(I)Lcom/google/gson/JsonElement;") => Some(
                array(ctx.rt, ctx.rt.get_field(recv, "$json"))?
                    .get(args[1].i() as usize)
                    .copied()
                    .unwrap_or_else(|| {
                        ctx.rt.throw_exception(
                            "java/lang/IndexOutOfBoundsException",
                            "JsonArray index out of bounds",
                        )
                    }),
            ),
            ("add", "(Ljava/lang/String;Lcom/google/gson/JsonElement;)V") => {
                let key = *args.get(1)?;
                let mut val = *args.get(2)?;
                if val.is_null() {
                    val = from_value(ctx.rt, Json::Null);
                }
                let raw = ctx.rt.get_field(recv, "$json");
                let key_text = ctx.rt.str_of(key)?.to_owned();
                let existing = match ctx.rt.get_obj(raw).map(|o| &o.data) {
                    Some(Data::Map(entries)) => entries
                        .iter()
                        .position(|(k, _)| ctx.rt.str_of(*k) == Some(&key_text)),
                    _ => return None,
                };
                if let Some(Data::Map(entries)) = ctx.rt.get_obj_mut(raw).map(|o| &mut o.data) {
                    if let Some(i) = existing {
                        entries[i].1 = val
                    } else {
                        entries.push((key, val));
                    }
                }
                Some(V::Void)
            }
            ("add", "(Lcom/google/gson/JsonElement;)V") => {
                let mut val = *args.get(1)?;
                if val.is_null() {
                    val = from_value(ctx.rt, Json::Null);
                }
                let raw = ctx.rt.get_field(recv, "$json");
                if let Some(Data::List(values)) = ctx.rt.get_obj_mut(raw).map(|o| &mut o.data) {
                    values.push(val);
                    Some(V::Void)
                } else {
                    None
                }
            }
            _ => None,
        };
    }
    None
}

pub(super) fn numeric_json(rt: &Rt, value: V) -> Option<Json> {
    let raw = if matches!(value, V::R(_)) {
        rt.get_field(value, "$value")
    } else {
        value
    };
    match raw {
        V::I(v) => Some(Json::from(v)),
        V::J(v) => Some(Json::from(v)),
        V::F(v) if v.is_finite() => Some(Json::from(v)),
        V::D(v) if v.is_finite() => Some(Json::from(v)),
        _ => None,
    }
}

pub(super) fn primitive_read(
    rt: &mut Rt,
    kind: &str,
    input: &Json,
    compressed: bool,
) -> Result<V, String> {
    if kind == "STRING" {
        return input
            .as_str()
            .map(|s| rt.new_str(s))
            .or_else(|| (compressed && input.is_number()).then(|| rt.new_str(input.to_string())))
            .ok_or_else(|| format!("Not a string: {input}"));
    }
    if kind == "BOOL" {
        return input
            .as_bool()
            .map(|v| box_primitive(rt, "java/lang/Boolean", V::bool(v)))
            .ok_or_else(|| format!("Not a boolean: {input}"));
    }
    let number = if input.is_number() {
        input.clone()
    } else if compressed {
        input
            .as_str()
            .and_then(|s| s.parse::<i32>().ok())
            .map(Json::from)
            .ok_or_else(|| format!("Not a number: {input}"))?
    } else {
        return Err(format!("Not a number: {input}"));
    };
    let double = number
        .as_f64()
        .ok_or_else(|| format!("Not a number: {input}"))?;
    let long = number.as_i64().unwrap_or(double as i64);
    let (class, raw) = match kind {
        "BYTE" => ("java/lang/Byte", V::I(long as i8 as i32)),
        "SHORT" => ("java/lang/Short", V::I(long as i16 as i32)),
        "INT" => ("java/lang/Integer", V::I(long as i32)),
        "LONG" => ("java/lang/Long", V::J(long)),
        "FLOAT" => ("java/lang/Float", V::F(double as f32)),
        "DOUBLE" => ("java/lang/Double", V::D(double)),
        "NUMBER" => {
            if number.is_i64() {
                ("java/lang/Long", V::J(long))
            } else {
                ("java/lang/Double", V::D(double))
            }
        }
        _ => return Err(format!("Unsupported primitive {kind}")),
    };
    Ok(box_primitive(rt, class, raw))
}

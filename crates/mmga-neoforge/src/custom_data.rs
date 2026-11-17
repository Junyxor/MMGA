//! Typed NBT values and CustomData copy semantics required by real data-component codecs.
//! Binary NBT, SNBT parsing, entity/block loading, and general NbtOps remain unsupported.
use mmga_classfile::{op, MemberRef};
use mmga_runtime::{
    invoke_member, ClosureData, ClosureTarget, Data, ExecCtx, HostApi, LinkedSpace, Rt, V,
};
use serde_json::Value as Json;

const CUSTOM: &str = "net/minecraft/world/item/component/CustomData";
const COMPOUND: &str = "net/minecraft/nbt/CompoundTag";
const CALLBACK: &str = "mmga/internal/CustomDataCodec";

const TAG_CLASSES: [&str; 13] = [
    "net/minecraft/nbt/EndTag",
    "net/minecraft/nbt/ByteTag",
    "net/minecraft/nbt/ShortTag",
    "net/minecraft/nbt/IntTag",
    "net/minecraft/nbt/LongTag",
    "net/minecraft/nbt/FloatTag",
    "net/minecraft/nbt/DoubleTag",
    "net/minecraft/nbt/ByteArrayTag",
    "net/minecraft/nbt/StringTag",
    "net/minecraft/nbt/ListTag",
    COMPOUND,
    "net/minecraft/nbt/IntArrayTag",
    "net/minecraft/nbt/LongArrayTag",
];

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    let valid = (owner == CUSTOM
        && matches!(
            (name, desc),
            ("EMPTY", "Lnet/minecraft/world/item/component/CustomData;")
                | (
                    "CODEC" | "CODEC_WITH_ID",
                    "Lcom/mojang/serialization/Codec;"
                )
                | ("STREAM_CODEC", "Lnet/minecraft/network/codec/StreamCodec;")
        ))
        || (owner == COMPOUND && name == "CODEC" && desc == "Lcom/mojang/serialization/Codec;");
    if !valid {
        return None;
    }
    if let Some(v) = rt.get_static(owner, name) {
        return Some(v);
    }
    let value = if name == "EMPTY" {
        let tag = compound(rt, vec![]);
        wrap(rt, tag)
    } else if name == "STREAM_CODEC" {
        let base = crate::codecs::get_static_field(
            rt,
            "net/minecraft/network/codec/ByteBufCodecs",
            "COMPOUND_TAG",
            "Lnet/minecraft/network/codec/StreamCodec;",
        )?;
        let from = callback(
            rt,
            "wrap",
            "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/world/item/component/CustomData;",
        );
        let to = callback(
            rt,
            "unwrap",
            "(Lnet/minecraft/world/item/component/CustomData;)Lnet/minecraft/nbt/CompoundTag;",
        );
        crate::codecs::call(rt,&LinkedSpace::new(),&mut mmga_runtime::NullHost,&MemberRef::new("net/minecraft/network/codec/StreamCodec","map","(Ljava/util/function/Function;Ljava/util/function/Function;)Lnet/minecraft/network/codec/StreamCodec;"),false,&[base,from,to])?
    } else {
        crate::dfu::custom_data_codec(rt, owner == CUSTOM, name == "CODEC_WITH_ID")
    };
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
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == CALLBACK && is_static {
        return match (name, desc) {
            (
                "wrap",
                "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/world/item/component/CustomData;",
            ) => Some(wrap(rt, args[0])),
            (
                "unwrap",
                "(Lnet/minecraft/world/item/component/CustomData;)Lnet/minecraft/nbt/CompoundTag;",
            ) => Some(rt.get_field(args[0], "tag")),
            _ => None,
        };
    }
    if owner == CUSTOM {
        if is_static&&name=="of"&&desc=="(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/world/item/component/CustomData;"{let copy=copy_tag(rt,args[0]);if rt.pending_exception.is_some(){return Some(V::NULL);}return Some(wrap(rt,copy));}
        if is_static {
            return None;
        }
        let recv = *args.first()?;
        let tag = rt.get_field(recv, "tag");
        return match (name, desc) {
            ("<init>", "(Lnet/minecraft/nbt/CompoundTag;)V") => {
                rt.put_field(recv, "tag", args[1]);
                Some(V::Void)
            }
            ("copyTag", "()Lnet/minecraft/nbt/CompoundTag;") => Some(copy_tag(rt, tag)),
            ("getUnsafe", "()Lnet/minecraft/nbt/CompoundTag;") => Some(tag),
            ("size", "()I") => Some(V::I(entries(rt, tag)?.len() as i32)),
            ("isEmpty", "()Z") => Some(V::bool(entries(rt, tag)?.is_empty())),
            ("contains", "(Ljava/lang/String;)Z") => {
                Some(V::bool(lookup(rt, tag, rt.str_of(args[1])?).is_some()))
            }
            (
                "update",
                "(Ljava/util/function/Consumer;)Lnet/minecraft/world/item/component/CustomData;",
            ) => {
                let copy = copy_tag(rt, tag);
                if rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host,
                    depth: 0,
                };
                invoke_member(
                    &mut ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Consumer",
                        "accept",
                        "(Ljava/lang/Object;)V",
                    ),
                    &[args[1], copy],
                );
                Some(if ctx.rt.pending_exception.is_some() {
                    V::NULL
                } else {
                    wrap(ctx.rt, copy)
                })
            }
            ("equals", "(Ljava/lang/Object;)Z") => Some(V::bool(
                rt.class_of(args[1]) == Some(CUSTOM)
                    && tag_equal(rt, tag, rt.get_field(args[1], "tag")),
            )),
            _ => None,
        };
    }
    if owner == COMPOUND && !is_static {
        let recv = *args.first()?;
        if name == "<init>" && desc == "()V" {
            let map = rt.alloc("java/util/HashMap", 0, Data::Map(Vec::new()));
            rt.put_field(recv, "tags", map);
            rt.put_field(recv, "$tag_id", V::I(10));
            return Some(V::Void);
        }
        let items = entries(rt, recv)?;
        match (name, desc) {
            ("getId", "()B") => return Some(V::I(10)),
            ("size", "()I") => return Some(V::I(items.len() as i32)),
            ("isEmpty", "()Z") => return Some(V::bool(items.is_empty())),
            ("copy", "()Lnet/minecraft/nbt/CompoundTag;" | "()Lnet/minecraft/nbt/Tag;") => {
                return Some(copy_tag(rt, recv))
            }
            ("get", "(Ljava/lang/String;)Lnet/minecraft/nbt/Tag;") => {
                return Some(lookup(rt, recv, rt.str_of(args[1])?).unwrap_or(V::NULL))
            }
            ("contains", "(Ljava/lang/String;)Z") => {
                return Some(V::bool(lookup(rt, recv, rt.str_of(args[1])?).is_some()))
            }
            ("contains", "(Ljava/lang/String;I)Z") => {
                let id = lookup(rt, recv, rt.str_of(args[1])?).and_then(|v| tag_id(rt, v));
                let requested = args[2].i();
                return Some(V::bool(if requested == 99 {
                    id.is_some_and(|id| (1..=6).contains(&id))
                } else {
                    id == Some(requested)
                }));
            }
            ("getTagType", "(Ljava/lang/String;)B") => {
                return Some(V::I(
                    lookup(rt, recv, rt.str_of(args[1])?)
                        .and_then(|v| tag_id(rt, v))
                        .unwrap_or(0),
                ))
            }
            ("put", "(Ljava/lang/String;Lnet/minecraft/nbt/Tag;)Lnet/minecraft/nbt/Tag;") => {
                return Some(put(rt, recv, args[1], args[2]))
            }
            ("remove", "(Ljava/lang/String;)V") => {
                let key = rt.str_of(args[1])?.to_owned();
                let map = rt.get_field(recv, "tags");
                let pos = items.iter().position(|(k, _)| rt.str_of(*k) == Some(&key));
                if let (Some(pos), Some(Data::Map(entries))) =
                    (pos, rt.get_obj_mut(map).map(|o| &mut o.data))
                {
                    entries.remove(pos);
                }
                return Some(V::Void);
            }
            ("equals", "(Ljava/lang/Object;)Z") => {
                return Some(V::bool(tag_equal(rt, recv, args[1])))
            }
            _ => {}
        }
        let spec = match (name, desc) {
            ("putByte", "(Ljava/lang/String;B)V") => Some((1, args[2])),
            ("putShort", "(Ljava/lang/String;S)V") => Some((2, args[2])),
            ("putInt", "(Ljava/lang/String;I)V") => Some((3, args[2])),
            ("putLong", "(Ljava/lang/String;J)V") => Some((4, args[2])),
            ("putFloat", "(Ljava/lang/String;F)V") => Some((5, args[2])),
            ("putDouble", "(Ljava/lang/String;D)V") => Some((6, args[2])),
            ("putString", "(Ljava/lang/String;Ljava/lang/String;)V") => Some((8, args[2])),
            ("putBoolean", "(Ljava/lang/String;Z)V") => Some((1, V::bool(args[2].i() != 0))),
            _ => None,
        };
        if let Some((id, value)) = spec {
            let value = scalar(rt, id, value);
            put(rt, recv, args[1], value);
            return Some(if rt.pending_exception.is_some() {
                V::NULL
            } else {
                V::Void
            });
        }
        if matches!(
            (name, desc),
            ("getInt", "(Ljava/lang/String;)I")
                | ("getLong", "(Ljava/lang/String;)J")
                | ("getByte", "(Ljava/lang/String;)B")
                | ("getShort", "(Ljava/lang/String;)S")
                | ("getFloat", "(Ljava/lang/String;)F")
                | ("getDouble", "(Ljava/lang/String;)D")
                | ("getBoolean", "(Ljava/lang/String;)Z")
        ) {
            let child = lookup(rt, recv, rt.str_of(args[1])?);
            let numeric = child.filter(|v| tag_id(rt, *v).is_some_and(|id| (1..=6).contains(&id)));
            let raw = numeric.map(|v| rt.get_field(v, "data")).unwrap_or(V::I(0));
            let n = number_f64(raw);
            let long = match raw {
                V::I(v) => v as i64,
                V::J(v) => v,
                V::F(v) => v as i64,
                V::D(v) => v.floor() as i64,
                _ => 0,
            };
            let int = match raw {
                V::F(_) | V::D(_) => {
                    let truncated = n as i32;
                    if n < (truncated as f64) {
                        truncated.wrapping_sub(1)
                    } else {
                        truncated
                    }
                }
                _ => long as i32,
            };
            return Some(match name {
                "getLong" => V::J(long),
                "getFloat" => V::F(n as f32),
                "getDouble" => V::D(n),
                "getByte" => V::I(int as i8 as i32),
                "getShort" => V::I(int as i16 as i32),
                "getBoolean" => V::bool(int as i8 != 0),
                _ => V::I(int),
            });
        }
        if name == "getString" && desc == "(Ljava/lang/String;)Ljava/lang/String;" {
            let v = lookup(rt, recv, rt.str_of(args[1])?)
                .filter(|v| tag_id(rt, *v) == Some(8))
                .map(|v| rt.get_field(v, "data"))
                .unwrap_or_else(|| rt.intern_str(""));
            return Some(v);
        }
        if name == "getCompound" && desc == "(Ljava/lang/String;)Lnet/minecraft/nbt/CompoundTag;" {
            return Some(
                lookup(rt, recv, rt.str_of(args[1])?)
                    .filter(|v| tag_id(rt, *v) == Some(10))
                    .unwrap_or_else(|| compound(rt, vec![])),
            );
        }
    }
    if !is_static && TAG_CLASSES.contains(&owner) {
        let recv = *args.first()?;
        return match (name, desc) {
            ("getId", "()B") => Some(V::I(tag_id(rt, recv)?)),
            ("copy", "()Lnet/minecraft/nbt/Tag;") => Some(copy_tag(rt, recv)),
            _ => None,
        };
    }
    None
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    (TAG_CLASSES.contains(&sub) && sup == "net/minecraft/nbt/Tag")
        || (TAG_CLASSES[1..=6].contains(&sub) && sup == "net/minecraft/nbt/NumericTag")
}

pub fn decode_json(rt: &mut Rt, input: &Json, custom: bool, require_id: bool) -> Result<V, String> {
    if custom && input.is_string() {
        rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "CustomData SNBT string parsing is not implemented",
        );
        return Err("Unsupported SNBT input".into());
    }
    if !input.is_object() {
        return Err(format!("Not a compound tag: {input}"));
    }
    let tag = from_json(rt, input, 0)?;
    if require_id && !lookup(rt, tag, "id").is_some_and(|v| tag_id(rt, v) == Some(8)) {
        return Err("CustomData requires string id".into());
    }
    Ok(if custom { wrap(rt, tag) } else { tag })
}
pub fn encode_json(rt: &mut Rt, value: V, custom: bool, require_id: bool) -> Result<Json, String> {
    let tag = if custom {
        if rt.class_of(value) != Some(CUSTOM) {
            return Err("Not CustomData".into());
        }
        rt.get_field(value, "tag")
    } else {
        value
    };
    if tag_id(rt, tag) != Some(10) {
        return Err("Not a compound tag".into());
    }
    if require_id && !lookup(rt, tag, "id").is_some_and(|v| tag_id(rt, v) == Some(8)) {
        return Err("CustomData requires string id".into());
    }
    to_json(rt, tag, 0)
}

fn scalar(rt: &mut Rt, id: usize, data: V) -> V {
    let tag = rt.new_instance(TAG_CLASSES[id], 0);
    rt.put_field(tag, "$tag_id", V::I(id as i32));
    rt.put_field(tag, "data", data);
    tag
}
fn compound(rt: &mut Rt, entries: Vec<(V, V)>) -> V {
    let tag = rt.new_instance(COMPOUND, 0);
    let map = rt.alloc("java/util/HashMap", 0, Data::Map(entries));
    rt.put_field(tag, "$tag_id", V::I(10));
    rt.put_field(tag, "tags", map);
    tag
}
fn wrap(rt: &mut Rt, tag: V) -> V {
    let value = rt.new_instance(CUSTOM, 0);
    rt.put_field(value, "tag", tag);
    value
}
fn entries(rt: &Rt, tag: V) -> Option<Vec<(V, V)>> {
    match rt.get_obj(rt.get_field(tag, "tags")).map(|o| &o.data) {
        Some(Data::Map(entries)) => Some(entries.clone()),
        _ => None,
    }
}
fn put(rt: &mut Rt, tag: V, key: V, value: V) -> V {
    let Some(text) = rt.str_of(key).map(str::to_owned) else {
        return rt.throw_exception("java/lang/NullPointerException", "CompoundTag key is null");
    };
    let map = rt.get_field(tag, "tags");
    let pos = entries(rt, tag).and_then(|entries| {
        entries
            .iter()
            .position(|(k, _)| rt.str_of(*k) == Some(&text))
    });
    if let Some(Data::Map(entries)) = rt.get_obj_mut(map).map(|o| &mut o.data) {
        if let Some(pos) = pos {
            let old = entries[pos].1;
            entries[pos].1 = value;
            old
        } else {
            entries.push((key, value));
            V::NULL
        }
    } else {
        rt.throw_exception(
            "java/lang/IllegalStateException",
            "Uninitialized CompoundTag",
        )
    }
}
fn number_f64(value: V) -> f64 {
    match value {
        V::I(v) => v as f64,
        V::J(v) => v as f64,
        V::F(v) => v as f64,
        V::D(v) => v,
        _ => 0.0,
    }
}
fn copy_tag(rt: &mut Rt, tag: V) -> V {
    if tag.is_null() {
        return rt.throw_exception("java/lang/NullPointerException", "Cannot copy null tag");
    }
    match tag_id(rt, tag) {
        Some(10) => {
            let Some(items) = entries(rt, tag) else {
                return rt
                    .throw_exception("java/lang/IllegalStateException", "Uninitialized compound");
            };
            let items = items
                .into_iter()
                .map(|(k, v)| (k, copy_tag(rt, v)))
                .collect();
            compound(rt, items)
        }
        Some(id @ (7 | 9 | 11 | 12)) => {
            let data = rt.get_field(tag, "data");
            let Some(Data::Array(items)) = rt.get_obj(data).map(|o| &o.data) else {
                return rt
                    .throw_exception("java/lang/IllegalStateException", "Uninitialized NBT array");
            };
            let items = items.clone();
            let items = if id == 9 {
                items.into_iter().map(|v| copy_tag(rt, v)).collect()
            } else {
                items
            };
            let data = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(items));
            scalar(rt, id as usize, data)
        }
        Some(_) => tag,
        None => rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Unknown NBT tag cannot be copied",
        ),
    }
}
fn tag_equal(rt: &Rt, a: V, b: V) -> bool {
    if a == b {
        return true;
    }
    if tag_id(rt, a) != tag_id(rt, b) {
        return false;
    }
    match tag_id(rt, a) {
        Some(10) => {
            let (Some(a), Some(b)) = (entries(rt, a), entries(rt, b)) else {
                return false;
            };
            a.len() == b.len()
                && a.iter().all(|(k, v)| {
                    b.iter()
                        .find(|(key, _)| rt.val_eq(*key, *k))
                        .is_some_and(|(_, value)| tag_equal(rt, *v, *value))
                })
        }
        Some(7 | 9 | 11 | 12) => {
            let (Some(Data::Array(a)), Some(Data::Array(b))) = (
                rt.get_obj(rt.get_field(a, "data")).map(|o| &o.data),
                rt.get_obj(rt.get_field(b, "data")).map(|o| &o.data),
            ) else {
                return false;
            };
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    if matches!(a, V::R(_)) {
                        tag_equal(rt, *a, *b)
                    } else {
                        a == b
                    }
                })
        }
        Some(_) => rt.val_eq(rt.get_field(a, "data"), rt.get_field(b, "data")),
        None => false,
    }
}

fn from_json(rt: &mut Rt, input: &Json, depth: usize) -> Result<V, String> {
    if depth > 128 {
        return Err("NBT nesting exceeds128".into());
    }
    Ok(match input {
        Json::Null => scalar(rt, 0, V::Void),
        Json::Bool(v) => scalar(rt, 1, V::bool(*v)),
        Json::String(s) => {
            let s = rt.new_str(s);
            scalar(rt, 8, s)
        }
        Json::Number(n) => {
            let value = n.as_f64().ok_or("Invalid numeric tag")?;
            let integral = n.as_i64().or_else(|| {
                (value.fract() == 0.0 && value >= i64::MIN as f64 && value < i64::MAX as f64)
                    .then_some(value as i64)
            });
            if let Some(value) = integral {
                if let Ok(value) = i8::try_from(value) {
                    scalar(rt, 1, V::I(value as i32))
                } else if let Ok(value) = i16::try_from(value) {
                    scalar(rt, 2, V::I(value as i32))
                } else if let Ok(value) = i32::try_from(value) {
                    scalar(rt, 3, V::I(value))
                } else {
                    scalar(rt, 4, V::J(value))
                }
            } else if (value as f32) as f64 == value {
                scalar(rt, 5, V::F(value as f32))
            } else {
                scalar(rt, 6, V::D(value))
            }
        }
        Json::Object(values) => {
            let mut items = Vec::new();
            for (k, v) in values {
                let key = rt.new_str(k);
                let value = from_json(rt, v, depth + 1)?;
                items.push((key, value));
            }
            compound(rt, items)
        }
        Json::Array(values) => {
            let mut items = Vec::new();
            for value in values {
                items.push(from_json(rt, value, depth + 1)?);
            }
            let first = items.first().and_then(|v| tag_id(rt, *v));
            let homogeneous =
                first.is_some_and(|id| items.iter().all(|v| tag_id(rt, *v) == Some(id)));
            let (id, data) = if homogeneous && matches!(first, Some(1 | 3 | 4)) {
                let array_type = match first.unwrap() {
                    1 => 7,
                    3 => 11,
                    _ => 12,
                };
                let data = items.iter().map(|v| rt.get_field(*v, "data")).collect();
                (array_type, data)
            } else {
                if !homogeneous || first == Some(10) {
                    for item in &mut items {
                        let is_compound = tag_id(rt, *item) == Some(10);
                        let has_empty = is_compound
                            && entries(rt, *item).is_some_and(|entries| {
                                entries.len() == 1 && rt.str_of(entries[0].0) == Some("")
                            });
                        if !is_compound || has_empty {
                            let key = rt.intern_str("");
                            *item = compound(rt, vec![(key, *item)]);
                        }
                    }
                }
                (9, items)
            };
            let data = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(data));
            scalar(rt, id, data)
        }
    })
}
fn to_json(rt: &mut Rt, tag: V, depth: usize) -> Result<Json, String> {
    if depth > 128 {
        return Err("NBT nesting exceeds128".into());
    }
    let data = rt.get_field(tag, "data");
    match tag_id(rt, tag) {
        Some(0) => Ok(Json::Null),
        Some(1 | 2 | 3) => Ok(Json::from(data.i())),
        Some(4) => Ok(Json::from(data.j())),
        Some(5) if data.f().is_finite() => Ok(Json::from(data.f())),
        Some(6) if data.d().is_finite() => Ok(Json::from(data.d())),
        Some(5 | 6) => {
            rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Nonfinite NBT to JsonPrimitive conversion is not implemented",
            );
            Err("Nonfinite JSON number".into())
        }
        Some(8) => rt
            .str_of(data)
            .map(|v| Json::String(v.into()))
            .ok_or("Invalid StringTag".into()),
        Some(10) => {
            let mut out = serde_json::Map::new();
            for (k, v) in entries(rt, tag).ok_or("Invalid CompoundTag")? {
                let key = rt.str_of(k).ok_or("Invalid compound key")?.to_owned();
                out.insert(key, to_json(rt, v, depth + 1)?);
            }
            Ok(Json::Object(out))
        }
        Some(id @ (7 | 9 | 11 | 12)) => {
            let Some(Data::Array(values)) = rt.get_obj(data).map(|o| &o.data) else {
                return Err("Invalid array/list tag".into());
            };
            let values = values.clone();
            let mut out = Vec::new();
            for value in values {
                out.push(if id == 9 {
                    let unwrapped = if tag_id(rt, value) == Some(10) {
                        let entries = entries(rt, value).unwrap();
                        if entries.len() == 1 && rt.str_of(entries[0].0) == Some("") {
                            entries[0].1
                        } else {
                            value
                        }
                    } else {
                        value
                    };
                    to_json(rt, unwrapped, depth + 1)?
                } else if id == 12 {
                    Json::from(value.j())
                } else {
                    Json::from(value.i())
                });
            }
            Ok(Json::Array(out))
        }
        _ => Err("Unknown NBT tag".into()),
    }
}
fn callback(rt: &mut Rt, name: &str, desc: &str) -> V {
    rt.alloc(
        "java/util/function/Function",
        0,
        Data::Closure(Box::new(ClosureData {
            interface_class: "java/util/function/Function".into(),
            sam_name: "apply".into(),
            sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
            instantiated_desc: desc.into(),
            captures: vec![],
            target: ClosureTarget::MethodHandle {
                handle_kind: 6,
                member: MemberRef::new(CALLBACK, name, desc),
            },
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::NullHost;
    #[test]
    fn custom_data_json_preserves_nbt_numeric_kinds_and_roundtrips_compounds() {
        let mut rt = Rt::new();
        let input = serde_json::json!({"byte":1,"short":128,"int":32768,"long":2147483648i64,"float":0.5,"double":0.1,"boolean":true,"nested":{"text":"x"},"bytes":[1,2,3]});
        let custom = decode_json(&mut rt, &input, true, false).unwrap();
        let tag = rt.get_field(custom, "tag");
        for (key, id) in [
            ("byte", 1),
            ("short", 2),
            ("int", 3),
            ("long", 4),
            ("float", 5),
            ("double", 6),
            ("boolean", 1),
            ("nested", 10),
            ("bytes", 7),
        ] {
            let child = lookup(&rt, tag, key).unwrap();
            assert_eq!(tag_id(&rt, child), Some(id), "{key}");
        }
        let output = encode_json(&mut rt, custom, true, false).unwrap();
        let mut expected = input;
        expected["boolean"] = serde_json::json!(1);
        assert_eq!(output, expected);
    }
    #[test]
    fn custom_data_of_and_copy_tag_deep_copy_while_get_unsafe_returns_stored_reference() {
        let mut rt = Rt::new();
        let source = decode_json(
            &mut rt,
            &serde_json::json!({"nested":{"value":1}}),
            false,
            false,
        )
        .unwrap();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let custom = call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(
                CUSTOM,
                "of",
                "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/world/item/component/CustomData;",
            ),
            true,
            &[source],
        )
        .unwrap();
        let stored = rt.get_field(custom, "tag");
        assert_ne!(source, stored);
        assert_ne!(lookup(&rt, source, "nested"), lookup(&rt, stored, "nested"));
        let got = call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(CUSTOM, "getUnsafe", "()Lnet/minecraft/nbt/CompoundTag;"),
            false,
            &[custom],
        )
        .unwrap();
        assert_eq!(got, stored);
        let copied = call(
            &mut rt,
            &linked,
            &mut host,
            &MemberRef::new(CUSTOM, "copyTag", "()Lnet/minecraft/nbt/CompoundTag;"),
            false,
            &[custom],
        )
        .unwrap();
        assert_ne!(copied, stored);
        assert_eq!(
            encode_json(&mut rt, copied, false, false).unwrap(),
            serde_json::json!({"nested":{"value":1}})
        );
    }

    #[test]
    fn custom_data_codec_public_json_path_and_snbt_or_nonfinite_fail_explicitly() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let codec =
            get_static_field(&mut rt, CUSTOM, "CODEC", "Lcom/mojang/serialization/Codec;").unwrap();
        let id_codec = get_static_field(
            &mut rt,
            CUSTOM,
            "CODEC_WITH_ID",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        let ops = crate::dfu::get_static_field(
            &mut rt,
            "com/mojang/serialization/JsonOps",
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let text = rt.new_str("{\"id\":\"example:value\",\"count\":5}");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let input = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "com/google/gson/JsonParser",
                "parseString",
                "(Ljava/lang/String;)Lcom/google/gson/JsonElement;",
            ),
            &[text],
        );
        let parsed=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new("com/mojang/serialization/Codec","parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),&[id_codec,ops,input]);
        let value = invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "com/mojang/serialization/DataResult",
                "getOrThrow",
                "()Ljava/lang/Object;",
            ),
            &[parsed],
        );
        assert_eq!(ctx.rt.class_of(value), Some(CUSTOM));
        assert!(ctx.rt.pending_exception.is_none());
        let encoded=invoke_member(&mut ctx,op::INVOKEINTERFACE,&MemberRef::new("com/mojang/serialization/Codec","encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;"),&[codec,ops,value]);
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "com/mojang/serialization/DataResult",
                "getOrThrow",
                "()Ljava/lang/Object;",
            ),
            &[encoded],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(decode_json(ctx.rt, &serde_json::json!({"id":2}), true, true).is_err());
        assert!(decode_json(
            ctx.rt,
            &serde_json::json!("{id:'example:value'}"),
            true,
            false
        )
        .is_err());
        assert!(ctx.rt.take_exception().is_some());
        let tag = compound(ctx.rt, vec![]);
        let key = ctx.rt.intern_str("bad");
        let value = scalar(ctx.rt, 6, V::D(f64::NAN));
        put(ctx.rt, tag, key, value);
        assert!(encode_json(ctx.rt, tag, false, false).is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }

    #[test]
    fn heterogeneous_list_wrapping_and_missing_numeric_getters_follow_nbt_rules() {
        let mut rt = Rt::new();
        let input = serde_json::json!({"values":[1,"two",{"":"reserved"},{"regular":3},null]});
        let tag = decode_json(&mut rt, &input, false, false).unwrap();
        assert_eq!(encode_json(&mut rt, tag, false, false).unwrap(), input);
        let key = rt.intern_str("float");
        let value = scalar(&mut rt, 5, V::F(-1.5));
        put(&mut rt, tag, key, value);
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        assert_eq!(
            call(
                &mut rt,
                &linked,
                &mut host,
                &MemberRef::new(COMPOUND, "getLong", "(Ljava/lang/String;)J"),
                false,
                &[tag, key]
            ),
            Some(V::J(-1))
        );
        assert_eq!(
            call(
                &mut rt,
                &linked,
                &mut host,
                &MemberRef::new(COMPOUND, "getInt", "(Ljava/lang/String;)I"),
                false,
                &[tag, key]
            ),
            Some(V::I(-2))
        );
        let missing = rt.intern_str("missing");
        assert_eq!(
            call(
                &mut rt,
                &linked,
                &mut host,
                &MemberRef::new(COMPOUND, "getInt", "(Ljava/lang/String;)I"),
                false,
                &[tag, missing]
            ),
            Some(V::I(0))
        );
    }
}

fn lookup(rt: &Rt, tag: V, key: &str) -> Option<V> {
    entries(rt, tag)?
        .into_iter()
        .find(|(k, _)| rt.str_of(*k) == Some(key))
        .map(|(_, v)| v)
}
fn tag_id(rt: &Rt, tag: V) -> Option<i32> {
    let class = rt.class_of(tag)?;
    TAG_CLASSES
        .iter()
        .position(|v| *v == class)
        .map(|id| id as i32)
}

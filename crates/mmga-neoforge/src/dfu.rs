//! Executable DataFixerUpper 8.0.16 codecs for Minecraft 1.21.1.
//! Graphs retain their mappings and callbacks; unsupported DynamicOps fail explicitly.

mod chat;
mod codec_result;
mod json;
mod map_result;
mod result;

use mmga_classfile::{op, MemberRef};
use mmga_runtime::{box_primitive, invoke_member, Data, ExecCtx, HostApi, LinkedSpace, Rt, V};

const CODEC: &str = "com/mojang/serialization/Codec";
const PRIMITIVE: &str = "com/mojang/serialization/codecs/PrimitiveCodec";
const MAP: &str = "com/mojang/serialization/MapCodec";
const BUILDER: &str = "com/mojang/serialization/codecs/RecordCodecBuilder";
const INSTANCE: &str = "com/mojang/serialization/codecs/RecordCodecBuilder$Instance";
const RESULT: &str = "com/mojang/serialization/DataResult";
const JSON_OPS: &str = "com/mojang/serialization/JsonOps";
const ENUM_CODEC: &str = "net/minecraft/util/StringRepresentable$EnumCodec";
const VALUE_CODEC: &str = "net/minecraft/util/StringRepresentable$StringRepresentableCodec";
const LAZY_CODEC: &str = "com/mojang/serialization/Codec$RecursiveCodec";
const UNBOUNDED_MAP: &str = "com/mojang/serialization/codecs/UnboundedMapCodec";
const DISPATCH_MAP: &str = "com/mojang/serialization/codecs/KeyDispatchCodec";

pub fn is_subtype(sub: &str, sup: &str) -> bool {
    if map_result::is_subtype(sub, sup) {
        return true;
    }
    (matches!(
        sub,
        CODEC | PRIMITIVE | ENUM_CODEC | VALUE_CODEC | LAZY_CODEC | UNBOUNDED_MAP
    ) && matches!(
        sup,
        CODEC | "com/mojang/serialization/Encoder" | "com/mojang/serialization/Decoder"
    )) || (sub == UNBOUNDED_MAP
        && matches!(
            sup,
            "com/mojang/serialization/codecs/BaseMapCodec" | "java/lang/Record"
        ))
        || (sub == ENUM_CODEC && sup == VALUE_CODEC)
        || (matches!(sub, MAP | DISPATCH_MAP)
            && matches!(
                sup,
                MAP | "com/mojang/serialization/MapEncoder"
                    | "com/mojang/serialization/MapDecoder"
                    | "com/mojang/serialization/CompressorHolder"
            ))
        || (sub == BUILDER && sup == "com/mojang/datafixers/kinds/App")
        || (sub == INSTANCE
            && matches!(
                sup,
                "com/mojang/datafixers/kinds/Applicative"
                    | "com/mojang/datafixers/kinds/Functor"
                    | "com/mojang/datafixers/kinds/Kind1"
            ))
        || (matches!(
            sub,
            "com/mojang/serialization/DataResult$Success"
                | "com/mojang/serialization/DataResult$Error"
        ) && matches!(sup, RESULT | "com/mojang/datafixers/kinds/App"))
        || (sub == JSON_OPS && sup == "com/mojang/serialization/DynamicOps")
        || (matches!(
            sub,
            "com/google/gson/JsonPrimitive"
                | "com/google/gson/JsonObject"
                | "com/google/gson/JsonArray"
                | "com/google/gson/JsonNull"
        ) && sup == "com/google/gson/JsonElement")
}

fn graph(rt: &mut Rt, class: &str, kind: &str, args: &[V]) -> V {
    let value = rt.new_instance(class, 0);
    let kind = rt.intern_str(kind);
    let args = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(args.to_vec()));
    rt.put_field(value, "$dfu_kind", kind);
    rt.put_field(value, "$dfu_args", args);
    value
}

fn graph_parts(rt: &Rt, value: V) -> Option<(String, Vec<V>)> {
    let kind = rt.str_of(rt.get_field(value, "$dfu_kind"))?.to_owned();
    Some((kind, array(rt, rt.get_field(value, "$dfu_args"))?))
}

fn array(rt: &Rt, value: V) -> Option<Vec<V>> {
    match rt.get_obj(value).map(|o| &o.data) {
        Some(Data::Array(values) | Data::List(values)) => Some(values.clone()),
        _ => None,
    }
}

/// The enum host supplies real name lookup and ordinal callbacks, including mapped names.
pub fn enum_codec(rt: &mut Rt, values: V, resolver: V, id_getter: V) -> V {
    graph(rt, ENUM_CODEC, "enum", &[values, resolver, id_getter])
}

/// A field graph and retained record getter have the same representation as forGetter/group/apply.
pub fn field_codec(rt: &mut Rt, codec: V, name: &str) -> V {
    let name = rt.intern_str(name);
    graph(rt, MAP, "field", &[codec, name])
}
pub fn record_map_codec(rt: &mut Rt, fields: &[(V, V)], constructor: V) -> V {
    let fields = fields
        .iter()
        .map(|(codec, getter)| {
            let builder = rt.new_instance(BUILDER, 0);
            rt.put_field(builder, "$builder_codec", *codec);
            rt.put_field(builder, "$builder_getter", *getter);
            builder
        })
        .collect();
    let fields = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(fields));
    graph(rt, MAP, "record", &[fields, constructor])
}
pub fn codec_from_map(rt: &mut Rt, map: V) -> V {
    graph(rt, CODEC, "map_codec", &[map])
}

pub fn custom_data_codec(rt: &mut Rt, custom: bool, require_id: bool) -> V {
    graph(
        rt,
        CODEC,
        "nbt_data",
        &[V::bool(custom), V::bool(require_id)],
    )
}
pub fn item_stack_codec(rt: &mut Rt, optional: bool) -> V {
    graph(rt, CODEC, "item_stack", &[V::bool(optional)])
}
pub fn fluid_stack_codec(rt: &mut Rt, optional: bool, content: bool) -> V {
    graph(
        rt,
        CODEC,
        "fluid_stack",
        &[V::bool(optional), V::bool(content)],
    )
}
pub fn decode_json_int(rt: &mut Rt, input: &serde_json::Value) -> Result<i32, String> {
    let value = json::primitive_read(rt, "INT", input, false)?;
    Ok(rt.get_field(value, "$value").i())
}
/// A named dependency whose encode/decode remain explicitly unsupported, usable in retained graphs.
pub fn unsupported_codec(rt: &mut Rt, label: &str) -> V {
    graph(rt, CODEC, &format!("unsupported:{label}"), &[])
}
pub fn number_provider_codec(rt: &mut Rt, mode: i32) -> V {
    graph(
        rt,
        if mode == 2 { MAP } else { CODEC },
        "number_provider",
        &[V::I(mode)],
    )
}

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if let Some(value) = chat::get_static(rt, owner, name, desc) {
        return Some(value);
    }
    if let Some(value) = json::get_static(rt, owner, name, desc) {
        return Some(value);
    }
    if owner == "net/minecraft/util/ExtraCodecs"
        && desc == "Lcom/mojang/serialization/Codec;"
        && matches!(name, "NON_NEGATIVE_INT" | "POSITIVE_INT")
    {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let base = get_static_field(
            rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )?;
        let minimum = if name == "NON_NEGATIVE_INT" { 0 } else { 1 };
        let message = rt.new_str(if minimum == 0 {
            "Value must be non-negative"
        } else {
            "Value must be positive"
        });
        let value = graph(
            rt,
            CODEC,
            "extra_int_range",
            &[base, V::I(minimum), V::I(i32::MAX), message],
        );
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if owner == CODEC
        && desc == "Lcom/mojang/serialization/codecs/PrimitiveCodec;"
        && matches!(
            name,
            "BOOL" | "BYTE" | "SHORT" | "INT" | "LONG" | "FLOAT" | "DOUBLE" | "STRING"
        )
    {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let value = graph(rt, PRIMITIVE, name, &[]);
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if owner == "net/minecraft/resources/ResourceLocation"
        && name == "CODEC"
        && desc == "Lcom/mojang/serialization/Codec;"
    {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let value = graph(rt, CODEC, "resource_location", &[]);
        rt.put_static(owner, name, value);
        return Some(value);
    }
    if owner == "net/minecraft/core/UUIDUtil"
        && name == "CODEC"
        && desc == "Lcom/mojang/serialization/Codec;"
    {
        if let Some(value) = rt.get_static(owner, name) {
            return Some(value);
        }
        let value = graph(rt, CODEC, "uuid_int_array", &[]);
        rt.put_static(owner, name, value);
        return Some(value);
    }
    None
}

pub fn call(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let mut ctx = ExecCtx {
        rt,
        linked,
        host,
        depth: 0,
    };
    if let Some(value) = json::call(&mut ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = result::call(&mut ctx, member, is_static, args) {
        return Some(value);
    }
    call_codec(&mut ctx, member, is_static, args)
}

fn call_codec(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if let Some(value) = map_result::call(ctx, member, is_static, args) {
        return Some(value);
    }
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == BUILDER
        && is_static
        && matches!(
            (name, desc),
            (
                "create",
                "(Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;"
            ) | (
                "mapCodec",
                "(Ljava/util/function/Function;)Lcom/mojang/serialization/MapCodec;"
            )
        )
    {
        let instance = ctx.rt.new_instance(INSTANCE, 0);
        let built = function(ctx, args[0], &[instance]);
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let map = ctx.rt.get_field(built, "$builder_codec");
        if map.is_null() {
            return Some(ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                "RecordCodecBuilder function did not return a record builder",
            ));
        }
        return Some(if name == "mapCodec" {
            map
        } else {
            graph(ctx.rt, CODEC, "map_codec", &[map])
        });
    }
    if !is_static
        && matches!(
            owner,
            INSTANCE
                | "com/mojang/datafixers/kinds/Applicative"
                | "com/mojang/datafixers/kinds/Kind1"
        )
        && name == "group"
    {
        let n = args.len().checked_sub(1)?;
        if !(1..=16).contains(&n)
            || desc
                != format!(
                    "({})Lcom/mojang/datafixers/Products$P{n};",
                    "Lcom/mojang/datafixers/kinds/App;".repeat(n)
                )
        {
            return None;
        }
        let group = ctx
            .rt
            .new_instance(format!("com/mojang/datafixers/Products$P{n}"), 0);
        let fields = ctx
            .rt
            .alloc("[Ljava/lang/Object;", 0, Data::Array(args[1..].to_vec()));
        ctx.rt.put_field(group, "$group_fields", fields);
        return Some(group);
    }
    if !is_static
        && owner == "com/mojang/datafixers/Products$P1"
        && name == "and"
        && desc == "(Lcom/mojang/datafixers/Products$P2;)Lcom/mojang/datafixers/Products$P3;"
    {
        let first = *args.first()?;
        let second = *args.get(1)?;
        if second.is_null() {
            return Some(
                ctx.rt
                    .throw_exception("java/lang/NullPointerException", "Products.P1.and(null)"),
            );
        }
        let mut fields = array(ctx.rt, ctx.rt.get_field(first, "$group_fields"))?;
        let rest = array(ctx.rt, ctx.rt.get_field(second, "$group_fields"))?;
        if fields.len() != 1 || rest.len() != 2 {
            return None;
        }
        fields.extend(rest);
        let product = ctx.rt.new_instance("com/mojang/datafixers/Products$P3", 0);
        let fields = ctx.rt.alloc("[Ljava/lang/Object;", 0, Data::Array(fields));
        ctx.rt.put_field(product, "$group_fields", fields);
        return Some(product);
    }
    if !is_static && name == "apply" && owner.starts_with("com/mojang/datafixers/Products$P") {
        let n = owner
            .strip_prefix("com/mojang/datafixers/Products$P")?
            .parse::<usize>()
            .ok()?;
        let function_desc = match n {
            1 => "Ljava/util/function/Function;".to_owned(),
            2 => "Ljava/util/function/BiFunction;".to_owned(),
            n => format!("Lcom/mojang/datafixers/util/Function{n};"),
        };
        if desc!=format!("(Lcom/mojang/datafixers/kinds/Applicative;{function_desc})Lcom/mojang/datafixers/kinds/App;"){return None;}
        let fields = ctx.rt.get_field(args[0], "$group_fields");
        if array(ctx.rt, fields)?.len() != n {
            return None;
        }
        let codec = graph(ctx.rt, MAP, "record", &[fields, args[2]]);
        let builder = ctx.rt.new_instance(BUILDER, 0);
        ctx.rt.put_field(builder, "$builder_codec", codec);
        return Some(builder);
    }
    let is_codec = matches!(
        owner,
        CODEC
            | PRIMITIVE
            | ENUM_CODEC
            | VALUE_CODEC
            | LAZY_CODEC
            | UNBOUNDED_MAP
            | "com/mojang/serialization/Encoder"
            | "com/mojang/serialization/Decoder"
    );
    let is_map = matches!(owner, MAP | DISPATCH_MAP);
    if is_codec && !is_static && name == "dispatchMap" {
        if desc == "(Ljava/lang/String;Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/MapCodec;" {
            return Some(graph(ctx.rt, DISPATCH_MAP, "dispatch_map", args));
        }
        if desc == "(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/MapCodec;" {
            let field=ctx.rt.intern_str("type");
            return Some(graph(ctx.rt, DISPATCH_MAP, "dispatch_map", &[args[0],field,args[1],args[2]]));
        }
    }
    if owner == CODEC && is_static && name == "unboundedMap"
        && desc == "(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/codecs/UnboundedMapCodec;" {
        return Some(graph(ctx.rt, UNBOUNDED_MAP, "unbounded_map", args));
    }
    if owner == UNBOUNDED_MAP && !is_static {
        if name == "<init>"
            && desc == "(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)V"
        {
            let kind = ctx.rt.intern_str("unbounded_map");
            let values = ctx
                .rt
                .alloc("[Ljava/lang/Object;", 0, Data::Array(args[1..].to_vec()));
            ctx.rt.put_field(args[0], "$dfu_kind", kind);
            ctx.rt.put_field(args[0], "$dfu_args", values);
            return Some(V::Void);
        }
        if matches!(name, "keyCodec" | "elementCodec")
            && desc == "()Lcom/mojang/serialization/Codec;"
        {
            let (_, values) = graph_parts(ctx.rt, args[0])?;
            return Some(values[usize::from(name == "elementCodec")]);
        }
        if matches!(
            (name, desc),
            ("equals", "(Ljava/lang/Object;)Z")
                | ("hashCode", "()I")
                | ("toString", "()Ljava/lang/String;")
        ) {
            let (_, values) = graph_parts(ctx.rt, args[0])?;
            if name == "equals" {
                if args[0] == args[1] {
                    return Some(V::TRUE);
                }
                if ctx.rt.class_of(args[1]) != Some(UNBOUNDED_MAP) {
                    return Some(V::FALSE);
                }
                let (_, other) = graph_parts(ctx.rt, args[1])?;
                for (left, right) in values.into_iter().zip(other) {
                    if left == right {
                        continue;
                    }
                    if left.is_null() {
                        return Some(V::FALSE);
                    }
                    let equal = invoke_member(
                        ctx,
                        op::INVOKEVIRTUAL,
                        &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                        &[left, right],
                    );
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    if equal.i() == 0 {
                        return Some(V::FALSE);
                    }
                }
                return Some(V::TRUE);
            }
            if name == "hashCode" {
                let mut hash = 0i32;
                for value in values {
                    let next = if value.is_null() {
                        V::I(0)
                    } else {
                        invoke_member(
                            ctx,
                            op::INVOKEVIRTUAL,
                            &MemberRef::new("java/lang/Object", "hashCode", "()I"),
                            &[value],
                        )
                    };
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    hash = hash.wrapping_mul(31).wrapping_add(next.i());
                }
                return Some(V::I(hash));
            }
            let mut labels = Vec::new();
            for value in values {
                if value.is_null() {
                    labels.push("null".into());
                    continue;
                }
                let label = invoke_member(
                    ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Object", "toString", "()Ljava/lang/String;"),
                    &[value],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                labels.push(ctx.rt.str_of(label)?.to_owned());
            }
            return Some(
                ctx.rt
                    .new_str(format!("UnboundedMapCodec[{} -> {}]", labels[0], labels[1])),
            );
        }
    }
    if owner == MAP && is_static && name == "unit" {
        return match desc {
            "(Ljava/lang/Object;)Lcom/mojang/serialization/MapCodec;" => {
                Some(graph(ctx.rt, MAP, "unit", args))
            }
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/MapCodec;" => {
                Some(graph(ctx.rt, MAP, "unit_supplier", args))
            }
            _ => None,
        };
    }
    if owner == "net/minecraft/util/ExtraCodecs"
        && is_static
        && name == "optionalEmptyMap"
        && desc == "(Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;"
    {
        return Some(graph(ctx.rt, CODEC, "optional_empty_map", args));
    }
    if owner == "net/minecraft/util/ExtraCodecs"
        && is_static
        && name == "nonEmptyList"
        && desc == "(Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;"
    {
        return Some(graph(ctx.rt, CODEC, "nonempty_list", args));
    }
    if owner == CODEC
        && is_static
        && name == "lazyInitialized"
        && desc == "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;"
    {
        let label = invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Object", "toString", "()Ljava/lang/String;"),
            &[args[0]],
        );
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        return Some(graph(ctx.rt, LAZY_CODEC, "lazy", &[args[0], label]));
    }
    if !is_static
        && name == "toString"
        && desc == "()Ljava/lang/String;"
        && ctx.rt.class_of(args[0]) == Some(LAZY_CODEC)
    {
        let (_, values) = graph_parts(ctx.rt, args[0])?;
        return Some(ctx.rt.new_str(format!(
            "RecursiveCodec[{}]",
            ctx.rt.value_to_string(values[1])
        )));
    }
    if owner == CODEC && is_static {
        return match (name,desc){
            ("either","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;")=>Some(graph(ctx.rt,CODEC,"either",args)),
            ("withAlternative","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;"|"(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;")=>Some(graph(ctx.rt,CODEC,"alternative",args)),
            ("list","(Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;")=>Some(graph(ctx.rt,CODEC,"list",&[args[0],V::I(0),V::I(i32::MAX)])),
            ("list","(Lcom/mojang/serialization/Codec;II)Lcom/mojang/serialization/Codec;")=>Some(graph(ctx.rt,CODEC,"list",args)),
            ("intRange","(II)Lcom/mojang/serialization/Codec;") | ("floatRange","(FF)Lcom/mojang/serialization/Codec;") | ("doubleRange","(DD)Lcom/mojang/serialization/Codec;")=>{let kind=match name{"intRange"=>"INT","floatRange"=>"FLOAT",_=>"DOUBLE"};let base=graph(ctx.rt,PRIMITIVE,kind,&[]);Some(graph(ctx.rt,CODEC,"range",&[base,args[0],args[1]]))},
            ("unit","(Ljava/lang/Object;)Lcom/mojang/serialization/Codec;")=>{let map=graph(ctx.rt,MAP,"unit",args);Some(graph(ctx.rt,CODEC,"map_codec",&[map]))},
            ("unit","(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;")=>{let map=graph(ctx.rt,MAP,"unit_supplier",args);Some(graph(ctx.rt,CODEC,"map_codec",&[map]))},
            _=>None,
        };
    }
    if !is_static && (is_codec || is_map) {
        let recv = *args.first()?;
        let class = if is_map { MAP } else { CODEC };
        let returns = |parameters: &str, ret: &str| desc == format!("({parameters})L{ret};");
        if is_map && name == "mapResult" && desc == "(Lcom/mojang/serialization/MapCodec$ResultFunction;)Lcom/mojang/serialization/MapCodec;" {
            return Some(graph(ctx.rt,MAP,"map_result",args));
        }
        if matches!(name, "xmap" | "comapFlatMap" | "flatComapMap" | "flatXmap")
            && returns(
                "Ljava/util/function/Function;Ljava/util/function/Function;",
                class,
            )
        {
            return Some(graph(ctx.rt, class, name, args));
        }
        if name == "listOf" && desc == "()Lcom/mojang/serialization/Codec;" {
            return Some(graph(
                ctx.rt,
                CODEC,
                "list",
                &[recv, V::I(0), V::I(i32::MAX)],
            ));
        }
        if name == "orElse" && returns("Ljava/lang/Object;", class) {
            return Some(graph(ctx.rt, class, "or_else", args));
        }
        if class == CODEC && name == "orElseGet" && returns("Ljava/util/function/Supplier;", CODEC)
        {
            return Some(graph(ctx.rt, CODEC, "or_else_get", args));
        }
        if class == CODEC && name == "mapResult" && desc == "(Lcom/mojang/serialization/Codec$ResultFunction;)Lcom/mojang/serialization/Codec;" {
            return Some(graph(ctx.rt,CODEC,"codec_result",args));
        }
        if name == "listOf" && desc == "(II)Lcom/mojang/serialization/Codec;" {
            return Some(graph(ctx.rt, CODEC, "list", args));
        }
        if name == "sizeLimitedListOf" && desc == "(I)Lcom/mojang/serialization/Codec;" {
            return Some(graph(ctx.rt, CODEC, "list", &[recv, V::I(0), args[1]]));
        }
        if name == "fieldOf" && returns("Ljava/lang/String;", MAP) {
            return Some(graph(ctx.rt, MAP, "field", args));
        }
        if matches!(name, "optionalFieldOf" | "lenientOptionalFieldOf")
            && matches!(
                desc,
                "(Ljava/lang/String;)Lcom/mojang/serialization/MapCodec;"
                    | "(Ljava/lang/String;Ljava/lang/Object;)Lcom/mojang/serialization/MapCodec;"
            )
        {
            let kind = match (name, args.len()) {
                ("optionalFieldOf", 2) => "optional_field",
                ("optionalFieldOf", _) => "default_field",
                (_, 2) => "lenient_optional_field",
                _ => "lenient_default_field",
            };
            return Some(graph(ctx.rt, MAP, kind, args));
        }
        if is_map && name == "forGetter" && returns("Ljava/util/function/Function;", BUILDER) {
            let builder = ctx.rt.new_instance(BUILDER, 0);
            ctx.rt.put_field(builder, "$builder_codec", recv);
            ctx.rt.put_field(builder, "$builder_getter", args[1]);
            return Some(builder);
        }
        if is_map && name == "codec" && desc == "()Lcom/mojang/serialization/Codec;" {
            return Some(graph(ctx.rt, CODEC, "map_codec", &[recv]));
        }
        if matches!(name, "encodeStart" | "parse" | "decode" | "encode") {
            let (ops,input,prefix,decode_pair)=match (name,desc){
                ("encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;")=>(args[1],args[2],None,false),
                ("encode","(Ljava/lang/Object;Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;")=>(args[2],args[1],Some(args[3]),false),
                ("parse"|"decode","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;")=>(args[1],args[2],None,name=="decode"),
                _=>return None,
            };
            let Some(compressed) = json::compressed(ctx.rt, ops) else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "MMGA DFU supports JsonOps only; this DynamicOps is not implemented",
                ));
            };
            if name == "parse" || name == "decode" {
                let Some(json) = json::to_value(ctx.rt, input) else {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/ClassCastException",
                        "JsonOps input must be a JsonElement",
                    ));
                };
                if let Some((_, args)) =
                    graph_parts(ctx.rt, recv).filter(|(kind, _)| kind == "codec_result")
                {
                    let result = codec_result::decode_result(
                        ctx,
                        &args,
                        ops,
                        &json,
                        compressed,
                        0,
                        Some(input),
                    );
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    return Some(match result {
                        Ok(result) if decode_pair => result,
                        Ok(result) => match codec_result::parsed_result(ctx, result) {
                            Ok(result) => result,
                            Err(error) => result::error(ctx.rt, &error.message, error.partial),
                        },
                        Err(error) => result::error(ctx.rt, &error.message, error.partial),
                    });
                }
                let decoded = decode_with_remainder(ctx, recv, ops, &json, compressed, 0);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                return Some(match decoded {
                    Ok((value, remainder)) => {
                        let value = if decode_pair {
                            let empty = json::from_value(ctx.rt, remainder);
                            let pair = ctx.rt.new_instance("com/mojang/datafixers/util/Pair", 0);
                            ctx.rt.put_field(pair, "first", value);
                            ctx.rt.put_field(pair, "second", empty);
                            pair
                        } else {
                            value
                        };
                        result::success(ctx.rt, value)
                    }
                    Err(error) => {
                        let partial = error.partial.map(|value| {
                            if !decode_pair {
                                return value;
                            }
                            let remainder = error
                                .remainder
                                .clone()
                                .unwrap_or_else(|| decode_remainder(ctx.rt, recv, &json));
                            let remainder = json::from_value(ctx.rt, remainder);
                            let pair = ctx.rt.new_instance("com/mojang/datafixers/util/Pair", 0);
                            ctx.rt.put_field(pair, "first", value);
                            ctx.rt.put_field(pair, "second", remainder);
                            pair
                        });
                        result::error(ctx.rt, &error.message, partial)
                    }
                });
            }
            if let Some((_, args)) =
                graph_parts(ctx.rt, recv).filter(|(kind, _)| kind == "codec_result")
            {
                let result =
                    codec_result::encode_result(ctx, &args, ops, input, compressed, 0, prefix);
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                return Some(match result {
                    Ok(result) => result,
                    Err(error) => {
                        let partial = error.partial.map(|json| json::from_value(ctx.rt, json));
                        result::error(ctx.rt, &error.message, partial)
                    }
                });
            }
            let encoded = encode_value(ctx, recv, ops, input, compressed, 0);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let prefix = if is_empty_map_encoding(ctx.rt, recv, input) {
                None
            } else {
                prefix
            };
            let encoded = match encoded {
                Ok(json) => merge_prefix(ctx.rt, prefix, json),
                Err(mut error) => {
                    if let Some(json) = error.partial.take() {
                        match merge_prefix(ctx.rt, prefix, json) {
                            Ok(json) => error.partial = Some(json),
                            Err(merge) => {
                                error.message.push_str("; ");
                                error.message.push_str(&merge.message);
                                error.partial = merge.partial;
                            }
                        }
                    }
                    Err(error)
                }
            };
            return Some(match encoded {
                Ok(json) => {
                    let value = json::from_value(ctx.rt, json);
                    result::success(ctx.rt, value)
                }
                Err(error) => {
                    let partial = error.partial.map(|json| json::from_value(ctx.rt, json));
                    result::error(ctx.rt, &error.message, partial)
                }
            });
        }
    }
    None
}

fn merge_prefix(
    rt: &Rt,
    prefix: Option<V>,
    value: serde_json::Value,
) -> Result<serde_json::Value, EncodeError> {
    let Some(prefix) = prefix else {
        return Ok(value);
    };
    let prefix = json::to_value(rt, prefix).ok_or_else(|| "Invalid JsonOps prefix".to_owned())?;
    match (prefix, value) {
        (serde_json::Value::Null, value) => Ok(value),
        (serde_json::Value::Object(mut prefix), serde_json::Value::Object(value)) => {
            prefix.extend(value);
            Ok(serde_json::Value::Object(prefix))
        }
        (serde_json::Value::Array(mut prefix), serde_json::Value::Array(value)) => {
            prefix.extend(value);
            Ok(serde_json::Value::Array(prefix))
        }
        (prefix, value) => {
            let primitive = !value.is_object() && !value.is_array();
            Err(EncodeError {
                message: format!("Cannot merge value into prefix: {prefix}"),
                partial: Some(if primitive { value } else { prefix }),
            })
        }
    }
}

fn is_empty_map_encoding(rt: &Rt, codec: V, input: V) -> bool {
    let Some((kind, args)) = graph_parts(rt, codec) else {
        return false;
    };
    match kind.as_str() {
        "fluid_stack" => false,
        "item_stack" => {
            args[0] == V::TRUE
                && (rt.get_static("net/minecraft/world/item/ItemStack", "EMPTY") == Some(input)
                    || rt.get_field(input, "count").i() <= 0)
        }
        "optional_empty_map" => matches!(
            rt.get_obj(input).map(|o| &o.data),
            Some(Data::Optional(None))
        ),
        "lazy" => {
            rt.get_field(codec, "$initialized") == V::TRUE
                && is_empty_map_encoding(rt, rt.get_field(codec, "$delegate"), input)
        }
        "or_else" | "or_else_get" => is_empty_map_encoding(rt, args[0], input),
        _ => false,
    }
}

#[derive(Debug)]
struct DecodeError {
    message: String,
    partial: Option<V>,
    remainder: Option<serde_json::Value>,
}

#[derive(Debug)]
struct EncodeError {
    message: String,
    partial: Option<serde_json::Value>,
}
impl From<String> for EncodeError {
    fn from(message: String) -> Self {
        Self {
            message,
            partial: None,
        }
    }
}
impl From<&str> for EncodeError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl From<String> for DecodeError {
    fn from(message: String) -> Self {
        Self {
            message,
            partial: None,
            remainder: None,
        }
    }
}
impl From<&str> for DecodeError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
fn flatten_decoded_result(ctx: &mut ExecCtx<'_>, value: V) -> Result<V, DecodeError> {
    if let Some(value) = result::value(ctx.rt, value) {
        return Ok(value);
    }
    let partial = ctx.rt.get_field(value, "$partial");
    Err(DecodeError {
        message: result::message(ctx, value),
        partial: (!partial.is_null()).then_some(partial),
        remainder: None,
    })
}

fn decode_remainder(rt: &Rt, codec: V, input: &serde_json::Value) -> serde_json::Value {
    let Some((kind, args)) = graph_parts(rt, codec) else {
        return serde_json::Value::Null;
    };
    match kind.as_str() {
        "map_result" => input.clone(),
        "map_codec" | "item_stack" | "fluid_stack" | "unbounded_map" => input.clone(),
        "list" => serde_json::json!([]),
        "xmap" | "comapFlatMap" | "flatComapMap" | "flatXmap" | "range" | "extra_int_range" => {
            decode_remainder(rt, args[0], input)
        }
        _ => serde_json::Value::Null,
    }
}

fn resolve_lazy(ctx: &mut ExecCtx<'_>, codec: V, supplier: V) -> V {
    if ctx.rt.get_field(codec, "$initialized") == V::TRUE {
        return ctx.rt.get_field(codec, "$delegate");
    }
    let delegate = function(ctx, supplier, &[]);
    if ctx.rt.pending_exception.is_none() {
        ctx.rt.put_field(codec, "$delegate", delegate);
        ctx.rt.put_field(codec, "$initialized", V::TRUE);
    }
    delegate
}

fn either_value(rt: &mut Rt, left: bool, value: V) -> V {
    let either = rt.new_instance("com/mojang/datafixers/util/Either", 0);
    rt.put_field(either, "$is_left", V::bool(left));
    rt.put_field(either, "$val", value);
    either
}

fn decode_with_remainder(
    ctx: &mut ExecCtx<'_>,
    codec: V,
    ops: V,
    input: &serde_json::Value,
    compressed: bool,
    depth: usize,
) -> Result<(V, serde_json::Value), DecodeError> {
    if depth >= 128 {
        return Err("Codec nesting exceeds128".into());
    }
    if let Some((kind, args)) = graph_parts(ctx.rt, codec) {
        if kind == "codec_result" {
            return codec_result::decode(ctx, &args, ops, input, compressed, depth);
        }
        if kind == "nonempty_list" {
            let decoded = decode_with_remainder(ctx, args[0], ops, input, compressed, depth + 1);
            return match decoded {
                Ok((value, remainder)) => {
                    if array(ctx.rt, value).is_some_and(|v| !v.is_empty()) {
                        Ok((value, remainder))
                    } else {
                        Err("List must have contents".into())
                    }
                }
                Err(mut error) => {
                    if error
                        .partial
                        .is_some_and(|value| array(ctx.rt, value).is_some_and(|v| v.is_empty()))
                    {
                        error.message.push_str("; List must have contents");
                        error.partial = None;
                    }
                    Err(error)
                }
            };
        }
        if kind == "either" {
            let first = decode_with_remainder(ctx, args[0], ops, input, compressed, depth + 1);
            if ctx.rt.pending_exception.is_some() {
                return Err("Either left codec failed".into());
            }
            match first {
                Ok((value, remainder)) => {
                    return Ok((either_value(ctx.rt, true, value), remainder))
                }
                Err(mut first) => {
                    let second =
                        decode_with_remainder(ctx, args[1], ops, input, compressed, depth + 1);
                    if ctx.rt.pending_exception.is_some() {
                        return Err("Either right codec failed".into());
                    }
                    return match second {
                        Ok((value, remainder)) => {
                            Ok((either_value(ctx.rt, false, value), remainder))
                        }
                        Err(mut second) => {
                            if let Some(partial) = first.partial {
                                first.partial = Some(either_value(ctx.rt, true, partial));
                                Err(first)
                            } else if let Some(partial) = second.partial {
                                second.partial = Some(either_value(ctx.rt, false, partial));
                                Err(second)
                            } else {
                                Err(format!(
                                    "Failed to parse either. First: {first}; Second: {second}"
                                )
                                .into())
                            }
                        }
                    };
                }
            }
        }
        if kind == "lazy" {
            let delegate = resolve_lazy(ctx, codec, args[0]);
            if ctx.rt.pending_exception.is_some() {
                return Err("Lazy codec supplier failed".into());
            }
            if delegate.is_null() {
                ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "Lazy codec supplier returned null",
                );
                return Err("Null lazy codec".into());
            }
            return decode_with_remainder(ctx, delegate, ops, input, compressed, depth + 1);
        }
        if kind == "optional_empty_map" {
            if input.as_object().is_some_and(|m| m.is_empty()) {
                let optional = ctx.rt.alloc("java/util/Optional", 0, Data::Optional(None));
                return Ok((optional, input.clone()));
            }
            return match decode_with_remainder(ctx, args[0], ops, input, compressed, depth + 1) {
                Ok((value, remainder)) => Ok((
                    ctx.rt
                        .alloc("java/util/Optional", 0, Data::Optional(Some(value))),
                    remainder,
                )),
                Err(mut error) => {
                    error.partial = error.partial.map(|value| {
                        ctx.rt
                            .alloc("java/util/Optional", 0, Data::Optional(Some(value)))
                    });
                    Err(error)
                }
            };
        }
        if kind == "or_else" || kind == "or_else_get" {
            return match decode_with_remainder(ctx, args[0], ops, input, compressed, depth + 1) {
                Ok(result) => Ok(result),
                Err(error) => {
                    if ctx.rt.pending_exception.is_some() {
                        Err(error)
                    } else {
                        let fallback = if kind == "or_else_get" {
                            function(ctx, args[1], &[])
                        } else {
                            args[1]
                        };
                        if ctx.rt.pending_exception.is_some() {
                            Err("Codec fallback supplier failed".into())
                        } else {
                            Ok((fallback, input.clone()))
                        }
                    }
                }
            };
        }
        if matches!(
            kind.as_str(),
            "xmap" | "comapFlatMap" | "flatComapMap" | "flatXmap"
        ) {
            let base = decode_with_remainder(ctx, args[0], ops, input, compressed, depth + 1);
            let (base, remainder, previous) = match base {
                Ok((value, remainder)) => (value, remainder, None),
                Err(error) => match error.partial {
                    Some(value) => (
                        value,
                        error
                            .remainder
                            .clone()
                            .unwrap_or_else(|| decode_remainder(ctx.rt, args[0], input)),
                        Some(error),
                    ),
                    None => return Err(error),
                },
            };
            let mapped = function(ctx, args[1], &[base]);
            if ctx.rt.pending_exception.is_some() {
                return Err("Java callback failed".into());
            }
            let next = if matches!(kind.as_str(), "comapFlatMap" | "flatXmap") {
                flatten_decoded_result(ctx, mapped)
            } else {
                Ok(mapped)
            };
            return match (previous, next) {
                (None, Ok(value)) => Ok((value, remainder)),
                (None, Err(mut error)) => {
                    error.remainder = Some(remainder);
                    Err(error)
                }
                (Some(mut error), Ok(value)) => {
                    error.partial = Some(value);
                    error.remainder = Some(remainder);
                    Err(error)
                }
                (Some(mut error), Err(next)) => {
                    error.message.push_str("; ");
                    error.message.push_str(&next.message);
                    error.partial = next.partial;
                    error.remainder = Some(remainder);
                    Err(error)
                }
            };
        }
    }
    decode_value(ctx, codec, ops, input, compressed, depth)
        .map(|value| (value, decode_remainder(ctx.rt, codec, input)))
}

fn decode_unbounded_map(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    input: &serde_json::Value,
    compressed: bool,
    depth: usize,
) -> Result<V, DecodeError> {
    let values = input
        .as_object()
        .ok_or_else(|| format!("Not a map: {input}"))?;
    let mut entries: Vec<(V, V)> = Vec::new();
    let mut messages = Vec::new();
    let mut failed = serde_json::Map::new();
    for (name, input_value) in values {
        let decoded_key = decode_value(
            ctx,
            args[0],
            ops,
            &serde_json::Value::String(name.clone()),
            compressed,
            depth + 1,
        );
        if ctx.rt.pending_exception.is_some() {
            return Err("Map key codec failed".into());
        }
        let decoded_value = decode_value(ctx, args[1], ops, input_value, compressed, depth + 1);
        if ctx.rt.pending_exception.is_some() {
            return Err("Map value codec failed".into());
        }
        let mut errors = Vec::new();
        let key = match decoded_key {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(error.message);
                error.partial
            }
        };
        let value = match decoded_value {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(error.message);
                error.partial
            }
        };
        let mut duplicate = false;
        if let (Some(key), Some(value)) = (key, value) {
            if key.is_null() || value.is_null() {
                ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "null map entry from codec",
                );
                return Err("Null map entry".into());
            }
            for (existing, _) in &entries {
                let equal = invoke_member(
                    ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Object", "equals", "(Ljava/lang/Object;)Z"),
                    &[key, *existing],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Err("Map key equality failed".into());
                }
                if equal.i() != 0 {
                    duplicate = true;
                    break;
                }
            }
            if duplicate {
                errors = vec![format!(
                    "Duplicate entry for key: '{}'",
                    ctx.rt.value_to_string(key)
                )];
            } else {
                entries.push((key, value));
            }
        }
        if !errors.is_empty() {
            messages.extend(errors);
            failed.insert(name.clone(), input_value.clone());
        }
    }
    let elements = ctx.rt.alloc(
        "com/google/common/collect/ImmutableMap",
        0,
        Data::Map(entries),
    );
    if messages.is_empty() {
        Ok(elements)
    } else {
        Err(DecodeError {
            message: format!(
                "{} missed input: {}",
                messages.join("; "),
                serde_json::Value::Object(failed)
            ),
            partial: Some(elements),
            remainder: Some(input.clone()),
        })
    }
}

fn selected_map_codec(ctx: &mut ExecCtx<'_>, selector: V, key: V) -> Result<V, DecodeError> {
    let codec = function(ctx, selector, &[key]);
    if ctx.rt.pending_exception.is_some() {
        return Err("Dispatch selector failed".into());
    }
    if codec.is_null() {
        ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            "Dispatch selector returned null codec",
        );
        return Err("Null dispatch codec".into());
    }
    if !ctx
        .linked
        .is_subtype(ctx.rt.class_of(codec).unwrap_or(""), MAP, ctx.host)
        && ctx.rt.class_of(codec) != Some(MAP)
    {
        ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "Dispatch selector did not return MapCodec",
        );
        return Err("Wrong dispatch codec".into());
    }
    Ok(codec)
}

fn decode_dispatch_map(
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
            "Compressed dispatch maps require a real key compressor",
        );
        return Err("Unsupported compressed dispatch".into());
    }
    let field = ctx
        .rt
        .str_of(args[1])
        .ok_or("Dispatch type key is not a String")?
        .to_owned();
    let value = input
        .as_object()
        .and_then(|map| map.get(&field))
        .ok_or_else(|| format!("Input does not contain a key [{field}]: {input}"))?;
    let (key, previous) = match decode_value(ctx, args[0], ops, value, compressed, depth + 1) {
        Ok(value) => (value, None),
        Err(error) => match error.partial {
            Some(value) => (value, Some(error)),
            None => return Err(error),
        },
    };
    let codec = selected_map_codec(ctx, args[3], key)?;
    let decoded = decode_value(ctx, codec, ops, input, compressed, depth + 1);
    match (previous, decoded) {
        (None, result) => result,
        (Some(mut error), Ok(value)) => {
            error.partial = Some(value);
            Err(error)
        }
        (Some(mut error), Err(next)) => {
            error.message.push_str("; ");
            error.message.push_str(&next.message);
            error.partial = next.partial;
            Err(error)
        }
    }
}

fn encode_dispatch_map(
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
            "Compressed dispatch maps require a real key compressor",
        );
        return Err("Unsupported compressed dispatch".into());
    }
    let field = ctx
        .rt
        .str_of(args[1])
        .ok_or("Dispatch type key is not a String")?
        .to_owned();
    let key = function(ctx, args[2], &[value]);
    if ctx.rt.pending_exception.is_some() {
        return Err("Dispatch type function failed".into());
    }
    let codec = selected_map_codec(ctx, args[3], key).map_err(|error| EncodeError {
        message: error.message,
        partial: None,
    })?;
    let encoded = encode_value(ctx, codec, ops, value, compressed, depth + 1);
    if ctx.rt.pending_exception.is_some() {
        return Err("Dispatch child encoder failed".into());
    }
    let key = function(ctx, args[2], &[value]);
    if ctx.rt.pending_exception.is_some() {
        return Err("Dispatch type function failed".into());
    }
    let encoded_key = encode_value(ctx, args[0], ops, key, compressed, depth + 1);
    if ctx.rt.pending_exception.is_some() {
        return Err("Dispatch key encoder failed".into());
    }
    let mut messages = Vec::new();
    let payload = match encoded {
        Ok(value) => Some(value),
        Err(error) => {
            messages.push(error.message);
            error.partial
        }
    };
    let key = match encoded_key {
        Ok(value) => Some(value),
        Err(error) => {
            messages.push(error.message);
            error.partial
        }
    };
    let payload = match payload {
        Some(serde_json::Value::Object(mut map)) => {
            if let Some(key) = key {
                map.insert(field, key);
            }
            Some(serde_json::Value::Object(map))
        }
        Some(other) => {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Dispatch child map encoder produced non-map {other}"),
            );
            return Err("Unsupported dispatch map".into());
        }
        None => key.map(|key| {
            let mut map = serde_json::Map::new();
            map.insert(field, key);
            serde_json::Value::Object(map)
        }),
    };
    if messages.is_empty() {
        payload.ok_or_else(|| "Dispatch encoder produced no data".into())
    } else {
        Err(EncodeError {
            message: messages.join("; "),
            partial: payload,
        })
    }
}

fn encode_unbounded_map(
    ctx: &mut ExecCtx<'_>,
    args: &[V],
    ops: V,
    value: V,
    compressed: bool,
    depth: usize,
) -> Result<serde_json::Value, EncodeError> {
    if value.is_null() {
        ctx.rt
            .throw_exception("java/lang/NullPointerException", "null map to encode");
        return Err("Null map".into());
    }
    let view = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/util/Map", "entrySet", "()Ljava/util/Set;"),
        &[value],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Map entrySet failed".into());
    }
    let iter = invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new("java/lang/Iterable", "iterator", "()Ljava/util/Iterator;"),
        &[view],
    );
    if ctx.rt.pending_exception.is_some() {
        return Err("Map iterator failed".into());
    }
    let mut encoded = serde_json::Map::new();
    let mut messages = Vec::new();
    loop {
        let next = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/Iterator", "hasNext", "()Z"),
            &[iter],
        );
        if ctx.rt.pending_exception.is_some() {
            return Err("Map hasNext failed".into());
        }
        if next.i() == 0 {
            break;
        }
        let entry = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/Iterator", "next", "()Ljava/lang/Object;"),
            &[iter],
        );
        if ctx.rt.pending_exception.is_some() {
            return Err("Map next failed".into());
        }
        let key = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/Map$Entry", "getKey", "()Ljava/lang/Object;"),
            &[entry],
        );
        if ctx.rt.pending_exception.is_some() {
            return Err("Map key failed".into());
        }
        let value = invoke_member(
            ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new("java/util/Map$Entry", "getValue", "()Ljava/lang/Object;"),
            &[entry],
        );
        if ctx.rt.pending_exception.is_some() {
            return Err("Map value failed".into());
        }
        let key = encode_value(ctx, args[0], ops, key, compressed, depth + 1);
        if ctx.rt.pending_exception.is_some() {
            return Err("Map key encoding failed".into());
        }
        let value = encode_value(ctx, args[1], ops, value, compressed, depth + 1);
        if ctx.rt.pending_exception.is_some() {
            return Err("Map value encoding failed".into());
        }
        let key = match key {
            Ok(key) => Some(key),
            Err(error) => {
                messages.push(error.message);
                error.partial
            }
        };
        let value = match value {
            Ok(value) => Some(value),
            Err(error) => {
                messages.push(error.message);
                error.partial
            }
        };
        if let (Some(key), Some(value)) = (key, value) {
            let name = match key {
                serde_json::Value::String(name) => name,
                serde_json::Value::Number(number) => number.to_string(),
                serde_json::Value::Bool(value) => value.to_string(),
                other => {
                    ctx.rt.throw_exception(
                        "java/lang/UnsupportedOperationException",
                        &format!("JsonOps cannot encode non-primitive map key {other}"),
                    );
                    return Err("Unsupported map key".into());
                }
            };
            encoded.insert(name, value);
        }
    }
    let map = serde_json::Value::Object(encoded);
    if messages.is_empty() {
        Ok(map)
    } else {
        Err(EncodeError {
            message: messages.join("; "),
            partial: Some(map),
        })
    }
}

fn decode_value(
    ctx: &mut ExecCtx<'_>,
    codec: V,
    ops: V,
    input: &serde_json::Value,
    compressed: bool,
    depth: usize,
) -> Result<V, DecodeError> {
    if ctx.rt.pending_exception.is_some() {
        return Err("Java callback failed".into());
    }
    if depth >= 128 {
        return Err("Codec nesting exceeds 128".into());
    }
    let Some((kind, args)) = graph_parts(ctx.rt, codec) else {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MMGA cannot execute unknown DFU codec",
        );
        return Err("Unknown codec".into());
    };
    match kind.as_str() {
        "codec_result" => {
            codec_result::decode(ctx, &args, ops, input, compressed, depth).map(|(value, _)| value)
        }
        "map_result" => map_result::decode(ctx, &args, ops, input, compressed, depth),
        "dispatch_map" => decode_dispatch_map(ctx, &args, ops, input, compressed, depth),
        "unbounded_map" => decode_unbounded_map(ctx, &args, ops, input, compressed, depth),
        "number_provider" => {
            crate::number_providers::decode_json(ctx.rt, input, args[0].i()).map_err(Into::into)
        }
        "lazy" | "optional_empty_map" | "either" | "nonempty_list" | "or_else_get" => {
            decode_with_remainder(ctx, codec, ops, input, compressed, depth + 1)
                .map(|(value, _)| value)
        }
        "fluid_stack" => {
            crate::fluid_content::decode_json(ctx, input, args[0] == V::TRUE, args[1] == V::TRUE)
                .map_err(Into::into)
        }
        "item_stack" => {
            crate::item_stack::decode_json(ctx, input, args[0] == V::TRUE).map_err(Into::into)
        }
        "or_else" => match decode_value(ctx, args[0], ops, input, compressed, depth + 1) {
            Ok(value) => Ok(value),
            Err(error) => {
                if ctx.rt.pending_exception.is_some() {
                    Err(error)
                } else {
                    Ok(args[1])
                }
            }
        },
        "nbt_data" => {
            crate::custom_data::decode_json(ctx.rt, input, args[0] == V::TRUE, args[1] == V::TRUE)
                .map_err(Into::into)
        }
        "component" => chat::decode(ctx, ops, input, compressed, depth),
        "BOOL" | "BYTE" | "SHORT" | "INT" | "LONG" | "FLOAT" | "DOUBLE" | "STRING" => {
            json::primitive_read(ctx.rt, &kind, input, compressed).map_err(Into::into)
        }
        "uuid_int_array" => {
            let values = input
                .as_array()
                .ok_or_else(|| format!("Not a list: {input}"))?;
            let mut ints = Vec::new();
            for value in values {
                let number = json::primitive_read(ctx.rt, "INT", value, compressed)?;
                ints.push(ctx.rt.get_field(number, "$value").i());
            }
            if ints.len() < 4 {
                return Err(format!("Input is not a list of 4 ints: {input}").into());
            }
            let most = ((ints[0] as i64) << 32) | (ints[1] as u32 as i64);
            let least = ((ints[2] as i64) << 32) | (ints[3] as u32 as i64);
            let uuid = ctx.rt.new_instance("java/util/UUID", 0);
            invoke_member(
                ctx,
                op::INVOKESPECIAL,
                &MemberRef::new("java/util/UUID", "<init>", "(JJ)V"),
                &[uuid, V::J(most), V::J(least)],
            );
            if ints.len() > 4 {
                Err(DecodeError {
                    message: format!("Input is not a list of 4 ints: {input}"),
                    partial: Some(uuid),
                    remainder: None,
                })
            } else {
                Ok(uuid)
            }
        }
        "resource_location" => {
            let raw = json::primitive_read(ctx.rt, "STRING", input, compressed)?;
            let text = ctx.rt.str_of(raw).unwrap();
            let (ns, path) = text.split_once(':').unwrap_or(("minecraft", text));
            let ns = if ns.is_empty() { "minecraft" } else { ns };
            if !ns
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
                || !path
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
            {
                return Err(format!("Not a valid resource location: {text}").into());
            }
            let id = format!("{ns}:{path}");
            Ok(ctx.rt.new_pinned_handle(
                "net/minecraft/resources/ResourceLocation",
                "resource_location",
                0,
                id,
            ))
        }
        "enum" => {
            if compressed {
                let index = json::primitive_read(ctx.rt, "INT", input, true)?;
                let index = ctx.rt.get_field(index, "$value").i();
                array(ctx.rt, args[0])
                    .and_then(|values| values.get(index as usize).copied())
                    .filter(|v| !v.is_null())
                    .ok_or_else(|| format!("Unknown element id: {index}").into())
            } else {
                let name = json::primitive_read(ctx.rt, "STRING", input, false)?;
                let value = function(ctx, args[1], &[name]);
                if value.is_null() {
                    Err(format!("Unknown element name: {}", ctx.rt.value_to_string(name)).into())
                } else {
                    Ok(value)
                }
            }
        }
        "xmap" | "comapFlatMap" | "flatComapMap" | "flatXmap" => {
            let base = decode_value(ctx, args[0], ops, input, compressed, depth + 1);
            let (base, first_error) = match base {
                Ok(value) => (value, None),
                Err(error) => match error.partial {
                    Some(value) => (value, Some(error)),
                    None => return Err(error),
                },
            };
            let value = function(ctx, args[1], &[base]);
            let mapped = if matches!(kind.as_str(), "comapFlatMap" | "flatXmap") {
                flatten_decoded_result(ctx, value)
            } else {
                Ok(value)
            };
            if let Some(mut error) = first_error {
                match mapped {
                    Ok(value) => error.partial = Some(value),
                    Err(next) => {
                        error.message.push_str("; ");
                        error.message.push_str(&next.message);
                        error.partial = next.partial;
                    }
                }
                Err(error)
            } else {
                mapped
            }
        }
        "list" => {
            let values = input
                .as_array()
                .ok_or_else(|| format!("Not a list: {input}"))?;
            let min = args[1].i().max(0) as usize;
            let max = args[2].i().max(0) as usize;
            let mut elements = Vec::new();
            let mut failed = Vec::new();
            let mut messages = Vec::new();
            for value in values {
                if elements.len() >= max {
                    failed.push(value.clone());
                    continue;
                }
                match decode_value(ctx, args[0], ops, value, compressed, depth + 1) {
                    Ok(value) => elements.push(value),
                    Err(error) => {
                        failed.push(value.clone());
                        messages.push(error.message);
                        if let Some(partial) = error.partial {
                            elements.push(partial);
                        }
                    }
                }
                if ctx.rt.pending_exception.is_some() {
                    return Err("Java callback failed".into());
                }
            }
            if elements.len() < min {
                return Err(format!(
                    "List is too short: {}, expected range [{min}-{max}]",
                    elements.len()
                )
                .into());
            }
            let partial = ctx.rt.alloc(
                "java/util/ImmutableCollections$ListN",
                0,
                Data::List(elements),
            );
            if values.len() > max {
                messages = vec![format!(
                    "List is too long: {}, expected range [{min}-{max}]",
                    values.len()
                )];
            }
            if messages.is_empty() {
                Ok(partial)
            } else {
                Err(DecodeError {
                    message: messages.join("; "),
                    partial: Some(partial),
                    remainder: Some(serde_json::Value::Array(failed)),
                })
            }
        }
        "map_codec" => {
            if !input.is_object() && !compressed {
                return Err(format!("Not a map: {input}").into());
            }
            decode_value(ctx, args[0], ops, input, compressed, depth + 1)
        }
        "field"
        | "optional_field"
        | "default_field"
        | "lenient_optional_field"
        | "lenient_default_field" => {
            if compressed {
                ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Compressed MapCodec fields require a key compressor",
                );
                return Err("Unsupported compressed map".into());
            }
            let map = input
                .as_object()
                .ok_or_else(|| format!("Not a map: {input}"))?;
            let key = ctx.rt.str_of(args[1]).unwrap_or("").to_owned();
            let entry = map.get(&key).filter(|v| !v.is_null());
            if let Some(entry) = entry {
                let parsed = decode_value(ctx, args[0], ops, entry, compressed, depth + 1);
                if parsed.is_ok() || !kind.starts_with("lenient") {
                    return match parsed {
                        Ok(value) => Ok(if kind.ends_with("optional_field") {
                            ctx.rt
                                .alloc("java/util/Optional", 0, Data::Optional(Some(value)))
                        } else {
                            value
                        }),
                        Err(mut error) => {
                            if kind != "field" {
                                error.partial = Some(if kind.ends_with("optional_field") {
                                    ctx.rt.alloc(
                                        "java/util/Optional",
                                        0,
                                        Data::Optional(error.partial),
                                    )
                                } else {
                                    error.partial.unwrap_or(args[2])
                                });
                            }
                            Err(error)
                        }
                    };
                }
            }
            match kind.as_str() {
                "field" => Err(format!("No key {key} in {input}").into()),
                "optional_field" | "lenient_optional_field" => {
                    Ok(ctx.rt.alloc("java/util/Optional", 0, Data::Optional(None)))
                }
                _ => Ok(args[2]),
            }
        }
        "record" => {
            let fields =
                array(ctx.rt, args[0]).ok_or_else(|| "Invalid record fields".to_owned())?;
            let mut values = Vec::new();
            let mut errors = Vec::new();
            let mut complete_partial = true;
            for field in fields {
                match decode_value(
                    ctx,
                    ctx.rt.get_field(field, "$builder_codec"),
                    ops,
                    input,
                    compressed,
                    depth + 1,
                ) {
                    Ok(v) => values.push(v),
                    Err(e) => {
                        errors.push(e.message);
                        if let Some(value) = e.partial {
                            values.push(value)
                        } else {
                            complete_partial = false;
                        }
                    }
                }
                if ctx.rt.pending_exception.is_some() {
                    return Err("Java callback failed".into());
                }
            }
            if !errors.is_empty() {
                let partial = complete_partial.then(|| function(ctx, args[1], &values));
                return Err(DecodeError {
                    message: errors.join("; "),
                    partial,
                    remainder: Some(input.clone()),
                });
            }
            Ok(function(ctx, args[1], &values))
        }
        "alternative" => match decode_value(ctx, args[0], ops, input, compressed, depth + 1) {
            Ok(v) => Ok(v),
            Err(first) => match decode_value(ctx, args[1], ops, input, compressed, depth + 1) {
                Ok(value) => Ok(if args.len() > 2 {
                    function(ctx, args[2], &[value])
                } else {
                    value
                }),
                Err(mut second) => {
                    if first.partial.is_some() {
                        Err(first)
                    } else if let Some(partial) = second.partial {
                        if args.len() > 2 {
                            second.partial = Some(function(ctx, args[2], &[partial]));
                        }
                        Err(second)
                    } else {
                        Err(
                            format!("Failed to parse either. First: {first}; Second: {second}")
                                .into(),
                        )
                    }
                }
            },
        },
        "extra_int_range" => {
            let value = decode_value(ctx, args[0], ops, input, compressed, depth + 1)?;
            let input = ctx.rt.get_field(value, "$value").i();
            if input < args[1].i() || input > args[2].i() {
                Err(format!("{}: {input}", ctx.rt.value_to_string(args[3])).into())
            } else {
                Ok(value)
            }
        }
        "range" => {
            let value = decode_value(ctx, args[0], ops, input, compressed, depth + 1)?;
            let raw = json::numeric_json(ctx.rt, value).unwrap();
            let n = raw.as_f64().unwrap();
            let min = json::numeric_json(ctx.rt, args[1])
                .unwrap()
                .as_f64()
                .unwrap();
            let max = json::numeric_json(ctx.rt, args[2])
                .unwrap()
                .as_f64()
                .unwrap();
            if n < min || n > max {
                Err(format!("Value {n} outside of range [{min}:{max}]").into())
            } else {
                Ok(value)
            }
        }
        "unit" => Ok(args[0]),
        "unit_supplier" => Ok(function(ctx, args[0], &[])),
        _ => {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("MMGA cannot decode DFU node {kind}"),
            );
            Err(format!("Unknown codec {kind}").into())
        }
    }
}

fn encode_value(
    ctx: &mut ExecCtx<'_>,
    codec: V,
    ops: V,
    value: V,
    compressed: bool,
    depth: usize,
) -> Result<serde_json::Value, EncodeError> {
    use serde_json::Value as Json;
    if ctx.rt.pending_exception.is_some() {
        return Err("Java callback failed".into());
    }
    if depth >= 128 {
        return Err("Codec nesting exceeds 128".into());
    }
    let Some((kind, args)) = graph_parts(ctx.rt, codec) else {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "MMGA cannot execute unknown DFU codec",
        );
        return Err("Unknown codec".into());
    };
    let expected = match kind.as_str() {
        "BOOL" => Some("java/lang/Boolean"),
        "BYTE" => Some("java/lang/Byte"),
        "SHORT" => Some("java/lang/Short"),
        "INT" => Some("java/lang/Integer"),
        "LONG" => Some("java/lang/Long"),
        "FLOAT" => Some("java/lang/Float"),
        "DOUBLE" => Some("java/lang/Double"),
        "STRING" => Some("java/lang/String"),
        _ => None,
    };
    if let Some(expected) = expected {
        if value.is_null() {
            ctx.rt.throw_exception(
                "java/lang/NullPointerException",
                "Codec cannot encode null primitive",
            );
            return Err("Null primitive".into());
        }
        if ctx.rt.class_of(value) != Some(expected) {
            ctx.rt.throw_exception(
                "java/lang/ClassCastException",
                &format!("{kind} codec requires {expected}"),
            );
            return Err("Wrong primitive wrapper".into());
        }
    }
    match kind.as_str() {
        "codec_result" => codec_result::encode(ctx, &args, ops, value, compressed, depth, None),
        "map_result" => map_result::encode(ctx, &args, ops, value, compressed, depth),
        "dispatch_map" => encode_dispatch_map(ctx, &args, ops, value, compressed, depth),
        "unbounded_map" => encode_unbounded_map(ctx, &args, ops, value, compressed, depth),
        "number_provider" => {
            crate::number_providers::encode_json(ctx.rt, value, args[0].i()).map_err(Into::into)
        }
        "nonempty_list" => {
            if array(ctx.rt, value).is_some_and(|v| !v.is_empty()) {
                encode_value(ctx, args[0], ops, value, compressed, depth + 1)
            } else {
                Err("List must have contents".into())
            }
        }
        "either" => {
            if ctx.rt.class_of(value) != Some("com/mojang/datafixers/util/Either") {
                return Err("Either codec requires Either value".into());
            }
            let inner = ctx.rt.get_field(value, "$val");
            let index = if ctx.rt.get_field(value, "$is_left") == V::TRUE {
                0
            } else {
                1
            };
            encode_value(ctx, args[index], ops, inner, compressed, depth + 1)
        }
        "lazy" => {
            let delegate = resolve_lazy(ctx, codec, args[0]);
            if ctx.rt.pending_exception.is_some() {
                return Err("Lazy codec supplier failed".into());
            }
            if delegate.is_null() {
                ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "Lazy codec supplier returned null",
                );
                return Err("Null lazy codec".into());
            }
            encode_value(ctx, delegate, ops, value, compressed, depth + 1)
        }
        "optional_empty_map" => match ctx.rt.get_obj(value).map(|o| &o.data) {
            Some(Data::Optional(None)) => Ok(serde_json::json!({})),
            Some(Data::Optional(Some(inner))) => {
                let inner = *inner;
                encode_value(ctx, args[0], ops, inner, compressed, depth + 1)
            }
            _ => Err("Expected Optional for optionalEmptyMap".into()),
        },
        "fluid_stack" => {
            crate::fluid_content::encode_json(ctx, value, args[0] == V::TRUE, args[1] == V::TRUE)
                .map_err(Into::into)
        }
        "item_stack" => {
            crate::item_stack::encode_json(ctx, value, args[0] == V::TRUE).map_err(Into::into)
        }
        "or_else" | "or_else_get" => encode_value(ctx, args[0], ops, value, compressed, depth + 1),
        "nbt_data" => {
            crate::custom_data::encode_json(ctx.rt, value, args[0] == V::TRUE, args[1] == V::TRUE)
                .map_err(Into::into)
        }
        "component" => chat::encode(ctx, ops, value, compressed, depth),
        "BOOL" => {
            if ctx.rt.class_of(value) != Some("java/lang/Boolean") {
                return Err("Not a Boolean".into());
            }
            Ok(Json::Bool(ctx.rt.get_field(value, "$value").i() != 0))
        }
        "STRING" => ctx
            .rt
            .str_of(value)
            .map(|s| Json::String(s.into()))
            .ok_or_else(|| "Not a String".into()),
        "BYTE" | "SHORT" | "INT" | "LONG" | "FLOAT" | "DOUBLE" => {
            json::numeric_json(ctx.rt, value).ok_or_else(|| "Not a finite number".into())
        }
        "resource_location" => ctx
            .rt
            .handle_of(value)
            .filter(|(kind, ..)| *kind == "resource_location")
            .map(|(_, _, id, _)| Json::String(id.into()))
            .ok_or_else(|| "Not a ResourceLocation".into()),
        "uuid_int_array" => {
            let Some(("uuid", _, _, bits)) = ctx.rt.handle_of(value) else {
                return Err("Not a UUID".into());
            };
            Ok(serde_json::json!([
                (bits[0] >> 32) as i32,
                bits[0] as i32,
                (bits[1] >> 32) as i32,
                bits[1] as i32
            ]))
        }
        "enum" => {
            if compressed {
                let id = invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/ToIntFunction",
                        "applyAsInt",
                        "(Ljava/lang/Object;)I",
                    ),
                    &[args[2], value],
                );
                if id.i() == -1 {
                    Err(format!("Unknown element: {}", ctx.rt.value_to_string(value)).into())
                } else {
                    Ok(Json::from(id.i()))
                }
            } else {
                let name = invoke_member(
                    ctx,
                    op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "net/minecraft/util/StringRepresentable",
                        "getSerializedName",
                        "()Ljava/lang/String;",
                    ),
                    &[value],
                );
                ctx.rt
                    .str_of(name)
                    .map(|s| Json::String(s.into()))
                    .ok_or_else(|| "Enum has no serialized name".into())
            }
        }
        "xmap" | "comapFlatMap" | "flatComapMap" | "flatXmap" => {
            let mapped = function(ctx, args[2], &[value]);
            if ctx.rt.pending_exception.is_some() {
                return Err("Java callback failed".into());
            }
            if matches!(kind.as_str(), "flatComapMap" | "flatXmap") {
                match flatten_decoded_result(ctx, mapped) {
                    Ok(value) => encode_value(ctx, args[0], ops, value, compressed, depth + 1),
                    Err(error) => {
                        let mut out = EncodeError {
                            message: error.message,
                            partial: None,
                        };
                        if let Some(value) = error.partial {
                            match encode_value(ctx, args[0], ops, value, compressed, depth + 1) {
                                Ok(json) => out.partial = Some(json),
                                Err(next) => {
                                    out.message.push_str("; ");
                                    out.message.push_str(&next.message);
                                    out.partial = next.partial;
                                }
                            }
                        }
                        Err(out)
                    }
                }
            } else {
                encode_value(ctx, args[0], ops, mapped, compressed, depth + 1)
            }
        }
        "list" => {
            let values = array(ctx.rt, value).ok_or_else(|| "Not a list".to_owned())?;
            if values.len() < args[1].i().max(0) as usize
                || values.len() > args[2].i().max(0) as usize
            {
                return Err(format!(
                    "List length {} outside [{}, {}]",
                    values.len(),
                    args[1].i(),
                    args[2].i()
                )
                .into());
            }
            let mut encoded = Vec::new();
            let mut messages = Vec::new();
            let mut complete_partial = true;
            for value in values {
                match encode_value(ctx, args[0], ops, value, compressed, depth + 1) {
                    Ok(json) => encoded.push(json),
                    Err(error) => {
                        messages.push(error.message);
                        if let Some(json) = error.partial {
                            encoded.push(json)
                        } else {
                            complete_partial = false;
                        }
                    }
                }
                if ctx.rt.pending_exception.is_some() {
                    return Err("Java callback failed".into());
                }
            }
            if messages.is_empty() {
                Ok(Json::Array(encoded))
            } else {
                Err(EncodeError {
                    message: messages.join("; "),
                    partial: complete_partial.then_some(Json::Array(encoded)),
                })
            }
        }
        "map_codec" => encode_value(ctx, args[0], ops, value, compressed, depth + 1),
        "field"
        | "optional_field"
        | "default_field"
        | "lenient_optional_field"
        | "lenient_default_field" => {
            if compressed {
                ctx.rt.throw_exception(
                    "java/lang/UnsupportedOperationException",
                    "Compressed MapCodec fields require a key compressor",
                );
                return Err("Unsupported compressed map".into());
            }
            let key = ctx.rt.str_of(args[1]).unwrap_or("").to_owned();
            let value = if kind.ends_with("optional_field") {
                match ctx.rt.get_obj(value).map(|o| &o.data) {
                    Some(Data::Optional(Some(v))) => *v,
                    Some(Data::Optional(None)) => return Ok(Json::Object(Default::default())),
                    _ => return Err("Optional field needs Optional".into()),
                }
            } else {
                value
            };
            if kind.ends_with("default_field") && ctx.rt.val_eq(value, args[2]) {
                return Ok(Json::Object(Default::default()));
            }
            match encode_value(ctx, args[0], ops, value, compressed, depth + 1) {
                Ok(val) => {
                    let mut map = serde_json::Map::new();
                    map.insert(key, val);
                    Ok(Json::Object(map))
                }
                Err(mut error) => {
                    error.partial = error.partial.map(|value| {
                        let mut map = serde_json::Map::new();
                        map.insert(key, value);
                        Json::Object(map)
                    });
                    Err(error)
                }
            }
        }
        "record" => {
            let fields =
                array(ctx.rt, args[0]).ok_or_else(|| "Invalid record fields".to_owned())?;
            let mut output = serde_json::Map::new();
            let mut messages = Vec::new();
            let mut complete_partial = true;
            for field in fields {
                let getter = ctx.rt.get_field(field, "$builder_getter");
                let part = function(ctx, getter, &[value]);
                if ctx.rt.pending_exception.is_some() {
                    return Err("Record getter failed".into());
                }
                let encoded = encode_value(
                    ctx,
                    ctx.rt.get_field(field, "$builder_codec"),
                    ops,
                    part,
                    compressed,
                    depth + 1,
                );
                let encoded = match encoded {
                    Ok(value) => value,
                    Err(error) => {
                        messages.push(error.message);
                        if let Some(value) = error.partial {
                            value
                        } else {
                            complete_partial = false;
                            continue;
                        }
                    }
                };
                let Json::Object(fields) = encoded else {
                    return Err("Record field did not produce a map".into());
                };
                output.extend(fields);
            }
            if messages.is_empty() {
                Ok(Json::Object(output))
            } else {
                Err(EncodeError {
                    message: messages.join("; "),
                    partial: complete_partial.then_some(Json::Object(output)),
                })
            }
        }
        "alternative" => encode_value(ctx, args[0], ops, value, compressed, depth + 1),
        "extra_int_range" => {
            // Validate before the underlying encoder, matching Codec.validate's
            // flatComapMap: invalid values have no encoded partial or prefix.
            if value.is_null() {
                ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "ExtraCodecs integer validator",
                );
                return Err("Null integer".into());
            }
            if ctx.rt.class_of(value) != Some("java/lang/Integer") {
                ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "ExtraCodecs integer validator requires Integer",
                );
                return Err("Wrong integer wrapper".into());
            }
            let input = ctx.rt.get_field(value, "$value").i();
            if input < args[1].i() || input > args[2].i() {
                Err(format!("{}: {input}", ctx.rt.value_to_string(args[3])).into())
            } else {
                encode_value(ctx, args[0], ops, value, compressed, depth + 1)
            }
        }
        "range" => {
            let encoded = encode_value(ctx, args[0], ops, value, compressed, depth + 1)?;
            let n = encoded.as_f64().ok_or_else(|| "Not a number".to_owned())?;
            let min = json::numeric_json(ctx.rt, args[1])
                .unwrap()
                .as_f64()
                .unwrap();
            let max = json::numeric_json(ctx.rt, args[2])
                .unwrap()
                .as_f64()
                .unwrap();
            if n < min || n > max {
                Err(format!("Value {n} outside of range [{min}:{max}]").into())
            } else {
                Ok(encoded)
            }
        }
        "unit" | "unit_supplier" => Ok(Json::Object(Default::default())),
        _ => {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("MMGA cannot encode DFU node {kind}"),
            );
            Err(format!("Unknown codec {kind}").into())
        }
    }
}

fn function(ctx: &mut ExecCtx<'_>, callback: V, args: &[V]) -> V {
    let owner = match args.len() {
        0 => "java/util/function/Supplier".to_owned(),
        1 => "java/util/function/Function".to_owned(),
        2 => "java/util/function/BiFunction".to_owned(),
        n => format!("com/mojang/datafixers/util/Function{n}"),
    };
    let desc = format!(
        "({})Ljava/lang/Object;",
        "Ljava/lang/Object;".repeat(args.len())
    );
    let mut all = vec![callback];
    all.extend_from_slice(args);
    invoke_member(
        ctx,
        op::INVOKEINTERFACE,
        &MemberRef::new(&owner, if args.is_empty() { "get" } else { "apply" }, &desc),
        &all,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::NullHost;

    #[test]
    fn extra_codecs_integer_bounds_validate_encode_and_decode_without_partial_defaults() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let owner = "net/minecraft/util/ExtraCodecs";
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        for (name, minimum, message) in [
            ("NON_NEGATIVE_INT", 0, "Value must be non-negative"),
            ("POSITIVE_INT", 1, "Value must be positive"),
        ] {
            let codec =
                get_static_field(&mut rt, owner, name, "Lcom/mojang/serialization/Codec;").unwrap();
            assert_eq!(
                get_static_field(&mut rt, owner, name, "Lcom/mojang/serialization/Codec;"),
                Some(codec)
            );
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            for input in [minimum, i32::MAX] {
                let json = json::from_value(ctx.rt, serde_json::json!(input));
                let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, json]);
                let value = result::value(ctx.rt, decoded).unwrap();
                assert_eq!(ctx.rt.get_field(value, "$value"), V::I(input));
                let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, value]);
                assert_eq!(
                    json::to_value(ctx.rt, result::value(ctx.rt, encoded).unwrap()),
                    Some(serde_json::json!(input))
                );
            }
            for input in [i32::MIN, minimum - 1] {
                let json = json::from_value(ctx.rt, serde_json::json!(input));
                let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, json]);
                assert!(result::value(ctx.rt, decoded).is_none());
                assert!(ctx.rt.get_field(decoded, "$partial").is_null());
                assert_eq!(
                    result::message(&mut ctx, decoded),
                    format!("{message}: {input}")
                );
                let input = box_primitive(ctx.rt, "java/lang/Integer", V::I(input));
                let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, input]);
                assert!(result::value(ctx.rt, encoded).is_none());
                assert!(ctx.rt.get_field(encoded, "$partial").is_null());
                assert_eq!(
                    result::message(&mut ctx, encoded),
                    format!("{message}: {}", ctx.rt.get_field(input, "$value").i())
                );
            }
            let input = json::from_value(ctx.rt, serde_json::json!("1"));
            let result = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, input]);
            assert!(result::value(ctx.rt, result).is_none());
            assert!(ctx.rt.pending_exception.is_none());
        }
        assert!(get_static_field(&mut rt, owner, "NON_NEGATIVE_INT", "I").is_none());
        assert!(get_static_field(
            &mut rt,
            owner,
            "MADE_UP",
            "Lcom/mojang/serialization/Codec;"
        )
        .is_none());
    }

    #[test]
    fn product_p1_and_p2_preserves_getter_and_constructor_argument_order() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let codec = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let mut fields = Vec::new();
        for (name, id) in [("first", 1), ("second", 2), ("third", 12)] {
            let field = field_codec(&mut rt, codec, name);
            let getter = closure(&mut rt, id, vec![]);
            let builder = rt.new_instance(BUILDER, 0);
            rt.put_field(builder, "$builder_codec", field);
            rt.put_field(builder, "$builder_getter", getter);
            fields.push(builder);
        }
        let instance = rt.new_instance(INSTANCE, 0);
        let constructor = closure(&mut rt, 13, vec![]);
        let ops = get_static_field(
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
        let first = invoke(
            &mut ctx,
            INSTANCE,
            "group",
            "(Lcom/mojang/datafixers/kinds/App;)Lcom/mojang/datafixers/Products$P1;",
            false,
            &[instance, fields[0]],
        );
        let rest=invoke(&mut ctx,INSTANCE,"group","(Lcom/mojang/datafixers/kinds/App;Lcom/mojang/datafixers/kinds/App;)Lcom/mojang/datafixers/Products$P2;",false,&[instance,fields[1],fields[2]]);
        let joined = invoke(
            &mut ctx,
            "com/mojang/datafixers/Products$P1",
            "and",
            "(Lcom/mojang/datafixers/Products$P2;)Lcom/mojang/datafixers/Products$P3;",
            false,
            &[first, rest],
        );
        let builder=invoke(&mut ctx,"com/mojang/datafixers/Products$P3","apply","(Lcom/mojang/datafixers/kinds/Applicative;Lcom/mojang/datafixers/util/Function3;)Lcom/mojang/datafixers/kinds/App;",false,&[joined,instance,constructor]);
        let map = ctx.rt.get_field(builder, "$builder_codec");
        let record = decode_value(
            &mut ctx,
            map,
            ops,
            &serde_json::json!({"first":7,"second":8,"third":9}),
            false,
            0,
        )
        .unwrap();
        assert_eq!(
            ctx.rt.get_field(ctx.rt.get_field(record, "a"), "$value"),
            V::I(7)
        );
        assert_eq!(
            ctx.rt.get_field(ctx.rt.get_field(record, "b"), "$value"),
            V::I(8)
        );
        assert_eq!(
            ctx.rt.get_field(ctx.rt.get_field(record, "c"), "$value"),
            V::I(9)
        );
        assert_eq!(
            encode_value(&mut ctx, map, ops, record, false, 0).unwrap(),
            serde_json::json!({"first":7,"second":8,"third":9})
        );
    }
    #[test]
    fn non_empty_list_validates_both_decode_and_encode_without_replacing_empty() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let element = get_static_field(
            &mut rt,
            CODEC,
            "STRING",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let list = graph(&mut rt, CODEC, "list", &[element, V::I(0), V::I(99)]);
        let ops = get_static_field(
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
        let codec = invoke(
            &mut ctx,
            "net/minecraft/util/ExtraCodecs",
            "nonEmptyList",
            "(Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;",
            true,
            &[list],
        );
        assert!(decode_value(&mut ctx, codec, ops, &serde_json::json!([]), false, 0).is_err());
        let empty = ctx.rt.alloc("java/util/List", 0, Data::List(vec![]));
        assert!(encode_value(&mut ctx, codec, ops, empty, false, 0).is_err());
        let nonempty =
            decode_value(&mut ctx, codec, ops, &serde_json::json!(["ring"]), false, 0).unwrap();
        assert_eq!(
            encode_value(&mut ctx, codec, ops, nonempty, false, 0).unwrap(),
            serde_json::json!(["ring"])
        );
    }
    #[test]
    fn either_codec_preserves_branch_choice_partial_side_and_public_pair_accessors() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let int = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let string = get_static_field(
            &mut rt,
            CODEC,
            "STRING",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let codec=invoke(&mut ctx,CODEC,"either","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;",true,&[int,string]);
        for (input, left) in [
            (serde_json::json!(3), true),
            (serde_json::json!("three"), false),
        ] {
            let value = decode_value(&mut ctx, codec, ops, &input, false, 0).unwrap();
            assert_eq!(ctx.rt.get_field(value, "$is_left"), V::bool(left));
            assert_eq!(
                encode_value(&mut ctx, codec, ops, value, false, 0).unwrap(),
                input
            );
        }
        let ints = graph(ctx.rt, CODEC, "list", &[int, V::I(0), V::I(5)]);
        let strings = graph(ctx.rt, CODEC, "list", &[string, V::I(0), V::I(5)]);
        let either=invoke(&mut ctx,CODEC,"either","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;",true,&[ints,strings]);
        let failure = decode_value(
            &mut ctx,
            either,
            ops,
            &serde_json::json!([1, "x"]),
            false,
            0,
        )
        .unwrap_err();
        let partial = failure.partial.unwrap();
        assert_eq!(ctx.rt.get_field(partial, "$is_left"), V::TRUE);
        assert_eq!(
            array(ctx.rt, ctx.rt.get_field(partial, "$val"))
                .unwrap()
                .len(),
            1
        );
        let input = json::from_value(ctx.rt, serde_json::json!(3));
        let decoded=invoke(&mut ctx,CODEC,"decode","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,input]);
        let pair = result::value(ctx.rt, decoded).unwrap();
        let first = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "com/mojang/datafixers/util/Pair",
                "getFirst",
                "()Ljava/lang/Object;",
            ),
            &[pair],
        );
        assert!(!first.is_null());
        assert_eq!(ctx.rt.get_field(first, "$is_left"), V::TRUE);
        let second = invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(
                "com/mojang/datafixers/util/Pair",
                "getSecond",
                "()Ljava/lang/Object;",
            ),
            &[pair],
        );
        assert_eq!(
            json::to_value(ctx.rt, second),
            Some(serde_json::Value::Null)
        );
    }
    #[test]
    fn lazy_codec_supplier_is_deferred_and_optional_empty_map_does_not_force_it() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let counter = rt.new_instance("test/Counter", 0);
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let supplier = closure(&mut rt, 9, vec![counter, base]);
        let ops = get_static_field(
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
        let lazy = invoke(
            &mut ctx,
            CODEC,
            "lazyInitialized",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            true,
            &[supplier],
        );
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        let optional = invoke(
            &mut ctx,
            "net/minecraft/util/ExtraCodecs",
            "optionalEmptyMap",
            "(Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;",
            true,
            &[lazy],
        );
        let none = decode_value(&mut ctx, optional, ops, &serde_json::json!({}), false, 0).unwrap();
        assert!(matches!(
            &ctx.rt.get_obj(none).unwrap().data,
            Data::Optional(None)
        ));
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        for value in [3, 4] {
            let decoded =
                decode_value(&mut ctx, lazy, ops, &serde_json::json!(value), false, 0).unwrap();
            assert_eq!(ctx.rt.get_field(decoded, "$value"), V::I(value));
        }
        assert_eq!(ctx.rt.get_field(counter, "calls"), V::I(1));
        let list = invoke(
            &mut ctx,
            CODEC,
            "sizeLimitedListOf",
            "(I)Lcom/mojang/serialization/Codec;",
            false,
            &[lazy, V::I(1)],
        );
        let too_long =
            decode_value(&mut ctx, list, ops, &serde_json::json!([1, 2]), false, 0).unwrap_err();
        assert!(too_long.message.contains("too long"));
        assert!(too_long.partial.is_some());
    }
    #[test]
    fn lazy_codec_caches_null_but_retries_thrown_supplier_and_optional_empty_ignores_prefix() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let counter = rt.new_instance("test/Counter", 0);
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let flaky = closure(&mut rt, 11, vec![counter, base]);
        let ops = get_static_field(
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
        let lazy = invoke(
            &mut ctx,
            CODEC,
            "lazyInitialized",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            true,
            &[flaky],
        );
        assert!(decode_value(&mut ctx, lazy, ops, &serde_json::json!(1), false, 0).is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("SupplierFailure"));
        assert!(decode_value(&mut ctx, lazy, ops, &serde_json::json!(1), false, 0).is_ok());
        assert_eq!(ctx.rt.get_field(counter, "calls"), V::I(2));
        let null_counter = ctx.rt.new_instance("test/Counter", 0);
        let null_supplier = closure(ctx.rt, 9, vec![null_counter, V::NULL]);
        let lazy_null = invoke(
            &mut ctx,
            CODEC,
            "lazyInitialized",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            true,
            &[null_supplier],
        );
        for _ in 0..2 {
            assert!(
                decode_value(&mut ctx, lazy_null, ops, &serde_json::json!(1), false, 0).is_err()
            );
            assert!(ctx.rt.take_exception().is_some());
        }
        assert_eq!(ctx.rt.get_field(null_counter, "calls"), V::I(1));
        let optional = invoke(
            &mut ctx,
            "net/minecraft/util/ExtraCodecs",
            "optionalEmptyMap",
            "(Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/Codec;",
            true,
            &[base],
        );
        let none = ctx.rt.alloc("java/util/Optional", 0, Data::Optional(None));
        let prefix = json::from_value(ctx.rt, serde_json::json!({"ignored":1}));
        let encoded=invoke(&mut ctx,CODEC,"encode","(Ljava/lang/Object;Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[optional,none,ops,prefix]);
        assert_eq!(
            json::to_value(ctx.rt, result::value(ctx.rt, encoded).unwrap()),
            Some(serde_json::json!({}))
        );
    }
    #[test]
    fn or_else_get_runs_supplier_only_for_decode_errors_and_keeps_input_remainder() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
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
        let fallback = box_primitive(&mut rt, "java/lang/Integer", V::I(7));
        let counter = rt.new_instance("test/Counter", 0);
        let supplier = closure(&mut rt, 9, vec![counter, fallback]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = invoke(
            &mut ctx,
            CODEC,
            "orElseGet",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            false,
            &[base, supplier],
        );
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        let valid = json::from_value(ctx.rt, serde_json::json!(3));
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, valid]);
        assert_eq!(
            ctx.rt
                .get_field(result::value(ctx.rt, decoded).unwrap(), "$value"),
            V::I(3)
        );
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        let input = json::from_value(ctx.rt, serde_json::json!("bad"));
        for _ in 0..2 {
            let decoded = invoke(&mut ctx, CODEC, "decode", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, input]);
            let pair = result::value(ctx.rt, decoded).unwrap();
            assert_eq!(ctx.rt.get_field(pair, "first"), fallback);
            assert_eq!(
                json::to_value(ctx.rt, ctx.rt.get_field(pair, "second")),
                Some(serde_json::json!("bad"))
            );
        }
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 2);
        let list = invoke(
            &mut ctx,
            CODEC,
            "listOf",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[codec],
        );
        let input = json::from_value(ctx.rt, serde_json::json!([1, "bad", 2]));
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[list, ops, input]);
        let values = array(ctx.rt, result::value(ctx.rt, decoded).unwrap()).unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[1], fallback);
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 3);
        let wrong = ctx.rt.new_str("bad");
        let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, wrong]);
        assert!(result::value(ctx.rt, encoded).is_none());
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 3);
    }

    #[test]
    fn or_else_get_does_not_swallow_host_or_supplier_exceptions_or_use_partial_values() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
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
        let counter = rt.new_instance("test/Counter", 0);
        let fallback = box_primitive(&mut rt, "java/lang/Integer", V::I(7));
        let supplier = closure(&mut rt, 9, vec![counter, fallback]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let ranged = graph(ctx.rt, CODEC, "range", &[base, V::I(0), V::I(2)]);
        let guarded = invoke(
            &mut ctx,
            CODEC,
            "orElseGet",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            false,
            &[ranged, supplier],
        );
        let input = json::from_value(ctx.rt, serde_json::json!(5));
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[guarded, ops, input]);
        assert_eq!(result::value(ctx.rt, decoded), Some(fallback));
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 1);
        let unknown = graph(ctx.rt, CODEC, "unimplemented_test_node", &[]);
        let guarded = invoke(
            &mut ctx,
            CODEC,
            "orElseGet",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            false,
            &[unknown, supplier],
        );
        invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[guarded, ops, input]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 1);
        let guarded = invoke(
            &mut ctx,
            CODEC,
            "orElseGet",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            false,
            &[base, V::NULL],
        );
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[guarded, ops, input]);
        assert!(result::value(ctx.rt, decoded).is_some());
        let bad = json::from_value(ctx.rt, serde_json::json!("bad"));
        invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[guarded, ops, bad]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
        let throw_counter = ctx.rt.new_instance("test/Counter", 0);
        let throwing = closure(ctx.rt, 11, vec![throw_counter, fallback]);
        let guarded = invoke(
            &mut ctx,
            CODEC,
            "orElseGet",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/Codec;",
            false,
            &[base, throwing],
        );
        invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[guarded, ops, bad]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("test.SupplierFailure"));
    }

    #[test]
    fn or_else_defaults_real_data_errors_preserves_input_remainder_and_propagates_host_failure() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let fallback = box_primitive(&mut rt, "java/lang/Integer", V::I(7));
        let ops = get_static_field(
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
        let codec = invoke(
            &mut ctx,
            CODEC,
            "orElse",
            "(Ljava/lang/Object;)Lcom/mojang/serialization/Codec;",
            false,
            &[base, fallback],
        );
        let input = json::from_value(ctx.rt, serde_json::json!("bad"));
        let decoded=invoke(&mut ctx,CODEC,"decode","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,input]);
        let pair = result::value(ctx.rt, decoded).unwrap();
        assert_eq!(ctx.rt.get_field(pair, "first"), fallback);
        assert_eq!(
            json::to_value(ctx.rt, ctx.rt.get_field(pair, "second")),
            Some(serde_json::json!("bad"))
        );
        let unknown = graph(ctx.rt, CODEC, "unimplemented_test_node", &[]);
        let guarded = invoke(
            &mut ctx,
            CODEC,
            "orElse",
            "(Ljava/lang/Object;)Lcom/mojang/serialization/Codec;",
            false,
            &[unknown, fallback],
        );
        invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[guarded,ops,input]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        is_static: bool,
        args: &[V],
    ) -> V {
        call(
            ctx.rt,
            ctx.linked,
            ctx.host,
            &MemberRef::new(owner, name, desc),
            is_static,
            args,
        )
        .expect("implemented DFU call")
    }
    #[test]
    fn unbounded_map_preserves_values_partial_errors_and_the_original_remainder() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let string = get_static_field(
            &mut rt,
            CODEC,
            "STRING",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let int = get_static_field(
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
        let codec=invoke(&mut ctx,CODEC,"unboundedMap","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/codecs/UnboundedMapCodec;",true,&[string,int]);
        let input = serde_json::json!({"one":1,"two":2});
        let decoded = decode_value(&mut ctx, codec, ops, &input, false, 0).unwrap();
        assert_eq!(
            ctx.rt.class_of(decoded),
            Some("com/google/common/collect/ImmutableMap")
        );
        assert_eq!(
            encode_value(&mut ctx, codec, ops, decoded, false, 0).unwrap(),
            input
        );
        let input = serde_json::json!({"bad":"wrong","good":7});
        let input_ref = json::from_value(ctx.rt, input.clone());
        let result=invoke(&mut ctx,CODEC,"decode","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,input_ref]);
        assert_eq!(ctx.rt.get_field(result, "$success"), V::FALSE);
        let pair = ctx.rt.get_field(result, "$partial");
        let map = ctx.rt.get_field(pair, "first");
        assert_eq!(
            json::to_value(ctx.rt, ctx.rt.get_field(pair, "second")),
            Some(input)
        );
        assert_eq!(
            encode_value(&mut ctx, codec, ops, map, false, 0).unwrap(),
            serde_json::json!({"good":7})
        );
        assert!(result::message(&mut ctx, result).contains("missed input"));
        let key = ctx.rt.intern_str("new");
        let value = box_primitive(ctx.rt, "java/lang/Integer", V::I(9));
        invoke_member(
            &mut ctx,
            op::INVOKEINTERFACE,
            &MemberRef::new(
                "java/util/Map",
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            ),
            &[map, key, value],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
    #[test]
    fn dispatch_map_executes_type_selector_child_codec_and_rechecks_type_on_encode() {
        fn kind(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            let calls = ctx.rt.get_field(args[0], "calls").i();
            ctx.rt.put_field(args[0], "calls", V::I(calls + 1));
            ctx.rt.get_field(args[0], "kind")
        }
        fn select(_ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            args[0]
        }
        fn get(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            ctx.rt.get_field(args[0], "value")
        }
        fn construct(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
            let value = ctx.rt.new_instance("test/Variant", 0);
            let kind = ctx.rt.intern_str("demo");
            ctx.rt.put_field(value, "kind", kind);
            ctx.rt.put_field(value, "value", args[0]);
            value
        }
        fn fn_value(rt: &mut Rt, id: u32, captures: Vec<V>) -> V {
            rt.alloc(
                "java/util/function/Function",
                0,
                Data::Closure(Box::new(mmga_runtime::ClosureData {
                    interface_class: "java/util/function/Function".into(),
                    sam_name: "apply".into(),
                    sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                    instantiated_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                    captures,
                    target: mmga_runtime::ClosureTarget::NativeLambda(id),
                })),
            )
        }
        let mut linked = LinkedSpace::new();
        linked.register_native_module(mmga_runtime::NativeModuleRegistry {
            classes: &[],
            lambdas: &[(501, kind), (502, select), (503, get), (504, construct)],
        });
        let mut host = NullHost;
        let mut rt = Rt::new();
        let string = get_static_field(
            &mut rt,
            CODEC,
            "STRING",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let int = get_static_field(
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
        let getter = fn_value(&mut rt, 503, vec![]);
        let constructor = fn_value(&mut rt, 504, vec![]);
        let field = field_codec(&mut rt, int, "value");
        let child = record_map_codec(&mut rt, &[(field, getter)], constructor);
        let type_fn = fn_value(&mut rt, 501, vec![]);
        let selector = fn_value(&mut rt, 502, vec![child]);
        let field_name = rt.intern_str("kind");
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let map=invoke(&mut ctx,CODEC,"dispatchMap","(Ljava/lang/String;Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/MapCodec;",false,&[string,field_name,type_fn,selector]);
        let data = decode_value(
            &mut ctx,
            map,
            ops,
            &serde_json::json!({"kind":"demo","value":7}),
            false,
            0,
        )
        .unwrap();
        assert_eq!(ctx.rt.get_field(data, "calls"), V::NULL);
        assert_eq!(
            encode_value(&mut ctx, map, ops, data, false, 0).unwrap(),
            serde_json::json!({"kind":"demo","value":7})
        );
        assert_eq!(ctx.rt.get_field(data, "calls"), V::I(2));
        let error = decode_value(
            &mut ctx,
            map,
            ops,
            &serde_json::json!({"value":7}),
            false,
            0,
        )
        .unwrap_err();
        assert!(error
            .message
            .contains("Input does not contain a key [kind]"));
        let bad = fn_value(ctx.rt, 502, vec![int]);
        let bad_map=invoke(&mut ctx,CODEC,"dispatchMap","(Ljava/lang/String;Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/MapCodec;",false,&[string,field_name,type_fn,bad]);
        assert!(decode_value(
            &mut ctx,
            bad_map,
            ops,
            &serde_json::json!({"kind":"demo","value":7}),
            false,
            0
        )
        .is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("ClassCastException"));
        assert!(decode_value(
            &mut ctx,
            map,
            ops,
            &serde_json::json!({"kind":"demo","value":7}),
            true,
            0
        )
        .is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
    #[test]
    fn unbounded_map_detects_duplicate_decoded_keys_and_never_swallow_unknown_codecs() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let key = get_static_field(
            &mut rt,
            "net/minecraft/resources/ResourceLocation",
            "CODEC",
            "Lcom/mojang/serialization/Codec;",
        )
        .unwrap();
        let value = get_static_field(
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
        let codec=invoke(&mut ctx,CODEC,"unboundedMap","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/codecs/UnboundedMapCodec;",true,&[key,value]);
        let error = decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!({"a":1,"minecraft:a":2}),
            false,
            0,
        )
        .unwrap_err();
        assert!(error.message.contains("Duplicate entry for key"));
        assert!(error.partial.is_some());
        assert_eq!(
            encode_value(&mut ctx, codec, ops, error.partial.unwrap(), false, 0).unwrap(),
            serde_json::json!({"minecraft:a":1})
        );
        let unknown = unsupported_codec(ctx.rt, "test");
        let map=invoke(&mut ctx,CODEC,"unboundedMap","(Lcom/mojang/serialization/Codec;Lcom/mojang/serialization/Codec;)Lcom/mojang/serialization/codecs/UnboundedMapCodec;",true,&[key,unknown]);
        assert!(decode_value(&mut ctx, map, ops, &serde_json::json!({"a":1}), false, 0).is_err());
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
    #[test]
    fn primitive_json_codecs_roundtrip_values_and_report_wrong_types() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let codec = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let input = box_primitive(&mut rt, "java/lang/Integer", V::I(129));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let encoded = invoke(&mut ctx, CODEC, "encodeStart", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, input]);
        let encoded = invoke(
            &mut ctx,
            RESULT,
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[encoded],
        );
        assert_eq!(
            json::to_value(ctx.rt, encoded).unwrap(),
            serde_json::json!(129)
        );
        let decoded = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, encoded]);
        let decoded = invoke(
            &mut ctx,
            RESULT,
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[decoded],
        );
        assert_eq!(ctx.rt.get_field(decoded, "$value"), V::I(129));
        let wrong = json::from_value(ctx.rt, serde_json::json!("129"));
        let failed = invoke(&mut ctx, CODEC, "parse", "(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;", false, &[codec, ops, wrong]);
        assert_eq!(
            invoke(&mut ctx, RESULT, "isError", "()Z", false, &[failed]),
            V::TRUE
        );
        invoke(
            &mut ctx,
            RESULT,
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[failed],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("Not a number"));
    }

    fn closure(rt: &mut Rt, id: u32, captures: Vec<V>) -> V {
        rt.alloc(
            "java/util/function/Function",
            0,
            Data::Closure(Box::new(mmga_runtime::ClosureData {
                interface_class: "java/util/function/Function".into(),
                sam_name: "apply".into(),
                sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                instantiated_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                captures,
                target: mmga_runtime::ClosureTarget::NativeLambda(id),
            })),
        )
    }
    fn field_a(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.get_field(args[0], "a")
    }
    fn field_b(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.get_field(args[0], "b")
    }
    fn field_c(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        ctx.rt.get_field(args[0], "c")
    }
    fn construct_three(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let value = construct(ctx, args);
        ctx.rt.put_field(value, "c", args[2]);
        value
    }
    fn construct(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let value = ctx.rt.new_instance("example/Record", 0);
        ctx.rt.put_field(value, "a", args[0]);
        ctx.rt.put_field(value, "b", args[1]);
        value
    }
    fn build_record(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let primitive = get_static_field(
            ctx.rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let string = get_static_field(
            ctx.rt,
            CODEC,
            "STRING",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let a = ctx.rt.intern_str("a");
        let b = ctx.rt.intern_str("b");
        let default = ctx.rt.intern_str("default");
        let field = invoke(
            ctx,
            CODEC,
            "fieldOf",
            "(Ljava/lang/String;)Lcom/mojang/serialization/MapCodec;",
            false,
            &[primitive, a],
        );
        let optional = invoke(
            ctx,
            CODEC,
            "optionalFieldOf",
            "(Ljava/lang/String;Ljava/lang/Object;)Lcom/mojang/serialization/MapCodec;",
            false,
            &[string, b, default],
        );
        let ga = closure(ctx.rt, 1, vec![]);
        let gb = closure(ctx.rt, 2, vec![]);
        let ctor = closure(ctx.rt, 3, vec![]);
        let ba = invoke(
            ctx,
            MAP,
            "forGetter",
            "(Ljava/util/function/Function;)Lcom/mojang/serialization/codecs/RecordCodecBuilder;",
            false,
            &[field, ga],
        );
        let bb = invoke(
            ctx,
            MAP,
            "forGetter",
            "(Ljava/util/function/Function;)Lcom/mojang/serialization/codecs/RecordCodecBuilder;",
            false,
            &[optional, gb],
        );
        let grouped=invoke(ctx,INSTANCE,"group","(Lcom/mojang/datafixers/kinds/App;Lcom/mojang/datafixers/kinds/App;)Lcom/mojang/datafixers/Products$P2;",false,&[args[0],ba,bb]);
        invoke(ctx,"com/mojang/datafixers/Products$P2","apply","(Lcom/mojang/datafixers/kinds/Applicative;Ljava/util/function/BiFunction;)Lcom/mojang/datafixers/kinds/App;",false,&[grouped,args[0],ctor])
    }
    fn increment(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let n = ctx.rt.get_field(args[0], "$value").i() + 1;
        box_primitive(ctx.rt, "java/lang/Integer", V::I(n))
    }
    fn decrement(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let n = ctx.rt.get_field(args[0], "$value").i() - 1;
        box_primitive(ctx.rt, "java/lang/Integer", V::I(n))
    }
    fn positive(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        if ctx.rt.get_field(args[0], "$value").i() > 0 {
            result::success(ctx.rt, args[0])
        } else {
            result::error(ctx.rt, "must be positive", None)
        }
    }
    fn identity(_ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        args[0]
    }
    fn unit_supplier(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let n = ctx.rt.get_field(args[0], "calls").i() + 1;
        ctx.rt.put_field(args[0], "calls", V::I(n));
        args[1]
    }
    fn fail_first_supplier(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let calls = ctx.rt.get_field(args[0], "calls").i() + 1;
        ctx.rt.put_field(args[0], "calls", V::I(calls));
        if calls == 1 {
            ctx.rt.throw_exception("test/SupplierFailure", "first")
        } else {
            args[1]
        }
    }
    fn partial_negative(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let value = box_primitive(
            ctx.rt,
            "java/lang/Integer",
            V::I(-ctx.rt.get_field(args[0], "$value").i()),
        );
        result::error(ctx.rt, "negated with warning", Some(value))
    }
    fn test_linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(mmga_runtime::NativeModuleRegistry {
            classes: &[],
            lambdas: &[
                (1, field_a),
                (2, field_b),
                (3, construct),
                (4, build_record),
                (5, increment),
                (6, decrement),
                (7, positive),
                (8, identity),
                (9, unit_supplier),
                (10, partial_negative),
                (11, fail_first_supplier),
                (12, field_c),
                (13, construct_three),
            ],
        });
        linked
    }

    #[test]
    fn record_builder_executes_getters_constructor_defaults_and_rejects_invalid_fields() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let builder = closure(&mut rt, 4, vec![]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let codec = invoke(
            &mut ctx,
            BUILDER,
            "create",
            "(Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;",
            true,
            &[builder],
        );
        let json = json::from_value(ctx.rt, serde_json::json!({"a":7}));
        let parsed=invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,json]);
        let record = invoke(
            &mut ctx,
            RESULT,
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[parsed],
        );
        assert_eq!(
            ctx.rt.get_field(ctx.rt.get_field(record, "a"), "$value"),
            V::I(7)
        );
        assert_eq!(
            ctx.rt.str_of(ctx.rt.get_field(record, "b")),
            Some("default")
        );
        let encoded=invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,record]);
        assert_eq!(
            json::to_value(ctx.rt, result::value(ctx.rt, encoded).unwrap()),
            Some(serde_json::json!({"a":7}))
        );
        for malformed in [serde_json::json!({}), serde_json::json!({"a":7,"b":false})] {
            let malformed = json::from_value(ctx.rt, malformed);
            let failed=invoke(&mut ctx,CODEC,"parse","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,malformed]);
            assert!(result::value(ctx.rt, failed).is_none());
            assert!(!result::message(&mut ctx, failed).is_empty());
        }
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn mapped_list_codecs_execute_both_directions_and_flat_mapping_reports_errors() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let forward = closure(&mut rt, 5, vec![]);
        let back = closure(&mut rt, 6, vec![]);
        let positive = closure(&mut rt, 7, vec![]);
        let identity = closure(&mut rt, 8, vec![]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let mapped=invoke(&mut ctx,CODEC,"xmap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;",false,&[base,forward,back]);
        let list = invoke(
            &mut ctx,
            CODEC,
            "listOf",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[mapped],
        );
        let decoded =
            decode_value(&mut ctx, list, ops, &serde_json::json!([1, 2]), false, 0).unwrap();
        let values = array(ctx.rt, decoded).unwrap();
        assert_eq!(ctx.rt.get_field(values[0], "$value"), V::I(2));
        assert_eq!(
            encode_value(&mut ctx, list, ops, decoded, false, 0).unwrap(),
            serde_json::json!([1, 2])
        );
        let checked=invoke(&mut ctx,CODEC,"comapFlatMap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;",false,&[base,positive,identity]);
        assert_eq!(
            decode_value(&mut ctx, checked, ops, &serde_json::json!(-1), false, 0)
                .unwrap_err()
                .message,
            "must be positive"
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn map_unit_supplier_is_lazy_and_runs_each_decode_with_empty_encoding() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let counter = rt.new_instance("example/Counter", 0);
        let value = rt.new_str("payload");
        let supplier = closure(&mut rt, 9, vec![counter, value]);
        let ops = get_static_field(
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
        let map = invoke(
            &mut ctx,
            MAP,
            "unit",
            "(Ljava/util/function/Supplier;)Lcom/mojang/serialization/MapCodec;",
            true,
            &[supplier],
        );
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        let codec = invoke(
            &mut ctx,
            MAP,
            "codec",
            "()Lcom/mojang/serialization/Codec;",
            false,
            &[map],
        );
        assert_eq!(
            encode_value(&mut ctx, codec, ops, value, false, 0).unwrap(),
            serde_json::json!({})
        );
        assert_eq!(ctx.rt.get_field(counter, "calls").i(), 0);
        for _ in 0..2 {
            assert_eq!(
                decode_value(&mut ctx, codec, ops, &serde_json::json!({}), false, 0).unwrap(),
                value
            );
        }
        assert_eq!(ctx.rt.get_field(counter, "calls"), V::I(2));
    }

    #[test]
    fn invalid_list_elements_remain_errors_with_partial_values_and_failed_remainder() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let codec = graph(&mut rt, CODEC, "list", &[base, V::I(0), V::I(10)]);
        let input = json::from_value(&mut rt, serde_json::json!([1, "bad", 3]));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let decoded=invoke(&mut ctx,CODEC,"decode","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[codec,ops,input]);
        assert_eq!(
            invoke(&mut ctx, RESULT, "isError", "()Z", false, &[decoded]),
            V::TRUE
        );
        let pair = invoke(
            &mut ctx,
            RESULT,
            "getPartialOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[decoded],
        );
        let values = array(ctx.rt, ctx.rt.get_field(pair, "first")).unwrap();
        assert_eq!(
            values
                .iter()
                .map(|v| ctx.rt.get_field(*v, "$value").i())
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(
            json::to_value(ctx.rt, ctx.rt.get_field(pair, "second")),
            Some(serde_json::json!(["bad"]))
        );
        invoke(
            &mut ctx,
            RESULT,
            "getOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[decoded],
        );
        assert!(ctx.rt.take_exception().is_some());
    }

    #[test]
    fn uuid_codec_preserves_signed_int_order_and_length_error_partial() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let codec = get_static_field(
            &mut rt,
            "net/minecraft/core/UUIDUtil",
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
        let input = serde_json::json!([-1, 1, i32::MIN, 42]);
        let uuid = decode_value(&mut ctx, codec, ops, &input, false, 0).unwrap();
        assert_eq!(
            ctx.rt.handle_of(uuid).unwrap().3,
            [-4294967295, i64::MIN + 42]
        );
        assert_eq!(
            encode_value(&mut ctx, codec, ops, uuid, false, 0).unwrap(),
            input
        );
        assert!(decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!([1, 2, 3]),
            false,
            0
        )
        .unwrap_err()
        .partial
        .is_none());
        let overlong = decode_value(
            &mut ctx,
            codec,
            ops,
            &serde_json::json!([-1, 1, i32::MIN, 42, 9]),
            false,
            0,
        )
        .unwrap_err();
        assert!(ctx.rt.val_eq(overlong.partial.unwrap(), uuid));
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn flat_encode_keeps_partial_payload_but_never_marks_it_successful() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let base = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let decoder = closure(&mut rt, 8, vec![]);
        let encoder = closure(&mut rt, 10, vec![]);
        let value = box_primitive(&mut rt, "java/lang/Integer", V::I(5));
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let mapped=invoke(&mut ctx,CODEC,"flatComapMap","(Ljava/util/function/Function;Ljava/util/function/Function;)Lcom/mojang/serialization/Codec;",false,&[base,decoder,encoder]);
        let encoded=invoke(&mut ctx,CODEC,"encodeStart","(Lcom/mojang/serialization/DynamicOps;Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",false,&[mapped,ops,value]);
        assert_eq!(
            invoke(&mut ctx, RESULT, "isSuccess", "()Z", false, &[encoded]),
            V::FALSE
        );
        let partial = invoke(
            &mut ctx,
            RESULT,
            "getPartialOrThrow",
            "()Ljava/lang/Object;",
            false,
            &[encoded],
        );
        assert_eq!(json::to_value(ctx.rt, partial), Some(serde_json::json!(-5)));
        assert_eq!(result::message(&mut ctx, encoded), "negated with warning");
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn alternative_prefers_first_partial_and_bad_prefix_preserves_correct_partial() {
        let mut rt = Rt::new();
        let linked = test_linked();
        let mut host = NullHost;
        let ops = get_static_field(
            &mut rt,
            JSON_OPS,
            "INSTANCE",
            "Lcom/mojang/serialization/JsonOps;",
        )
        .unwrap();
        let int = get_static_field(
            &mut rt,
            CODEC,
            "INT",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let string = get_static_field(
            &mut rt,
            CODEC,
            "STRING",
            "Lcom/mojang/serialization/codecs/PrimitiveCodec;",
        )
        .unwrap();
        let ints = graph(&mut rt, CODEC, "list", &[int, V::I(0), V::I(8)]);
        let strings = graph(&mut rt, CODEC, "list", &[string, V::I(0), V::I(8)]);
        let alternative = graph(&mut rt, CODEC, "alternative", &[ints, strings]);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let failed = decode_value(
            &mut ctx,
            alternative,
            ops,
            &serde_json::json!([1, "x"]),
            false,
            0,
        )
        .unwrap_err();
        let partial = array(ctx.rt, failed.partial.unwrap()).unwrap();
        assert_eq!(partial.len(), 1);
        assert_eq!(ctx.rt.get_field(partial[0], "$value"), V::I(1));
        let prefix = json::from_value(ctx.rt, serde_json::json!({"existing":true}));
        let wrong = merge_prefix(ctx.rt, Some(prefix), serde_json::json!(5)).unwrap_err();
        assert_eq!(wrong.partial, Some(serde_json::json!(5)));
        let scalar = json::from_value(ctx.rt, serde_json::json!(42));
        let wrong = merge_prefix(ctx.rt, Some(scalar), serde_json::json!([1])).unwrap_err();
        assert_eq!(wrong.partial, Some(serde_json::json!(42)));
    }

    #[test]
    fn unrepresentable_json_numbers_and_null_optional_results_do_not_fake_values() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let ops = get_static_field(
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
        invoke(
            &mut ctx,
            JSON_OPS,
            "createDouble",
            "(D)Ljava/lang/Object;",
            false,
            &[ops, V::D(f64::NAN)],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        let success = invoke(
            &mut ctx,
            RESULT,
            "success",
            "(Ljava/lang/Object;)Lcom/mojang/serialization/DataResult;",
            true,
            &[V::NULL],
        );
        invoke(
            &mut ctx,
            RESULT,
            "resultOrPartial",
            "()Ljava/util/Optional;",
            false,
            &[success],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NullPointerException"));
    }
}

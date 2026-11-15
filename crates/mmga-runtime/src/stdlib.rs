//! Native Rust implementation of the Java standard library (`java.*`, `org.slf4j`, `com.google.common`,
//! `org.spongepowered.asm.mixin.injection.callback.CallbackInfo*`) for NeoForge mods.

use crate::heap::{Data, Rt};
use crate::linked::ExecCtx;
use crate::value::V;
use mmga_classfile::MemberRef;

/// Resolves `GETSTATIC` on built-in JDK / logging / Mixin classes.
pub fn stdlib_get_static(rt: &mut Rt, owner: &str, name: &str, _desc: &str) -> Option<V> {
    match (owner, name) {
        ("java/lang/System", "out") => {
            Some(rt.new_pinned_handle("java/io/PrintStream", "print_stream", 1, "System.out"))
        }
        ("java/lang/System", "err") => {
            Some(rt.new_pinned_handle("java/io/PrintStream", "print_stream", 2, "System.err"))
        }
        ("java/lang/Boolean", "TRUE") => Some(box_primitive(rt, "java/lang/Boolean", V::I(1))),
        ("java/lang/Boolean", "FALSE") => Some(box_primitive(rt, "java/lang/Boolean", V::I(0))),
        ("java/util/Collections", "EMPTY_LIST") => {
            Some(rt.alloc_pinned("java/util/List", 0, Data::List(Vec::new())))
        }
        ("java/util/Collections", "EMPTY_SET") => {
            Some(rt.alloc_pinned("java/util/Set", 0, Data::Set(Vec::new())))
        }
        ("java/util/Collections", "EMPTY_MAP") => {
            Some(rt.alloc_pinned("java/util/Map", 0, Data::Map(Vec::new())))
        }
        ("java/util/Locale", "ROOT" | "US" | "ENGLISH") => {
            Some(rt.new_pinned_handle("java/util/Locale", "locale", 0, name))
        }
        ("java/nio/charset/StandardCharsets", "UTF_8") => {
            Some(rt.new_pinned_handle("java/nio/charset/Charset", "charset", 0, "UTF-8"))
        }
        _ => None,
    }
}

pub fn box_primitive(rt: &mut Rt, box_class: &str, raw: V) -> V {
    let obj = rt.new_instance(box_class, 0);
    rt.put_field(obj, "$value", raw);
    obj
}

pub fn unbox_primitive(rt: &Rt, v: V) -> V {
    match v {
        V::I(_) | V::J(_) | V::F(_) | V::D(_) => v,
        V::R(_) => rt
            .get_obj(v)
            .and_then(|o| match &o.data {
                Data::Fields(m) => m.get("$value").copied(),
                _ => None,
            })
            .unwrap_or(V::I(0)),
        V::Void => V::I(0),
    }
}

/// Attempts to invoke a built-in JDK / logging / functional / Mixin callback method natively.
/// `args` includes `this` at `args[0]` when `!is_static`.
pub fn stdlib_call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    // 0. Functional interfaces (`Consumer`, `Supplier`, `Function`, `BiFunction`, `Predicate`, `Runnable`, `Callable`, etc.)
    //    when the receiver is a `Data::Closure`!
    if !is_static {
        if let Some(&recv) = args.first() {
            let is_closure = matches!(ctx.rt.get_obj(recv).map(|o| &o.data), Some(Data::Closure(_)));
            if is_closure && name != "getClass" && name != "hashCode" && name != "toString" && name != "equals" {
                return Some(crate::vm::invoke_closure(ctx, recv, &args[1..]));
            }
        }
    }

    // 1. Mixin `CallbackInfo` / `CallbackInfoReturnable`
    if owner == "org/spongepowered/asm/mixin/injection/callback/CallbackInfo"
        || owner == "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable"
    {
        let recv = args.first().copied().unwrap_or(V::NULL);
        match name {
            "cancel" => {
                if let Some(obj) = ctx.rt.get_obj_mut(recv) {
                    if let Data::CallbackInfo {
                        cancellable,
                        cancelled,
                        ..
                    } = &mut obj.data
                    {
                        if *cancellable {
                            *cancelled = true;
                        }
                    }
                }
                return Some(V::Void);
            }
            "isCancelled" => {
                let c = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                    Some(Data::CallbackInfo { cancelled, .. }) => *cancelled,
                    _ => false,
                };
                return Some(V::bool(c));
            }
            "isCancellable" => {
                let c = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                    Some(Data::CallbackInfo { cancellable, .. }) => *cancellable,
                    _ => false,
                };
                return Some(V::bool(c));
            }
            "setReturnValue" => {
                let ret_val = args.get(1).copied().unwrap_or(V::NULL);
                if let Some(obj) = ctx.rt.get_obj_mut(recv) {
                    if let Data::CallbackInfo {
                        cancellable,
                        cancelled,
                        return_value,
                        ..
                    } = &mut obj.data
                    {
                        if *cancellable {
                            *cancelled = true;
                            *return_value = Some(ret_val);
                        }
                    }
                }
                return Some(V::Void);
            }
            "getReturnValue"
            | "getReturnValueI"
            | "getReturnValueJ"
            | "getReturnValueF"
            | "getReturnValueD"
            | "getReturnValueZ" => {
                let rv = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                    Some(Data::CallbackInfo { return_value, .. }) => {
                        return_value.unwrap_or(V::NULL)
                    }
                    _ => V::NULL,
                };
                return Some(rv);
            }
            _ => {}
        }
    }

    // 2. `java/lang/Object` & `java/lang/Record` & `java/lang/Throwable` constructors/methods
    match (owner, name, desc) {
        ("java/lang/Object" | "java/lang/Record" | "java/lang/Enum", "<init>", _) => {
            if owner == "java/lang/Enum" && args.len() >= 3 {
                ctx.rt.put_field(args[0], "$name", args[1]);
                ctx.rt.put_field(args[0], "$ordinal", args[2]);
            }
            return Some(V::Void);
        }
        (
            "java/lang/Throwable"
            | "java/lang/Exception"
            | "java/lang/RuntimeException"
            | "java/lang/IllegalArgumentException"
            | "java/lang/IllegalStateException"
            | "java/lang/NullPointerException"
            | "java/lang/UnsupportedOperationException"
            | "java/lang/IndexOutOfBoundsException"
            | "java/lang/AssertionError"
            | "java/lang/Error",
            "<init>",
            _,
        ) => {
            if let Some(&this) = args.first() {
                if let Some(&msg) = args.get(1) {
                    if ctx.rt.str_of(msg).is_some() {
                        ctx.rt.put_field(this, "detailMessage", msg);
                    }
                }
            }
            return Some(V::Void);
        }
        (_, "getMessage", "()Ljava/lang/String;") if !is_static => {
            let msg = ctx.rt.get_field(args[0], "detailMessage");
            return Some(msg);
        }
        ("java/lang/Enum", "name", "()Ljava/lang/String;") => {
            return Some(ctx.rt.get_field(args[0], "$name"));
        }
        ("java/lang/Enum", "ordinal", "()I") => {
            return Some(ctx.rt.get_field(args[0], "$ordinal"));
        }
        (_, "getClass", "()Ljava/lang/Class;") if !is_static => {
            let cls = ctx
                .rt
                .class_of(args[0])
                .unwrap_or("java/lang/Object")
                .to_owned();
            return Some(ctx.rt.new_pinned_handle("java/lang/Class", "class", 0, cls));
        }
        ("java/lang/Class", "getName", "()Ljava/lang/String;") => {
            let tag = ctx
                .rt
                .handle_of(args[0])
                .map(|(_, _, t, _)| t.replace('/', "."))
                .unwrap_or_else(|| "java.lang.Object".into());
            return Some(ctx.rt.new_str(tag));
        }
        ("java/lang/Class", "getSimpleName", "()Ljava/lang/String;") => {
            let simple = ctx
                .rt
                .handle_of(args[0])
                .map(|(_, _, t, _)| {
                    t.rsplit(['/', '$', '.'])
                        .next()
                        .unwrap_or(t)
                        .to_owned()
                })
                .unwrap_or_else(|| "Object".into());
            return Some(ctx.rt.new_str(simple));
        }
        _ => {}
    }

    // 3. `java/lang/Objects`
    if owner == "java/lang/Objects" {
        return match name {
            "requireNonNull" => {
                let v = args.first().copied().unwrap_or(V::NULL);
                if v.is_null() {
                    let msg = args
                        .get(1)
                        .and_then(|m| ctx.rt.str_of(*m))
                        .unwrap_or("Objects.requireNonNull")
                        .to_owned();
                    Some(ctx.rt.throw_exception("java/lang/NullPointerException", &msg))
                } else {
                    Some(v)
                }
            }
            "equals" => Some(V::bool(ctx.rt.val_eq(args[0], args[1]))),
            "nonNull" => Some(V::bool(!args[0].is_null())),
            "isNull" => Some(V::bool(args[0].is_null())),
            "hashCode" => Some(V::I(args[0].r() as i32 ^ args[0].i())),
            "hash" => {
                let arr = args.first().copied().unwrap_or(V::NULL);
                let mut h = 1i32;
                if let Some(Data::Array(items)) = ctx.rt.get_obj(arr).map(|o| &o.data) {
                    for &item in items {
                        h = h.wrapping_mul(31).wrapping_add(item.i() ^ item.r() as i32);
                    }
                }
                Some(V::I(h))
            }
            "toString" => {
                let s = ctx.rt.value_to_string(args[0]);
                Some(ctx.rt.new_str(s))
            }
            _ => None,
        };
    }

    // 4. Primitive wrappers (`Integer`, `Long`, `Float`, `Double`, `Boolean`, `Byte`, `Short`, `Character`)
    if let Some(res) = dispatch_boxed_primitives(ctx.rt, owner, name, desc, is_static, args) {
        return Some(res);
    }

    // 5. `java/lang/Math` & `java/lang/StrictMath`
    if owner == "java/lang/Math" || owner == "java/lang/StrictMath" {
        return dispatch_math(name, desc, args);
    }

    // 6. `java/lang/String` & `java/lang/StringBuilder`
    if owner == "java/lang/String" {
        return dispatch_string(ctx.rt, name, desc, is_static, args);
    }
    if owner == "java/lang/StringBuilder" || owner == "java/lang/StringBuffer" {
        return dispatch_string_builder(ctx.rt, owner, name, desc, args);
    }

    // 7. `java/lang/System` & `java/io/PrintStream` & SLF4J / Log4j Loggers
    if owner == "java/lang/System" {
        if name == "load" || name == "loadLibrary" {
            let lib = args
                .first()
                .map(|&v| ctx.rt.value_to_string(v))
                .unwrap_or_default();
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsatisfiedLinkError",
                &format!("MMGA does not run JNI libraries ({lib})"),
            ));
        }
        return match name {
            "currentTimeMillis" => Some(V::J(1_763_138_880_000)),
            "nanoTime" => Some(V::J(1_000_000_000)),
            "identityHashCode" => Some(V::I(args.first().map_or(0, |v| v.r() as i32))),
            "arraycopy" if args.len() == 5 => {
                let (src, src_pos, dst, dst_pos, len) = (
                    args[0],
                    args[1].i().max(0) as usize,
                    args[2],
                    args[3].i().max(0) as usize,
                    args[4].i().max(0) as usize,
                );
                let slice: Option<Vec<V>> = match ctx.rt.get_obj(src).map(|o| &o.data) {
                    Some(Data::Array(items)) => items.get(src_pos..src_pos + len).map(|s| s.to_vec()),
                    _ => None,
                };
                if let (Some(copied), Some(dst_obj)) = (slice, ctx.rt.get_obj_mut(dst)) {
                    if let Data::Array(dst_items) = &mut dst_obj.data {
                        for (k, v) in copied.into_iter().enumerate() {
                            if dst_pos + k < dst_items.len() {
                                dst_items[dst_pos + k] = v;
                            }
                        }
                    }
                }
                Some(V::Void)
            }
            _ => None,
        };
    }

    if owner == "java/io/PrintStream" && (name == "println" || name == "print") {
        let text = args
            .get(1)
            .map(|&v| ctx.rt.value_to_string(v))
            .unwrap_or_default();
        ctx.rt.telemetry.logs.push(text);
        return Some(V::Void);
    }

    if owner == "org/slf4j/LoggerFactory"
        || owner == "com/mojang/logging/LogUtils"
        || owner == "org/apache/logging/log4j/LogManager"
    {
        if name == "getLogger" {
            let tag = args
                .first()
                .map(|&v| ctx.rt.value_to_string(v))
                .unwrap_or_else(|| "mod".into());
            return Some(ctx.rt.new_pinned_handle("org/slf4j/Logger", "logger", 0, tag));
        }
    }
    if owner == "org/slf4j/Logger" || owner == "org/apache/logging/log4j/Logger" {
        match name {
            "info" | "warn" | "error" | "debug" | "trace" => {
                let fmt = args
                    .get(1)
                    .map(|&v| ctx.rt.value_to_string(v))
                    .unwrap_or_default();
                let formatted = format_slf4j(&fmt, &args[2..], ctx.rt);
                ctx.rt
                    .telemetry
                    .logs
                    .push(format!("[{}] {}", name.to_ascii_uppercase(), formatted));
                return Some(V::Void);
            }
            "isDebugEnabled" | "isTraceEnabled" => return Some(V::FALSE),
            "isInfoEnabled" | "isWarnEnabled" | "isErrorEnabled" => return Some(V::TRUE),
            _ => {}
        }
    }

    // 8. Collections, Optional, Arrays, Streams, Iterators (`java/util/*`)
    if owner.starts_with("java/util/") {
        if let Some(res) = dispatch_java_util(ctx, owner, name, desc, is_static, args) {
            return Some(res);
        }
    }

    // 9. Fallback `Object.equals` / `Object.toString` / `Object.hashCode` on heap objects
    if !is_static {
        match (name, desc) {
            ("equals", "(Ljava/lang/Object;)Z") => {
                return Some(V::bool(ctx.rt.val_eq(args[0], args[1])));
            }
            ("hashCode", "()I") => {
                return Some(V::I(args[0].r() as i32));
            }
            ("toString", "()Ljava/lang/String;") => {
                let s = ctx.rt.value_to_string(args[0]);
                return Some(ctx.rt.new_str(s));
            }
            _ => {}
        }
    }

    None
}

fn format_slf4j(template: &str, params: &[V], rt: &Rt) -> String {
    let mut flat_params: Vec<V> = Vec::new();
    if params.len() == 1 {
        if let Some(Data::Array(arr)) = rt.get_obj(params[0]).map(|o| &o.data) {
            flat_params.extend_from_slice(arr);
        } else {
            flat_params.push(params[0]);
        }
    } else {
        flat_params.extend_from_slice(params);
    }

    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    let mut idx = 0usize;
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        if let Some(&val) = flat_params.get(idx) {
            out.push_str(&rt.value_to_string(val));
            idx += 1;
        } else {
            out.push_str("{}");
        }
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

fn dispatch_boxed_primitives(
    rt: &mut Rt,
    owner: &str,
    name: &str,
    desc: &str,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    match (owner, name, desc) {
        ("java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;") => {
            Some(box_primitive(rt, owner, V::I(args[0].i())))
        }
        ("java/lang/Integer", "intValue", "()I") => Some(V::I(unbox_primitive(rt, args[0]).i())),
        ("java/lang/Integer", "parseInt", "(Ljava/lang/String;)I") => {
            let s = rt.str_of(args[0]).unwrap_or("0").trim();
            Some(V::I(s.parse::<i32>().unwrap_or(0)))
        }
        ("java/lang/Integer", "toString", "(I)Ljava/lang/String;") if is_static => {
            Some(rt.new_str(args[0].i().to_string()))
        }
        ("java/lang/Integer", "compare", "(II)I") => Some(V::I(args[0].i().cmp(&args[1].i()) as i32)),
        ("java/lang/Integer", "sum", "(II)I") => Some(V::I(args[0].i().wrapping_add(args[1].i()))),
        ("java/lang/Integer", "max", "(II)I") => Some(V::I(args[0].i().max(args[1].i()))),
        ("java/lang/Integer", "min", "(II)I") => Some(V::I(args[0].i().min(args[1].i()))),
        ("java/lang/Integer", "hashCode", "(I)I") if is_static => Some(args[0]),

        ("java/lang/Long", "valueOf", "(J)Ljava/lang/Long;") => {
            Some(box_primitive(rt, owner, V::J(args[0].j())))
        }
        ("java/lang/Long", "longValue", "()J") => Some(V::J(unbox_primitive(rt, args[0]).j())),
        ("java/lang/Long", "parseLong", "(Ljava/lang/String;)J") => {
            let s = rt.str_of(args[0]).unwrap_or("0").trim();
            Some(V::J(s.parse::<i64>().unwrap_or(0)))
        }
        ("java/lang/Long", "hashCode", "(J)I") if is_static => {
            let x = args[0].j() as u64;
            Some(V::I((x ^ (x >> 32)) as i32))
        }

        ("java/lang/Float", "valueOf", "(F)Ljava/lang/Float;") => {
            Some(box_primitive(rt, owner, V::F(args[0].f())))
        }
        ("java/lang/Float", "floatValue", "()F") => Some(V::F(unbox_primitive(rt, args[0]).f())),
        ("java/lang/Float", "floatToIntBits" | "floatToRawIntBits", "(F)I") => {
            Some(V::I(args[0].f().to_bits() as i32))
        }
        ("java/lang/Float", "intBitsToFloat", "(I)F") => {
            Some(V::F(f32::from_bits(args[0].i() as u32)))
        }

        ("java/lang/Double", "valueOf", "(D)Ljava/lang/Double;") => {
            Some(box_primitive(rt, owner, V::D(args[0].d())))
        }
        ("java/lang/Double", "doubleValue", "()D") => Some(V::D(unbox_primitive(rt, args[0]).d())),
        ("java/lang/Double", "doubleToLongBits" | "doubleToRawLongBits", "(D)J") => {
            Some(V::J(args[0].d().to_bits() as i64))
        }
        ("java/lang/Double", "longBitsToDouble", "(J)D") => {
            Some(V::D(f64::from_bits(args[0].j() as u64)))
        }

        ("java/lang/Boolean", "valueOf", "(Z)Ljava/lang/Boolean;") => {
            Some(box_primitive(rt, owner, V::I(i32::from(args[0].i() != 0))))
        }
        ("java/lang/Boolean", "booleanValue", "()Z") => {
            Some(V::bool(unbox_primitive(rt, args[0]).i() != 0))
        }
        ("java/lang/Boolean", "parseBoolean", "(Ljava/lang/String;)Z") => {
            let b = rt
                .str_of(args[0])
                .is_some_and(|s| s.eq_ignore_ascii_case("true"));
            Some(V::bool(b))
        }

        ("java/lang/Byte", "valueOf", "(B)Ljava/lang/Byte;") => {
            Some(box_primitive(rt, owner, V::I(args[0].i() as i8 as i32)))
        }
        ("java/lang/Byte", "byteValue", "()B") => Some(V::I(unbox_primitive(rt, args[0]).i())),
        ("java/lang/Short", "valueOf", "(S)Ljava/lang/Short;") => {
            Some(box_primitive(rt, owner, V::I(args[0].i() as i16 as i32)))
        }
        ("java/lang/Short", "shortValue", "()S") => Some(V::I(unbox_primitive(rt, args[0]).i())),
        ("java/lang/Character", "valueOf", "(C)Ljava/lang/Character;") => {
            Some(box_primitive(rt, owner, V::I(args[0].i() & 0xFFFF)))
        }
        ("java/lang/Character", "charValue", "()C") => Some(V::I(unbox_primitive(rt, args[0]).i())),
        ("java/lang/Number", "intValue", "()I") => {
            let u = unbox_primitive(rt, args[0]);
            Some(V::I(match u {
                V::I(v) => v,
                V::J(v) => v as i32,
                V::F(v) => v as i32,
                V::D(v) => v as i32,
                _ => 0,
            }))
        }
        ("java/lang/Number", "doubleValue", "()D") => {
            let u = unbox_primitive(rt, args[0]);
            Some(V::D(match u {
                V::I(v) => f64::from(v),
                V::J(v) => v as f64,
                V::F(v) => f64::from(v),
                V::D(v) => v,
                _ => 0.0,
            }))
        }
        _ => None,
    }
}

fn dispatch_math(name: &str, desc: &str, args: &[V]) -> Option<V> {
    Some(match (name, desc) {
        ("abs", "(I)I") => V::I(args[0].i().wrapping_abs()),
        ("abs", "(J)J") => V::J(args[0].j().wrapping_abs()),
        ("abs", "(F)F") => V::F(args[0].f().abs()),
        ("abs", "(D)D") => V::D(args[0].d().abs()),
        ("max", "(II)I") => V::I(args[0].i().max(args[1].i())),
        ("max", "(JJ)J") => V::J(args[0].j().max(args[1].j())),
        ("max", "(FF)F") => V::F(args[0].f().max(args[1].f())),
        ("max", "(DD)D") => V::D(args[0].d().max(args[1].d())),
        ("min", "(II)I") => V::I(args[0].i().min(args[1].i())),
        ("min", "(JJ)J") => V::J(args[0].j().min(args[1].j())),
        ("min", "(FF)F") => V::F(args[0].f().min(args[1].f())),
        ("min", "(DD)D") => V::D(args[0].d().min(args[1].d())),
        ("clamp", "(JII)I") => V::I(args[0].j().clamp(args[1].i() as i64, args[2].i() as i64) as i32),
        ("clamp", "(FFF)F") => V::F(args[0].f().clamp(args[1].f(), args[2].f())),
        ("clamp", "(DDD)D") => V::D(args[0].d().clamp(args[1].d(), args[2].d())),
        ("sqrt", "(D)D") => V::D(args[0].d().sqrt()),
        ("sin", "(D)D") => V::D(args[0].d().sin()),
        ("cos", "(D)D") => V::D(args[0].d().cos()),
        ("atan2", "(DD)D") => V::D(args[0].d().atan2(args[1].d())),
        ("floor", "(D)D") => V::D(args[0].d().floor()),
        ("ceil", "(D)D") => V::D(args[0].d().ceil()),
        ("round", "(F)I") => V::I((args[0].f() + 0.5).floor() as i32),
        ("round", "(D)J") => V::J((args[0].d() + 0.5).floor() as i64),
        ("pow", "(DD)D") => V::D(args[0].d().powf(args[1].d())),
        ("toRadians", "(D)D") => V::D(args[0].d().to_radians()),
        ("toDegrees", "(D)D") => V::D(args[0].d().to_degrees()),
        ("floorDiv", "(II)I") => V::I(args[0].i().div_euclid(args[1].i())),
        ("floorMod", "(II)I") => V::I(args[0].i().rem_euclid(args[1].i())),
        _ => return None,
    })
}

fn dispatch_string(rt: &mut Rt, name: &str, desc: &str, is_static: bool, args: &[V]) -> Option<V> {
    if is_static {
        return match name {
            "valueOf" => {
                let s = rt.value_to_string(args[0]);
                Some(rt.new_str(s))
            }
            "format" => {
                let fmt = rt.str_of(args[0]).unwrap_or("").to_owned();
                let arr = args.get(1).copied().unwrap_or(V::NULL);
                let items: Vec<V> = match rt.get_obj(arr).map(|o| &o.data) {
                    Some(Data::Array(v)) => v.clone(),
                    _ => Vec::new(),
                };
                let rendered = simple_java_printf(&fmt, &items, rt);
                Some(rt.new_str(rendered))
            }
            "join" => {
                let delim = rt.str_of(args[0]).unwrap_or("").to_owned();
                let src = args.get(1).copied().unwrap_or(V::NULL);
                let elems: Vec<String> = match rt.get_obj(src).map(|o| &o.data) {
                    Some(Data::Array(xs) | Data::List(xs) | Data::Set(xs)) => {
                        xs.iter().map(|&v| rt.value_to_string(v)).collect()
                    }
                    _ => Vec::new(),
                };
                Some(rt.new_str(elems.join(&delim)))
            }
            _ => None,
        };
    }

    let s = rt.str_of(args[0])?.to_owned();
    let utf16: Vec<u16> = s.encode_utf16().collect();
    Some(match (name, desc) {
        ("length", "()I") => V::I(utf16.len() as i32),
        ("isEmpty", "()Z") => V::bool(s.is_empty()),
        ("isBlank", "()Z") => V::bool(s.trim().is_empty()),
        ("charAt", "(I)C") => {
            let idx = args[1].i() as usize;
            V::I(utf16.get(idx).copied().unwrap_or(0) as i32)
        }
        ("toLowerCase", _) => rt.new_str(s.to_lowercase()),
        ("toUpperCase", _) => rt.new_str(s.to_uppercase()),
        ("trim" | "strip", "()Ljava/lang/String;") => rt.new_str(s.trim()),
        ("startsWith", "(Ljava/lang/String;)Z") => {
            let p = rt.str_of(args[1]).unwrap_or("");
            V::bool(s.starts_with(p))
        }
        ("endsWith", "(Ljava/lang/String;)Z") => {
            let p = rt.str_of(args[1]).unwrap_or("");
            V::bool(s.ends_with(p))
        }
        ("contains", "(Ljava/lang/CharSequence;)Z") => {
            let p = rt.str_of(args[1]).unwrap_or("");
            V::bool(s.contains(p))
        }
        ("equals", "(Ljava/lang/Object;)Z") => {
            let other = rt.str_of(args[1]);
            V::bool(other == Some(s.as_str()))
        }
        ("equalsIgnoreCase", "(Ljava/lang/String;)Z") => {
            let other = rt.str_of(args[1]);
            V::bool(other.is_some_and(|o| s.eq_ignore_ascii_case(o)))
        }
        ("indexOf", "(I)I") => {
            let needle = args[1].i() as u16;
            V::I(utf16.iter().position(|&c| c == needle).map_or(-1, |i| i as i32))
        }
        ("indexOf", "(Ljava/lang/String;)I") => {
            let needle = rt.str_of(args[1]).unwrap_or("");
            let pos = s.find(needle).map_or(-1, |byte_pos| {
                s[..byte_pos].encode_utf16().count() as i32
            });
            V::I(pos)
        }
        ("substring", "(I)Ljava/lang/String;") => {
            let from = (args[1].i().max(0) as usize).min(utf16.len());
            rt.new_str(String::from_utf16_lossy(&utf16[from..]))
        }
        ("substring", "(II)Ljava/lang/String;") => {
            let from = (args[1].i().max(0) as usize).min(utf16.len());
            let to = (args[2].i().max(0) as usize).clamp(from, utf16.len());
            rt.new_str(String::from_utf16_lossy(&utf16[from..to]))
        }
        ("replace", "(CC)Ljava/lang/String;") => {
            let old_c = char::from_u32(args[1].i() as u32).unwrap_or('\0');
            let new_c = char::from_u32(args[2].i() as u32).unwrap_or('\0');
            rt.new_str(s.replace(old_c, &new_c.to_string()))
        }
        ("replace", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/lang/String;") => {
            let from = rt.str_of(args[1]).unwrap_or("").to_owned();
            let to = rt.str_of(args[2]).unwrap_or("").to_owned();
            rt.new_str(s.replace(&from, &to))
        }
        ("concat", "(Ljava/lang/String;)Ljava/lang/String;") => {
            let rhs = rt.str_of(args[1]).unwrap_or("");
            rt.new_str(format!("{s}{rhs}"))
        }
        ("split", "(Ljava/lang/String;)[Ljava/lang/String;") => {
            let pat = rt.str_of(args[1]).unwrap_or(",").to_owned();
            let lit = pat.strip_prefix('\\').unwrap_or(&pat);
            let parts: Vec<V> = s.split(lit).map(|p| rt.new_str(p)).collect();
            rt.alloc("[Ljava/lang/String;", 0, Data::Array(parts))
        }
        ("hashCode", "()I") => {
            let mut h = 0i32;
            for &u in &utf16 {
                h = h.wrapping_mul(31).wrapping_add(i32::from(u));
            }
            V::I(h)
        }
        ("toString" | "intern", "()Ljava/lang/String;") => args[0],
        _ => return None,
    })
}

fn simple_java_printf(fmt: &str, items: &[V], rt: &Rt) -> String {
    let mut out = String::new();
    let bytes = fmt.as_bytes();
    let mut i = 0usize;
    let mut arg_idx = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            let spec = bytes[i + 1];
            if spec == b'%' {
                out.push('%');
                i += 2;
                continue;
            }
            if matches!(spec, b's' | b'd' | b'f' | b'x' | b'b') {
                if let Some(&v) = items.get(arg_idx) {
                    arg_idx += 1;
                    match spec {
                        b'x' => out.push_str(&format!("{:x}", v.j())),
                        _ => out.push_str(&rt.value_to_string(v)),
                    }
                }
                i += 2;
                continue;
            }
        }
        let ch = fmt[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn dispatch_string_builder(
    rt: &mut Rt,
    _owner: &str,
    name: &str,
    desc: &str,
    args: &[V],
) -> Option<V> {
    let recv = args.first().copied().unwrap_or(V::NULL);
    match name {
        "<init>" => {
            let init = if desc == "(Ljava/lang/String;)V" || desc == "(Ljava/lang/CharSequence;)V" {
                rt.str_of(args[1]).unwrap_or("").to_owned()
            } else {
                String::new()
            };
            if let Some(obj) = rt.get_obj_mut(recv) {
                obj.data = Data::StringBuilder(init);
            }
            Some(V::Void)
        }
        "append" => {
            let piece = if desc == "(C)Ljava/lang/StringBuilder;" {
                char::from_u32(args[1].i() as u32)
                    .unwrap_or('\u{FFFD}')
                    .to_string()
            } else if desc == "(Z)Ljava/lang/StringBuilder;" {
                if args[1].i() != 0 {
                    "true".into()
                } else {
                    "false".into()
                }
            } else {
                rt.value_to_string(args[1])
            };
            if let Some(obj) = rt.get_obj_mut(recv) {
                match &mut obj.data {
                    Data::StringBuilder(buf) => buf.push_str(&piece),
                    _ => obj.data = Data::StringBuilder(piece),
                }
            }
            Some(recv)
        }
        "length" => {
            let len = match rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::StringBuilder(s)) => s.encode_utf16().count() as i32,
                _ => 0,
            };
            Some(V::I(len))
        }
        "toString" => {
            let text = match rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::StringBuilder(s)) => s.clone(),
                _ => String::new(),
            };
            Some(rt.new_str(text))
        }
        _ => None,
    }
}

fn dispatch_java_util(
    ctx: &mut ExecCtx<'_>,
    owner: &str,
    name: &str,
    desc: &str,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    // `java/util/Optional`
    if owner == "java/util/Optional" {
        if is_static {
            return match name {
                "empty" => Some(ctx.rt.alloc("java/util/Optional", 0, Data::Optional(None))),
                "of" => {
                    if args[0].is_null() {
                        Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Optional.of(null)"))
                    } else {
                        Some(ctx.rt.alloc("java/util/Optional", 0, Data::Optional(Some(args[0]))))
                    }
                }
                "ofNullable" => {
                    let inner = if args[0].is_null() { None } else { Some(args[0]) };
                    Some(ctx.rt.alloc("java/util/Optional", 0, Data::Optional(inner)))
                }
                _ => None,
            };
        }
        let inner = match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
            Some(Data::Optional(opt)) => *opt,
            _ => None,
        };
        return match name {
            "isPresent" => Some(V::bool(inner.is_some())),
            "isEmpty" => Some(V::bool(inner.is_none())),
            "get" | "orElseThrow" => match inner {
                Some(v) => Some(v),
                None => Some(
                    ctx.rt
                        .throw_exception("java/util/NoSuchElementException", "No value present"),
                ),
            },
            "orElse" => Some(inner.unwrap_or(args[1])),
            "orElseGet" => match inner {
                Some(v) => Some(v),
                None => Some(crate::vm::invoke_closure(ctx, args[1], &[])),
            },
            "ifPresent" => {
                if let Some(v) = inner {
                    let _ = crate::vm::invoke_closure(ctx, args[1], &[v]);
                }
                Some(V::Void)
            }
            "map" => match inner {
                Some(v) => {
                    let mapped = crate::vm::invoke_closure(ctx, args[1], &[v]);
                    let next = if mapped.is_null() { None } else { Some(mapped) };
                    Some(ctx.rt.alloc("java/util/Optional", 0, Data::Optional(next)))
                }
                None => Some(args[0]),
            },
            _ => None,
        };
    }

    // `java/util/UUID`
    if owner == "java/util/UUID" {
        return match (name, is_static) {
            ("fromString", true) => {
                let text = ctx.rt.str_of(args[0]).unwrap_or("00000000-0000-0000-0000-000000000000").to_owned();
                Some(ctx.rt.new_handle("java/util/UUID", "uuid", 0, text))
            }
            ("randomUUID", true) => Some(ctx.rt.new_handle(
                "java/util/UUID",
                "uuid",
                1,
                "550e8400-e29b-41d4-a716-446655440000",
            )),
            ("<init>", false) if args.len() == 3 => {
                let tag = format!("{:016x}-{:016x}", args[1].j() as u64, args[2].j() as u64);
                if let Some(o) = ctx.rt.get_obj_mut(args[0]) {
                    o.data = Data::NativeHandle {
                        kind: "uuid",
                        id: 0,
                        tag,
                        payload: [args[1].j(), args[2].j()],
                    };
                }
                Some(V::Void)
            }
            ("toString", false) => {
                let s = ctx.rt.value_to_string(args[0]);
                Some(ctx.rt.new_str(s))
            }
            _ => None,
        };
    }

    // `java/util/Arrays` & `java/util/Collections`
    if owner == "java/util/Arrays" && name == "asList" {
        let items = match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
            Some(Data::Array(v)) => v.clone(),
            _ => Vec::new(),
        };
        return Some(ctx.rt.alloc("java/util/ArrayList", 0, Data::List(items)));
    }
    if owner == "java/util/Collections" {
        return match name {
            "emptyList" => Some(ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new()))),
            "emptySet" => Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(Vec::new()))),
            "emptyMap" => Some(ctx.rt.alloc("java/util/Map", 0, Data::Map(Vec::new()))),
            "singletonList" => Some(ctx.rt.alloc("java/util/List", 0, Data::List(vec![args[0]]))),
            "singleton" => Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(vec![args[0]]))),
            "unmodifiableList" | "unmodifiableSet" | "unmodifiableMap" | "unmodifiableCollection"
            | "synchronizedList" | "synchronizedMap" | "synchronizedSet" => Some(args[0]),
            _ => None,
        };
    }

    // `List.of(...)` / `Set.of(...)` / `Map.of(...)`
    if is_static && name == "of" {
        match owner {
            "java/util/List" => {
                let items = if desc == "([Ljava/lang/Object;)Ljava/util/List;" {
                    match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
                        Some(Data::Array(v)) => v.clone(),
                        _ => Vec::new(),
                    }
                } else {
                    args.to_vec()
                };
                return Some(ctx.rt.alloc("java/util/List", 0, Data::List(items)));
            }
            "java/util/Set" => {
                let items = if desc == "([Ljava/lang/Object;)Ljava/util/Set;" {
                    match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
                        Some(Data::Array(v)) => v.clone(),
                        _ => Vec::new(),
                    }
                } else {
                    args.to_vec()
                };
                return Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(items)));
            }
            "java/util/Map" => {
                let mut pairs = Vec::new();
                for chunk in args.chunks_exact(2) {
                    pairs.push((chunk[0], chunk[1]));
                }
                return Some(ctx.rt.alloc("java/util/Map", 0, Data::Map(pairs)));
            }
            _ => {}
        }
    }

    // Constructors for List / Set / Map
    if !is_static && name == "<init>" {
        let recv = args[0];
        if matches!(
            owner,
            "java/util/ArrayList" | "java/util/LinkedList" | "java/util/ArrayDeque" | "java/util/concurrent/CopyOnWriteArrayList"
        ) {
            let initial = args
                .get(1)
                .and_then(|&src| match ctx.rt.get_obj(src).map(|o| &o.data) {
                    Some(Data::List(v) | Data::Set(v) | Data::Array(v)) => Some(v.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if let Some(obj) = ctx.rt.get_obj_mut(recv) {
                obj.data = Data::List(initial);
            }
            return Some(V::Void);
        }
        if matches!(
            owner,
            "java/util/HashSet" | "java/util/LinkedHashSet" | "java/util/TreeSet"
        ) {
            let initial = args
                .get(1)
                .and_then(|&src| match ctx.rt.get_obj(src).map(|o| &o.data) {
                    Some(Data::List(v) | Data::Set(v) | Data::Array(v)) => Some(v.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if let Some(obj) = ctx.rt.get_obj_mut(recv) {
                obj.data = Data::Set(initial);
            }
            return Some(V::Void);
        }
        if matches!(
            owner,
            "java/util/HashMap"
                | "java/util/LinkedHashMap"
                | "java/util/IdentityHashMap"
                | "java/util/EnumMap"
                | "java/util/TreeMap"
                | "java/util/concurrent/ConcurrentHashMap"
        ) {
            let initial = args
                .get(1)
                .and_then(|&src| match ctx.rt.get_obj(src).map(|o| &o.data) {
                    Some(Data::Map(m)) => Some(m.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if let Some(obj) = ctx.rt.get_obj_mut(recv) {
                obj.data = Data::Map(initial);
            }
            return Some(V::Void);
        }
    }

    // Instance operations on List, Set, Map, Iterator, Stream
    if !is_static {
        let recv = args[0];
        let kind_tag = match ctx.rt.get_obj(recv).map(|o| &o.data) {
            Some(Data::List(_)) => 1,
            Some(Data::Set(_)) => 2,
            Some(Data::Map(_)) => 3,
            Some(Data::Iter { .. }) => 4,
            Some(Data::Stream(_)) => 5,
            _ => 0,
        };

        match kind_tag {
            1 => return dispatch_list_instance(ctx, recv, name, desc, &args[1..]),
            2 => return dispatch_set_instance(ctx, recv, name, &args[1..]),
            3 => return dispatch_map_instance(ctx, recv, name, &args[1..]),
            4 => return dispatch_iter_instance(ctx.rt, recv, name),
            5 => return dispatch_stream_instance(ctx, recv, name, &args[1..]),
            _ => {}
        }

        // `Collectors.toList()` / `Collectors.toSet()`
        if owner == "java/util/stream/Collectors" {
            return match name {
                "toList" | "toUnmodifiableList" => Some(ctx.rt.new_pinned_handle(
                    "java/util/stream/Collector",
                    "collector",
                    1,
                    "toList",
                )),
                "toSet" | "toUnmodifiableSet" => Some(ctx.rt.new_pinned_handle(
                    "java/util/stream/Collector",
                    "collector",
                    2,
                    "toSet",
                )),
                _ => None,
            };
        }
    } else if owner == "java/util/stream/Collectors" {
        return match name {
            "toList" | "toUnmodifiableList" => Some(ctx.rt.new_pinned_handle(
                "java/util/stream/Collector",
                "collector",
                1,
                "toList",
            )),
            "toSet" | "toUnmodifiableSet" => Some(ctx.rt.new_pinned_handle(
                "java/util/stream/Collector",
                "collector",
                2,
                "toSet",
            )),
            _ => None,
        };
    } else if owner == "java/util/stream/Stream" && name == "of" {
        let items = if desc == "([Ljava/lang/Object;)Ljava/util/stream/Stream;" {
            match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
                Some(Data::Array(v)) => v.clone(),
                _ => Vec::new(),
            }
        } else {
            args.to_vec()
        };
        return Some(ctx.rt.alloc("java/util/stream/Stream", 0, Data::Stream(items)));
    }

    None
}

fn dispatch_list_instance(
    ctx: &mut ExecCtx<'_>,
    recv: V,
    name: &str,
    desc: &str,
    args: &[V],
) -> Option<V> {
    Some(match name {
        "size" => {
            let n = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.len() as i32,
                _ => 0,
            };
            V::I(n)
        }
        "isEmpty" => {
            let empty = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.is_empty(),
                _ => true,
            };
            V::bool(empty)
        }
        "get" => {
            let idx = args[0].i() as usize;
            let val = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.get(idx).copied(),
                _ => None,
            };
            match val {
                Some(v) => v,
                None => ctx.rt.throw_exception(
                    "java/lang/IndexOutOfBoundsException",
                    &format!("Index {idx}"),
                ),
            }
        }
        "set" => {
            let idx = args[0].i() as usize;
            let mut prev = V::NULL;
            if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                if idx < v.len() {
                    prev = v[idx];
                    v[idx] = args[1];
                }
            }
            prev
        }
        "add" | "addLast" | "offer" => {
            if desc.starts_with("(I") && args.len() == 2 {
                let idx = (args[0].i().max(0) as usize).min(
                    match ctx.rt.get_obj(recv).map(|o| &o.data) {
                        Some(Data::List(v)) => v.len(),
                        _ => 0,
                    },
                );
                if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    v.insert(idx, args[1]);
                }
                V::Void
            } else {
                if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    v.push(args[0]);
                }
                V::TRUE
            }
        }
        "addAll" => {
            let more: Vec<V> = match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
                Some(Data::List(v) | Data::Set(v) | Data::Array(v)) => v.clone(),
                _ => Vec::new(),
            };
            let added = !more.is_empty();
            if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                v.extend(more);
            }
            V::bool(added)
        }
        "contains" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            V::bool(items.iter().any(|&x| ctx.rt.val_eq(x, args[0])))
        }
        "remove" => {
            if desc == "(I)Ljava/lang/Object;" {
                let idx = args[0].i() as usize;
                let mut removed = V::NULL;
                if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    if idx < v.len() {
                        removed = v.remove(idx);
                    }
                }
                removed
            } else {
                let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                    Some(Data::List(v)) => v.clone(),
                    _ => Vec::new(),
                };
                if let Some(pos) = items.iter().position(|&x| ctx.rt.val_eq(x, args[0])) {
                    if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                        v.remove(pos);
                    }
                    V::TRUE
                } else {
                    V::FALSE
                }
            }
        }
        "clear" => {
            if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                v.clear();
            }
            V::Void
        }
        "iterator" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            ctx.rt
                .alloc("java/util/Iterator", 0, Data::Iter { items, pos: 0 })
        }
        "stream" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            ctx.rt
                .alloc("java/util/stream/Stream", 0, Data::Stream(items))
        }
        "forEach" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            for item in items {
                let _ = crate::vm::invoke_closure(ctx, args[0], &[item]);
            }
            V::Void
        }
        "toArray" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            ctx.rt.alloc("[Ljava/lang/Object;", 0, Data::Array(items))
        }
        _ => return None,
    })
}

fn dispatch_set_instance(ctx: &mut ExecCtx<'_>, recv: V, name: &str, args: &[V]) -> Option<V> {
    Some(match name {
        "size" => {
            let n = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.len() as i32,
                _ => 0,
            };
            V::I(n)
        }
        "isEmpty" => {
            let empty = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.is_empty(),
                _ => true,
            };
            V::bool(empty)
        }
        "contains" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.clone(),
                _ => Vec::new(),
            };
            V::bool(items.iter().any(|&x| ctx.rt.val_eq(x, args[0])))
        }
        "add" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.clone(),
                _ => Vec::new(),
            };
            if items.iter().any(|&x| ctx.rt.val_eq(x, args[0])) {
                V::FALSE
            } else {
                if let Some(Data::Set(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    v.push(args[0]);
                }
                V::TRUE
            }
        }
        "remove" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.clone(),
                _ => Vec::new(),
            };
            if let Some(pos) = items.iter().position(|&x| ctx.rt.val_eq(x, args[0])) {
                if let Some(Data::Set(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    v.remove(pos);
                }
                V::TRUE
            } else {
                V::FALSE
            }
        }
        "clear" => {
            if let Some(Data::Set(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                v.clear();
            }
            V::Void
        }
        "iterator" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.clone(),
                _ => Vec::new(),
            };
            ctx.rt
                .alloc("java/util/Iterator", 0, Data::Iter { items, pos: 0 })
        }
        "stream" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(v)) => v.clone(),
                _ => Vec::new(),
            };
            ctx.rt
                .alloc("java/util/stream/Stream", 0, Data::Stream(items))
        }
        _ => return None,
    })
}

fn dispatch_map_instance(ctx: &mut ExecCtx<'_>, recv: V, name: &str, args: &[V]) -> Option<V> {
    let snapshot = || -> Vec<(V, V)> {
        match ctx.rt.get_obj(recv).map(|o| &o.data) {
            Some(Data::Map(m)) => m.clone(),
            _ => Vec::new(),
        }
    };

    Some(match name {
        "size" => V::I(snapshot().len() as i32),
        "isEmpty" => V::bool(snapshot().is_empty()),
        "containsKey" => {
            let entries = snapshot();
            V::bool(entries.iter().any(|&(k, _)| ctx.rt.val_eq(k, args[0])))
        }
        "get" => {
            let entries = snapshot();
            entries
                .iter()
                .find(|&&(k, _)| ctx.rt.val_eq(k, args[0]))
                .map_or(V::NULL, |&(_, v)| v)
        }
        "getOrDefault" => {
            let entries = snapshot();
            entries
                .iter()
                .find(|&&(k, _)| ctx.rt.val_eq(k, args[0]))
                .map_or(args[1], |&(_, v)| v)
        }
        "put" => {
            let entries = snapshot();
            let pos = entries.iter().position(|&(k, _)| ctx.rt.val_eq(k, args[0]));
            let mut prev = V::NULL;
            if let Some(Data::Map(m)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                if let Some(idx) = pos {
                    prev = m[idx].1;
                    m[idx].1 = args[1];
                } else {
                    m.push((args[0], args[1]));
                }
            }
            prev
        }
        "putIfAbsent" => {
            let entries = snapshot();
            if let Some(&(_, existing)) = entries
                .iter()
                .find(|&&(k, v)| ctx.rt.val_eq(k, args[0]) && !v.is_null())
            {
                existing
            } else {
                if let Some(Data::Map(m)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    m.push((args[0], args[1]));
                }
                V::NULL
            }
        }
        "computeIfAbsent" => {
            let entries = snapshot();
            if let Some(&(_, existing)) = entries
                .iter()
                .find(|&&(k, v)| ctx.rt.val_eq(k, args[0]) && !v.is_null())
            {
                existing
            } else {
                let computed = crate::vm::invoke_closure(ctx, args[1], &[args[0]]);
                if !computed.is_null() {
                    if let Some(Data::Map(m)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                        m.push((args[0], computed));
                    }
                }
                computed
            }
        }
        "remove" => {
            let entries = snapshot();
            if let Some(pos) = entries.iter().position(|&(k, _)| ctx.rt.val_eq(k, args[0])) {
                let mut old = V::NULL;
                if let Some(Data::Map(m)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                    old = m.remove(pos).1;
                }
                old
            } else {
                V::NULL
            }
        }
        "clear" => {
            if let Some(Data::Map(m)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                m.clear();
            }
            V::Void
        }
        "keySet" => {
            let keys: Vec<V> = snapshot().into_iter().map(|(k, _)| k).collect();
            ctx.rt.alloc("java/util/Set", 0, Data::Set(keys))
        }
        "values" => {
            let vals: Vec<V> = snapshot().into_iter().map(|(_, v)| v).collect();
            ctx.rt.alloc("java/util/List", 0, Data::List(vals))
        }
        "forEach" => {
            for (k, v) in snapshot() {
                let _ = crate::vm::invoke_closure(ctx, args[0], &[k, v]);
            }
            V::Void
        }
        _ => return None,
    })
}

fn dispatch_iter_instance(rt: &mut Rt, recv: V, name: &str) -> Option<V> {
    match name {
        "hasNext" => {
            let has = match rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Iter { items, pos }) => *pos < items.len(),
                _ => false,
            };
            Some(V::bool(has))
        }
        "next" => {
            let mut item = None;
            if let Some(Data::Iter { items, pos }) = rt.get_obj_mut(recv).map(|o| &mut o.data) {
                if *pos < items.len() {
                    item = Some(items[*pos]);
                    *pos += 1;
                }
            }
            match item {
                Some(v) => Some(v),
                None => Some(rt.throw_exception("java/util/NoSuchElementException", "Iterator.next")),
            }
        }
        _ => None,
    }
}

fn dispatch_stream_instance(
    ctx: &mut ExecCtx<'_>,
    recv: V,
    name: &str,
    args: &[V],
) -> Option<V> {
    let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
        Some(Data::Stream(v)) => v.clone(),
        _ => Vec::new(),
    };

    Some(match name {
        "filter" => {
            let mut kept = Vec::new();
            for item in items {
                if crate::vm::invoke_closure(ctx, args[0], &[item]).i() != 0 {
                    kept.push(item);
                }
            }
            ctx.rt
                .alloc("java/util/stream/Stream", 0, Data::Stream(kept))
        }
        "map" => {
            let mut mapped = Vec::with_capacity(items.len());
            for item in items {
                mapped.push(crate::vm::invoke_closure(ctx, args[0], &[item]));
            }
            ctx.rt
                .alloc("java/util/stream/Stream", 0, Data::Stream(mapped))
        }
        "forEach" => {
            for item in items {
                let _ = crate::vm::invoke_closure(ctx, args[0], &[item]);
            }
            V::Void
        }
        "count" => V::J(items.len() as i64),
        "findFirst" | "findAny" => {
            let first = items.into_iter().next();
            ctx.rt.alloc("java/util/Optional", 0, Data::Optional(first))
        }
        "anyMatch" => {
            for item in items {
                if crate::vm::invoke_closure(ctx, args[0], &[item]).i() != 0 {
                    return Some(V::TRUE);
                }
            }
            V::FALSE
        }
        "allMatch" => {
            for item in items {
                if crate::vm::invoke_closure(ctx, args[0], &[item]).i() == 0 {
                    return Some(V::FALSE);
                }
            }
            V::TRUE
        }
        "toList" => ctx.rt.alloc("java/util/List", 0, Data::List(items)),
        "reduce" => {
            if args.len() == 2 {
                let mut acc = args[0];
                for item in items {
                    acc = crate::vm::invoke_closure(ctx, args[1], &[acc, item]);
                }
                acc
            } else {
                let mut iter = items.into_iter();
                let Some(mut acc) = iter.next() else {
                    return Some(ctx.rt.alloc("java/util/Optional", 0, Data::Optional(None)));
                };
                for item in iter {
                    acc = crate::vm::invoke_closure(ctx, args[0], &[acc, item]);
                }
                ctx.rt
                    .alloc("java/util/Optional", 0, Data::Optional(Some(acc)))
            }
        }
        "collect" => {
            let collector_id = ctx.rt.handle_of(args[0]).map(|(_, id, _, _)| id).unwrap_or(1);
            if collector_id == 2 {
                let mut set_items = Vec::new();
                for item in items {
                    if !set_items.iter().any(|&x| ctx.rt.val_eq(x, item)) {
                        set_items.push(item);
                    }
                }
                ctx.rt.alloc("java/util/Set", 0, Data::Set(set_items))
            } else {
                ctx.rt.alloc("java/util/List", 0, Data::List(items))
            }
        }
        _ => return None,
    })
}

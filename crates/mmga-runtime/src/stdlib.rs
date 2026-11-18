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
        ("java/nio/charset/StandardCharsets", "UTF_8" | "US_ASCII" | "ISO_8859_1") => {
            Some(rt.new_pinned_handle("java/nio/charset/Charset", "charset", 0, name))
        }
        ("java/lang/StackWalker$Option", "RETAIN_CLASS_REFERENCE" | "SHOW_REFLECT_FRAMES" | "SHOW_HIDDEN_FRAMES") => {
            Some(rt.new_pinned_handle("java/lang/StackWalker$Option", "stack_walker_option", 0, name))
        }
        ("java/lang/Integer", "MAX_VALUE") => Some(V::I(i32::MAX)),
        ("java/lang/Integer", "MIN_VALUE") => Some(V::I(i32::MIN)),
        ("java/lang/Long", "MAX_VALUE") => Some(V::J(i64::MAX)),
        ("java/lang/Long", "MIN_VALUE") => Some(V::J(i64::MIN)),
        ("java/lang/Float", "MAX_VALUE") => Some(V::F(f32::MAX)),
        ("java/lang/Float", "MIN_VALUE") => Some(V::F(f32::MIN_POSITIVE)),
        ("java/lang/Double", "MAX_VALUE") => Some(V::D(f64::MAX)),
        ("java/lang/Double", "MIN_VALUE") => Some(V::D(f64::MIN_POSITIVE)),
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

    if let Some(value) = crate::class_loading::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::numeric_compare::call(ctx, member, is_static, args) {
        return Some(value);
    }

    if let Some(value) = crate::jdk_state::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::reflection::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::guava_views::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::enum_sets::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::array_utils::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::string_utils::call(ctx, member, is_static, args) {
        return Some(value);
    }
    if let Some(value) = crate::stream_collectors::call(ctx, member, is_static, args) {
        return Some(value);
    }

    // 0. Functional interfaces (`Consumer`, `Supplier`, `Function`, `BiFunction`, `Predicate`, `Runnable`, `Callable`, etc.)
    //    when the receiver is a `Data::Closure`!
    if !is_static {
        if let Some(&recv) = args.first() {
            let is_closure = matches!(ctx.rt.get_obj(recv).map(|o| &o.data), Some(Data::Closure(_)));
            // This inherited default method is not the three-argument SAM:
            // NeoForge's host supplies the null extra-data argument before
            // invoking the IContainerFactory closure.
            let container_default = name == "create"
                && desc == "(ILnet/minecraft/world/entity/player/Inventory;)Lnet/minecraft/world/inventory/AbstractContainerMenu;"
                && matches!(owner, "net/neoforged/neoforge/network/IContainerFactory" | "net/minecraft/world/inventory/MenuType$MenuSupplier")
                && matches!(ctx.rt.get_obj(recv).map(|o| &o.data), Some(Data::Closure(c)) if c.interface_class == "net/neoforged/neoforge/network/IContainerFactory");
            if is_closure && !container_default && name != "getClass" && name != "hashCode" && name != "toString" && name != "equals" {
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
            | "java/lang/Error"
            | "java/io/IOException",
            "<init>",
            _,
        ) => {
            if let Some(&this) = args.first() {
                if let Some(&arg1) = args.get(1) {
                    if ctx.rt.str_of(arg1).is_some() {
                        ctx.rt.put_field(this, "detailMessage", arg1);
                        if let Some(&cause) = args.get(2) {
                            ctx.rt.put_field(this, "cause", cause);
                        }
                    } else if !arg1.is_null() {
                        ctx.rt.put_field(this, "cause", arg1);
                    }
                }
            }
            return Some(V::Void);
        }
        (_, "getMessage" | "getLocalizedMessage", "()Ljava/lang/String;") if !is_static => {
            let msg = ctx.rt.get_field(args[0], "detailMessage");
            return Some(msg);
        }
        (_, "getCause", "()Ljava/lang/Throwable;") if !is_static => {
            return Some(ctx.rt.get_field(args[0], "cause"));
        }
        (_, "initCause", "(Ljava/lang/Throwable;)Ljava/lang/Throwable;") if !is_static => {
            if let Some(&cause) = args.get(1) {
                ctx.rt.put_field(args[0], "cause", cause);
            }
            return Some(args[0]);
        }
        (_, "addSuppressed", "(Ljava/lang/Throwable;)V") if !is_static => {
            return Some(V::Void);
        }
        (_, "name", "()Ljava/lang/String;")
            if !is_static && !ctx.rt.get_field(args[0], "$name").is_null() =>
        {
            return Some(ctx.rt.get_field(args[0], "$name"));
        }
        (_, "ordinal", "()I")
            if !is_static && !ctx.rt.get_field(args[0], "$name").is_null() =>
        {
            return Some(ctx.rt.get_field(args[0], "$ordinal"));
        }
        (_, "compareTo", _)
            if !is_static
                && args.len() == 2
                && !ctx.rt.get_field(args[0], "$name").is_null()
                && !ctx.rt.get_field(args[1], "$name").is_null() =>
        {
            let a = ctx.rt.get_field(args[0], "$ordinal").i();
            let b = ctx.rt.get_field(args[1], "$ordinal").i();
            return Some(V::I(a.cmp(&b) as i32));
        }
        ("java/text/NumberFormat" | "java/text/DecimalFormat", _, _) => {
            return dispatch_number_format(ctx.rt, name, desc, is_static, args);
        }
        ("java/lang/Enum", "valueOf", "(Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Enum;")
            if is_static =>
        {
            let cls_tag = ctx
                .rt
                .handle_of(args[0])
                .map(|(_, _, t, _)| t.to_owned())
                .unwrap_or_default();
            let const_name = ctx
                .rt
                .str_of(args[1])
                .map(str::to_owned)
                .ok_or(())
                .ok()?;
            crate::vm::ensure_class_init(ctx, &cls_tag);
            if let Some(val) = ctx.rt.get_static(&cls_tag, &const_name) {
                return Some(val);
            }
            return Some(ctx.rt.throw_exception(
                "java/lang/IllegalArgumentException",
                &format!("No enum constant {cls_tag}.{const_name}"),
            ));
        }
        (_, "clone", "()Ljava/lang/Object;") if !is_static && owner.starts_with('[') => {
            let cloned = match ctx.rt.get_obj(args[0]) {
                Some(o) => match &o.data {
                    Data::Array(xs) => Some((o.class.clone(), xs.clone())),
                    _ => None,
                },
                None => None,
            };
            if let Some((cls, items)) = cloned {
                return Some(ctx.rt.alloc(cls, 0, Data::Array(items)));
            }
        }
        (_, "getClass", "()Ljava/lang/Class;") if !is_static => {
            let cls = ctx
                .rt
                .class_of(args[0])
                .unwrap_or("java/lang/Object")
                .to_owned();
            return Some(ctx.rt.new_pinned_handle("java/lang/Class", "class", 0, cls));
        }
        ("java/lang/Class", "getName" | "getCanonicalName" | "getTypeName", "()Ljava/lang/String;") => {
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
        ("java/lang/Class", "getPackageName", "()Ljava/lang/String;") => {
            let pkg = ctx
                .rt
                .handle_of(args[0])
                .and_then(|(_, _, t, _)| {
                    let clean = t.trim_start_matches('[').strip_prefix('L').and_then(|s| s.strip_suffix(';')).unwrap_or(t);
                    clean.rsplit_once('/').map(|(p, _)| p.replace('/', "."))
                })
                .unwrap_or_default();
            return Some(ctx.rt.new_str(pkg));
        }
        ("java/lang/Class", "desiredAssertionStatus", "()Z") => {
            return Some(V::FALSE);
        }
        ("java/lang/Class", "isInstance", "(Ljava/lang/Object;)Z") => {
            let candidate = args.get(1).copied().unwrap_or(V::NULL);
            if candidate.is_null() {
                return Some(V::FALSE);
            }
            let target = ctx
                .rt
                .handle_of(args[0])
                .map(|(_, _, t, _)| t.to_owned())
                .unwrap_or_else(|| "java/lang/Object".into());
            let actual = ctx.rt.class_of(candidate).unwrap_or("java/lang/Object");
            return Some(V::bool(ctx.linked.is_subtype(actual, &target, ctx.host)));
        }
        ("java/lang/Class", "isAssignableFrom", "(Ljava/lang/Class;)Z") => {
            let other = args.get(1).copied().unwrap_or(V::NULL);
            if other.is_null() {
                return Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Class.isAssignableFrom(null)"));
            }
            let sup = ctx.rt.handle_of(args[0]).map(|(_, _, t, _)| t.to_owned()).unwrap_or_default();
            let sub = ctx.rt.handle_of(other).map(|(_, _, t, _)| t.to_owned()).unwrap_or_default();
            return Some(V::bool(ctx.linked.is_subtype(&sub, &sup, ctx.host)));
        }
        ("java/lang/Class", "cast", "(Ljava/lang/Object;)Ljava/lang/Object;") => {
            return Some(args.get(1).copied().unwrap_or(V::NULL));
        }
        ("java/lang/StackWalker", "getInstance", _) if is_static => {
            return Some(ctx.rt.new_pinned_handle("java/lang/StackWalker", "stack_walker", 0, ""));
        }
        ("java/lang/StackWalker", "getCallerClass", "()Ljava/lang/Class;") if !is_static => {
            let n = ctx.rt.call_stack.len();
            if n < 2 {
                return Some(ctx.rt.throw_exception(
                    "java/lang/IllegalCallerException",
                    "StackWalker.getCallerClass called with fewer than 2 stack frames",
                ));
            }
            let caller = ctx.rt.call_stack[n - 2].0.clone();
            return Some(ctx.rt.new_pinned_handle("java/lang/Class", "class", 0, caller));
        }
        _ => {}
    }

    // 3. `java/lang/Objects` / `java/util/Objects`
    if owner == "java/lang/Objects" || owner == "java/util/Objects" {
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
            "requireNonNullElse" => {
                let v = args.first().copied().unwrap_or(V::NULL);
                if !v.is_null() {
                    Some(v)
                } else {
                    let def = args.get(1).copied().unwrap_or(V::NULL);
                    if def.is_null() {
                        Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Objects.requireNonNullElse defaultObj"))
                    } else {
                        Some(def)
                    }
                }
            }
            "requireNonNullElseGet" => {
                let v = args.first().copied().unwrap_or(V::NULL);
                if !v.is_null() {
                    Some(v)
                } else {
                    let sup = args.get(1).copied().unwrap_or(V::NULL);
                    let got = crate::vm::invoke_closure(ctx, sup, &[]);
                    if got.is_null() {
                        Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Objects.requireNonNullElseGet"))
                    } else {
                        Some(got)
                    }
                }
            }
            "checkIndex" => {
                let (idx, len) = (args[0].i(), args[1].i());
                if idx < 0 || idx >= len {
                    Some(ctx.rt.throw_exception("java/lang/IndexOutOfBoundsException", &format!("Index {idx} out of bounds for length {len}")))
                } else {
                    Some(V::I(idx))
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
            "lineSeparator" => Some(ctx.rt.intern_str("\n")),
            "getenv" if desc == "(Ljava/lang/String;)Ljava/lang/String;" => {
                let Some(key) = args.first().and_then(|&v| ctx.rt.str_of(v)) else {
                    return Some(ctx.rt.throw_exception("java/lang/NullPointerException", "System.getenv(null)"));
                };
                match std::env::var(key) {
                    Ok(val) => Some(ctx.rt.new_str(val)),
                    Err(_) => Some(V::NULL),
                }
            }
            "getProperty" => {
                let Some(key) = args.first().and_then(|&v| ctx.rt.str_of(v)).map(str::to_owned) else {
                    return Some(ctx.rt.throw_exception("java/lang/NullPointerException", "System.getProperty(null)"));
                };
                let found: Option<String> = match key.as_str() {
                    "java.io.tmpdir" => Some(std::env::temp_dir().display().to_string()),
                    "user.home" => std::env::var("HOME")
                        .ok()
                        .or_else(|| Some(std::env::temp_dir().display().to_string())),
                    "user.dir" => std::env::current_dir()
                        .ok()
                        .map(|p| p.display().to_string()),
                    "os.name" => Some(std::env::consts::OS.to_owned()),
                    "os.arch" => Some(std::env::consts::ARCH.to_owned()),
                    "file.separator" => Some(std::path::MAIN_SEPARATOR.to_string()),
                    "path.separator" => Some(if cfg!(windows) { ";".into() } else { ":".into() }),
                    "line.separator" => Some("\n".into()),
                    "java.version" => Some("21.0.4".into()),
                    _ => None,
                };
                match found {
                    Some(val) => Some(ctx.rt.new_str(val)),
                    None => Some(args.get(1).copied().unwrap_or(V::NULL)),
                }
            }
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
                .or_else(|| ctx.rt.call_stack.last().map(|(cls, _)| cls.replace('/', ".")))
                .unwrap_or_else(|| "mod".into());
            return Some(ctx.rt.new_pinned_handle("org/slf4j/Logger", "logger", 0, tag));
        }
    }
    if (owner == "java/io/InputStream" || owner == "java/io/ByteArrayInputStream") && !is_static {
        return dispatch_input_stream(ctx.rt, args[0], name, desc, &args[1..]);
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

    // 8. Collections, Optional, Arrays, Streams, Iterators (`java/util/*`, `Iterable`, Fastutil, Guava Map/List/Set views)
    if owner.starts_with("java/util/")
        || owner == "java/lang/Iterable"
        || owner.starts_with("it/unimi/dsi/fastutil/")
        || (!is_static && owner.starts_with("com/google/common/collect/"))
    {
        if let Some(res) = dispatch_java_util(ctx, owner, name, desc, is_static, args) {
            return Some(res);
        }
    }

    None
}

/// Object defaults are used only after the host has had a chance to implement
/// the receiver's override. Otherwise external value types silently get identity
/// equality or an Object string instead of their actual methods.
pub(crate) fn object_fallback(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let name = member.name.as_str();
    let desc = member.desc.as_str();
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
        ("java/lang/Long", "toString", "(J)Ljava/lang/String;") if is_static => {
            Some(rt.new_str(args[0].j().to_string()))
        }
        ("java/lang/Float", "toString", "(F)Ljava/lang/String;") if is_static => {
            Some(rt.new_str(rt.value_to_string(V::F(args[0].f()))))
        }
        ("java/lang/Double", "toString", "(D)Ljava/lang/String;") if is_static => {
            Some(rt.new_str(rt.value_to_string(V::D(args[0].d()))))
        }
        ("java/lang/Boolean", "toString", "(Z)Ljava/lang/String;") if is_static => {
            Some(rt.intern_str(if args[0].i() != 0 { "true" } else { "false" }))
        }
        ("java/lang/Double", "parseDouble", "(Ljava/lang/String;)D") if is_static => {
            let s = rt.str_of(args[0]).unwrap_or("0").trim();
            Some(V::D(s.parse::<f64>().unwrap_or(0.0)))
        }
        ("java/lang/Float", "parseFloat", "(Ljava/lang/String;)F") if is_static => {
            let s = rt.str_of(args[0]).unwrap_or("0").trim();
            Some(V::F(s.parse::<f32>().unwrap_or(0.0)))
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
        ("java/lang/Character", "toUpperCase", "(C)C") => {
            let c = char::from_u32(args[0].i() as u32)
                .and_then(|ch| ch.to_uppercase().next())
                .map_or(args[0].i(), |u| u as i32);
            Some(V::I(c))
        }
        ("java/lang/Character", "toLowerCase", "(C)C") => {
            let c = char::from_u32(args[0].i() as u32)
                .and_then(|ch| ch.to_lowercase().next())
                .map_or(args[0].i(), |l| l as i32);
            Some(V::I(c))
        }
        ("java/lang/Character", "isWhitespace", "(C)Z") | ("java/lang/Character", "isWhitespace", "(I)Z") => {
            let ws = char::from_u32(args[0].i() as u32).is_some_and(char::is_whitespace);
            Some(V::bool(ws))
        }
        ("java/lang/Character", "isDigit", "(C)Z") | ("java/lang/Character", "isDigit", "(I)Z") => {
            let d = char::from_u32(args[0].i() as u32).is_some_and(|c| c.is_ascii_digit());
            Some(V::bool(d))
        }
        ("java/lang/Character", "isLetter", "(C)Z") | ("java/lang/Character", "isLetter", "(I)Z") => {
            let l = char::from_u32(args[0].i() as u32).is_some_and(char::is_alphabetic);
            Some(V::bool(l))
        }
        ("java/lang/Character", "isLetterOrDigit", "(C)Z") | ("java/lang/Character", "isLetterOrDigit", "(I)Z") => {
            let ld = char::from_u32(args[0].i() as u32).is_some_and(char::is_alphanumeric);
            Some(V::bool(ld))
        }
        ("java/lang/Character", "isUpperCase", "(C)Z") => {
            let u = char::from_u32(args[0].i() as u32).is_some_and(char::is_uppercase);
            Some(V::bool(u))
        }
        ("java/lang/Character", "isLowerCase", "(C)Z") => {
            let l = char::from_u32(args[0].i() as u32).is_some_and(char::is_lowercase);
            Some(V::bool(l))
        }
        ("java/lang/Character", "toString", "(C)Ljava/lang/String;") if is_static => {
            let ch = char::from_u32(args[0].i() as u32).unwrap_or('\u{FFFD}');
            Some(rt.new_str(ch.to_string()))
        }
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
                if desc == "([C)Ljava/lang/String;" {
                    let u16s: Vec<u16> = match rt.get_obj(args[0]).map(|o| &o.data) {
                        Some(Data::Array(xs)) => xs.iter().map(|v| v.i() as u16).collect(),
                        _ => Vec::new(),
                    };
                    Some(rt.new_str(String::from_utf16_lossy(&u16s)))
                } else if desc == "([CII)Ljava/lang/String;" {
                    let (off, count) = (args[1].i().max(0) as usize, args[2].i().max(0) as usize);
                    let u16s: Vec<u16> = match rt.get_obj(args[0]).map(|o| &o.data) {
                        Some(Data::Array(xs)) => xs
                            .iter()
                            .skip(off)
                            .take(count)
                            .map(|v| v.i() as u16)
                            .collect(),
                        _ => Vec::new(),
                    };
                    Some(rt.new_str(String::from_utf16_lossy(&u16s)))
                } else if desc == "(C)Ljava/lang/String;" {
                    let ch = char::from_u32(args[0].i() as u32).unwrap_or('\u{FFFD}');
                    Some(rt.new_str(ch.to_string()))
                } else {
                    let s = rt.value_to_string(args[0]);
                    Some(rt.new_str(s))
                }
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

    let s = if name == "<init>" {
        String::new()
    } else {
        rt.str_of(args[0])?.to_owned()
    };
    let utf16: Vec<u16> = s.encode_utf16().collect();
    Some(match (name, desc) {
        ("length", "()I") => V::I(utf16.len() as i32),
        ("isEmpty", "()Z") => V::bool(s.is_empty()),
        ("isBlank", "()Z") => V::bool(s.trim().is_empty()),
        ("charAt", "(I)C") => {
            let idx = args[1].i() as usize;
            V::I(utf16.get(idx).copied().unwrap_or(0) as i32)
        }
        ("toCharArray", "()[C") => {
            let chars: Vec<V> = utf16.iter().map(|&u| V::I(i32::from(u))).collect();
            rt.alloc("[C", 0, Data::Array(chars))
        }
        ("<init>", "([C)V") => {
            let u16s: Vec<u16> = match rt.get_obj(args[1]).map(|o| &o.data) {
                Some(Data::Array(xs)) => xs.iter().map(|v| v.i() as u16).collect(),
                _ => Vec::new(),
            };
            if let Some(o) = rt.get_obj_mut(args[0]) {
                o.data = Data::Str(String::from_utf16_lossy(&u16s));
            }
            V::Void
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
        ("getBytes", "()[B")
        | ("getBytes", "(Ljava/nio/charset/Charset;)[B")
        | ("getBytes", "(Ljava/lang/String;)[B") => {
            let arr: Vec<V> = s.as_bytes().iter().map(|&b| V::I(i32::from(b as i8))).collect();
            rt.alloc("[B", 0, Data::Array(arr))
        }
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
    // `java/util/concurrent/atomic/*`
    if matches!(
        owner,
        "java/util/concurrent/atomic/AtomicInteger"
            | "java/util/concurrent/atomic/AtomicLong"
            | "java/util/concurrent/atomic/AtomicBoolean"
            | "java/util/concurrent/atomic/AtomicReference"
    ) && !is_static
    {
        return dispatch_atomic(ctx.rt, owner, args[0], name, &args[1..]);
    }

    // `java/util/Comparator`
    if owner == "java/util/Comparator" {
        return dispatch_comparator(ctx, name, desc, is_static, args);
    }

    // `java/util/concurrent/Executors` & `ExecutorService`
    if owner == "java/util/concurrent/Executors" && is_static {
        return match name {
            "newSingleThreadExecutor"
            | "newCachedThreadPool"
            | "newFixedThreadPool"
            | "newSingleThreadScheduledExecutor"
            | "newScheduledThreadPool" => Some(ctx.rt.new_pinned_handle(
                "java/util/concurrent/ExecutorService",
                "executor",
                0,
                name,
            )),
            _ => None,
        };
    }
    if (owner == "java/util/concurrent/ExecutorService"
        || owner == "java/util/concurrent/Executor")
        && !is_static
    {
        return match name {
            "execute" => {
                let _ = crate::vm::invoke_closure(ctx, args[1], &[]);
                Some(V::Void)
            }
            "submit" => {
                let res = crate::vm::invoke_closure(ctx, args[1], &[]);
                let fut = ctx
                    .rt
                    .new_instance("java/util/concurrent/CompletableFuture", 0);
                ctx.rt.put_field(fut, "$done", V::I(1));
                ctx.rt.put_field(
                    fut,
                    "$value",
                    if matches!(res, V::Void) { V::NULL } else { res },
                );
                Some(fut)
            }
            "shutdown" => Some(V::Void),
            "shutdownNow" => Some(ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new()))),
            "isShutdown" | "isTerminated" => Some(V::FALSE),
            _ => None,
        };
    }

    // `java/util/concurrent/CompletableFuture`
    if owner == "java/util/concurrent/CompletableFuture" {
        if is_static && name == "completedFuture" {
            let fut = ctx
                .rt
                .new_instance("java/util/concurrent/CompletableFuture", 0);
            ctx.rt.put_field(fut, "$done", V::I(1));
            ctx.rt
                .put_field(fut, "$value", args.first().copied().unwrap_or(V::NULL));
            return Some(fut);
        }
        if !is_static {
            let recv = args[0];
            return match name {
                "isDone" => Some(V::bool(ctx.rt.get_field(recv, "$done").i() != 0)),
                "join" | "get" => Some(ctx.rt.get_field(recv, "$value")),
                "thenRun" => {
                    let _ = crate::vm::invoke_closure(ctx, args[1], &[]);
                    let fut = ctx
                        .rt
                        .new_instance("java/util/concurrent/CompletableFuture", 0);
                    ctx.rt.put_field(fut, "$done", V::I(1));
                    Some(fut)
                }
                "thenAccept" => {
                    let val = ctx.rt.get_field(recv, "$value");
                    let _ = crate::vm::invoke_closure(ctx, args[1], &[val]);
                    let fut = ctx
                        .rt
                        .new_instance("java/util/concurrent/CompletableFuture", 0);
                    ctx.rt.put_field(fut, "$done", V::I(1));
                    Some(fut)
                }
                "thenApply" => {
                    let val = ctx.rt.get_field(recv, "$value");
                    let next = crate::vm::invoke_closure(ctx, args[1], &[val]);
                    let fut = ctx
                        .rt
                        .new_instance("java/util/concurrent/CompletableFuture", 0);
                    ctx.rt.put_field(fut, "$done", V::I(1));
                    ctx.rt.put_field(fut, "$value", next);
                    Some(fut)
                }
                _ => None,
            };
        }
    }

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
            "orElseThrow" => match inner {
            Some(v) => Some(v),
            None => {
                if let Some(&exc_supplier) = args.get(1) {
                    let exc = crate::vm::invoke_closure(ctx, exc_supplier, &[]);
                    if !exc.is_null() {
                        ctx.rt.pending_exception = Some(exc.r());
                    }
                    Some(V::NULL)
                } else {
                    Some(
                        ctx.rt
                            .throw_exception("java/util/NoSuchElementException", "No value present"),
                    )
                }
            }
        },
        "get" => match inner {
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
        return dispatch_uuid(ctx.rt, name, is_static, args);
    }

    // `java/util/Random`
    if owner == "java/util/Random" && !is_static {
        return dispatch_random(ctx.rt, args[0], name, desc, &args[1..]);
    }

    // `java/util/Arrays` & `java/util/Collections`
    if owner == "java/util/Arrays" && is_static {
        return dispatch_arrays(ctx.rt, name, desc, args);
    }
    if (owner == "java/util/Map$Entry"
        || owner == "java/util/AbstractMap$SimpleEntry"
        || owner == "java/util/AbstractMap$SimpleImmutableEntry")
        && !is_static
    {
        return match name {
            "<init>" if args.len() >= 3 => {
                ctx.rt.put_field(args[0], "key", args[1]);
                ctx.rt.put_field(args[0], "value", args[2]);
                Some(V::Void)
            }
            "getKey" => Some(ctx.rt.get_field(args[0], "key")),
            "getValue" => Some(ctx.rt.get_field(args[0], "value")),
            "setValue" if owner != "java/util/AbstractMap$SimpleImmutableEntry" => {
                let prev = ctx.rt.get_field(args[0], "value");
                if let Some(&v) = args.get(1) {
                    ctx.rt.put_field(args[0], "value", v);
                }
                Some(prev)
            }
            _ => None,
        };
    }
    if owner == "java/util/Map" && is_static && name == "entry" && args.len() == 2 {
        let entry = ctx
            .rt
            .new_instance("java/util/AbstractMap$SimpleImmutableEntry", 0);
        ctx.rt.put_field(entry, "key", args[0]);
        ctx.rt.put_field(entry, "value", args[1]);
        return Some(entry);
    }
    if owner == "java/util/Collections" {
        return match name {
            "emptyList" => Some(ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new()))),
            "emptySet" => Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(Vec::new()))),
            "emptyMap" => Some(ctx.rt.alloc("java/util/Map", 0, Data::Map(Vec::new()))),
            "singletonList" => Some(ctx.rt.alloc("java/util/List", 0, Data::List(vec![args[0]]))),
            "singleton" => Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(vec![args[0]]))),
            "unmodifiableList" | "unmodifiableSet" | "unmodifiableMap" | "unmodifiableCollection"
            | "synchronizedList" | "synchronizedMap" | "synchronizedSet" | "synchronizedCollection" => Some(args[0]),
            "newSetFromMap" => {
                let map_v = *args.first()?;
                if map_v.is_null() {
                    return Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Collections.newSetFromMap(null)"));
                }
                let not_empty = match ctx.rt.get_obj(map_v).map(|o| &o.data) {
                    Some(Data::Map(m)) => !m.is_empty(),
                    _ => false,
                };
                if not_empty {
                    return Some(ctx.rt.throw_exception("java/lang/IllegalArgumentException", "Map is non-empty"));
                }
                Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(Vec::new())))
            }
            _ => None,
        };
    }

    // `List.of(...)` / `Set.of(...)` / `Map.of(...)` & Fastutil `.of(...)` / `.wrap(...)`
    if is_static && (name == "of" || name == "wrap") {
        match owner {
            "java/util/List"
            | "it/unimi/dsi/fastutil/objects/ObjectArrayList"
            | "it/unimi/dsi/fastutil/objects/ObjectList"
            | "it/unimi/dsi/fastutil/ints/IntArrayList" => {
                let items = if desc.starts_with("([") {
                    match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
                        Some(Data::Array(v)) => v.clone(),
                        _ => Vec::new(),
                    }
                } else {
                    args.to_vec()
                };
                return Some(ctx.rt.alloc(owner, 0, Data::List(items)));
            }
            "java/util/Set"
            | "it/unimi/dsi/fastutil/objects/ObjectOpenHashSet"
            | "it/unimi/dsi/fastutil/objects/ObjectLinkedOpenHashSet"
            | "it/unimi/dsi/fastutil/objects/ReferenceOpenHashSet"
            | "it/unimi/dsi/fastutil/objects/ObjectArraySet"
            | "it/unimi/dsi/fastutil/objects/ObjectSet" => {
                let items = if desc.starts_with("([") {
                    match ctx.rt.get_obj(args[0]).map(|o| &o.data) {
                        Some(Data::Array(v)) => v.clone(),
                        _ => Vec::new(),
                    }
                } else {
                    args.to_vec()
                };
                return Some(ctx.rt.alloc(owner, 0, Data::Set(items)));
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
    if is_static && owner == "it/unimi/dsi/fastutil/objects/ObjectLists" && name == "emptyList" {
        return Some(ctx.rt.alloc("java/util/List", 0, Data::List(Vec::new())));
    }
    if is_static && owner == "it/unimi/dsi/fastutil/objects/ObjectSets" && name == "emptySet" {
        return Some(ctx.rt.alloc("java/util/Set", 0, Data::Set(Vec::new())));
    }
    if is_static && owner == "it/unimi/dsi/fastutil/objects/Object2ObjectMaps" && name == "emptyMap" {
        return Some(ctx.rt.alloc("java/util/Map", 0, Data::Map(Vec::new())));
    }

    // Constructors for List / Set / Map
    if !is_static && name == "<init>" {
        let recv = args[0];
        if matches!(
            owner,
            "java/util/ArrayList"
                | "java/util/LinkedList"
                | "java/util/ArrayDeque"
                | "java/util/concurrent/CopyOnWriteArrayList"
                | "it/unimi/dsi/fastutil/objects/ObjectArrayList"
                | "it/unimi/dsi/fastutil/ints/IntArrayList"
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
            "java/util/HashSet"
                | "java/util/LinkedHashSet"
                | "java/util/TreeSet"
                | "it/unimi/dsi/fastutil/objects/ObjectOpenHashSet"
                | "it/unimi/dsi/fastutil/objects/ObjectLinkedOpenHashSet"
                | "it/unimi/dsi/fastutil/objects/ReferenceOpenHashSet"
                | "it/unimi/dsi/fastutil/objects/ObjectArraySet"
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
                | "it/unimi/dsi/fastutil/objects/Object2ObjectOpenHashMap"
                | "it/unimi/dsi/fastutil/objects/Object2ObjectLinkedOpenHashMap"
                | "it/unimi/dsi/fastutil/objects/Object2ObjectArrayMap"
                | "it/unimi/dsi/fastutil/objects/Reference2ObjectOpenHashMap"
                | "it/unimi/dsi/fastutil/objects/Object2IntOpenHashMap"
                | "it/unimi/dsi/fastutil/objects/Object2BooleanOpenHashMap"
                | "it/unimi/dsi/fastutil/ints/Int2ObjectOpenHashMap"
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
            2 => return dispatch_set_instance(ctx, recv, name, desc, &args[1..]),
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
    let immutable = matches!(
        ctx.rt.class_of(recv),
        Some("java/util/ImmutableCollections$List12" | "java/util/ImmutableCollections$ListN")
    );
    // List.of rejects mutation before index checks or callback invocation,
    // including operations that would leave its contents unchanged.
    if immutable
        && matches!(
            (name, desc),
            ("add", "(Ljava/lang/Object;)Z")
                | ("add", "(ILjava/lang/Object;)V")
                | ("addAll", "(Ljava/util/Collection;)Z")
                | ("addAll", "(ILjava/util/Collection;)Z")
                | ("set", "(ILjava/lang/Object;)Ljava/lang/Object;")
                | ("remove", "(I)Ljava/lang/Object;")
                | ("remove", "(Ljava/lang/Object;)Z")
                | ("clear", "()V")
                | ("removeAll" | "retainAll", "(Ljava/util/Collection;)Z")
                | ("removeIf", "(Ljava/util/function/Predicate;)Z")
                | ("replaceAll", "(Ljava/util/function/UnaryOperator;)V")
                | ("sort", "(Ljava/util/Comparator;)V")
                | ("addFirst" | "addLast", "(Ljava/lang/Object;)V")
                | ("removeFirst" | "removeLast", "()Ljava/lang/Object;")
        )
    {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "immutable List",
        ));
    }
    if immutable
        && matches!(
            name,
            "add" | "addAll" | "set" | "remove" | "clear" | "removeAll" | "retainAll"
                | "removeIf" | "replaceAll" | "sort" | "addFirst" | "addLast"
                | "removeFirst" | "removeLast" | "offer"
        )
    {
        // A nonexistent overload or Queue-only method must not fall through to
        // the name-based mutable-list implementation below.
        return None;
    }
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
        "getFirst" | "getLast" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            let picked = if name == "getFirst" {
                items.first().copied()
            } else {
                items.last().copied()
            };
            match picked {
                Some(v) => v,
                None => ctx.rt.throw_exception(
                    "java/lang/NoSuchElementException",
                    "List is empty",
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
        "sort" => {
            let mut items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            let cmp = args.first().copied().unwrap_or(V::NULL);
            let n = items.len();
            for i in 1..n {
                let mut j = i;
                while j > 0 {
                    let ord = eval_comparator(ctx, cmp, items[j - 1], items[j]);
                    if ord > 0 {
                        items.swap(j - 1, j);
                        j -= 1;
                    } else {
                        break;
                    }
                }
            }
            if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                *v = items;
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
        "removeIf" => {
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::List(v)) => v.clone(),
                _ => Vec::new(),
            };
            let mut kept = Vec::with_capacity(items.len());
            let mut removed = false;
            for item in items {
                if crate::vm::invoke_closure(ctx, args[0], &[item]).i() != 0 {
                    removed = true;
                } else {
                    kept.push(item);
                }
            }
            if let Some(Data::List(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                *v = kept;
            }
            V::bool(removed)
        }
        _ => return None,
    })
}

fn dispatch_set_instance(
    ctx: &mut ExecCtx<'_>,
    recv: V,
    name: &str,
    desc: &str,
    args: &[V],
) -> Option<V> {
    Some(match name {
        "addAll" if desc == "(Ljava/util/Collection;)Z" => {
            let source = *args.first()?;
            let iterator = crate::invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE,
                &MemberRef::new("java/util/Collection", "iterator", "()Ljava/util/Iterator;"), &[source]);
            if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
            let mut changed = false;
            loop {
                let has_next = crate::invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Iterator", "hasNext", "()Z"), &[iterator]);
                if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                if has_next.i() == 0 { break; }
                let next = crate::invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Iterator", "next", "()Ljava/lang/Object;"), &[iterator]);
                if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                let added = crate::invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new("java/util/Set", "add", "(Ljava/lang/Object;)Z"), &[recv, next]);
                if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
                changed |= added.i() != 0;
            }
            V::bool(changed)
        }
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
                bump_set_modification(ctx.rt, recv);
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
                bump_set_modification(ctx.rt, recv);
                V::TRUE
            } else {
                V::FALSE
            }
        }
        "clear" => {
            if let Some(Data::Set(v)) = ctx.rt.get_obj_mut(recv).map(|o| &mut o.data) {
                v.clear();
            }
            bump_set_modification(ctx.rt, recv);
            V::Void
        }
        "forEach" if desc == "(Ljava/util/function/Consumer;)V" => {
            let consumer = *args.first()?;
            if consumer.is_null() {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "Collection.forEach consumer",
                ));
            }
            let items = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::Set(values)) => values.clone(),
                _ => return None,
            };
            let expected = set_modification(ctx.rt, recv);
            for item in &items {
                // HashSet inherits Iterable.forEach: its iterator checks
                // structural modification on next(), before the next callback.
                let unchanged = match ctx.rt.get_obj(recv).map(|o| &o.data) {
                    Some(Data::Set(current)) => current == &items,
                    _ => false,
                };
                if expected != set_modification(ctx.rt, recv) || !unchanged {
                    return Some(ctx.rt.throw_exception(
                        "java/util/ConcurrentModificationException",
                        "Set modified during forEach",
                    ));
                }
                crate::vm::invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "java/util/function/Consumer",
                        "accept",
                        "(Ljava/lang/Object;)V",
                    ),
                    &[consumer, *item],
                );
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
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

fn set_modification(rt: &Rt, set: V) -> i64 {
    rt.get_static("mmga/set_modifications", &set.r().to_string())
        .map_or(0, V::j)
}

fn bump_set_modification(rt: &mut Rt, set: V) {
    let next = set_modification(rt, set).wrapping_add(1);
    rt.put_static("mmga/set_modifications", &set.r().to_string(), V::J(next));
}

#[cfg(test)]
mod collection_for_each_tests {
    use super::*;
    use crate::{HostApi, LinkedSpace};
    use mmga_classfile::op;

    #[derive(Default)]
    struct ConsumerHost {
        seen: Vec<V>,
        mode: u8,
        source: V,
    }
    impl HostApi for ConsumerHost {
        fn get_static_field(
            &mut self, _rt: &mut Rt, _owner: &str, _name: &str, _desc: &str,
        ) -> Option<V> {
            None
        }
        fn call_method(
            &mut self, rt: &mut Rt, linked: &LinkedSpace, member: &MemberRef,
            _is_static: bool, args: &[V],
        ) -> Option<V> {
            if member.owner != "java/util/function/Consumer" || member.name != "accept" {
                return None;
            }
            self.seen.push(args[1]);
            if self.mode == 1 {
                return Some(rt.throw_exception("example/CallbackFailure", "stop"));
            }
            if self.mode == 2 && self.seen.len() == 1 {
                let source = self.source;
                let mut ctx = ExecCtx { rt, linked, host: self, depth: 0 };
                for name in ["remove", "add"] {
                    crate::vm::invoke_member(
                        &mut ctx, op::INVOKEVIRTUAL,
                        &MemberRef::new("java/util/Set", name, "(Ljava/lang/Object;)Z"),
                        &[source, args[1]],
                    );
                }
            }
            Some(V::Void)
        }
    }
    fn for_each(ctx: &mut ExecCtx<'_>, set: V, consumer: V) {
        crate::vm::invoke_member(
            ctx, op::INVOKEINTERFACE,
            &MemberRef::new("java/util/Collection", "forEach", "(Ljava/util/function/Consumer;)V"),
            &[set, consumer],
        );
    }

    #[test]
    fn registrate_set_for_each_invokes_every_consumer_and_checks_empty_null() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = ConsumerHost::default();
        let values = vec![rt.intern_str("first"), rt.intern_str("second")];
        let set = rt.alloc("java/util/LinkedHashSet", 0, Data::Set(values.clone()));
        let consumer = rt.new_instance("example/Consumer", 0);
        {
            let mut ctx = ExecCtx { rt: &mut rt, linked: &linked, host: &mut host, depth: 0 };
            for_each(&mut ctx, set, consumer);
            assert!(ctx.rt.pending_exception.is_none());
        }
        assert_eq!(host.seen, values);
        let empty = rt.alloc("java/util/Set", 0, Data::Set(Vec::new()));
        let mut ctx = ExecCtx { rt: &mut rt, linked: &linked, host: &mut host, depth: 0 };
        for_each(&mut ctx, empty, V::NULL);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx.rt.exception_summary(error).contains("NullPointerException"));
    }

    #[test]
    fn set_for_each_stops_at_callback_exception_and_structural_mutation() {
        for mode in [1, 2] {
            let mut rt = Rt::new();
            let linked = LinkedSpace::new();
            let values = vec![rt.intern_str("first"), rt.intern_str("second")];
            let set = rt.alloc("java/util/LinkedHashSet", 0, Data::Set(values));
            let consumer = rt.new_instance("example/Consumer", 0);
            let mut host = ConsumerHost { mode, source: set, ..ConsumerHost::default() };
            {
                let mut ctx = ExecCtx { rt: &mut rt, linked: &linked, host: &mut host, depth: 0 };
                for_each(&mut ctx, set, consumer);
                let error = ctx.rt.take_exception().unwrap();
                let expected = if mode == 1 { "CallbackFailure" } else { "ConcurrentModificationException" };
                assert!(ctx.rt.exception_summary(error).contains(expected));
            }
            assert_eq!(host.seen.len(), 1);
        }
    }

    #[test]
    fn ae2_set_add_all_retains_unique_values_and_reports_change() {
        let mut rt=Rt::new();let linked=LinkedSpace::new();let mut host=crate::NullHost;
        let a=rt.intern_str("model:a");let b=rt.intern_str("model:b");
        let set=rt.alloc("java/util/HashSet",0,Data::Set(vec![a]));
        let list=rt.alloc("java/util/ArrayList",0,Data::List(vec![a,b,b,V::NULL]));
        let mut ctx=ExecCtx{rt:&mut rt,linked:&linked,host:&mut host,depth:0};
        let call=MemberRef::new("java/util/Set","addAll","(Ljava/util/Collection;)Z");
        assert_eq!(crate::invoke_member(&mut ctx,op::INVOKEINTERFACE,&call,&[set,list]),V::TRUE);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(crate::invoke_member(&mut ctx,op::INVOKEINTERFACE,&call,&[set,list]),V::FALSE);
        let values=match &ctx.rt.get_obj(set).unwrap().data {Data::Set(v)=>v,_=>panic!()};
        assert_eq!(values,&vec![a,b,V::NULL]);
        crate::invoke_member(&mut ctx,op::INVOKEINTERFACE,&call,&[set,V::NULL]);
        let error=ctx.rt.take_exception().unwrap();assert!(ctx.rt.exception_summary(error).contains("NullPointerException"));
    }
}

fn dispatch_map_instance(ctx: &mut ExecCtx<'_>, recv: V, name: &str, args: &[V]) -> Option<V> {
    if ctx.rt.class_of(recv)==Some("com/google/common/collect/ImmutableMap") {
        if matches!(name,"put"|"putAll"|"putIfAbsent"|"remove"|"clear"|"replace"|"replaceAll"|"compute"|"computeIfAbsent"|"computeIfPresent"|"merge") {
            return Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException","immutable Map"));
        }
        if matches!(name,"keySet"|"values"|"entrySet") {
            return Some(ctx.rt.throw_exception("java/lang/UnsupportedOperationException","ImmutableMap collection views are not implemented"));
        }
    }
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
        "entrySet" => {
            let entries: Vec<V> = snapshot()
                .into_iter()
                .map(|(k, v)| {
                    let e = ctx.rt.new_instance("java/util/AbstractMap$SimpleEntry", 0);
                    ctx.rt.put_field(e, "key", k);
                    ctx.rt.put_field(e, "value", v);
                    e
                })
                .collect();
            ctx.rt.alloc("java/util/Set", 0, Data::Set(entries))
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

fn dispatch_input_stream(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    match (name, desc) {
        ("<init>", "([B)V") => {
            let bytes = args
                .first()
                .and_then(|&arr| match rt.get_obj(arr).map(|o| &o.data) {
                    Some(Data::Array(items)) => {
                        Some(items.iter().map(|v| v.i() as u8).collect::<Vec<u8>>())
                    }
                    _ => None,
                })
                .unwrap_or_default();
            if let Some(obj) = rt.get_obj_mut(recv) {
                obj.data = Data::ByteBuf {
                    bytes,
                    reader_idx: 0,
                };
            }
            Some(V::Void)
        }
        ("available", "()I") => {
            let rem = match rt.get_obj(recv).map(|o| &o.data) {
                Some(Data::ByteBuf { bytes, reader_idx }) => {
                    bytes.len().saturating_sub(*reader_idx) as i32
                }
                _ => 0,
            };
            Some(V::I(rem))
        }
        ("read", "()I") => {
            let mut out = -1i32;
            if let Some(Data::ByteBuf { bytes, reader_idx }) =
                rt.get_obj_mut(recv).map(|o| &mut o.data)
            {
                if *reader_idx < bytes.len() {
                    out = i32::from(bytes[*reader_idx]);
                    *reader_idx += 1;
                }
            }
            Some(V::I(out))
        }
        ("read", "([B)I") | ("read", "([BII)I") => {
            let dst = args.first().copied().unwrap_or(V::NULL);
            if dst.is_null() {
                return Some(
                    rt.throw_exception("java/lang/NullPointerException", "InputStream.read(null)"),
                );
            }
            let dst_len = match rt.get_obj(dst).map(|o| &o.data) {
                Some(Data::Array(items)) => items.len(),
                _ => 0,
            };
            let (off, req_len) = if desc == "([BII)I" {
                let o = args.get(1).map_or(0, |v| v.i().max(0) as usize);
                let l = args.get(2).map_or(0, |v| v.i().max(0) as usize);
                (o, l)
            } else {
                (0, dst_len)
            };
            if req_len == 0 {
                return Some(V::I(0));
            }
            let chunk: Option<Vec<u8>> = match rt.get_obj_mut(recv).map(|o| &mut o.data) {
                Some(Data::ByteBuf { bytes, reader_idx }) => {
                    if *reader_idx >= bytes.len() {
                        None
                    } else {
                        let n = req_len
                            .min(bytes.len() - *reader_idx)
                            .min(dst_len.saturating_sub(off));
                        let slice = bytes[*reader_idx..*reader_idx + n].to_vec();
                        *reader_idx += n;
                        Some(slice)
                    }
                }
                _ => None,
            };
            match chunk {
                None => Some(V::I(-1)),
                Some(slice) => {
                    let n = slice.len() as i32;
                    if let Some(Data::Array(items)) = rt.get_obj_mut(dst).map(|o| &mut o.data) {
                        for (i, b) in slice.into_iter().enumerate() {
                            if off + i < items.len() {
                                items[off + i] = V::I(i32::from(b as i8));
                            }
                        }
                    }
                    Some(V::I(n))
                }
            }
        }
        ("readAllBytes", "()[B") => {
            let chunk: Vec<V> = match rt.get_obj_mut(recv).map(|o| &mut o.data) {
                Some(Data::ByteBuf { bytes, reader_idx }) => {
                    let out = bytes[*reader_idx..]
                        .iter()
                        .map(|&b| V::I(i32::from(b as i8)))
                        .collect();
                    *reader_idx = bytes.len();
                    out
                }
                _ => Vec::new(),
            };
            Some(rt.alloc("[B", 0, Data::Array(chunk)))
        }
        ("close", "()V") => Some(V::Void),
        _ => None,
    }
}

fn format_uuid_bits(msb: u64, lsb: u64) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (msb >> 32) as u32,
        ((msb >> 16) & 0xFFFF) as u16,
        (msb & 0xFFFF) as u16,
        ((lsb >> 48) & 0xFFFF) as u16,
        lsb & 0xFFFF_FFFF_FFFF
    )
}

fn parse_uuid_bits(text: &str) -> Result<(i64, i64), ()> {
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 5 {
        return Err(());
    }
    let p0 = u64::from_str_radix(parts[0], 16).map_err(|_| ())?;
    let p1 = u64::from_str_radix(parts[1], 16).map_err(|_| ())?;
    let p2 = u64::from_str_radix(parts[2], 16).map_err(|_| ())?;
    let p3 = u64::from_str_radix(parts[3], 16).map_err(|_| ())?;
    let p4 = u64::from_str_radix(parts[4], 16).map_err(|_| ())?;
    if p0 > 0xFFFF_FFFF
        || p1 > 0xFFFF
        || p2 > 0xFFFF
        || p3 > 0xFFFF
        || p4 > 0xFFFF_FFFF_FFFF
    {
        return Err(());
    }
    let msb = (p0 << 32) | (p1 << 16) | p2;
    let lsb = (p3 << 48) | p4;
    Ok((msb as i64, lsb as i64))
}

fn dispatch_uuid(rt: &mut Rt, name: &str, is_static: bool, args: &[V]) -> Option<V> {
    match (name, is_static) {
        ("fromString", true) => {
            let Some(text) = args.first().and_then(|&v| rt.str_of(v)).map(str::to_owned) else {
                return Some(rt.throw_exception("java/lang/NullPointerException", "UUID.fromString(null)"));
            };
            let Ok((msb, lsb)) = parse_uuid_bits(&text) else {
                return Some(rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    &format!("Invalid UUID string: {text}"),
                ));
            };
            let tag = format_uuid_bits(msb as u64, lsb as u64);
            Some(rt.alloc(
                "java/util/UUID",
                0,
                Data::NativeHandle {
                    kind: "uuid",
                    id: 0,
                    tag,
                    payload: [msb, lsb],
                },
            ))
        }
        ("randomUUID", true) => {
            let counter = rt.live_object_count() as u64 + 1;
            let msb = 0x550e_8400_e29b_41d4u64 ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let msb_v4 = (msb & !0xF000) | 0x4000;
            let lsb = (0xa716_4466_5544_0000u64 ^ counter) & !(3u64 << 62) | (2u64 << 62);
            let tag = format_uuid_bits(msb_v4, lsb);
            Some(rt.alloc(
                "java/util/UUID",
                0,
                Data::NativeHandle {
                    kind: "uuid",
                    id: 0,
                    tag,
                    payload: [msb_v4 as i64, lsb as i64],
                },
            ))
        }
        ("nameUUIDFromBytes", true) => {
            let bytes: Vec<u8> = match args.first().and_then(|&v| rt.get_obj(v)).map(|o| &o.data) {
                Some(Data::Array(items)) => items.iter().map(|x| x.i() as u8).collect(),
                _ => return Some(rt.throw_exception("java/lang/NullPointerException", "UUID.nameUUIDFromBytes(null)")),
            };
            let mut digest = md5_digest(&bytes);
            digest[6] = (digest[6] & 0x0f) | 0x30; // version 3
            digest[8] = (digest[8] & 0x3f) | 0x80; // IETF variant
            let msb = u64::from_be_bytes(digest[0..8].try_into().unwrap());
            let lsb = u64::from_be_bytes(digest[8..16].try_into().unwrap());
            let tag = format_uuid_bits(msb, lsb);
            Some(rt.alloc(
                "java/util/UUID",
                0,
                Data::NativeHandle {
                    kind: "uuid",
                    id: 0,
                    tag,
                    payload: [msb as i64, lsb as i64],
                },
            ))
        }
        ("<init>", false) if args.len() == 3 => {
            let (msb, lsb) = (args[1].j(), args[2].j());
            let tag = format_uuid_bits(msb as u64, lsb as u64);
            if let Some(o) = rt.get_obj_mut(args[0]) {
                o.data = Data::NativeHandle {
                    kind: "uuid",
                    id: 0,
                    tag,
                    payload: [msb, lsb],
                };
            }
            Some(V::Void)
        }
        ("getMostSignificantBits", false) => {
            let msb = rt.handle_of(args[0]).map_or(0, |(_, _, _, p)| p[0]);
            Some(V::J(msb))
        }
        ("getLeastSignificantBits", false) => {
            let lsb = rt.handle_of(args[0]).map_or(0, |(_, _, _, p)| p[1]);
            Some(V::J(lsb))
        }
        ("hashCode", false) => {
            let (msb, lsb) = rt.handle_of(args[0]).map_or((0, 0), |(_, _, _, p)| (p[0], p[1]));
            let hilo = (msb ^ lsb) as u64;
            Some(V::I(((hilo >> 32) as i32) ^ (hilo as i32)))
        }
        ("equals", false) => {
            let other = args.get(1).copied().unwrap_or(V::NULL);
            let eq = match (rt.handle_of(args[0]), rt.handle_of(other)) {
                (Some(("uuid", _, _, a)), Some(("uuid", _, _, b))) => a == b,
                _ => false,
            };
            Some(V::bool(eq))
        }
        ("compareTo", false) => {
            let other = args.get(1).copied().unwrap_or(V::NULL);
            let (Some(("uuid", _, _, a)), Some(("uuid", _, _, b))) =
                (rt.handle_of(args[0]), rt.handle_of(other))
            else {
                return Some(rt.throw_exception("java/lang/NullPointerException", "UUID.compareTo"));
            };
            let cmp = a[0].cmp(&b[0]).then_with(|| a[1].cmp(&b[1])) as i32;
            Some(V::I(cmp))
        }
        ("toString", false) => {
            let s = rt.value_to_string(args[0]);
            Some(rt.new_str(s))
        }
        _ => None,
    }
}

fn dispatch_random(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    const MULT: i64 = 0x5DEECE66D;
    const ADD: i64 = 0xB;
    const MASK: i64 = (1i64 << 48) - 1;

    let step_bits = |rt: &mut Rt, bits: u32| -> i32 {
        let cur = rt.get_field(recv, "$seed").j();
        let next = (cur.wrapping_mul(MULT).wrapping_add(ADD)) & MASK;
        rt.put_field(recv, "$seed", V::J(next));
        ((next as u64) >> (48 - bits)) as i32
    };

    match (name, desc) {
        ("<init>", "()V") => {
            let init = ((0x8682_5228_0714_8012u64 as i64) ^ MULT) & MASK;
            rt.put_field(recv, "$seed", V::J(init));
            Some(V::Void)
        }
        ("<init>", "(J)V") | ("setSeed", "(J)V") => {
            let seed = (args[0].j() ^ MULT) & MASK;
            rt.put_field(recv, "$seed", V::J(seed));
            Some(V::Void)
        }
        ("nextInt", "()I") => Some(V::I(step_bits(rt, 32))),
        ("nextInt", "(I)I") => {
            let bound = args[0].i();
            if bound <= 0 {
                return Some(rt.throw_exception("java/lang/IllegalArgumentException", "bound must be positive"));
            }
            let r = (step_bits(rt, 31) as u32 % bound as u32) as i32;
            Some(V::I(r))
        }
        ("nextLong", "()J") => {
            let hi = i64::from(step_bits(rt, 32));
            let lo = i64::from(step_bits(rt, 32));
            Some(V::J((hi << 32).wrapping_add(lo)))
        }
        ("nextBoolean", "()Z") => Some(V::bool(step_bits(rt, 1) != 0)),
        ("nextFloat", "()F") => Some(V::F(step_bits(rt, 24) as f32 / (1u32 << 24) as f32)),
        ("nextDouble", "()D") => {
            let hi = (step_bits(rt, 26) as i64) << 27;
            let lo = step_bits(rt, 27) as i64;
            Some(V::D((hi + lo) as f64 / (1u64 << 53) as f64))
        }
        _ => None,
    }
}

fn dispatch_arrays(rt: &mut Rt, name: &str, desc: &str, args: &[V]) -> Option<V> {
    match name {
        "asList" => {
            let items = match rt.get_obj(args[0]).map(|o| &o.data) {
                Some(Data::Array(v)) => v.clone(),
                _ => Vec::new(),
            };
            Some(rt.alloc("java/util/ArrayList", 0, Data::List(items)))
        }
        "equals" if args.len() == 2 => {
            if args[0] == args[1] {
                return Some(V::TRUE);
            }
            let (Some(Data::Array(a)), Some(Data::Array(b))) = (
                rt.get_obj(args[0]).map(|o| &o.data),
                rt.get_obj(args[1]).map(|o| &o.data),
            ) else {
                return Some(V::FALSE);
            };
            if a.len() != b.len() {
                return Some(V::FALSE);
            }
            let a_copy = a.clone();
            let b_copy = b.clone();
            let eq = a_copy
                .into_iter()
                .zip(b_copy)
                .all(|(x, y)| rt.val_eq(x, y));
            Some(V::bool(eq))
        }
        "copyOf" if args.len() >= 2 => {
            let new_len = args[1].i().max(0) as usize;
            let (cls, mut items) = match rt.get_obj(args[0]) {
                Some(o) => match &o.data {
                    Data::Array(v) => (o.class.clone(), v.clone()),
                    _ => return None,
                },
                None => return Some(rt.throw_exception("java/lang/NullPointerException", "Arrays.copyOf(null)")),
            };
            let fill = if cls.starts_with("[L") || cls.starts_with("[[") {
                V::NULL
            } else if cls == "[J" {
                V::J(0)
            } else if cls == "[F" {
                V::F(0.0)
            } else if cls == "[D" {
                V::D(0.0)
            } else {
                V::I(0)
            };
            items.resize(new_len, fill);
            Some(rt.alloc(cls, 0, Data::Array(items)))
        }
        "stream" if !args.is_empty() => {
            let items = match rt.get_obj(args[0]).map(|o| &o.data) {
                Some(Data::Array(v)) => v.clone(),
                _ => Vec::new(),
            };
            if desc.starts_with("([Ljava/lang/Object;II)") && args.len() >= 3 {
                let from = (args[1].i().max(0) as usize).min(items.len());
                let to = (args[2].i().max(0) as usize).clamp(from, items.len());
                Some(rt.alloc("java/util/stream/Stream", 0, Data::Stream(items[from..to].to_vec())))
            } else {
                Some(rt.alloc("java/util/stream/Stream", 0, Data::Stream(items)))
            }
        }
        "fill" if args.len() == 2 => {
            let val = args[1];
            if let Some(Data::Array(items)) = rt.get_obj_mut(args[0]).map(|o| &mut o.data) {
                for slot in items.iter_mut() {
                    *slot = val;
                }
            }
            Some(V::Void)
        }
        "toString" if args.len() == 1 => {
            if args[0].is_null() {
                return Some(rt.intern_str("null"));
            }
            let s = rt.value_to_string(args[0]);
            Some(rt.new_str(s))
        }
        _ => None,
    }
}

fn dispatch_atomic(rt: &mut Rt, owner: &str, recv: V, name: &str, args: &[V]) -> Option<V> {
    match owner {
        "java/util/concurrent/atomic/AtomicInteger" => match name {
            "<init>" => {
                let init = args.first().map_or(0, |v| v.i());
                rt.put_field(recv, "$val", V::I(init));
                Some(V::Void)
            }
            "get" | "intValue" => Some(V::I(rt.get_field(recv, "$val").i())),
            "set" | "lazySet" => {
                rt.put_field(recv, "$val", V::I(args.first()?.i()));
                Some(V::Void)
            }
            "getAndSet" => {
                let old = rt.get_field(recv, "$val").i();
                rt.put_field(recv, "$val", V::I(args.first()?.i()));
                Some(V::I(old))
            }
            "incrementAndGet" => {
                let next = rt.get_field(recv, "$val").i().wrapping_add(1);
                rt.put_field(recv, "$val", V::I(next));
                Some(V::I(next))
            }
            "getAndIncrement" => {
                let old = rt.get_field(recv, "$val").i();
                rt.put_field(recv, "$val", V::I(old.wrapping_add(1)));
                Some(V::I(old))
            }
            "decrementAndGet" => {
                let next = rt.get_field(recv, "$val").i().wrapping_sub(1);
                rt.put_field(recv, "$val", V::I(next));
                Some(V::I(next))
            }
            "getAndDecrement" => {
                let old = rt.get_field(recv, "$val").i();
                rt.put_field(recv, "$val", V::I(old.wrapping_sub(1)));
                Some(V::I(old))
            }
            "addAndGet" => {
                let next = rt.get_field(recv, "$val").i().wrapping_add(args.first()?.i());
                rt.put_field(recv, "$val", V::I(next));
                Some(V::I(next))
            }
            "getAndAdd" => {
                let old = rt.get_field(recv, "$val").i();
                rt.put_field(recv, "$val", V::I(old.wrapping_add(args.first()?.i())));
                Some(V::I(old))
            }
            "compareAndSet" => {
                let cur = rt.get_field(recv, "$val").i();
                if cur == args.first()?.i() {
                    rt.put_field(recv, "$val", V::I(args.get(1)?.i()));
                    Some(V::TRUE)
                } else {
                    Some(V::FALSE)
                }
            }
            _ => None,
        },
        "java/util/concurrent/atomic/AtomicLong" => match name {
            "<init>" => {
                let init = args.first().map_or(0, |v| v.j());
                rt.put_field(recv, "$val", V::J(init));
                Some(V::Void)
            }
            "get" | "longValue" => Some(V::J(rt.get_field(recv, "$val").j())),
            "set" | "lazySet" => {
                rt.put_field(recv, "$val", V::J(args.first()?.j()));
                Some(V::Void)
            }
            "incrementAndGet" => {
                let next = rt.get_field(recv, "$val").j().wrapping_add(1);
                rt.put_field(recv, "$val", V::J(next));
                Some(V::J(next))
            }
            "getAndIncrement" => {
                let old = rt.get_field(recv, "$val").j();
                rt.put_field(recv, "$val", V::J(old.wrapping_add(1)));
                Some(V::J(old))
            }
            "addAndGet" => {
                let next = rt.get_field(recv, "$val").j().wrapping_add(args.first()?.j());
                rt.put_field(recv, "$val", V::J(next));
                Some(V::J(next))
            }
            "compareAndSet" => {
                let cur = rt.get_field(recv, "$val").j();
                if cur == args.first()?.j() {
                    rt.put_field(recv, "$val", V::J(args.get(1)?.j()));
                    Some(V::TRUE)
                } else {
                    Some(V::FALSE)
                }
            }
            _ => None,
        },
        "java/util/concurrent/atomic/AtomicBoolean" => match name {
            "<init>" => {
                let init = args.first().map_or(0, |v| i32::from(v.i() != 0));
                rt.put_field(recv, "$val", V::I(init));
                Some(V::Void)
            }
            "get" => Some(V::bool(rt.get_field(recv, "$val").i() != 0)),
            "set" | "lazySet" => {
                rt.put_field(recv, "$val", V::bool(args.first()?.i() != 0));
                Some(V::Void)
            }
            "getAndSet" => {
                let old = rt.get_field(recv, "$val").i() != 0;
                rt.put_field(recv, "$val", V::bool(args.first()?.i() != 0));
                Some(V::bool(old))
            }
            "compareAndSet" => {
                let cur = rt.get_field(recv, "$val").i() != 0;
                if cur == (args.first()?.i() != 0) {
                    rt.put_field(recv, "$val", V::bool(args.get(1)?.i() != 0));
                    Some(V::TRUE)
                } else {
                    Some(V::FALSE)
                }
            }
            _ => None,
        },
        "java/util/concurrent/atomic/AtomicReference" => match name {
            "<init>" => {
                let init = args.first().copied().unwrap_or(V::NULL);
                rt.put_field(recv, "$val", init);
                Some(V::Void)
            }
            "get" => Some(rt.get_field(recv, "$val")),
            "set" | "lazySet" => {
                rt.put_field(recv, "$val", *args.first()?);
                Some(V::Void)
            }
            "getAndSet" => {
                let old = rt.get_field(recv, "$val");
                rt.put_field(recv, "$val", *args.first()?);
                Some(old)
            }
            "compareAndSet" => {
                let cur = rt.get_field(recv, "$val");
                if cur == *args.first()? {
                    rt.put_field(recv, "$val", *args.get(1)?);
                    Some(V::TRUE)
                } else {
                    Some(V::FALSE)
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn new_comparator_obj(rt: &mut Rt, kind: &str, f1: V, f2: V) -> V {
    let obj = rt.new_instance("java/util/Comparator", 0);
    let k = rt.intern_str(kind);
    rt.put_field(obj, "$cmp_kind", k);
    rt.put_field(obj, "$f1", f1);
    rt.put_field(obj, "$f2", f2);
    obj
}

fn dispatch_comparator(
    ctx: &mut ExecCtx<'_>,
    name: &str,
    _desc: &str,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return match name {
            "naturalOrder" => Some(new_comparator_obj(ctx.rt, "natural", V::NULL, V::NULL)),
            "reverseOrder" => Some(new_comparator_obj(ctx.rt, "reverse_natural", V::NULL, V::NULL)),
            "comparing" => {
                let key_fn = *args.first()?;
                let key_cmp = args.get(1).copied().unwrap_or(V::NULL);
                Some(new_comparator_obj(ctx.rt, "key", key_fn, key_cmp))
            }
            "comparingInt" => Some(new_comparator_obj(ctx.rt, "key_int", *args.first()?, V::NULL)),
            "comparingLong" => Some(new_comparator_obj(ctx.rt, "key_long", *args.first()?, V::NULL)),
            "comparingDouble" => {
                Some(new_comparator_obj(ctx.rt, "key_double", *args.first()?, V::NULL))
            }
            "nullsFirst" => Some(new_comparator_obj(ctx.rt, "nulls_first", *args.first()?, V::NULL)),
            "nullsLast" => Some(new_comparator_obj(ctx.rt, "nulls_last", *args.first()?, V::NULL)),
            _ => None,
        };
    }

    let recv = *args.first()?;
    match name {
        "compare" if args.len() == 3 => Some(V::I(eval_comparator(ctx, recv, args[1], args[2]))),
        "reversed" => Some(new_comparator_obj(ctx.rt, "reversed", recv, V::NULL)),
        "thenComparing" => {
            let arg1 = *args.get(1)?;
            let second = if args.len() == 3 {
                new_comparator_obj(ctx.rt, "key", arg1, args[2])
            } else if ctx
                .rt
                .get_obj(arg1)
                .and_then(|o| match &o.data {
                    Data::Closure(c) => Some(c.sam_name.as_str()),
                    _ => None,
                })
                == Some("apply")
            {
                new_comparator_obj(ctx.rt, "key", arg1, V::NULL)
            } else {
                arg1
            };
            Some(new_comparator_obj(ctx.rt, "chain", recv, second))
        }
        "thenComparingInt" => {
            let second = new_comparator_obj(ctx.rt, "key_int", *args.get(1)?, V::NULL);
            Some(new_comparator_obj(ctx.rt, "chain", recv, second))
        }
        "thenComparingLong" => {
            let second = new_comparator_obj(ctx.rt, "key_long", *args.get(1)?, V::NULL);
            Some(new_comparator_obj(ctx.rt, "chain", recv, second))
        }
        "thenComparingDouble" => {
            let second = new_comparator_obj(ctx.rt, "key_double", *args.get(1)?, V::NULL);
            Some(new_comparator_obj(ctx.rt, "chain", recv, second))
        }
        _ => None,
    }
}

fn eval_comparator(ctx: &mut ExecCtx<'_>, cmp: V, a: V, b: V) -> i32 {
    if cmp.is_null() {
        return eval_natural_order(ctx, a, b);
    }
    if matches!(ctx.rt.get_obj(cmp).map(|o| &o.data), Some(Data::Closure(_))) {
        return crate::vm::invoke_closure(ctx, cmp, &[a, b]).i();
    }
    let kind = ctx
        .rt
        .str_of(ctx.rt.get_field(cmp, "$cmp_kind"))
        .unwrap_or("")
        .to_owned();
    let f1 = ctx.rt.get_field(cmp, "$f1");
    let f2 = ctx.rt.get_field(cmp, "$f2");
    match kind.as_str() {
        "natural" => eval_natural_order(ctx, a, b),
        "reverse_natural" => -eval_natural_order(ctx, a, b),
        "reversed" => -eval_comparator(ctx, f1, a, b),
        "chain" => {
            let first = eval_comparator(ctx, f1, a, b);
            if first != 0 {
                first
            } else {
                eval_comparator(ctx, f2, a, b)
            }
        }
        "key" => {
            let ka = crate::vm::invoke_closure(ctx, f1, &[a]);
            let kb = crate::vm::invoke_closure(ctx, f1, &[b]);
            if f2.is_null() {
                eval_natural_order(ctx, ka, kb)
            } else {
                eval_comparator(ctx, f2, ka, kb)
            }
        }
        "key_int" => {
            let ka = crate::vm::invoke_closure(ctx, f1, &[a]).i();
            let kb = crate::vm::invoke_closure(ctx, f1, &[b]).i();
            ka.cmp(&kb) as i32
        }
        "key_long" => {
            let ka = crate::vm::invoke_closure(ctx, f1, &[a]).j();
            let kb = crate::vm::invoke_closure(ctx, f1, &[b]).j();
            ka.cmp(&kb) as i32
        }
        "key_double" => {
            let ka = crate::vm::invoke_closure(ctx, f1, &[a]).d();
            let kb = crate::vm::invoke_closure(ctx, f1, &[b]).d();
            ka.total_cmp(&kb) as i32
        }
        "nulls_first" => match (a.is_null(), b.is_null()) {
            (true, true) => 0,
            (true, false) => -1,
            (false, true) => 1,
            (false, false) => eval_comparator(ctx, f1, a, b),
        },
        "nulls_last" => match (a.is_null(), b.is_null()) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => -1,
            (false, false) => eval_comparator(ctx, f1, a, b),
        },
        _ => {
            let mref = MemberRef::new("java/util/Comparator", "compare", "(Ljava/lang/Object;Ljava/lang/Object;)I");
            crate::vm::invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE, &mref, &[cmp, a, b]).i()
        }
    }
}

fn eval_natural_order(ctx: &mut ExecCtx<'_>, a: V, b: V) -> i32 {
    if let (Some(sa), Some(sb)) = (ctx.rt.str_of(a), ctx.rt.str_of(b)) {
        return sa.cmp(sb) as i32;
    }
    let mref = MemberRef::new("java/lang/Comparable", "compareTo", "(Ljava/lang/Object;)I");
    crate::vm::invoke_member(ctx, mmga_classfile::op::INVOKEINTERFACE, &mref, &[a, b]).i()
}

fn dispatch_number_format(
    rt: &mut Rt,
    name: &str,
    desc: &str,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return match name {
            "getIntegerInstance" => {
                let nf = rt.new_instance("java/text/DecimalFormat", 0);
                rt.put_field(nf, "$int_only", V::I(1));
                rt.put_field(nf, "$grouping", V::I(1));
                rt.put_field(nf, "$max_frac", V::I(0));
                Some(nf)
            }
            "getNumberInstance" | "getInstance" => {
                let nf = rt.new_instance("java/text/DecimalFormat", 0);
                rt.put_field(nf, "$int_only", V::I(0));
                rt.put_field(nf, "$grouping", V::I(1));
                rt.put_field(nf, "$max_frac", V::I(3));
                Some(nf)
            }
            _ => None,
        };
    }
    let recv = *args.first()?;
    match (name, desc) {
        ("<init>", _) => {
            rt.put_field(recv, "$int_only", V::I(0));
            rt.put_field(recv, "$grouping", V::I(1));
            rt.put_field(recv, "$max_frac", V::I(3));
            Some(V::Void)
        }
        ("setMaximumFractionDigits", "(I)V") => {
            rt.put_field(recv, "$max_frac", V::I(args[1].i().max(0)));
            Some(V::Void)
        }
        ("setMinimumFractionDigits", "(I)V") => {
            rt.put_field(recv, "$min_frac", V::I(args[1].i().max(0)));
            Some(V::Void)
        }
        ("setGroupingUsed", "(Z)V") => {
            rt.put_field(recv, "$grouping", V::bool(args[1].i() != 0));
            Some(V::Void)
        }
        ("setRoundingMode", _) => Some(V::Void),
        ("format", "(J)Ljava/lang/String;") => {
            let group = rt.get_field(recv, "$grouping").i() != 0;
            let s = format_grouped_int(args[1].j(), group);
            Some(rt.new_str(s))
        }
        ("format", "(D)Ljava/lang/String;") => {
            let group = rt.get_field(recv, "$grouping").i() != 0;
            let max_frac = rt.get_field(recv, "$max_frac").i().clamp(0, 12) as usize;
            let d = args[1].d();
            if max_frac == 0 || d.fract() == 0.0 {
                Some(rt.new_str(format_grouped_int(d.round() as i64, group)))
            } else {
                let raw = format!("{d:.max_frac$}");
                let trimmed = raw.trim_end_matches('0').trim_end_matches('.');
                let (int_part, frac_part) = trimmed.split_once('.').unwrap_or((trimmed, ""));
                let i_val = int_part.parse::<i64>().unwrap_or(0);
                let g = format_grouped_int(i_val, group);
                let out = if frac_part.is_empty() {
                    if d < 0.0 && i_val == 0 {
                        format!("-{g}")
                    } else {
                        g
                    }
                } else if d < 0.0 && i_val == 0 {
                    format!("-{g}.{frac_part}")
                } else {
                    format!("{g}.{frac_part}")
                };
                Some(rt.new_str(out))
            }
        }
        ("format", "(Ljava/lang/Object;)Ljava/lang/String;") => {
            let u = unbox_primitive(rt, args[1]);
            match u {
                V::J(j) => Some(rt.new_str(format_grouped_int(j, rt.get_field(recv, "$grouping").i() != 0))),
                V::I(i) => Some(rt.new_str(format_grouped_int(i64::from(i), rt.get_field(recv, "$grouping").i() != 0))),
                V::F(f) => dispatch_number_format(rt, "format", "(D)Ljava/lang/String;", false, &[recv, V::D(f64::from(f))]),
                V::D(d) => dispatch_number_format(rt, "format", "(D)Ljava/lang/String;", false, &[recv, V::D(d)]),
                _ => Some(rt.new_str(rt.value_to_string(args[1]))),
            }
        }
        _ => None,
    }
}

fn format_grouped_int(val: i64, grouping: bool) -> String {
    if !grouping {
        return val.to_string();
    }
    let neg = val < 0;
    let digits = val.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if neg {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// RFC 1321 MD5 digest used exclusively for `java.util.UUID.nameUUIDFromBytes` (RFC 4122 v3).
fn md5_digest(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let (mut a0, mut b0, mut c0, mut d0) =
        (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | ((!b) & d), i)
            } else if i < 32 {
                ((d & b) | ((!d) & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | (!d)), (7 * i) % 16)
            };
            let tmp = f
                .wrapping_add(a)
                .wrapping_add(K[i])
                .wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(tmp.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

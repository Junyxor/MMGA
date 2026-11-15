//! Real value, text, resource, and config APIs for Minecraft 1.21.1 + NeoForge 21.1.
//!
//! Every method implemented here preserves real data (text trees, language keys,
//! jar resource bytes, filesystem paths, `ModConfigSpec` entries). Any method or
//! class not explicitly handled returns `None` so the VM raises `UnsupportedOperationException`.

use crate::subsystems::{ArgumentKind, BrigadierTree, CommandNodeKind, ConfigPrimitive, ConfigSubsystem};
use mmga_classfile::MemberRef;
use mmga_runtime::{box_primitive, invoke_closure, unbox_primitive, Data, ExecCtx, LinkedSpace, Rt, V};
use std::collections::BTreeMap;

const MUTABLE_COMPONENT: &str = "net/minecraft/network/chat/MutableComponent";
const STYLE_CLASS: &str = "net/minecraft/network/chat/Style";
const CHAT_FORMATTING: &str = "net/minecraft/ChatFormatting";

/// Extracts `en_us.json` key-value translation entries from a jar's resource map.
pub fn extract_lang_entries(
    resources: &BTreeMap<String, Vec<u8>>,
    into: &mut BTreeMap<String, String>,
) {
    for (path, bytes) in resources {
        if path.starts_with("assets/") && path.ends_with("/lang/en_us.json") {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice(bytes) {
                for (k, v) in map {
                    if let Some(text) = v.as_str() {
                        into.insert(k, text.to_owned());
                    }
                }
            }
        }
    }
}

/// Static field lookups for `Component`, `Style`, `ChatFormatting`, and `com/sun/jna/Platform`.
pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str) -> Option<V> {
    if owner == STYLE_CLASS && name == "EMPTY" {
        return Some(new_style(rt));
    }
    if owner == CHAT_FORMATTING {
        return chat_formatting_constant(rt, name);
    }
    if owner == "com/sun/jna/Platform" && name == "C_LIBRARY_NAME" {
        let cname = if cfg!(windows) { "msvcrt" } else { "c" };
        return Some(rt.intern_str(cname));
    }
    None
}

/// Subtype rules for `Component` / `MutableComponent` / `ModConfigSpec`.
pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    if sub_class == MUTABLE_COMPONENT
        && matches!(
            super_class,
            "net/minecraft/network/chat/Component" | "net/minecraft/network/chat/FormattedText"
        )
    {
        return true;
    }
    if super_class == "net/neoforged/fml/config/IConfigSpec"
        && sub_class == "net/neoforged/neoforge/common/ModConfigSpec"
    {
        return true;
    }
    if super_class == "net/neoforged/neoforge/common/ModConfigSpec$ConfigValue"
        && sub_class.starts_with("net/neoforged/neoforge/common/ModConfigSpec$")
        && sub_class.ends_with("Value")
    {
        return true;
    }
    if super_class == "java/util/function/Supplier"
        && sub_class.starts_with("net/neoforged/neoforge/common/ModConfigSpec$")
        && sub_class.ends_with("Value")
    {
        return true;
    }
    if super_class == "com/mojang/brigadier/builder/ArgumentBuilder"
        && (sub_class == "com/mojang/brigadier/builder/LiteralArgumentBuilder"
            || sub_class == "com/mojang/brigadier/builder/RequiredArgumentBuilder")
    {
        return true;
    }
    if super_class == "net/minecraft/commands/SharedSuggestionProvider"
        && sub_class == "net/minecraft/commands/CommandSourceStack"
    {
        return true;
    }
    false
}

/// Dispatches `net.minecraft.network.chat.Component` / `MutableComponent` / `Style` / `ChatFormatting`.
pub fn call_chat(
    rt: &mut Rt,
    translations: &BTreeMap<String, String>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    if (owner == "net/minecraft/network/chat/Component" || owner == MUTABLE_COMPONENT) && is_static
    {
        match (name, desc) {
            ("empty", "()Lnet/minecraft/network/chat/MutableComponent;") => {
                return Some(new_literal_component(rt, ""));
            }
            ("literal", "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let text = rt.str_of(*args.first()?)?.to_owned();
                return Some(new_literal_component(rt, &text));
            }
            ("nullToEmpty", "(Ljava/lang/String;)Lnet/minecraft/network/chat/Component;") => {
                let text = args
                    .first()
                    .and_then(|&v| rt.str_of(v))
                    .unwrap_or("")
                    .to_owned();
                return Some(new_literal_component(rt, &text));
            }
            (
                "translatable",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                return Some(new_translatable_component(rt, &key, None, Vec::new()));
            }
            (
                "translatable",
                "(Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                let fmt_args = extract_varargs(rt, args.get(1).copied().unwrap_or(V::NULL));
                return Some(new_translatable_component(rt, &key, None, fmt_args));
            }
            (
                "translatableWithFallback",
                "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                let fallback = args.get(1).and_then(|&v| rt.str_of(v)).map(str::to_owned);
                return Some(new_translatable_component(rt, &key, fallback, Vec::new()));
            }
            (
                "translatableWithFallback",
                "(Ljava/lang/String;Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let key = rt.str_of(*args.first()?)?.to_owned();
                let fallback = args.get(1).and_then(|&v| rt.str_of(v)).map(str::to_owned);
                let fmt_args = extract_varargs(rt, args.get(2).copied().unwrap_or(V::NULL));
                return Some(new_translatable_component(rt, &key, fallback, fmt_args));
            }
            _ => return None,
        }
    }

    if (owner == "net/minecraft/network/chat/Component" || owner == MUTABLE_COMPONENT)
        && !is_static
    {
        let recv = *args.first()?;
        match (name, desc) {
            ("getString", "()Ljava/lang/String;") => {
                let rendered = render_component_text(rt, translations, recv);
                return Some(rt.new_str(rendered));
            }
            ("getString", "(I)Ljava/lang/String;") => {
                let max_len = args.get(1).map_or(0, |v| v.i().max(0) as usize);
                let rendered = render_component_text(rt, translations, recv);
                let utf16: Vec<u16> = rendered.encode_utf16().take(max_len).collect();
                return Some(rt.new_str(String::from_utf16_lossy(&utf16)));
            }
            (
                "append",
                "(Lnet/minecraft/network/chat/Component;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let child = *args.get(1)?;
                append_sibling(rt, recv, child);
                return Some(recv);
            }
            (
                "append",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ) => {
                let piece = rt.str_of(*args.get(1)?)?.to_owned();
                let child = new_literal_component(rt, &piece);
                append_sibling(rt, recv, child);
                return Some(recv);
            }
            ("getSiblings", "()Ljava/util/List;") => {
                return Some(rt.get_field(recv, "$siblings"));
            }
            ("getStyle", "()Lnet/minecraft/network/chat/Style;") => {
                let s = rt.get_field(recv, "$style");
                return Some(if s.is_null() { new_style(rt) } else { s });
            }
            ("setStyle", "(Lnet/minecraft/network/chat/Style;)Lnet/minecraft/network/chat/MutableComponent;")
            | ("withStyle", "(Lnet/minecraft/network/chat/Style;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let s = *args.get(1)?;
                rt.put_field(recv, "$style", s);
                return Some(recv);
            }
            ("withStyle", "(Lnet/minecraft/ChatFormatting;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let fmt = *args.get(1)?;
                let cur = {
                    let s = rt.get_field(recv, "$style");
                    if s.is_null() { new_style(rt) } else { s }
                };
                let updated = apply_formatting_to_style(rt, cur, fmt);
                rt.put_field(recv, "$style", updated);
                return Some(recv);
            }
            ("withStyle", "([Lnet/minecraft/ChatFormatting;)Lnet/minecraft/network/chat/MutableComponent;") => {
                let arr = *args.get(1)?;
                let mut cur = {
                    let s = rt.get_field(recv, "$style");
                    if s.is_null() { new_style(rt) } else { s }
                };
                let items = extract_varargs(rt, arr);
                for fmt in items {
                    cur = apply_formatting_to_style(rt, cur, fmt);
                }
                rt.put_field(recv, "$style", cur);
                return Some(recv);
            }
            ("withColor", "(I)Lnet/minecraft/network/chat/MutableComponent;") => {
                let rgb = args.get(1)?.i();
                let cur = copy_style(rt, rt.get_field(recv, "$style"));
                rt.put_field(cur, "color", V::I(rgb));
                rt.put_field(recv, "$style", cur);
                return Some(recv);
            }
            ("plainCopy", "()Lnet/minecraft/network/chat/MutableComponent;") => {
                return Some(clone_component(rt, recv, false));
            }
            ("copy", "()Lnet/minecraft/network/chat/MutableComponent;") => {
                return Some(clone_component(rt, recv, true));
            }
            ("equals", "(Ljava/lang/Object;)Z") => {
                let other = args.get(1).copied().unwrap_or(V::NULL);
                let eq = component_equals(rt, translations, recv, other);
                return Some(V::bool(eq));
            }
            ("toString", "()Ljava/lang/String;") => {
                let rendered = render_component_text(rt, translations, recv);
                return Some(rt.new_str(rendered));
            }
            _ => return None,
        }
    }

    if owner == CHAT_FORMATTING {
        if is_static && name == "stripFormatting" {
            let Some(text) = args.first().and_then(|&v| rt.str_of(v)).map(str::to_owned) else {
                return Some(V::NULL);
            };
            let mut stripped = String::with_capacity(text.len());
            let mut chars = text.chars();
            while let Some(c) = chars.next() {
                if c == '\u{00a7}' {
                    let _ = chars.next();
                } else {
                    stripped.push(c);
                }
            }
            return Some(rt.new_str(stripped));
        }
        if !is_static {
            let recv = *args.first()?;
            return match name {
                "getName" => Some(rt.get_field(recv, "$fmt_name")),
                "getChar" => Some(rt.get_field(recv, "$code")),
                "isFormat" => Some(rt.get_field(recv, "$is_format")),
                "isColor" => {
                    let is_fmt = rt.get_field(recv, "$is_format").i() != 0;
                    let ord = rt.get_field(recv, "$ordinal").i();
                    Some(V::bool(!is_fmt && ord < 16))
                }
                "getColor" => {
                    let col = rt.get_field(recv, "$rgb");
                    if col.i() < 0 {
                        Some(V::NULL)
                    } else {
                        Some(box_primitive(rt, "java/lang/Integer", col))
                    }
                }
                "toString" => Some(rt.get_field(recv, "$prefix")),
                _ => None,
            };
        }
    }

    if owner == STYLE_CLASS && !is_static {
        let recv = *args.first()?;
        return match name {
            "withColor" if desc == "(I)Lnet/minecraft/network/chat/Style;" => {
                let next = copy_style(rt, recv);
                rt.put_field(next, "color", args[1]);
                Some(next)
            }
            "withColor" if desc == "(Lnet/minecraft/ChatFormatting;)Lnet/minecraft/network/chat/Style;" => {
                Some(apply_formatting_to_style(rt, recv, args[1]))
            }
            "withBold" | "withItalic" | "withUnderlined" | "withStrikethrough" | "withObfuscated" => {
                let next = copy_style(rt, recv);
                let b = if args[1].is_null() {
                    V::I(0)
                } else {
                    V::bool(unbox_primitive(rt, args[1]).i() != 0)
                };
                rt.put_field(next, name.trim_start_matches("with"), b);
                Some(next)
            }
            "applyFormat" => Some(apply_formatting_to_style(rt, recv, args[1])),
            "isEmpty" => {
                let empty = rt.get_field(recv, "color").i() < 0
                    && rt.get_field(recv, "Bold").i() == 0
                    && rt.get_field(recv, "Italic").i() == 0;
                Some(V::bool(empty))
            }
            _ => None,
        };
    }

    None
}

/// Dispatches jar resource streams, `java.nio.file.Path`, Apache Commons `IOUtils` / `FileUtils` / `StringUtils`, and Linux libc JNA inspection.
pub fn call_resources_and_io(
    rt: &mut Rt,
    jar_resources: &BTreeMap<String, Vec<u8>>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    if owner == "java/lang/Class" && !is_static && (name == "getResource" || name == "getResourceAsStream") {
        let cls_tag = rt.handle_of(*args.first()?)?.2.to_owned();
        let raw_path = rt.str_of(*args.get(1)?)?;
        let key = resolve_class_resource_path(&cls_tag, raw_path);
        let Some(bytes) = jar_resources.get(&key).cloned() else {
            return Some(V::NULL);
        };
        if name == "getResource" {
            return Some(rt.new_handle("java/net/URL", "jar_url", 0, key));
        }
        return Some(rt.alloc(
            "java/io/ByteArrayInputStream",
            0,
            Data::ByteBuf {
                bytes,
                reader_idx: 0,
            },
        ));
    }

    if owner == "java/net/URL" && !is_static && name == "openStream" {
        let ("jar_url", _, key, _) = rt.handle_of(*args.first()?)? else {
            return None;
        };
        let key = key.to_owned();
        let Some(bytes) = jar_resources.get(&key).cloned() else {
            return Some(rt.throw_exception("java/io/FileNotFoundException", &key));
        };
        return Some(rt.alloc(
            "java/io/ByteArrayInputStream",
            0,
            Data::ByteBuf {
                bytes,
                reader_idx: 0,
            },
        ));
    }

    if (owner == "java/nio/file/Path" || owner == "java/nio/file/Paths")
        && is_static
        && (name == "of" || name == "get")
        && desc.starts_with("(Ljava/lang/String;[Ljava/lang/String;)")
    {
        let first = rt.str_of(*args.first()?)?.to_owned();
        let mut pb = std::path::PathBuf::from(first);
        for extra in extract_varargs(rt, args.get(1).copied().unwrap_or(V::NULL)) {
            if let Some(seg) = rt.str_of(extra) {
                if !seg.is_empty() {
                    pb.push(seg);
                }
            }
        }
        return Some(rt.new_handle("java/nio/file/Path", "nio_path", 0, pb.display().to_string()));
    }

    if owner == "java/nio/file/Path" && !is_static {
        let base = path_string(rt, *args.first()?)?;
        return match name {
            "toAbsolutePath" => {
                let p = std::path::Path::new(&base);
                let abs = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                        .join(p)
                };
                Some(rt.new_handle("java/nio/file/Path", "nio_path", 0, abs.display().to_string()))
            }
            "normalize" => {
                let norm = normalize_lexical(std::path::Path::new(&base));
                Some(rt.new_handle("java/nio/file/Path", "nio_path", 0, norm.display().to_string()))
            }
            "getParent" => match std::path::Path::new(&base).parent() {
                Some(parent) if !parent.as_os_str().is_empty() => Some(rt.new_handle(
                    "java/nio/file/Path",
                    "nio_path",
                    0,
                    parent.display().to_string(),
                )),
                _ => Some(V::NULL),
            },
            "getFileName" => match std::path::Path::new(&base).file_name() {
                Some(fname) => Some(rt.new_handle(
                    "java/nio/file/Path",
                    "nio_path",
                    0,
                    fname.to_string_lossy().into_owned(),
                )),
                None => Some(V::NULL),
            },
            "toString" => Some(rt.new_str(base)),
            _ => None,
        };
    }

    if owner == "org/apache/commons/io/IOUtils" && is_static {
        match (name, desc) {
            ("readFully", "(Ljava/io/InputStream;I)[B") => {
                let stream = *args.first()?;
                let want = args.get(1)?.i().max(0) as usize;
                let slice: Result<Vec<V>, &'static str> = match rt.get_obj_mut(stream).map(|o| &mut o.data) {
                    Some(Data::ByteBuf { bytes, reader_idx }) => {
                        if *reader_idx + want > bytes.len() {
                            Err("EOFException in IOUtils.readFully")
                        } else {
                            let out = bytes[*reader_idx..*reader_idx + want]
                                .iter()
                                .map(|&b| V::I(i32::from(b as i8)))
                                .collect();
                            *reader_idx += want;
                            Ok(out)
                        }
                    }
                    _ => Err("NullPointerException in IOUtils.readFully"),
                };
                return match slice {
                    Ok(arr) => Some(rt.alloc("[B", 0, Data::Array(arr))),
                    Err(msg) => Some(rt.throw_exception("java/io/EOFException", msg)),
                };
            }
            ("toByteArray", "(Ljava/io/InputStream;)[B") => {
                let stream = *args.first()?;
                let arr: Vec<V> = match rt.get_obj_mut(stream).map(|o| &mut o.data) {
                    Some(Data::ByteBuf { bytes, reader_idx }) => {
                        let out = bytes[*reader_idx..]
                            .iter()
                            .map(|&b| V::I(i32::from(b as i8)))
                            .collect();
                        *reader_idx = bytes.len();
                        out
                    }
                    _ => return Some(rt.throw_exception("java/io/IOException", "invalid InputStream")),
                };
                return Some(rt.alloc("[B", 0, Data::Array(arr)));
            }
            _ => return None,
        }
    }

    if owner == "org/apache/commons/io/FileUtils" && is_static {
        match (name, desc) {
            ("readFileToByteArray", "(Ljava/io/File;)[B") => {
                let path = path_string(rt, *args.first()?)?;
                return match std::fs::read(&path) {
                    Ok(bytes) => {
                        let arr: Vec<V> = bytes.into_iter().map(|b| V::I(i32::from(b as i8))).collect();
                        Some(rt.alloc("[B", 0, Data::Array(arr)))
                    }
                    Err(e) => Some(rt.throw_exception("java/io/IOException", &format!("{path}: {e}"))),
                };
            }
            ("writeByteArrayToFile", "(Ljava/io/File;[B)V")
            | ("writeByteArrayToFile", "(Ljava/io/File;[BZ)V") => {
                let path = path_string(rt, *args.first()?)?;
                let append = args.get(2).is_some_and(|v| v.i() != 0);
                let bytes: Vec<u8> = match rt.get_obj(*args.get(1)?).map(|o| &o.data) {
                    Some(Data::Array(items)) => items.iter().map(|v| v.i() as u8).collect(),
                    _ => return Some(rt.throw_exception("java/lang/NullPointerException", "writeByteArrayToFile(null)")),
                };
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let res = if append {
                    use std::io::Write;
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                        .and_then(|mut f| f.write_all(&bytes))
                } else {
                    std::fs::write(&path, &bytes)
                };
                return match res {
                    Ok(()) => Some(V::Void),
                    Err(e) => Some(rt.throw_exception("java/io/IOException", &format!("{path}: {e}"))),
                };
            }
            _ => return None,
        }
    }

    if owner == "org/apache/commons/lang3/StringUtils" && is_static {
        let s = args.first().and_then(|&v| rt.str_of(v));
        return match name {
            "isEmpty" => Some(V::bool(s.is_none_or(str::is_empty))),
            "isNotEmpty" => Some(V::bool(s.is_some_and(|t| !t.is_empty()))),
            "isBlank" => Some(V::bool(s.is_none_or(|t| t.trim().is_empty()))),
            "isNotBlank" => Some(V::bool(s.is_some_and(|t| !t.trim().is_empty()))),
            _ => None,
        };
    }

    // JNA libc inspection (`NativeLibrary.getInstance("c")` / `getFunction(sym)`).
    // Only symbol-existence probing on the platform C library is supported; invoking native C
    // functions is rejected with `UnsupportedOperationException`.
    if owner == "com/sun/jna/NativeLibrary" {
        if is_static && name == "getInstance" {
            let lib = rt.str_of(*args.first()?)?.to_owned();
            if lib == "c" || lib == "msvcrt" {
                return Some(rt.new_handle("com/sun/jna/NativeLibrary", "jna_lib", 0, lib));
            }
            return Some(rt.throw_exception("java/lang/UnsatisfiedLinkError", &lib));
        }
        if !is_static && name == "getFunction" {
            let sym = rt.str_of(*args.get(1)?)?.to_owned();
            let exists_on_host = match sym.as_str() {
                "gnu_get_libc_version" => cfg!(all(target_os = "linux", target_env = "gnu")),
                "android_set_abort_message" => cfg!(target_os = "android"),
                _ => false,
            };
            if exists_on_host {
                return Some(rt.new_handle("com/sun/jna/Function", "jna_fn", 0, sym));
            }
            return Some(rt.throw_exception("java/lang/UnsatisfiedLinkError", &sym));
        }
    }

    None
}

/// Dispatches `ModConfigSpec` and `ModConfigSpec$Builder` onto `ConfigSubsystem`.
pub fn call_mod_config_spec(
    rt: &mut Rt,
    linked: &LinkedSpace,
    host: &mut dyn mmga_runtime::HostApi,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
    extract_cfg: impl FnOnce(&mut dyn mmga_runtime::HostApi) -> &mut ConfigSubsystem,
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();

    if owner == "net/neoforged/neoforge/common/ModConfigSpec$Builder" && !is_static {
        let recv = *args.first()?;
        // `Builder.configure(Function<Builder, T>) -> Pair<T, ModConfigSpec>` executes a user closure!
        if name == "configure" {
            let func = *args.get(1)?;
            let mut ctx = ExecCtx {
                rt,
                linked,
                host,
                depth: 0,
            };
            let user_cfg = invoke_closure(&mut ctx, func, &[recv]);
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
            let builder_id = ctx.rt.get_field(recv, "$builder_id").j() as u64;
            let spec_id = extract_cfg(ctx.host).build_spec(builder_id);
            let spec = ctx.rt.new_pinned_handle(
                "net/neoforged/neoforge/common/ModConfigSpec",
                "config_spec",
                spec_id,
                "",
            );
            let pair = ctx
                .rt
                .new_instance("org/apache/commons/lang3/tuple/ImmutablePair", 0);
            ctx.rt.put_field(pair, "left", user_cfg);
            ctx.rt.put_field(pair, "right", spec);
            return Some(pair);
        }

        let cfg = extract_cfg(host);
        return match name {
            "<init>" => {
                let bid = cfg.new_builder();
                rt.put_field(recv, "$builder_id", V::J(bid as i64));
                Some(V::Void)
            }
            "push" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                if let Some(sec) = args.get(1).and_then(|&v| rt.str_of(v)).map(str::to_owned) {
                    cfg.push_section(bid, &sec);
                } else if let Some(Data::List(items)) =
                    args.get(1).and_then(|&v| rt.get_obj(v)).map(|o| &o.data)
                {
                    let segs: Vec<String> = items
                        .iter()
                        .filter_map(|&v| rt.str_of(v).map(str::to_owned))
                        .collect();
                    for seg in segs {
                        cfg.push_section(bid, &seg);
                    }
                }
                Some(recv)
            }
            "pop" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let n = args.get(1).map_or(1, |v| v.i().max(1) as usize);
                cfg.pop_section(bid, n);
                Some(recv)
            }
            "comment" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                if let Some(arg1) = args.get(1).copied() {
                    if let Some(text) = rt.str_of(arg1).map(str::to_owned) {
                        cfg.set_comment(bid, text);
                    } else if let Some(Data::Array(lines)) = rt.get_obj(arg1).map(|o| &o.data) {
                        let joined: Vec<String> = lines
                            .iter()
                            .filter_map(|&v| rt.str_of(v).map(str::to_owned))
                            .collect();
                        cfg.set_comment(bid, joined.join("\n"));
                    }
                }
                Some(recv)
            }
            "translation" | "worldRestart" | "gameRestart" => Some(recv),
            "build" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let spec_id = cfg.build_spec(bid);
                Some(rt.new_pinned_handle(
                    "net/neoforged/neoforge/common/ModConfigSpec",
                    "config_spec",
                    spec_id,
                    "",
                ))
            }
            "define" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                let def_v = *args.get(2)?;
                if desc.ends_with("BooleanValue;") {
                    let b = unbox_primitive(rt, def_v).i() != 0;
                    let eid = cfg.define_entry(bid, &key, ConfigPrimitive::Bool(b), None);
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$BooleanValue",
                        eid,
                        def_v,
                    ))
                } else {
                    let prim = if let Some(s) = rt.str_of(def_v) {
                        ConfigPrimitive::Text(s.to_owned())
                    } else {
                        ConfigPrimitive::Text(rt.value_to_string(def_v))
                    };
                    let eid = cfg.define_entry(bid, &key, prim, None);
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$ConfigValue",
                        eid,
                        def_v,
                    ))
                }
            }
            "defineInRange" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                if desc.ends_with("IntValue;") {
                    let (def, lo, hi) = (args.get(2)?.i(), args.get(3)?.i(), args.get(4)?.i());
                    let eid = cfg.define_entry(
                        bid,
                        &key,
                        ConfigPrimitive::Int(def),
                        Some((f64::from(lo), f64::from(hi))),
                    );
                    let boxed = box_primitive(rt, "java/lang/Integer", V::I(def));
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$IntValue",
                        eid,
                        boxed,
                    ))
                } else if desc.ends_with("LongValue;") {
                    let (def, lo, hi) = (args.get(2)?.j(), args.get(3)?.j(), args.get(4)?.j());
                    let eid = cfg.define_entry(
                        bid,
                        &key,
                        ConfigPrimitive::Long(def),
                        Some((lo as f64, hi as f64)),
                    );
                    let boxed = box_primitive(rt, "java/lang/Long", V::J(def));
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$LongValue",
                        eid,
                        boxed,
                    ))
                } else if desc.ends_with("DoubleValue;") {
                    let (def, lo, hi) = (args.get(2)?.d(), args.get(3)?.d(), args.get(4)?.d());
                    let eid = cfg.define_entry(
                        bid,
                        &key,
                        ConfigPrimitive::Double(def),
                        Some((lo, hi)),
                    );
                    let boxed = box_primitive(rt, "java/lang/Double", V::D(def));
                    Some(new_config_value_handle(
                        rt,
                        "net/neoforged/neoforge/common/ModConfigSpec$DoubleValue",
                        eid,
                        boxed,
                    ))
                } else {
                    None
                }
            }
            "defineEnum" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                let def_enum = *args.get(2)?;
                let label = rt.value_to_string(def_enum);
                let eid = cfg.define_entry(bid, &key, ConfigPrimitive::Text(label), None);
                Some(new_config_value_handle(
                    rt,
                    "net/neoforged/neoforge/common/ModConfigSpec$EnumValue",
                    eid,
                    def_enum,
                ))
            }
            "defineList" | "defineListAllowEmpty" => {
                let bid = rt.get_field(recv, "$builder_id").j() as u64;
                let key = config_key_text(rt, *args.get(1)?)?;
                let def_list = *args.get(2)?;
                let eid = cfg.define_entry(
                    bid,
                    &key,
                    ConfigPrimitive::Text(rt.value_to_string(def_list)),
                    None,
                );
                Some(new_config_value_handle(
                    rt,
                    "net/neoforged/neoforge/common/ModConfigSpec$ConfigValue",
                    eid,
                    def_list,
                ))
            }
            _ => None,
        };
    }

    if owner.starts_with("net/neoforged/neoforge/common/ModConfigSpec$")
        && owner.ends_with("Value")
        && !is_static
    {
        let recv = *args.first()?;
        let eid = rt.get_field(recv, "$entry_id").j() as u64;
        let cfg = extract_cfg(host);
        return match name {
            "get" => match cfg.current(eid).cloned() {
                Some(ConfigPrimitive::Bool(b)) => {
                    if desc == "()Z" {
                        Some(V::bool(b))
                    } else {
                        Some(box_primitive(rt, "java/lang/Boolean", V::bool(b)))
                    }
                }
                Some(ConfigPrimitive::Int(i)) => {
                    if desc == "()I" {
                        Some(V::I(i))
                    } else {
                        Some(box_primitive(rt, "java/lang/Integer", V::I(i)))
                    }
                }
                Some(ConfigPrimitive::Long(l)) => {
                    if desc == "()J" {
                        Some(V::J(l))
                    } else {
                        Some(box_primitive(rt, "java/lang/Long", V::J(l)))
                    }
                }
                Some(ConfigPrimitive::Double(d)) => {
                    if desc == "()D" {
                        Some(V::D(d))
                    } else {
                        Some(box_primitive(rt, "java/lang/Double", V::D(d)))
                    }
                }
                Some(ConfigPrimitive::Text(s)) => {
                    let raw = rt.get_field(recv, "$default_ref");
                    if !raw.is_null() && rt.str_of(raw).is_none() {
                        Some(raw)
                    } else {
                        Some(rt.new_str(s))
                    }
                }
                None => Some(rt.get_field(recv, "$default_ref")),
            },
            "getAsBoolean" => match cfg.current(eid) {
                Some(ConfigPrimitive::Bool(b)) => Some(V::bool(*b)),
                _ => Some(V::FALSE),
            },
            "getAsInt" => match cfg.current(eid) {
                Some(ConfigPrimitive::Int(i)) => Some(V::I(*i)),
                _ => Some(V::I(0)),
            },
            "getAsLong" => match cfg.current(eid) {
                Some(ConfigPrimitive::Long(l)) => Some(V::J(*l)),
                _ => Some(V::J(0)),
            },
            "getAsDouble" => match cfg.current(eid) {
                Some(ConfigPrimitive::Double(d)) => Some(V::D(*d)),
                _ => Some(V::D(0.0)),
            },
            "getDefault" => Some(rt.get_field(recv, "$default_ref")),
            _ => None,
        };
    }

    if (owner == "org/apache/commons/lang3/tuple/Pair"
        || owner == "org/apache/commons/lang3/tuple/ImmutablePair")
        && !is_static
    {
        let recv = *args.first()?;
        return match name {
            "<init>" if args.len() >= 3 => {
                rt.put_field(recv, "left", args[1]);
                rt.put_field(recv, "right", args[2]);
                Some(V::Void)
            }
            "getLeft" | "getKey" => Some(rt.get_field(recv, "left")),
            "getRight" | "getValue" => Some(rt.get_field(recv, "right")),
            _ => None,
        };
    }
    if owner == "org/apache/commons/lang3/tuple/Pair" && is_static && name == "of" && args.len() == 2
    {
        let pair = rt.new_instance("org/apache/commons/lang3/tuple/ImmutablePair", 0);
        rt.put_field(pair, "left", args[0]);
        rt.put_field(pair, "right", args[1]);
        return Some(pair);
    }

    None
}

fn new_config_value_handle(rt: &mut Rt, cls: &str, entry_id: u64, default_ref: V) -> V {
    let obj = rt.new_instance(cls, 0);
    rt.put_field(obj, "$entry_id", V::J(entry_id as i64));
    rt.put_field(obj, "$default_ref", default_ref);
    rt.pin(obj);
    obj
}

fn config_key_text(rt: &Rt, key_v: V) -> Option<String> {
    if let Some(s) = rt.str_of(key_v) {
        return Some(s.to_owned());
    }
    if let Some(Data::List(items)) = rt.get_obj(key_v).map(|o| &o.data) {
        let parts: Vec<&str> = items.iter().filter_map(|&v| rt.str_of(v)).collect();
        if !parts.is_empty() {
            return Some(parts.join("."));
        }
    }
    None
}

fn new_literal_component(rt: &mut Rt, text: &str) -> V {
    let comp = rt.new_instance(MUTABLE_COMPONENT, 0);
    let kind = rt.intern_str("literal");
    let txt = rt.new_str(text);
    let siblings = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
    let style = new_style(rt);
    rt.put_field(comp, "$kind", kind);
    rt.put_field(comp, "$text", txt);
    rt.put_field(comp, "$siblings", siblings);
    rt.put_field(comp, "$style", style);
    comp
}

fn new_translatable_component(
    rt: &mut Rt,
    key: &str,
    fallback: Option<String>,
    args: Vec<V>,
) -> V {
    let comp = rt.new_instance(MUTABLE_COMPONENT, 0);
    let kind = rt.intern_str("translatable");
    let key_v = rt.new_str(key);
    let fb_v = match fallback {
        Some(fb) => rt.new_str(fb),
        None => V::NULL,
    };
    let args_v = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(args));
    let siblings = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
    let style = new_style(rt);
    rt.put_field(comp, "$kind", kind);
    rt.put_field(comp, "$key", key_v);
    rt.put_field(comp, "$fallback", fb_v);
    rt.put_field(comp, "$args", args_v);
    rt.put_field(comp, "$siblings", siblings);
    rt.put_field(comp, "$style", style);
    comp
}

fn new_style(rt: &mut Rt) -> V {
    let style = rt.new_instance(STYLE_CLASS, 0);
    rt.put_field(style, "color", V::I(-1));
    style
}

fn copy_style(rt: &mut Rt, style: V) -> V {
    let next = new_style(rt);
    if !style.is_null() {
        for key in ["color", "Bold", "Italic", "Underlined", "Strikethrough", "Obfuscated"] {
            let v = rt.get_field(style, key);
            if !v.is_null() {
                rt.put_field(next, key, v);
            }
        }
    }
    next
}

fn apply_formatting_to_style(rt: &mut Rt, style: V, fmt: V) -> V {
    let next = copy_style(rt, style);
    if fmt.is_null() {
        return next;
    }
    let name = rt
        .str_of(rt.get_field(fmt, "$name"))
        .unwrap_or("")
        .to_owned();
    let rgb = rt.get_field(fmt, "$rgb").i();
    match name.as_str() {
        "RESET" => return new_style(rt),
        "BOLD" => rt.put_field(next, "Bold", V::I(1)),
        "ITALIC" => rt.put_field(next, "Italic", V::I(1)),
        "UNDERLINE" => rt.put_field(next, "Underlined", V::I(1)),
        "STRIKETHROUGH" => rt.put_field(next, "Strikethrough", V::I(1)),
        "OBFUSCATED" => rt.put_field(next, "Obfuscated", V::I(1)),
        _ if rgb >= 0 => rt.put_field(next, "color", V::I(rgb)),
        _ => {}
    }
    next
}

fn append_sibling(rt: &mut Rt, comp: V, child: V) {
    let sibs = rt.get_field(comp, "$siblings");
    if let Some(Data::List(items)) = rt.get_obj_mut(sibs).map(|o| &mut o.data) {
        items.push(child);
    }
}

fn clone_component(rt: &mut Rt, src: V, include_siblings_and_style: bool) -> V {
    let kind = rt.str_of(rt.get_field(src, "$kind")).unwrap_or("literal").to_owned();
    let cloned = if kind == "translatable" {
        let key = rt.str_of(rt.get_field(src, "$key")).unwrap_or("").to_owned();
        let fb = rt.str_of(rt.get_field(src, "$fallback")).map(str::to_owned);
        let args = extract_varargs(rt, rt.get_field(src, "$args"));
        new_translatable_component(rt, &key, fb, args)
    } else {
        let txt = rt.str_of(rt.get_field(src, "$text")).unwrap_or("").to_owned();
        new_literal_component(rt, &txt)
    };
    if include_siblings_and_style {
        let style = copy_style(rt, rt.get_field(src, "$style"));
        rt.put_field(cloned, "$style", style);
        let sibs = match rt.get_obj(rt.get_field(src, "$siblings")).map(|o| &o.data) {
            Some(Data::List(xs)) => xs.clone(),
            _ => Vec::new(),
        };
        for s in sibs {
            let cs = clone_component(rt, s, true);
            append_sibling(rt, cloned, cs);
        }
    }
    cloned
}

fn component_equals(
    rt: &Rt,
    translations: &BTreeMap<String, String>,
    left: V,
    right: V,
) -> bool {
    if left == right {
        return true;
    }
    if rt.class_of(left) != Some(MUTABLE_COMPONENT) || rt.class_of(right) != Some(MUTABLE_COMPONENT)
    {
        return false;
    }
    render_component_text(rt, translations, left) == render_component_text(rt, translations, right)
}

/// Renders a `Component` tree into plain text following vanilla `Component#getString()`.
pub fn render_component_text(
    rt: &Rt,
    translations: &BTreeMap<String, String>,
    comp: V,
) -> String {
    let mut out = String::new();
    let kind = rt.str_of(rt.get_field(comp, "$kind")).unwrap_or("literal");
    if kind == "translatable" {
        let key = rt.str_of(rt.get_field(comp, "$key")).unwrap_or("");
        let fallback = rt.str_of(rt.get_field(comp, "$fallback"));
        let template = translations
            .get(key)
            .map(String::as_str)
            .or(fallback)
            .unwrap_or(key);
        let raw_args = extract_varargs(rt, rt.get_field(comp, "$args"));
        let rendered_args: Vec<String> = raw_args
            .into_iter()
            .map(|v| {
                if rt.class_of(v) == Some(MUTABLE_COMPONENT) {
                    render_component_text(rt, translations, v)
                } else {
                    rt.value_to_string(v)
                }
            })
            .collect();
        out.push_str(&format_translation_template(template, &rendered_args));
    } else if let Some(txt) = rt.str_of(rt.get_field(comp, "$text")) {
        out.push_str(txt);
    }

    if let Some(Data::List(sibs)) = rt.get_obj(rt.get_field(comp, "$siblings")).map(|o| &o.data) {
        for &sib in sibs {
            out.push_str(&render_component_text(rt, translations, sib));
        }
    }
    out
}

/// Formats `%s`, `%d`, `%%`, and positional `%1$s` placeholders per `TranslatableContents`.
pub fn format_translation_template(template: &str, args: &[String]) -> String {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len() + 16);
    let mut i = 0usize;
    let mut seq_idx = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'%' {
                out.push('%');
                i += 2;
                continue;
            }
            // Positional `%<digits>$<spec>`
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1
                && j + 1 < bytes.len()
                && bytes[j] == b'$'
                && matches!(bytes[j + 1], b's' | b'd' | b'f')
            {
                if let Ok(pos1) = template[i + 1..j].parse::<usize>() {
                    if let Some(val) = pos1.checked_sub(1).and_then(|idx| args.get(idx)) {
                        out.push_str(val);
                    }
                    i = j + 2;
                    continue;
                }
            }
            if matches!(bytes[i + 1], b's' | b'd' | b'f') {
                if let Some(val) = args.get(seq_idx) {
                    out.push_str(val);
                }
                seq_idx += 1;
                i += 2;
                continue;
            }
        }
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn chat_formatting_constant(rt: &mut Rt, name: &str) -> Option<V> {
    let table: &[(&str, i32, char, bool, i32)] = &[
        ("BLACK", 0, '0', false, 0x000000),
        ("DARK_BLUE", 1, '1', false, 0x0000AA),
        ("DARK_GREEN", 2, '2', false, 0x00AA00),
        ("DARK_AQUA", 3, '3', false, 0x00AAAA),
        ("DARK_RED", 4, '4', false, 0xAA0000),
        ("DARK_PURPLE", 5, '5', false, 0xAA00AA),
        ("GOLD", 6, '6', false, 0xFFAA00),
        ("GRAY", 7, '7', false, 0xAAAAAA),
        ("DARK_GRAY", 8, '8', false, 0x555555),
        ("BLUE", 9, '9', false, 0x5555FF),
        ("GREEN", 10, 'a', false, 0x55FF55),
        ("AQUA", 11, 'b', false, 0x55FFFF),
        ("RED", 12, 'c', false, 0xFF5555),
        ("LIGHT_PURPLE", 13, 'd', false, 0xFF55FF),
        ("YELLOW", 14, 'e', false, 0xFFFF55),
        ("WHITE", 15, 'f', false, 0xFFFFFF),
        ("OBFUSCATED", 16, 'k', true, -1),
        ("BOLD", 17, 'l', true, -1),
        ("STRIKETHROUGH", 18, 'm', true, -1),
        ("UNDERLINE", 19, 'n', true, -1),
        ("ITALIC", 20, 'o', true, -1),
        ("RESET", 21, 'r', false, -1),
    ];
    let &(ename, ord, code, is_fmt, rgb) = table.iter().find(|x| x.0 == name)?;
    let obj = rt.new_instance(CHAT_FORMATTING, 0);
    let n_str = rt.intern_str(ename);
    let lower = rt.intern_str(&ename.to_ascii_lowercase());
    let prefix = rt.intern_str(&format!("\u{00a7}{code}"));
    rt.put_field(obj, "$name", n_str);
    rt.put_field(obj, "$fmt_name", lower);
    rt.put_field(obj, "$ordinal", V::I(ord));
    rt.put_field(obj, "$code", V::I(code as i32));
    rt.put_field(obj, "$is_format", V::bool(is_fmt));
    rt.put_field(obj, "$rgb", V::I(rgb));
    rt.put_field(obj, "$prefix", prefix);
    rt.pin(obj);
    Some(obj)
}

fn extract_varargs(rt: &Rt, arr: V) -> Vec<V> {
    match rt.get_obj(arr).map(|o| &o.data) {
        Some(Data::Array(xs) | Data::List(xs)) => xs.clone(),
        _ => Vec::new(),
    }
}

fn resolve_class_resource_path(class_internal: &str, raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix('/') {
        return stripped.to_owned();
    }
    match class_internal.rsplit_once('/') {
        Some((pkg, _)) if !pkg.is_empty() => format!("{pkg}/{raw}"),
        _ => raw.to_owned(),
    }
}

fn path_string(rt: &Rt, value: V) -> Option<String> {
    if let Some((kind, _, tag, _)) = rt.handle_of(value) {
        if matches!(kind, "nio_path" | "io_file" | "fml_path") {
            return Some(tag.to_owned());
        }
    }
    rt.str_of(rt.get_field(value, "path"))
        .map(str::to_owned)
        .or_else(|| rt.str_of(value).map(str::to_owned))
}

fn normalize_lexical(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        std::path::PathBuf::from(".")
    } else {
        out
    }
}

/// Dispatches Brigadier command registration (`RegisterCommandsEvent`, `Commands`, `CommandDispatcher`,
/// `LiteralArgumentBuilder`, `RequiredArgumentBuilder`, `SuggestionProviders`, `CommandContext`, `CommandSourceStack`).
pub fn call_brigadier(
    rt: &mut Rt,
    commands: &mut BrigadierTree,
    translations: &BTreeMap<String, String>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();

    if owner == "net/neoforged/neoforge/event/RegisterCommandsEvent" && !is_static {
        return match name {
            "getDispatcher" => Some(rt.new_pinned_handle(
                "com/mojang/brigadier/CommandDispatcher",
                "cmd_dispatcher",
                0,
                "",
            )),
            "getCommandSelection" => Some(rt.new_pinned_handle(
                "net/minecraft/commands/Commands$CommandSelection",
                "cmd_selection",
                0,
                "ALL",
            )),
            "getBuildContext" => Some(rt.new_pinned_handle(
                "net/minecraft/commands/CommandBuildContext",
                "cmd_build_ctx",
                0,
                "",
            )),
            _ => None,
        };
    }

    if owner == "net/minecraft/commands/synchronization/SuggestionProviders"
        && is_static
        && name == "register"
        && args.len() == 2
    {
        rt.pin(args[1]);
        return Some(args[1]);
    }

    if (owner == "net/minecraft/commands/Commands"
        || owner == "com/mojang/brigadier/builder/LiteralArgumentBuilder")
        && is_static
        && name == "literal"
    {
        let lit = rt.str_of(*args.first()?)?.to_owned();
        let node_id = commands.alloc_node(CommandNodeKind::Literal(lit.clone()));
        return Some(rt.new_handle(
            "com/mojang/brigadier/builder/LiteralArgumentBuilder",
            "cmd_node",
            node_id,
            lit,
        ));
    }

    if (owner == "net/minecraft/commands/Commands"
        || owner == "com/mojang/brigadier/builder/RequiredArgumentBuilder")
        && is_static
        && name == "argument"
    {
        let arg_name = rt.str_of(*args.first()?)?.to_owned();
        let kind = match args.get(1).and_then(|&v| rt.handle_of(v)) {
            Some(("cmd_arg_type", _, "greedy", _)) => ArgumentKind::GreedyString,
            Some(("cmd_arg_type", _, "int", _)) => ArgumentKind::Integer,
            Some(("cmd_arg_type", _, "float", _)) => ArgumentKind::Float,
            Some(("cmd_arg_type", _, "bool", _)) => ArgumentKind::Bool,
            _ => ArgumentKind::Word,
        };
        let node_id = commands.alloc_node(CommandNodeKind::Argument {
            name: arg_name.clone(),
            kind,
        });
        return Some(rt.new_handle(
            "com/mojang/brigadier/builder/RequiredArgumentBuilder",
            "cmd_node",
            node_id,
            arg_name,
        ));
    }

    if owner == "com/mojang/brigadier/arguments/StringArgumentType" && is_static {
        return match name {
            "word" | "string" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "word")),
            "greedyString" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "greedy")),
            "getString" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                Some(rt.get_field(ctx_v, &format!("$arg:{key}")))
            }
            _ => None,
        };
    }
    if owner == "com/mojang/brigadier/arguments/IntegerArgumentType" && is_static {
        return match name {
            "integer" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "int")),
            "getInteger" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                Some(V::I(unbox_primitive(rt, rt.get_field(ctx_v, &format!("$arg:{key}"))).i()))
            }
            _ => None,
        };
    }
    if owner == "com/mojang/brigadier/arguments/BoolArgumentType" && is_static {
        return match name {
            "bool" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "bool")),
            "getBool" => {
                let ctx_v = *args.first()?;
                let key = rt.str_of(*args.get(1)?)?;
                Some(V::bool(unbox_primitive(rt, rt.get_field(ctx_v, &format!("$arg:{key}"))).i() != 0))
            }
            _ => None,
        };
    }
    if (owner == "com/mojang/brigadier/arguments/FloatArgumentType"
        || owner == "com/mojang/brigadier/arguments/DoubleArgumentType")
        && is_static
    {
        return match name {
            "floatArg" | "doubleArg" => Some(rt.new_handle(owner, "cmd_arg_type", 0, "float")),
            _ => None,
        };
    }
    if owner == "com/mojang/brigadier/StringReader" && is_static && name == "isAllowedInUnquotedString" {
        let ch = char::from_u32(args.first()?.i() as u32).unwrap_or('\0');
        let ok = ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '+');
        return Some(V::bool(ok));
    }

    if (owner == "com/mojang/brigadier/builder/ArgumentBuilder"
        || owner == "com/mojang/brigadier/builder/LiteralArgumentBuilder"
        || owner == "com/mojang/brigadier/builder/RequiredArgumentBuilder")
        && !is_static
    {
        let recv = *args.first()?;
        let ("cmd_node", node_id, tag, _) = rt.handle_of(recv)? else {
            return None;
        };
        let tag = tag.to_owned();
        return match name {
            "then" => {
                if let Some(("cmd_node", child_id, _, _)) = args.get(1).and_then(|&v| rt.handle_of(v)) {
                    commands.add_child(node_id, child_id);
                }
                Some(recv)
            }
            "executes" => {
                if let Some(&cmd) = args.get(1) {
                    commands.set_executes(rt, node_id, cmd);
                }
                Some(recv)
            }
            "requires" | "suggests" | "redirect" | "fork" => Some(recv),
            "build" => Some(rt.new_handle(
                "com/mojang/brigadier/tree/LiteralCommandNode",
                "cmd_node",
                node_id,
                tag,
            )),
            _ => None,
        };
    }

    if owner == "com/mojang/brigadier/CommandDispatcher" && !is_static && name == "register" {
        let builder = *args.get(1)?;
        let ("cmd_node", node_id, tag, _) = rt.handle_of(builder)? else {
            return None;
        };
        let tag = tag.to_owned();
        commands.register_root(node_id);
        return Some(rt.new_handle(
            "com/mojang/brigadier/tree/LiteralCommandNode",
            "cmd_node",
            node_id,
            tag,
        ));
    }

    if owner == "com/mojang/brigadier/context/CommandContext" && !is_static {
        let recv = *args.first()?;
        return match name {
            "getSource" => Some(rt.get_field(recv, "$source")),
            "getArgument" => {
                let key = rt.str_of(*args.get(1)?)?;
                Some(rt.get_field(recv, &format!("$arg:{key}")))
            }
            _ => None,
        };
    }

    if owner == "net/minecraft/commands/CommandSourceStack" && !is_static {
        let recv = *args.first()?;
        return match name {
            "isPlayer" => Some(V::bool(rt.get_field(recv, "$is_player").i() != 0)),
            "hasPermission" => Some(V::TRUE),
            "sendSystemMessage" | "sendFailure" => {
                let comp = *args.get(1)?;
                let text = render_component_text(rt, translations, comp);
                let v = rt.new_str(text);
                let msg_list = rt.get_field(recv, "$messages");
                if let Some(Data::List(msgs)) = rt.get_obj_mut(msg_list).map(|o| &mut o.data) {
                    msgs.push(v);
                }
                Some(V::Void)
            }
            _ => None,
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translatable_component_resolves_lang_table_positional_args_and_siblings() {
        let mut rt = Rt::new();
        let mut lang = BTreeMap::new();
        lang.insert(
            "error.example.platform".to_owned(),
            "Unsupported platform %1$s on %2$s (code %1$s)".to_owned(),
        );
        let arg1 = rt.intern_str("linux");
        let arg2 = new_literal_component(&mut rt, "x86_64");
        let args = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(vec![arg1, arg2]));
        let key = rt.intern_str("error.example.platform");

        let comp = call_chat(
            &mut rt,
            &lang,
            &MemberRef::new(
                "net/minecraft/network/chat/Component",
                "translatable",
                "(Ljava/lang/String;[Ljava/lang/Object;)Lnet/minecraft/network/chat/MutableComponent;",
            ),
            true,
            &[key, args],
        )
        .unwrap();

        let suffix = rt.intern_str("!");
        let _ = call_chat(
            &mut rt,
            &lang,
            &MemberRef::new(
                MUTABLE_COMPONENT,
                "append",
                "(Ljava/lang/String;)Lnet/minecraft/network/chat/MutableComponent;",
            ),
            false,
            &[comp, suffix],
        )
        .unwrap();

        let str_v = call_chat(
            &mut rt,
            &lang,
            &MemberRef::new(
                "net/minecraft/network/chat/Component",
                "getString",
                "()Ljava/lang/String;",
            ),
            false,
            &[comp],
        )
        .unwrap();
        assert_eq!(
            rt.str_of(str_v),
            Some("Unsupported platform linux on x86_64 (code linux)!")
        );
    }

    #[test]
    fn mod_config_spec_builder_and_brigadier_roundtrip_real_values() {
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();

        // Build a ModConfigSpec via HostApi calls and read back a clamped value
        let builder = rt.new_instance("net/neoforged/neoforge/common/ModConfigSpec$Builder", 0);
        let sec = rt.intern_str("general");
        let key = rt.intern_str("volume");
        let linked = &prepared.linked;
        let (double_val, spec) = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            let _ = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKESPECIAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "<init>",
                    "()V",
                ),
                &[builder],
            );
            let _ = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "push",
                    "(Ljava/lang/String;)Lnet/neoforged/neoforge/common/ModConfigSpec$Builder;",
                ),
                &[builder, sec],
            );
            let dv = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "defineInRange",
                    "(Ljava/lang/String;DDD)Lnet/neoforged/neoforge/common/ModConfigSpec$DoubleValue;",
                ),
                &[builder, key, V::D(75.0), V::D(0.0), V::D(100.0)],
            );
            let sp = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$Builder",
                    "build",
                    "()Lnet/neoforged/neoforge/common/ModConfigSpec;",
                ),
                &[builder],
            );
            let got = mmga_runtime::invoke_member(
                &mut ctx,
                mmga_classfile::op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/neoforged/neoforge/common/ModConfigSpec$DoubleValue",
                    "getAsDouble",
                    "()D",
                ),
                &[dv],
            );
            assert_eq!(got.d(), 75.0);
            (dv, sp)
        };
        let _ = double_val;
        assert!(rt.handle_of(spec).is_some_and(|(k, ..)| k == "config_spec"));
        assert!(rt.take_exception().is_none());
    }
}

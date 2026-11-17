//! Logical class loading over the actual linked class space. No arbitrary
//! external classes or resources are invented for unresolved loader requests.
use crate::{Data, ExecCtx, Rt, V};
use mmga_classfile::{op, MemberRef};

const CLASS: &str = "java/lang/Class";
const LOADER: &str = "java/lang/ClassLoader";
const SERVICE: &str = "java/util/ServiceLoader";
const APPLICATION: &str = "mmga/internal/ApplicationClassLoader";
const PLATFORM: &str = "mmga/internal/PlatformClassLoader";
const SERVICE_ITER: &str = "mmga/internal/ServiceIterator";
const SERVICE_PROVIDER: &str = "mmga/internal/ServiceProvider";
const SERVICE_STREAM: &str = "mmga/internal/ServiceStream";

#[derive(Clone, Copy, PartialEq)]
enum LoaderKind {
    Bootstrap,
    Platform,
    Application(u64),
}

fn application(rt: &mut Rt, linked: &crate::LinkedSpace) -> V {
    let key = format!("$application:{}", linked.class_space_id());
    if let Some(loader) = rt.get_static(LOADER, &key) {
        return loader;
    }
    let loader = rt.new_pinned_handle(
        APPLICATION,
        "class_loader",
        linked.class_space_id(),
        "application",
    );
    rt.put_static(LOADER, &key, loader);
    loader
}
fn platform(rt: &mut Rt) -> V {
    if let Some(loader) = rt.get_static(LOADER, "$platform") {
        return loader;
    }
    let loader = rt.new_pinned_handle(PLATFORM, "class_loader", 0, "platform");
    rt.put_static(LOADER, "$platform", loader);
    loader
}
fn loader_kind(ctx: &mut ExecCtx<'_>, loader: V) -> Option<LoaderKind> {
    if loader.is_null() {
        return Some(LoaderKind::Bootstrap);
    }
    let Some(("class_loader", id, tag, _)) = ctx.rt.handle_of(loader) else {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ClassLoader metadata is unavailable",
        );
        return None;
    };
    match tag {
        "application" if id == ctx.linked.class_space_id() => Some(LoaderKind::Application(id)),
        "platform" => Some(LoaderKind::Platform),
        _ => {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Requested ClassLoader belongs to another or unknown class space",
            );
            None
        }
    }
}
fn primitive(name: &str) -> bool {
    matches!(
        name,
        "void" | "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double"
    )
}
fn known_bootstrap(name: &str) -> bool {
    if crate::linked::known_native_jdk_class(name) {
        return true;
    }
    matches!(
        name,
        "java/lang/Object"
            | "java/lang/Class"
            | "java/lang/ClassLoader"
            | "java/lang/String"
            | "java/lang/StringBuilder"
            | "java/lang/StringBuffer"
            | "java/lang/Integer"
            | "java/lang/Long"
            | "java/lang/Float"
            | "java/lang/Double"
            | "java/lang/Byte"
            | "java/lang/Short"
            | "java/lang/Boolean"
            | "java/lang/Character"
            | "java/lang/Number"
            | "java/lang/Enum"
            | "java/lang/Record"
            | "java/lang/System"
            | "java/lang/Math"
            | "java/lang/Thread"
            | "java/lang/ThreadGroup"
            | "java/lang/ThreadLocal"
            | "java/lang/Throwable"
            | "java/lang/Error"
            | "java/lang/Exception"
            | "java/lang/RuntimeException"
            | "java/lang/Runnable"
            | "java/lang/Iterable"
            | "java/lang/Comparable"
            | "java/lang/CharSequence"
            | "java/lang/Cloneable"
            | "java/util/ServiceLoader"
            | "java/util/List"
            | "java/util/Set"
            | "java/util/Map"
            | "java/util/Collection"
            | "java/util/Iterator"
            | "java/util/ArrayList"
            | "java/util/HashMap"
            | "java/util/HashSet"
            | "java/util/TreeMap"
            | "java/util/Optional"
            | "java/util/UUID"
            | "java/util/function/Supplier"
            | "java/util/function/Consumer"
            | "java/util/function/Function"
            | "java/util/function/Predicate"
            | "java/util/function/BiFunction"
            | "java/util/stream/Stream"
            | "java/io/Serializable"
            | "java/io/InputStream"
            | "java/io/ByteArrayInputStream"
            | "java/io/IOException"
            | "java/net/URL"
    )
}
fn class_origin(linked: &crate::LinkedSpace, name: &str) -> Option<LoaderKind> {
    if primitive(name) || known_bootstrap(name) {
        return Some(LoaderKind::Bootstrap);
    }
    if name.starts_with('[') {
        let component = name.trim_start_matches('[');
        if matches!(component, "Z" | "B" | "C" | "S" | "I" | "J" | "F" | "D") {
            return Some(LoaderKind::Bootstrap);
        }
        if let Some(component) = component
            .strip_prefix('L')
            .and_then(|v| v.strip_suffix(';'))
        {
            return class_origin(linked, component);
        }
        return None;
    }
    linked
        .has_class(name)
        .then_some(LoaderKind::Application(linked.class_space_id()))
}
fn class_name(rt: &Rt, class: V) -> Option<String> {
    let ("class", _, name, _) = rt.handle_of(class)? else {
        return None;
    };
    Some(name.into())
}
fn resolve_class(
    ctx: &mut ExecCtx<'_>,
    name: &str,
    loader: V,
    initialize: bool,
    arrays: bool,
) -> V {
    let Some(kind) = loader_kind(ctx, loader) else {
        return V::NULL;
    };
    if primitive(name) || name.contains('/') || (!arrays && name.starts_with('[')) {
        return ctx
            .rt
            .throw_exception("java/lang/ClassNotFoundException", name);
    }
    let internal = name.replace('.', "/");
    let Some(origin) = class_origin(ctx.linked, &internal) else {
        return ctx
            .rt
            .throw_exception("java/lang/ClassNotFoundException", name);
    };
    if matches!(kind, LoaderKind::Bootstrap | LoaderKind::Platform)
        && matches!(origin, LoaderKind::Application(_))
    {
        return ctx
            .rt
            .throw_exception("java/lang/ClassNotFoundException", name);
    }
    if initialize && matches!(origin, LoaderKind::Application(_)) && !internal.starts_with('[') {
        crate::ensure_class_init(ctx, &internal);
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
    }
    ctx.rt.new_pinned_handle(CLASS, "class", 0, internal)
}
fn resource(ctx: &mut ExecCtx<'_>, loader: V, path: &str, url: bool) -> V {
    let Some(kind) = loader_kind(ctx, loader) else {
        return V::NULL;
    };
    if !matches!(kind, LoaderKind::Application(_))
        || path.starts_with("java/")
        || path.starts_with("jdk/")
        || path.starts_with("sun/")
    {
        return ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Bootstrap/platform resource catalogue is not implemented",
        );
    }
    match ctx.host.classpath_resource(path) {
        Ok(Some(bytes)) => {
            if url {
                ctx.rt.new_handle("java/net/URL", "jar_url", 0, path)
            } else {
                ctx.rt.alloc(
                    "java/io/ByteArrayInputStream",
                    0,
                    Data::ByteBuf {
                        bytes,
                        reader_idx: 0,
                    },
                )
            }
        }
        Ok(None) => V::NULL,
        Err(message) => ctx
            .rt
            .throw_exception("java/lang/UnsupportedOperationException", &message),
    }
}

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if owner == CLASS && !is_static && name == "isEnum" && desc == "()Z" {
        let class = class_name(ctx.rt, *args.first()?)?;
        let is_enum = if let Some(cf) = ctx.linked.bytecode_classes.get(&class) {
            cf.access & mmga_classfile::access::ENUM != 0
                && cf.superclass.as_deref() == Some("java/lang/Enum")
        } else if ctx.linked.has_native_class(&class) {
            ctx.linked.superclass_of(&class) == Some("java/lang/Enum")
        } else if known_bootstrap(&class) || primitive(&class) || class.starts_with('[') {
            false
        } else {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Enum class metadata is unavailable for {class}"),
            ));
        };
        return Some(V::bool(is_enum));
    }
    if owner == CLASS
        && !is_static
        && name == "getClassLoader"
        && desc == "()Ljava/lang/ClassLoader;"
    {
        let class = class_name(ctx.rt, *args.first()?)?;
        return Some(match class_origin(ctx.linked, &class) {
            Some(LoaderKind::Bootstrap) => V::NULL,
            Some(LoaderKind::Application(_)) => application(ctx.rt, ctx.linked),
            _ => ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Class loader metadata is unavailable for {class}"),
            ),
        });
    }
    if owner == CLASS
        && is_static
        && name == "forName"
        && matches!(
            desc,
            "(Ljava/lang/String;)Ljava/lang/Class;"
                | "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;"
        )
    {
        let raw = *args.first()?;
        if raw.is_null() {
            return Some(
                ctx.rt
                    .throw_exception("java/lang/NullPointerException", "Class.forName(null)"),
            );
        }
        let name = ctx.rt.str_of(raw)?.to_owned();
        let loader = if args.len() == 1 {
            application(ctx.rt, ctx.linked)
        } else {
            args[2]
        };
        let initialize = args.len() == 1 || args[1] == V::TRUE;
        return Some(resolve_class(ctx, &name, loader, initialize, true));
    }
    if matches!(owner, LOADER | APPLICATION | PLATFORM) {
        if is_static {
            return match (name, desc) {
                ("getSystemClassLoader", "()Ljava/lang/ClassLoader;") => {
                    Some(application(ctx.rt, ctx.linked))
                }
                ("getPlatformClassLoader", "()Ljava/lang/ClassLoader;") => Some(platform(ctx.rt)),
                _ => None,
            };
        }
        let loader = *args.first()?;
        if name == "getParent" && desc == "()Ljava/lang/ClassLoader;" {
            return Some(match loader_kind(ctx, loader)? {
                LoaderKind::Application(_) => platform(ctx.rt),
                _ => V::NULL,
            });
        }
        if name == "loadClass"
            && matches!(
                desc,
                "(Ljava/lang/String;)Ljava/lang/Class;" | "(Ljava/lang/String;Z)Ljava/lang/Class;"
            )
        {
            if args.get(1)?.is_null() {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "ClassLoader.loadClass(null)",
                ));
            }
            let name = ctx.rt.str_of(*args.get(1)?)?.to_owned();
            return Some(resolve_class(ctx, &name, loader, false, false));
        }
        if matches!(name, "getResource" | "getResourceAsStream")
            && matches!(
                desc,
                "(Ljava/lang/String;)Ljava/net/URL;" | "(Ljava/lang/String;)Ljava/io/InputStream;"
            )
        {
            if args.get(1)?.is_null() {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "ClassLoader resource name",
                ));
            }
            let path = ctx.rt.str_of(*args.get(1)?)?.to_owned();
            return Some(resource(ctx, loader, &path, name == "getResource"));
        }
    }
    if owner == "java/lang/Thread" && !is_static {
        if name == "getContextClassLoader" && desc == "()Ljava/lang/ClassLoader;" {
            return Some(
                if ctx.rt.get_field(args[0], "$contextLoaderSet") == V::TRUE {
                    ctx.rt.get_field(args[0], "$contextLoader")
                } else {
                    application(ctx.rt, ctx.linked)
                },
            );
        }
        if name == "setContextClassLoader" && desc == "(Ljava/lang/ClassLoader;)V" {
            let loader = *args.get(1)?;
            if loader_kind(ctx, loader).is_none() {
                return Some(V::NULL);
            }
            ctx.rt.put_field(args[0], "$contextLoader", loader);
            ctx.rt.put_field(args[0], "$contextLoaderSet", V::TRUE);
            return Some(V::Void);
        }
    }
    service_call(ctx, member, is_static, args)
}

fn list_values(rt: &Rt, list: V) -> Option<Vec<V>> {
    match rt.get_obj(list).map(|o| &o.data) {
        Some(Data::List(v) | Data::Array(v)) => Some(v.clone()),
        _ => None,
    }
}
fn empty_list(rt: &mut Rt) -> V {
    rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()))
}
fn new_iterator(rt: &mut Rt, service: V, metadata: bool) -> V {
    let iterator = rt.new_instance(SERVICE_ITER, 0);
    rt.put_field(iterator, "$service", service);
    rt.put_field(iterator, "$metadata", V::bool(metadata));
    rt.put_field(iterator, "$index", V::I(0));
    rt.put_field(iterator, "$epoch", rt.get_field(service, "$epoch"));
    iterator
}
fn service_error(ctx: &mut ExecCtx<'_>, class: &str, message: &str, cause: Option<V>) -> V {
    let summary = cause.map(|cause| ctx.rt.exception_summary(cause));
    ctx.rt.throw_exception(
        "java/util/ServiceConfigurationError",
        &format!(
            "Provider {class}: {message}{}",
            summary.map_or(String::new(), |summary| format!(": {summary}"))
        ),
    );
    let error = ctx.rt.take_exception().unwrap();
    if let Some(cause) = cause {
        ctx.rt.put_field(error, "cause", cause);
    }
    error
}
fn provider_metadata(ctx: &mut ExecCtx<'_>, service: V, name: &str) -> Result<V, V> {
    if !name.is_ascii() {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Unicode provider-name validation is not implemented",
        );
        return Err(ctx.rt.take_exception().unwrap());
    }
    if name.split('/').any(|part| {
        part.is_empty()
            || !part
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
            || !part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    }) {
        return Err(service_error(
            ctx,
            name,
            "Illegal provider-class name",
            None,
        ));
    }
    let spi = class_name(ctx.rt, ctx.rt.get_field(service, "$service_type")).unwrap();
    let Some(cf) = ctx.linked.bytecode_classes.get(name) else {
        if ctx.linked.has_class(name) {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Original provider constructor/access metadata is unavailable for {name}"),
            );
            return Err(ctx.rt.take_exception().unwrap());
        }
        return Err(service_error(ctx, name, "not found", None));
    };
    if cf.access & mmga_classfile::access::PUBLIC == 0 {
        return Err(service_error(ctx, name, "is not public", None));
    }
    if !ctx.linked.is_subtype(name, &spi, ctx.host) {
        return Err(service_error(
            ctx,
            name,
            "is not a subtype of its service",
            None,
        ));
    }
    if !cf.methods.iter().any(|m| {
        m.name == "<init>" && m.desc == "()V" && m.access & mmga_classfile::access::PUBLIC != 0
    }) {
        return Err(service_error(
            ctx,
            name,
            "Unable to get public no-arg constructor",
            None,
        ));
    }
    let provider = ctx.rt.new_instance(SERVICE_PROVIDER, 0);
    let class = ctx.rt.new_pinned_handle(CLASS, "class", 0, name);
    let loader = ctx.rt.get_field(service, "$class_loader");
    rt_provider_fields(ctx.rt, provider, class, loader);
    Ok(provider)
}
fn rt_provider_fields(rt: &mut Rt, provider: V, class: V, loader: V) {
    rt.put_field(provider, "$class", class);
    rt.put_field(provider, "$class_loader", loader);
}
fn lookup(ctx: &mut ExecCtx<'_>, service: V, metadata: bool) -> Option<V> {
    let field = if metadata {
        "$stream_lookup"
    } else {
        "$instance_lookup"
    };
    let old = ctx.rt.get_field(service, field);
    if !old.is_null() {
        return Some(old);
    }
    let loader = ctx.rt.get_field(service, "$class_loader");
    let kind = loader_kind(ctx, loader)?;
    let spi = class_name(ctx.rt, ctx.rt.get_field(service, "$service_type"))?;
    let names = match kind {
        LoaderKind::Application(_) => ctx.linked.services.get(&spi).cloned().unwrap_or_default(),
        LoaderKind::Platform
            if matches!(
                class_origin(ctx.linked, &spi),
                Some(LoaderKind::Application(_))
            ) =>
        {
            Vec::new()
        }
        _ => {
            ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Installed bootstrap/platform service-provider catalogue is unavailable",
            );
            return None;
        }
    };
    let values: Vec<V> = names.into_iter().map(|name| ctx.rt.new_str(name)).collect();
    let values = ctx.rt.alloc("[Ljava/lang/String;", 0, Data::Array(values));
    let lookup = ctx.rt.new_instance("mmga/internal/ServiceLookup", 0);
    ctx.rt.put_field(lookup, "$names", values);
    ctx.rt.put_field(lookup, "$cursor", V::I(0));
    ctx.rt.put_field(service, field, lookup);
    Some(lookup)
}
fn iterator_ready(ctx: &mut ExecCtx<'_>, iterator: V) -> Option<bool> {
    let service = ctx.rt.get_field(iterator, "$service");
    if ctx.rt.get_field(iterator, "$epoch") != ctx.rt.get_field(service, "$epoch") {
        ctx.rt.throw_exception(
            "java/util/ConcurrentModificationException",
            "ServiceLoader was reloaded",
        );
        return None;
    }
    let metadata = ctx.rt.get_field(iterator, "$metadata") == V::TRUE;
    let cache_field = if metadata {
        "$metadata_cache"
    } else {
        "$instances"
    };
    let Some(cached) = list_values(ctx.rt, ctx.rt.get_field(service, cache_field)) else {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ServiceLoader instance/cache metadata is unavailable",
        );
        return None;
    };
    if (ctx.rt.get_field(iterator, "$index").i() as usize) < cached.len() {
        return Some(true);
    }
    let lookup = lookup(ctx, service, metadata)?;
    if !ctx.rt.get_field(lookup, "$next").is_null() || !ctx.rt.get_field(lookup, "$error").is_null()
    {
        return Some(true);
    }
    let Some(names) = list_values(ctx.rt, ctx.rt.get_field(lookup, "$names")) else {
        ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ServiceLoader lookup metadata is unavailable",
        );
        return None;
    };
    let cursor = ctx.rt.get_field(lookup, "$cursor").i() as usize;
    let Some(name) = names.get(cursor) else {
        return Some(false);
    };
    let name = ctx.rt.str_of(*name)?.to_owned();
    ctx.rt
        .put_field(lookup, "$cursor", V::I((cursor + 1) as i32));
    match provider_metadata(ctx, service, &name) {
        Ok(provider) => ctx.rt.put_field(lookup, "$next", provider),
        Err(error) => {
            if ctx.rt.class_of(error) != Some("java/util/ServiceConfigurationError") {
                ctx.rt.pending_exception = Some(error.r());
                return None;
            }
            ctx.rt.put_field(lookup, "$error", error);
        }
    }
    // Java 21 lookup hasNext caches ServiceConfigurationError and returns true;
    // the iterator's next() is the operation that raises the cached error.
    Some(true)
}
fn provider_get(ctx: &mut ExecCtx<'_>, provider: V) -> V {
    if loader_kind(ctx, ctx.rt.get_field(provider, "$class_loader")).is_none() {
        return V::NULL;
    }
    let Some(name) = class_name(ctx.rt, ctx.rt.get_field(provider, "$class")) else {
        return ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Provider class metadata is unavailable",
        );
    };
    if ctx.linked.bytecode_classes.get(&name).is_some_and(|cf| {
        cf.access & (mmga_classfile::access::ABSTRACT | mmga_classfile::access::INTERFACE) != 0
    }) {
        ctx.rt
            .throw_exception("java/lang/InstantiationException", &name);
        let cause = ctx.rt.take_exception().unwrap();
        let error = service_error(ctx, &name, "could not be instantiated", Some(cause));
        ctx.rt.pending_exception = Some(error.r());
        return V::NULL;
    }
    let instance = crate::alloc_new_object(ctx, &name);
    if ctx.rt.pending_exception.is_none() {
        crate::invoke_member(
            ctx,
            op::INVOKESPECIAL,
            &MemberRef::new(&name, "<init>", "()V"),
            &[instance],
        );
    }
    if let Some(cause) = ctx.rt.take_exception() {
        let error = service_error(ctx, &name, "could not be instantiated", Some(cause));
        ctx.rt.pending_exception = Some(error.r());
        return V::NULL;
    }
    instance
}
fn iterator_next(ctx: &mut ExecCtx<'_>, iterator: V) -> V {
    match iterator_ready(ctx, iterator) {
        Some(true) => {}
        Some(false) => {
            return ctx.rt.throw_exception(
                "java/util/NoSuchElementException",
                "ServiceLoader iterator exhausted",
            )
        }
        None => return V::NULL,
    }
    let service = ctx.rt.get_field(iterator, "$service");
    let metadata = ctx.rt.get_field(iterator, "$metadata") == V::TRUE;
    let cache = ctx.rt.get_field(
        service,
        if metadata {
            "$metadata_cache"
        } else {
            "$instances"
        },
    );
    let index = ctx.rt.get_field(iterator, "$index").i() as usize;
    let Some(cached) = list_values(ctx.rt, cache) else {
        return ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ServiceLoader cache metadata is unavailable",
        );
    };
    if let Some(value) = cached.get(index) {
        ctx.rt
            .put_field(iterator, "$index", V::I((index + 1) as i32));
        return *value;
    }
    let Some(lookup) = lookup(ctx, service, metadata) else {
        return V::NULL;
    };
    let error = ctx.rt.get_field(lookup, "$error");
    if !error.is_null() {
        ctx.rt.put_field(lookup, "$error", V::NULL);
        ctx.rt.pending_exception = Some(error.r());
        return V::NULL;
    }
    let provider = ctx.rt.get_field(lookup, "$next");
    ctx.rt.put_field(lookup, "$next", V::NULL);
    let value = if metadata {
        provider
    } else {
        provider_get(ctx, provider)
    };
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    if let Some(Data::List(values)) = ctx.rt.get_obj_mut(cache).map(|o| &mut o.data) {
        values.push(value);
    }
    ctx.rt
        .put_field(iterator, "$index", V::I((index + 1) as i32));
    value
}
fn new_service_stream(rt: &mut Rt, service: V, steps: Vec<V>) -> V {
    let stream = rt.new_instance(SERVICE_STREAM, 0);
    let steps = rt.alloc("java/util/ArrayList", 0, Data::List(steps));
    rt.put_field(stream, "$service", service);
    rt.put_field(stream, "$steps", steps);
    rt.put_field(stream, "$used", V::FALSE);
    rt.put_field(stream, "$epoch", rt.get_field(service, "$epoch"));
    stream
}
fn service_stream_call(ctx: &mut ExecCtx<'_>, member: &MemberRef, args: &[V]) -> Option<V> {
    let stream = *args.first()?;
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    let service = ctx.rt.get_field(stream, "$service");
    let mut steps = list_values(ctx.rt, ctx.rt.get_field(stream, "$steps"))?;
    if name == "close" && desc == "()V" {
        ctx.rt.put_field(stream, "$used", V::TRUE);
        return Some(V::Void);
    }
    if ctx.rt.get_field(stream, "$used") == V::TRUE {
        return Some(ctx.rt.throw_exception(
            "java/lang/IllegalStateException",
            "stream has already been operated upon or closed",
        ));
    }
    if matches!(
        (name, desc),
        (
            "map",
            "(Ljava/util/function/Function;)Ljava/util/stream/Stream;"
        ) | (
            "filter",
            "(Ljava/util/function/Predicate;)Ljava/util/stream/Stream;"
        ) | ("limit", "(J)Ljava/util/stream/Stream;")
    ) {
        let value = *args.get(1)?;
        if (name != "limit" && value.is_null()) || (name == "limit" && value.j() < 0) {
            return Some(ctx.rt.throw_exception(
                if name == "limit" {
                    "java/lang/IllegalArgumentException"
                } else {
                    "java/lang/NullPointerException"
                },
                "invalid stream operation argument",
            ));
        }
        let step = ctx.rt.new_instance("mmga/internal/ServiceStreamStep", 0);
        let kind = ctx.rt.new_str(name);
        ctx.rt.put_field(step, "$kind", kind);
        ctx.rt.put_field(step, "$value", value);
        steps.push(step);
        ctx.rt.put_field(stream, "$used", V::TRUE);
        let next = new_service_stream(ctx.rt, service, steps);
        ctx.rt
            .put_field(next, "$epoch", ctx.rt.get_field(stream, "$epoch"));
        return Some(next);
    }
    if !matches!(
        (name, desc),
        ("findFirst", "()Ljava/util/Optional;")
            | ("findAny", "()Ljava/util/Optional;")
            | ("toList", "()Ljava/util/List;")
            | ("count", "()J")
            | ("forEach", "(Ljava/util/function/Consumer;)V")
            | (
                "collect",
                "(Ljava/util/stream/Collector;)Ljava/lang/Object;"
            )
    ) {
        return None;
    }
    if name == "collect"
        && !ctx
            .rt
            .handle_of(*args.get(1)?)
            .is_some_and(|(kind, id, ..)| kind == "collector" && id == 1)
    {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "ServiceLoader stream collector is not implemented",
        ));
    }
    if name == "forEach" && args.get(1)?.is_null() {
        return Some(
            ctx.rt
                .throw_exception("java/lang/NullPointerException", "stream consumer"),
        );
    }
    ctx.rt.put_field(stream, "$used", V::TRUE);
    let iterator = new_iterator(ctx.rt, service, true);
    ctx.rt
        .put_field(iterator, "$epoch", ctx.rt.get_field(stream, "$epoch"));
    let mut limits = vec![0_i64; steps.len()];
    let mut out = Vec::new();
    loop {
        if steps.iter().enumerate().any(|(i, step)| {
            ctx.rt.str_of(ctx.rt.get_field(*step, "$kind")) == Some("limit")
                && limits[i] >= ctx.rt.get_field(*step, "$value").j()
        }) {
            break;
        }
        match iterator_ready(ctx, iterator) {
            Some(true) => {}
            Some(false) => break,
            None => return Some(V::NULL),
        }
        let mut value = iterator_next(ctx, iterator);
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        let mut keep = true;
        for (i, step) in steps.iter().enumerate() {
            let operation = ctx.rt.str_of(ctx.rt.get_field(*step, "$kind"))?.to_owned();
            let callback = ctx.rt.get_field(*step, "$value");
            match operation.as_str() {
                "filter" => {
                    let allowed = crate::invoke_member(
                        ctx,
                        op::INVOKEINTERFACE,
                        &MemberRef::new(
                            "java/util/function/Predicate",
                            "test",
                            "(Ljava/lang/Object;)Z",
                        ),
                        &[callback, value],
                    );
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                    if allowed == V::FALSE {
                        keep = false;
                        break;
                    }
                }
                "map" => {
                    value = crate::invoke_member(
                        ctx,
                        op::INVOKEINTERFACE,
                        &MemberRef::new(
                            "java/util/function/Function",
                            "apply",
                            "(Ljava/lang/Object;)Ljava/lang/Object;",
                        ),
                        &[callback, value],
                    );
                    if ctx.rt.pending_exception.is_some() {
                        return Some(V::NULL);
                    }
                }
                "limit" => limits[i] += 1,
                _ => {
                    return Some(ctx.rt.throw_exception(
                        "java/lang/UnsupportedOperationException",
                        "Unknown service-stream operation",
                    ))
                }
            }
        }
        if !keep {
            continue;
        }
        if matches!(name, "findFirst" | "findAny") {
            if value.is_null() {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "stream findFirst encountered null",
                ));
            }
            return Some(
                ctx.rt
                    .alloc("java/util/Optional", 0, Data::Optional(Some(value))),
            );
        }
        if name == "forEach" {
            crate::invoke_member(
                ctx,
                op::INVOKEINTERFACE,
                &MemberRef::new(
                    "java/util/function/Consumer",
                    "accept",
                    "(Ljava/lang/Object;)V",
                ),
                &[*args.get(1)?, value],
            );
            if ctx.rt.pending_exception.is_some() {
                return Some(V::NULL);
            }
        } else {
            out.push(value);
        }
    }
    Some(match name {
        "findFirst" | "findAny" => ctx.rt.alloc("java/util/Optional", 0, Data::Optional(None)),
        "count" => V::J(out.len() as i64),
        "forEach" => V::Void,
        "toList" => ctx
            .rt
            .alloc("java/util/ImmutableCollections$ListN", 0, Data::List(out)),
        _ => ctx.rt.alloc("java/util/ArrayList", 0, Data::List(out)),
    })
}
fn service_call(
    ctx: &mut ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let owner = member.owner.as_str();
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    if owner == SERVICE && is_static && name == "load" {
        if !matches!(
            desc,
            "(Ljava/lang/Class;)Ljava/util/ServiceLoader;"
                | "(Ljava/lang/Class;Ljava/lang/ClassLoader;)Ljava/util/ServiceLoader;"
        ) {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Module-layer ServiceLoader is not implemented",
            ));
        }
        let spi = *args.first()?;
        if spi.is_null() {
            return Some(
                ctx.rt
                    .throw_exception("java/lang/NullPointerException", "service"),
            );
        }
        let spi_name = class_name(ctx.rt, spi)?;
        if class_origin(ctx.linked, &spi_name).is_none() {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "Service type class metadata is unavailable",
            ));
        }
        let mut loader = if args.len() == 2 {
            args[1]
        } else {
            let thread = crate::invoke_member(
                ctx,
                op::INVOKESTATIC,
                &MemberRef::new("java/lang/Thread", "currentThread", "()Ljava/lang/Thread;"),
                &[],
            );
            crate::invoke_member(
                ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "java/lang/Thread",
                    "getContextClassLoader",
                    "()Ljava/lang/ClassLoader;",
                ),
                &[thread],
            )
        };
        if ctx.rt.pending_exception.is_some() {
            return Some(V::NULL);
        }
        if loader.is_null() {
            loader = application(ctx.rt, ctx.linked);
        }
        if loader_kind(ctx, loader).is_none() {
            return Some(V::NULL);
        }
        let service = ctx.rt.new_instance(SERVICE, 0);
        ctx.rt.put_field(service, "$service_type", spi);
        ctx.rt.put_field(service, "$class_loader", loader);
        ctx.rt.put_field(service, "$epoch", V::I(0));
        let instances = empty_list(ctx.rt);
        let metadata = empty_list(ctx.rt);
        ctx.rt.put_field(service, "$instances", instances);
        ctx.rt.put_field(service, "$metadata_cache", metadata);
        return Some(service);
    }
    if is_static {
        return None;
    }
    let receiver = *args.first()?;
    let class = ctx.rt.class_of(receiver)?.to_owned();
    if class == SERVICE_ITER {
        return match (name, desc) {
            ("hasNext", "()Z") => Some(iterator_ready(ctx, receiver).map_or(V::NULL, V::bool)),
            ("next", "()Ljava/lang/Object;") => Some(iterator_next(ctx, receiver)),
            ("remove", "()V") => Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                "ServiceLoader iterator.remove",
            )),
            _ => None,
        };
    }
    if class == SERVICE_PROVIDER {
        return match (name, desc) {
            ("get", "()Ljava/lang/Object;") => Some(provider_get(ctx, receiver)),
            ("type", "()Ljava/lang/Class;") => Some(ctx.rt.get_field(receiver, "$class")),
            _ => None,
        };
    }
    if class == SERVICE_STREAM {
        return service_stream_call(ctx, member, args);
    }
    if owner == SERVICE {
        return match (name, desc) {
            ("iterator", "()Ljava/util/Iterator;") => Some(new_iterator(ctx.rt, receiver, false)),
            ("stream", "()Ljava/util/stream/Stream;") => {
                Some(new_service_stream(ctx.rt, receiver, Vec::new()))
            }
            ("findFirst", "()Ljava/util/Optional;") => {
                let iterator = new_iterator(ctx.rt, receiver, false);
                let first = match iterator_ready(ctx, iterator) {
                    Some(true) => Some(iterator_next(ctx, iterator)),
                    Some(false) => None,
                    None => return Some(V::NULL),
                };
                Some(if ctx.rt.pending_exception.is_some() {
                    V::NULL
                } else {
                    ctx.rt.alloc("java/util/Optional", 0, Data::Optional(first))
                })
            }
            ("reload", "()V") => {
                let epoch = ctx.rt.get_field(receiver, "$epoch").i().wrapping_add(1);
                ctx.rt.put_field(receiver, "$epoch", V::I(epoch));
                ctx.rt.put_field(receiver, "$stream_lookup", V::NULL);
                ctx.rt.put_field(receiver, "$instance_lookup", V::NULL);
                let instances = empty_list(ctx.rt);
                let metadata = empty_list(ctx.rt);
                ctx.rt.put_field(receiver, "$instances", instances);
                ctx.rt.put_field(receiver, "$metadata_cache", metadata);
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
    use crate::{
        invoke_member, HostApi, LinkedSpace, NativeClassDef, NativeModuleRegistry, NullHost,
    };
    fn linked() -> LinkedSpace {
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: "test/Mod",
                    superclass: Some("java/lang/Object"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
                NativeClassDef {
                    name: "test/OtherMod",
                    superclass: Some("java/lang/Object"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
            ],
            lambdas: &[],
        });
        linked
    }
    fn class(rt: &mut Rt, name: &str) -> V {
        rt.new_pinned_handle(CLASS, "class", 0, name)
    }
    fn invoke(
        ctx: &mut ExecCtx<'_>,
        owner: &str,
        name: &str,
        desc: &str,
        static_call: bool,
        args: &[V],
    ) -> V {
        invoke_member(
            ctx,
            if static_call {
                op::INVOKESTATIC
            } else {
                op::INVOKEVIRTUAL
            },
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    fn error(ctx: &mut ExecCtx<'_>, expected: &str) {
        let exception = ctx.rt.take_exception().unwrap();
        let summary = ctx.rt.exception_summary(exception);
        assert!(summary.contains(expected), "{summary}");
    }
    #[test]
    fn linked_mods_share_an_application_loader_and_known_bootstrap_types_have_no_loader() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let first = class(ctx.rt, "test/Mod");
        let second = class(ctx.rt, "test/OtherMod");
        let loader = invoke(
            &mut ctx,
            CLASS,
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            false,
            &[first],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert!(!loader.is_null());
        assert_eq!(
            invoke(
                &mut ctx,
                CLASS,
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                false,
                &[second]
            ),
            loader
        );
        assert_eq!(
            invoke(
                &mut ctx,
                LOADER,
                "getSystemClassLoader",
                "()Ljava/lang/ClassLoader;",
                true,
                &[]
            ),
            loader
        );
        for name in [
            "java/lang/Object",
            "java/lang/String",
            "java/util/EnumSet",
            "java/util/LinkedHashMap",
            "int",
            "[I",
            "[[Ljava/lang/String;",
        ] {
            let klass = class(ctx.rt, name);
            assert_eq!(
                invoke(
                    &mut ctx,
                    CLASS,
                    "getClassLoader",
                    "()Ljava/lang/ClassLoader;",
                    false,
                    &[klass]
                ),
                V::NULL
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
        let unknown = class(ctx.rt, "java/lang/UnmodeledClass");
        invoke(
            &mut ctx,
            CLASS,
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            false,
            &[unknown],
        );
        error(&mut ctx, "UnsupportedOperationException");
    }
    #[test]
    fn load_class_resolves_real_metadata_without_initializing_or_inventing_missing_classes() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let loader = invoke(
            &mut ctx,
            LOADER,
            "getSystemClassLoader",
            "()Ljava/lang/ClassLoader;",
            true,
            &[],
        );
        let name = ctx.rt.new_str("test.Mod");
        let actual = invoke(
            &mut ctx,
            LOADER,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            false,
            &[loader, name],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(actual, class(ctx.rt, "test/Mod"));
        let absent = ctx.rt.new_str("test.Missing");
        invoke(
            &mut ctx,
            LOADER,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            false,
            &[loader, absent],
        );
        error(&mut ctx, "ClassNotFoundException");
        let foreign = ctx.rt.new_pinned_handle(
            APPLICATION,
            "class_loader",
            linked.class_space_id() + 99,
            "application",
        );
        invoke(
            &mut ctx,
            LOADER,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            false,
            &[foreign, name],
        );
        error(&mut ctx, "UnsupportedOperationException");
    }
    struct ResourceHost;
    impl HostApi for ResourceHost {
        fn get_static_field(
            &mut self,
            _rt: &mut Rt,
            _owner: &str,
            _name: &str,
            _desc: &str,
        ) -> Option<V> {
            None
        }
        fn call_method(
            &mut self,
            _rt: &mut Rt,
            _linked: &LinkedSpace,
            _member: &MemberRef,
            _is_static: bool,
            _args: &[V],
        ) -> Option<V> {
            None
        }
        fn classpath_resource(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
            Ok((path == "assets/test/data.bin").then(|| vec![0, 128, 255]))
        }
    }
    #[test]
    fn class_loader_resources_return_actual_host_bytes_and_do_not_rewrite_absolute_names() {
        let mut rt = Rt::new();
        let linked = linked();
        let mut host = ResourceHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let loader = invoke(
            &mut ctx,
            LOADER,
            "getSystemClassLoader",
            "()Ljava/lang/ClassLoader;",
            true,
            &[],
        );
        let path = ctx.rt.new_str("assets/test/data.bin");
        let stream = invoke(
            &mut ctx,
            LOADER,
            "getResourceAsStream",
            "(Ljava/lang/String;)Ljava/io/InputStream;",
            false,
            &[loader, path],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let bytes = invoke(
            &mut ctx,
            "java/io/InputStream",
            "readAllBytes",
            "()[B",
            false,
            &[stream],
        );
        assert!(
            matches!(ctx.rt.get_obj(bytes).map(|o| &o.data), Some(Data::Array(values)) if values == &vec![V::I(0), V::I(-128), V::I(-1)])
        );
        let path = ctx.rt.new_str("/assets/test/data.bin");
        assert_eq!(
            invoke(
                &mut ctx,
                LOADER,
                "getResourceAsStream",
                "(Ljava/lang/String;)Ljava/io/InputStream;",
                false,
                &[loader, path]
            ),
            V::NULL
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn enum_class_flags_and_explicit_native_hierarchies_do_not_turn_constant_bodies_into_enums() {
        let mut rt = Rt::new();
        let mut linked = linked();
        let mut host = NullHost;
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: "test/Enum",
                    superclass: Some("java/lang/Enum"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
                NativeClassDef {
                    name: "test/Enum$1",
                    superclass: Some("test/Enum"),
                    interfaces: &[],
                    is_interface: false,
                    methods: &[],
                },
            ],
            lambdas: &[],
        });
        let template = service_linked().bytecode_classes["test/First"].clone();
        let mut classes = std::collections::BTreeMap::new();
        for (name, parent, flags) in [
            (
                "test/DeclaredEnum",
                "java/lang/Enum",
                mmga_classfile::access::ENUM,
            ),
            (
                "test/DeclaredEnum$1",
                "test/DeclaredEnum",
                mmga_classfile::access::ENUM,
            ),
            ("test/Unflagged", "java/lang/Enum", 0),
        ] {
            let mut class = template.clone();
            class.name = name.into();
            class.superclass = Some(parent.into());
            class.access = mmga_classfile::access::PUBLIC | flags;
            classes.insert(name.into(), class);
        }
        linked.add_bytecode_classes(classes);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, expected) in [
            ("test/Enum", true),
            ("test/Enum$1", false),
            ("test/DeclaredEnum", true),
            ("test/DeclaredEnum$1", false),
            ("test/Unflagged", false),
            ("java/lang/Enum", false),
            ("test/Mod", false),
            ("[Ltest/Enum;", false),
            ("int", false),
        ] {
            let klass = class(ctx.rt, name);
            assert_eq!(
                invoke(&mut ctx, CLASS, "isEnum", "()Z", false, &[klass]),
                V::bool(expected)
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
        let unknown = class(ctx.rt, "test/Unknown");
        invoke(&mut ctx, CLASS, "isEnum", "()Z", false, &[unknown]);
        error(&mut ctx, "UnsupportedOperationException");
    }

    fn first_constructor(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let count = ctx.rt.get_static("test/First", "calls").map_or(0, V::i) + 1;
        ctx.rt.put_static("test/First", "calls", V::I(count));
        ctx.rt.put_field(args[0], "number", V::I(count));
        V::Void
    }
    #[test]
    fn load_class_and_service_lookup_do_not_initialize_classes_until_explicit_initialization_or_construction(
    ) {
        let mut rt = Rt::new();
        let linked = service_linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let loader = invoke(
            &mut ctx,
            LOADER,
            "getSystemClassLoader",
            "()Ljava/lang/ClassLoader;",
            true,
            &[],
        );
        let name = ctx.rt.new_str("test.First");
        let actual = invoke(
            &mut ctx,
            LOADER,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            false,
            &[loader, name],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                CLASS,
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                false,
                &[actual]
            ),
            loader
        );
        let uninitialized = invoke(
            &mut ctx,
            CLASS,
            "forName",
            "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
            true,
            &[name, V::FALSE, loader],
        );
        assert_eq!(uninitialized, actual);
        assert_eq!(ctx.rt.get_static("test/First", "initializations"), None);
        let services = service(&mut ctx, loader);
        let iterator = invoke(
            &mut ctx,
            SERVICE,
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[services],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "hasNext",
                "()Z",
                false,
                &[iterator]
            ),
            V::TRUE
        );
        assert_eq!(ctx.rt.get_static("test/First", "initializations"), None);
        for _ in 0..2 {
            assert_eq!(
                invoke(
                    &mut ctx,
                    CLASS,
                    "forName",
                    "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
                    true,
                    &[name, V::TRUE, loader]
                ),
                actual
            );
        }
        assert_eq!(
            ctx.rt.get_static("test/First", "initializations"),
            Some(V::I(1))
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
            false,
            &[iterator],
        );
        assert_eq!(
            ctx.rt.get_static("test/First", "initializations"),
            Some(V::I(1))
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), Some(V::I(1)));
        assert!(ctx.rt.pending_exception.is_none());
    }

    fn first_initializer(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        let count = ctx
            .rt
            .get_static("test/First", "initializations")
            .map_or(0, V::i)
            + 1;
        ctx.rt
            .put_static("test/First", "initializations", V::I(count));
        V::Void
    }
    fn failing_constructor(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
        let count = ctx.rt.get_static("test/Second", "calls").map_or(0, V::i) + 1;
        ctx.rt.put_static("test/Second", "calls", V::I(count));
        ctx.rt
            .throw_exception("test/ProviderFailure", "original constructor failure")
    }
    fn service_linked() -> LinkedSpace {
        use mmga_classfile::{access, ClassFile, Const, MethodInfo};
        let mut linked = LinkedSpace::new();
        linked.register_native_module(NativeModuleRegistry {
            classes: &[
                NativeClassDef {
                    name: "test/First",
                    superclass: Some("java/lang/Object"),
                    interfaces: &["test/Spi"],
                    is_interface: false,
                    methods: &[
                        ("<init>.()V", first_constructor, false, false),
                        ("<clinit>.()V", first_initializer, true, false),
                    ],
                },
                NativeClassDef {
                    name: "test/Second",
                    superclass: Some("java/lang/Object"),
                    interfaces: &["test/Spi"],
                    is_interface: false,
                    methods: &[("<init>.()V", failing_constructor, false, false)],
                },
            ],
            lambdas: &[],
        });
        let mut classes = std::collections::BTreeMap::new();
        for name in ["test/Spi", "test/First", "test/Second"] {
            classes.insert(
                name.into(),
                ClassFile {
                    major_version: 65,
                    minor_version: 0,
                    pool: vec![Const::Unused],
                    access: access::PUBLIC
                        | if name == "test/Spi" {
                            access::INTERFACE | access::ABSTRACT
                        } else {
                            0
                        },
                    name: name.into(),
                    superclass: Some("java/lang/Object".into()),
                    interfaces: if name == "test/Spi" {
                        vec![]
                    } else {
                        vec!["test/Spi".into()]
                    },
                    fields: vec![],
                    methods: if name == "test/Spi" {
                        vec![]
                    } else {
                        vec![MethodInfo {
                            access: access::PUBLIC,
                            name: "<init>".into(),
                            desc: "()V".into(),
                            signature: None,
                            code: None,
                            annotations: vec![],
                            param_annotations: vec![],
                            exceptions: vec![],
                        }]
                    },
                    annotations: vec![],
                    bootstrap_methods: vec![],
                    signature: None,
                    record_components: vec![],
                },
            );
        }
        linked.add_bytecode_classes(classes);
        linked.services.insert(
            "test/Spi".into(),
            vec!["test/First".into(), "test/Second".into()],
        );
        linked
    }
    fn service(ctx: &mut ExecCtx<'_>, loader: V) -> V {
        let spi = class(ctx.rt, "test/Spi");
        invoke(
            ctx,
            SERVICE,
            "load",
            "(Ljava/lang/Class;Ljava/lang/ClassLoader;)Ljava/util/ServiceLoader;",
            true,
            &[spi, loader],
        )
    }
    #[test]
    fn service_loader_is_lazy_replays_successful_instances_and_preserves_constructor_failure_causes(
    ) {
        let mut rt = Rt::new();
        let linked = service_linked();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let loader = invoke(
            &mut ctx,
            LOADER,
            "getSystemClassLoader",
            "()Ljava/lang/ClassLoader;",
            true,
            &[],
        );
        let services = service(&mut ctx, loader);
        assert!(
            ctx.rt.pending_exception.is_none(),
            "{:?}",
            ctx.rt.pending_exception
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), None);
        let iterator = invoke(
            &mut ctx,
            SERVICE,
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[services],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "hasNext",
                "()Z",
                false,
                &[iterator]
            ),
            V::TRUE
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), None);
        let first = invoke(
            &mut ctx,
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
            false,
            &[iterator],
        );
        assert_eq!(ctx.rt.get_field(first, "number"), V::I(1));
        let replay = invoke(
            &mut ctx,
            SERVICE,
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[services],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "next",
                "()Ljava/lang/Object;",
                false,
                &[replay]
            ),
            first
        );
        assert_eq!(ctx.rt.get_static("test/Second", "calls"), None);
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "hasNext",
                "()Z",
                false,
                &[iterator]
            ),
            V::TRUE
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
            false,
            &[iterator],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/util/ServiceConfigurationError")
        );
        assert_eq!(
            ctx.rt.class_of(ctx.rt.get_field(error, "cause")),
            Some("test/ProviderFailure")
        );
        assert_eq!(ctx.rt.get_static("test/Second", "calls"), Some(V::I(1)));
    }
    #[test]
    fn service_loader_respects_platform_and_unknown_loaders_and_reload_invalidates_existing_iterators(
    ) {
        let mut rt = Rt::new();
        let mut linked = service_linked();
        linked.services.get_mut("test/Spi").unwrap().truncate(1);
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let unknown = ctx.rt.new_instance(LOADER, 0);
        service(&mut ctx, unknown);
        error(&mut ctx, "UnsupportedOperationException");
        let platform = invoke(
            &mut ctx,
            LOADER,
            "getPlatformClassLoader",
            "()Ljava/lang/ClassLoader;",
            true,
            &[],
        );
        let services = service(&mut ctx, platform);
        let iterator = invoke(
            &mut ctx,
            SERVICE,
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[services],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/Iterator",
                "hasNext",
                "()Z",
                false,
                &[iterator]
            ),
            V::FALSE
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), None);
        let services = service(&mut ctx, V::NULL);
        let iterator = invoke(
            &mut ctx,
            SERVICE,
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[services],
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
            false,
            &[iterator],
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), Some(V::I(1)));
        invoke(&mut ctx, SERVICE, "reload", "()V", false, &[services]);
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "hasNext",
            "()Z",
            false,
            &[iterator],
        );
        error(&mut ctx, "ConcurrentModificationException");
        let fresh = invoke(
            &mut ctx,
            SERVICE,
            "iterator",
            "()Ljava/util/Iterator;",
            false,
            &[services],
        );
        invoke(
            &mut ctx,
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
            false,
            &[fresh],
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), Some(V::I(2)));
    }
    #[test]
    fn service_provider_validation_is_deferred_and_stream_provider_get_constructs_each_time() {
        let mut rt = Rt::new();
        let mut linked = service_linked();
        let mut host = NullHost;
        linked
            .bytecode_classes
            .get_mut("test/Second")
            .unwrap()
            .access &= !mmga_classfile::access::PUBLIC;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let loader = invoke(
            &mut ctx,
            LOADER,
            "getSystemClassLoader",
            "()Ljava/lang/ClassLoader;",
            true,
            &[],
        );
        let services = service(&mut ctx, loader);
        let stream = invoke(
            &mut ctx,
            SERVICE,
            "stream",
            "()Ljava/util/stream/Stream;",
            false,
            &[services],
        );
        assert_eq!(ctx.rt.get_static("test/First", "calls"), None);
        let first = invoke(
            &mut ctx,
            "java/util/stream/Stream",
            "findFirst",
            "()Ljava/util/Optional;",
            false,
            &[stream],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let Some(Data::Optional(Some(provider))) = ctx.rt.get_obj(first).map(|o| &o.data) else {
            panic!("missing provider")
        };
        let provider = *provider;
        assert_eq!(
            invoke(
                &mut ctx,
                "java/util/ServiceLoader$Provider",
                "type",
                "()Ljava/lang/Class;",
                false,
                &[provider]
            ),
            class(ctx.rt, "test/First")
        );
        let one = invoke(
            &mut ctx,
            "java/util/ServiceLoader$Provider",
            "get",
            "()Ljava/lang/Object;",
            false,
            &[provider],
        );
        let two = invoke(
            &mut ctx,
            "java/util/ServiceLoader$Provider",
            "get",
            "()Ljava/lang/Object;",
            false,
            &[provider],
        );
        assert_ne!(one, two);
        assert_eq!(ctx.rt.get_static("test/First", "calls"), Some(V::I(2)));
        assert_eq!(ctx.rt.get_static("test/Second", "calls"), None);
    }
}

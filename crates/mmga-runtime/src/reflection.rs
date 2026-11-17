//! Reflection metadata is read from the linked original classfiles. Unsupported
//! external classes are not represented as having an empty method list.
use crate::{Data, ExecCtx, Rt, V};
use mmga_classfile::{
    access, parse_field_descriptor, parse_method_descriptor, Annotation, DescriptorType, FieldInfo,
    MemberRef, MethodInfo,
};

fn class_object(rt: &mut Rt, name: &str) -> V {
    rt.new_pinned_handle("java/lang/Class", "class", 0, name)
}

fn type_name(typ: &DescriptorType) -> String {
    match typ {
        DescriptorType::Void => "void".into(),
        DescriptorType::Boolean => "boolean".into(),
        DescriptorType::Byte => "byte".into(),
        DescriptorType::Char => "char".into(),
        DescriptorType::Short => "short".into(),
        DescriptorType::Int => "int".into(),
        DescriptorType::Long => "long".into(),
        DescriptorType::Float => "float".into(),
        DescriptorType::Double => "double".into(),
        DescriptorType::Object(name) => name.clone(),
        DescriptorType::Array(_) => typ.to_descriptor(),
    }
}

fn metadata(rt: &mut Rt, owner: &str, method: &MethodInfo) -> V {
    let class = if method.name == "<init>" {
        "java/lang/reflect/Constructor"
    } else {
        "java/lang/reflect/Method"
    };
    let value = rt.new_instance(class, 0);
    let declaring = class_object(rt, owner);
    let name = rt.intern_str(&method.name);
    let descriptor = rt.intern_str(&method.desc);
    rt.put_field(value, "$declaring", declaring);
    rt.put_field(value, "$name", name);
    rt.put_field(value, "$descriptor", descriptor);
    rt.put_field(value, "$modifiers", V::I(method.access as i32));
    value
}

fn field_metadata(rt: &mut Rt, owner: &str, field: &FieldInfo) -> V {
    let value = rt.new_instance("java/lang/reflect/Field", 0);
    let declaring = class_object(rt, owner);
    let name = rt.intern_str(&field.name);
    let descriptor = rt.intern_str(&field.desc);
    rt.put_field(value, "$declaring", declaring);
    rt.put_field(value, "$name", name);
    rt.put_field(value, "$descriptor", descriptor);
    rt.put_field(value, "$modifiers", V::I(field.access as i32));
    value
}

fn annotations(ctx: &ExecCtx<'_>, value: V) -> Option<Vec<Annotation>> {
    let class = ctx.rt.handle_of(ctx.rt.get_field(value, "$declaring"))?.2;
    if class == "java/lang/Object" && !ctx.linked.bytecode_classes.contains_key(class) {
        return Some(Vec::new());
    }
    let cf = ctx.linked.bytecode_classes.get(class)?;
    let name = ctx.rt.str_of(ctx.rt.get_field(value, "$name"))?;
    let desc = ctx.rt.str_of(ctx.rt.get_field(value, "$descriptor"))?;
    if ctx.rt.class_of(value) == Some("java/lang/reflect/Field") {
        Some(
            cf.fields
                .iter()
                .find(|f| f.name == name && f.desc == desc)?
                .annotations
                .clone(),
        )
    } else {
        Some(
            cf.methods
                .iter()
                .find(|m| m.name == name && m.desc == desc)?
                .annotations
                .clone(),
        )
    }
}

fn public_method(
    ctx: &ExecCtx<'_>,
    class: &str,
    name: &str,
    parameters: &[String],
    visited: &mut Vec<String>,
) -> Result<Option<(String, MethodInfo)>, String> {
    if visited.iter().any(|v| v == class) {
        return Ok(None);
    }
    visited.push(class.to_owned());
    if class == "java/lang/Object" && !ctx.linked.bytecode_classes.contains_key(class) {
        // Java 21 Object public method metadata, verified with javap -p -s.
        // clone/finalize are protected; wait0 is private and not exposed here.
        let definitions = [
            (
                "getClass",
                "()Ljava/lang/Class;",
                access::PUBLIC | access::FINAL | access::NATIVE,
            ),
            ("hashCode", "()I", access::PUBLIC | access::NATIVE),
            ("equals", "(Ljava/lang/Object;)Z", access::PUBLIC),
            ("toString", "()Ljava/lang/String;", access::PUBLIC),
            (
                "notify",
                "()V",
                access::PUBLIC | access::FINAL | access::NATIVE,
            ),
            (
                "notifyAll",
                "()V",
                access::PUBLIC | access::FINAL | access::NATIVE,
            ),
            ("wait", "()V", access::PUBLIC | access::FINAL),
            ("wait", "(J)V", access::PUBLIC | access::FINAL),
            ("wait", "(JI)V", access::PUBLIC | access::FINAL),
        ];
        return Ok(definitions
            .into_iter()
            .find(|(method, descriptor, _)| {
                *method == name
                    && parse_method_descriptor(descriptor).is_ok_and(|(args, _)| {
                        args.iter().map(type_name).collect::<Vec<_>>() == parameters
                    })
            })
            .map(|(name, desc, flags)| {
                (
                    class.to_owned(),
                    MethodInfo {
                        name: name.into(),
                        desc: desc.into(),
                        access: flags,
                        signature: None,
                        code: None,
                        annotations: vec![],
                        param_annotations: vec![],
                        exceptions: if name == "wait" {
                            vec!["java/lang/InterruptedException".into()]
                        } else {
                            vec![]
                        },
                    },
                )
            }));
    }
    let cf = ctx.linked.bytecode_classes.get(class).ok_or_else(|| {
        format!("Reflection method metadata for external ancestor {class} is not implemented")
    })?;
    let matched = cf
        .methods
        .iter()
        .filter(|m| m.name == name && m.access & access::PUBLIC != 0)
        .filter(|m| {
            parse_method_descriptor(&m.desc)
                .is_ok_and(|(args, _)| args.iter().map(type_name).collect::<Vec<_>>() == parameters)
        })
        .min_by_key(|m| m.access & access::BRIDGE != 0);
    if let Some(m) = matched {
        return Ok(Some((class.to_owned(), m.clone())));
    }
    if cf.access & access::INTERFACE == 0 {
        if let Some(parent) = &cf.superclass {
            if let Some(found) = public_method(ctx, parent, name, parameters, visited)? {
                return Ok(Some(found));
            }
        }
    }
    for interface in &cf.interfaces {
        if let Some(found) = public_method(ctx, interface, name, parameters, visited)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if is_static {
        if owner == "java/lang/reflect/Modifier" && desc == "(I)Z" {
            let flag = match name {
                "isStatic" => access::STATIC,
                "isPublic" => access::PUBLIC,
                "isPrivate" => access::PRIVATE,
                "isProtected" => access::PROTECTED,
                "isFinal" => access::FINAL,
                "isAbstract" => access::ABSTRACT,
                "isInterface" => access::INTERFACE,
                _ => return None,
            };
            return Some(V::bool(args[0].i() & flag as i32 != 0));
        }
        return None;
    }
    if owner == "java/lang/reflect/Parameter" {
        let executable = ctx.rt.get_field(args[0], "$executable");
        let index = ctx.rt.get_field(args[0], "$index").i() as usize;
        let descriptor = ctx
            .rt
            .str_of(ctx.rt.get_field(executable, "$descriptor"))?
            .to_owned();
        let (parameters, _) = parse_method_descriptor(&descriptor).ok()?;
        return match (name, desc) {
            ("getType", "()Ljava/lang/Class;") => {
                Some(class_object(ctx.rt, &type_name(parameters.get(index)?)))
            }
            ("getDeclaringExecutable", "()Ljava/lang/reflect/Executable;") => Some(executable),
            ("isVarArgs", "()Z") => Some(V::bool(
                index + 1 == parameters.len()
                    && ctx.rt.get_field(executable, "$modifiers").i() & access::VARARGS as i32 != 0,
            )),
            // MethodParameters names and flags are not retained by the parser.
            // Do not fabricate argN when the original class may name it.
            _ => None,
        };
    }
    if owner == "java/lang/reflect/InvocationTargetException"
        && name == "getTargetException"
        && desc == "()Ljava/lang/Throwable;"
    {
        return Some(ctx.rt.get_field(args[0], "target"));
    }
    if owner == "java/lang/Class"
        && matches!(
            name,
            "getConstructors"
                | "getDeclaredConstructors"
                | "getMethod"
                | "getDeclaredFields"
                | "getDeclaredMethods"
                | "getSuperclass"
        )
    {
        let ("class", _, class, _) = ctx.rt.handle_of(args[0])? else {
            return None;
        };
        let class = class.to_owned();
        let Some(cf) = ctx.linked.bytecode_classes.get(&class) else {
            if class == "java/lang/Object" && name == "getSuperclass" {
                return Some(V::NULL);
            }
            if class == "java/lang/Object"
                && name == "getDeclaredFields"
                && desc == "()[Ljava/lang/reflect/Field;"
            {
                return Some(ctx.rt.alloc(
                    "[Ljava/lang/reflect/Field;",
                    0,
                    Data::Array(Vec::new()),
                ));
            }
            if class == "java/lang/Object"
                && name == "getDeclaredMethods"
                && desc == "()[Ljava/lang/reflect/Method;"
            {
                let definitions = [
                    (
                        "getClass",
                        "()Ljava/lang/Class;",
                        access::PUBLIC | access::FINAL | access::NATIVE,
                    ),
                    ("hashCode", "()I", access::PUBLIC | access::NATIVE),
                    ("equals", "(Ljava/lang/Object;)Z", access::PUBLIC),
                    (
                        "clone",
                        "()Ljava/lang/Object;",
                        access::PROTECTED | access::NATIVE,
                    ),
                    ("toString", "()Ljava/lang/String;", access::PUBLIC),
                    (
                        "notify",
                        "()V",
                        access::PUBLIC | access::FINAL | access::NATIVE,
                    ),
                    (
                        "notifyAll",
                        "()V",
                        access::PUBLIC | access::FINAL | access::NATIVE,
                    ),
                    ("wait", "()V", access::PUBLIC | access::FINAL),
                    ("wait", "(J)V", access::PUBLIC | access::FINAL),
                    (
                        "wait0",
                        "(J)V",
                        access::PRIVATE | access::FINAL | access::NATIVE,
                    ),
                    ("wait", "(JI)V", access::PUBLIC | access::FINAL),
                    ("finalize", "()V", access::PROTECTED),
                ];
                let values = definitions
                    .into_iter()
                    .map(|(name, desc, flags)| {
                        metadata(
                            ctx.rt,
                            &class,
                            &MethodInfo {
                                name: name.into(),
                                desc: desc.into(),
                                access: flags,
                                signature: None,
                                code: None,
                                annotations: vec![],
                                param_annotations: vec![],
                                exceptions: vec![],
                            },
                        )
                    })
                    .collect();
                return Some(
                    ctx.rt
                        .alloc("[Ljava/lang/reflect/Method;", 0, Data::Array(values)),
                );
            }
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Reflection metadata for external class {class} is not implemented"),
            ));
        };
        if name == "getSuperclass" && desc == "()Ljava/lang/Class;" {
            let superclass = if cf.access & access::INTERFACE == 0 {
                cf.superclass.clone()
            } else {
                None
            };
            return Some(superclass.map_or(V::NULL, |name| class_object(ctx.rt, &name)));
        }
        if name == "getDeclaredFields" && desc == "()[Ljava/lang/reflect/Field;" {
            let fields = cf.fields.clone();
            let values = fields
                .iter()
                .map(|f| field_metadata(ctx.rt, &class, f))
                .collect();
            return Some(
                ctx.rt
                    .alloc("[Ljava/lang/reflect/Field;", 0, Data::Array(values)),
            );
        }
        if name == "getDeclaredMethods" && desc == "()[Ljava/lang/reflect/Method;" {
            let methods = cf
                .methods
                .iter()
                .filter(|m| m.name != "<init>" && m.name != "<clinit>")
                .cloned()
                .collect::<Vec<_>>();
            let values = methods
                .iter()
                .map(|m| metadata(ctx.rt, &class, m))
                .collect();
            return Some(
                ctx.rt
                    .alloc("[Ljava/lang/reflect/Method;", 0, Data::Array(values)),
            );
        }
        if matches!(name, "getConstructors" | "getDeclaredConstructors")
            && desc == "()[Ljava/lang/reflect/Constructor;"
        {
            let constructors = cf
                .methods
                .iter()
                .filter(|m| {
                    m.name == "<init>"
                        && (name == "getDeclaredConstructors" || m.access & access::PUBLIC != 0)
                })
                .cloned()
                .collect::<Vec<_>>();
            let values = constructors
                .iter()
                .map(|m| metadata(ctx.rt, &class, m))
                .collect();
            return Some(
                ctx.rt
                    .alloc("[Ljava/lang/reflect/Constructor;", 0, Data::Array(values)),
            );
        }
        if name == "getMethod"
            && desc == "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;"
        {
            if args[1].is_null() {
                return Some(
                    ctx.rt
                        .throw_exception("java/lang/NullPointerException", "getMethod name"),
                );
            }
            let method_name = ctx.rt.str_of(args[1])?.to_owned();
            let parameters = if args[2].is_null() {
                Vec::new()
            } else {
                match ctx.rt.get_obj(args[2]).map(|o| &o.data) {
                    Some(Data::Array(xs)) => xs.clone(),
                    _ => return None,
                }
            };
            let mut names = Vec::new();
            for parameter in parameters {
                if parameter.is_null() {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/NoSuchMethodException", &method_name),
                    );
                }
                let ("class", _, tag, _) = ctx.rt.handle_of(parameter)? else {
                    return None;
                };
                names.push(tag.to_owned());
            }
            return Some(
                match public_method(ctx, &class, &method_name, &names, &mut Vec::new()) {
                    Ok(Some((declaring, m))) => metadata(ctx.rt, &declaring, &m),
                    Ok(None) => ctx.rt.throw_exception(
                        "java/lang/NoSuchMethodException",
                        &format!("{class}.{method_name}"),
                    ),
                    Err(message) => ctx
                        .rt
                        .throw_exception("java/lang/UnsupportedOperationException", &message),
                },
            );
        }
        return None;
    }
    if matches!(
        owner,
        "java/lang/reflect/Constructor"
            | "java/lang/reflect/Method"
            | "java/lang/reflect/Executable"
            | "java/lang/reflect/Member"
            | "java/lang/reflect/Field"
            | "java/lang/reflect/AccessibleObject"
    ) {
        let recv = args[0];
        return match (name, desc) {
            ("getParameters", "()[Ljava/lang/reflect/Parameter;") => {
                let descriptor = ctx
                    .rt
                    .str_of(ctx.rt.get_field(recv, "$descriptor"))?
                    .to_owned();
                let (parameters, _) = parse_method_descriptor(&descriptor).ok()?;
                let values = parameters
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        let parameter = ctx.rt.new_instance("java/lang/reflect/Parameter", 0);
                        ctx.rt.put_field(parameter, "$executable", recv);
                        ctx.rt.put_field(parameter, "$index", V::I(index as i32));
                        parameter
                    })
                    .collect();
                Some(
                    ctx.rt
                        .alloc("[Ljava/lang/reflect/Parameter;", 0, Data::Array(values)),
                )
            }
            ("getReturnType", "()Ljava/lang/Class;")
                if ctx.rt.class_of(recv) == Some("java/lang/reflect/Method") =>
            {
                let descriptor = ctx
                    .rt
                    .str_of(ctx.rt.get_field(recv, "$descriptor"))?
                    .to_owned();
                Some(class_object(
                    ctx.rt,
                    &type_name(&parse_method_descriptor(&descriptor).ok()?.1),
                ))
            }
            ("invoke", "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;")
                if ctx.rt.class_of(recv) == Some("java/lang/reflect/Method") =>
            {
                Some(invoke_reflected(ctx, recv, args[1], args[2]))
            }
            ("getAnnotation", "(Ljava/lang/Class;)Ljava/lang/annotation/Annotation;") => {
                if args[1].is_null() {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/NullPointerException", "annotationClass"),
                    );
                }
                let annotation_class = ctx.rt.handle_of(args[1])?.2.to_owned();
                let wanted = format!("L{annotation_class};");
                let found = annotations(ctx, recv)?
                    .into_iter()
                    .find(|a| a.runtime_visible && a.desc == wanted);
                Some(match found {
                    None => V::NULL,
                    Some(annotation) => {
                        let value = ctx.rt.new_instance("mmga/internal/Annotation", 0);
                        let typ = class_object(ctx.rt, &annotation_class);
                        ctx.rt.put_field(value, "$annotation_type", typ);
                        // Preserve the exact encoded elements; value-method dispatch
                        // remains unsupported until each element type is implemented.
                        let raw = ctx.rt.new_str(format!("{:?}", annotation.values));
                        ctx.rt.put_field(value, "$annotation_elements", raw);
                        value
                    }
                })
            }
            ("setAccessible", "(Z)V") => {
                ctx.rt.put_field(recv, "$accessible", args[1]);
                Some(V::Void)
            }
            ("get", "(Ljava/lang/Object;)Ljava/lang/Object;")
                if ctx.rt.class_of(recv) == Some("java/lang/reflect/Field") =>
            {
                let declaring = ctx
                    .rt
                    .handle_of(ctx.rt.get_field(recv, "$declaring"))?
                    .2
                    .to_owned();
                let field_name = ctx.rt.str_of(ctx.rt.get_field(recv, "$name"))?.to_owned();
                let field_desc = ctx
                    .rt
                    .str_of(ctx.rt.get_field(recv, "$descriptor"))?
                    .to_owned();
                let flags = ctx.rt.get_field(recv, "$modifiers").i();
                if flags & access::PUBLIC as i32 == 0
                    && ctx.rt.get_field(recv, "$accessible") != V::TRUE
                {
                    return Some(
                        ctx.rt
                            .throw_exception("java/lang/IllegalAccessException", &field_name),
                    );
                }
                let value = if flags & access::STATIC as i32 != 0 {
                    crate::read_static_field(ctx, &declaring, &field_name, &field_desc)
                } else {
                    if args[1].is_null() {
                        return Some(ctx.rt.throw_exception(
                            "java/lang/NullPointerException",
                            "Field.get receiver",
                        ));
                    }
                    let actual = ctx.rt.class_of(args[1])?;
                    if !ctx.linked.is_subtype(actual, &declaring, ctx.host) {
                        return Some(ctx.rt.throw_exception(
                            "java/lang/IllegalArgumentException",
                            "Field receiver type",
                        ));
                    }
                    crate::read_instance_field(ctx, args[1], &declaring, &field_name, &field_desc)
                };
                if ctx.rt.pending_exception.is_some() {
                    return Some(V::NULL);
                }
                let typ = parse_field_descriptor(&field_desc).ok()?;
                Some(match typ {
                    DescriptorType::Boolean => {
                        crate::box_primitive(ctx.rt, "java/lang/Boolean", value)
                    }
                    DescriptorType::Byte => crate::box_primitive(ctx.rt, "java/lang/Byte", value),
                    DescriptorType::Char => {
                        crate::box_primitive(ctx.rt, "java/lang/Character", value)
                    }
                    DescriptorType::Short => crate::box_primitive(ctx.rt, "java/lang/Short", value),
                    DescriptorType::Int => crate::box_primitive(ctx.rt, "java/lang/Integer", value),
                    DescriptorType::Long => crate::box_primitive(ctx.rt, "java/lang/Long", value),
                    DescriptorType::Float => crate::box_primitive(ctx.rt, "java/lang/Float", value),
                    DescriptorType::Double => {
                        crate::box_primitive(ctx.rt, "java/lang/Double", value)
                    }
                    _ => value,
                })
            }
            ("getType", "()Ljava/lang/Class;")
                if ctx.rt.class_of(recv) == Some("java/lang/reflect/Field") =>
            {
                let descriptor = ctx
                    .rt
                    .str_of(ctx.rt.get_field(recv, "$descriptor"))?
                    .to_owned();
                Some(class_object(
                    ctx.rt,
                    &type_name(&parse_field_descriptor(&descriptor).ok()?),
                ))
            }
            ("getDeclaringClass", "()Ljava/lang/Class;") => {
                Some(ctx.rt.get_field(recv, "$declaring"))
            }
            ("getModifiers", "()I") => Some(ctx.rt.get_field(recv, "$modifiers")),
            ("getName", "()Ljava/lang/String;") => Some(
                if ctx.rt.class_of(recv) == Some("java/lang/reflect/Constructor") {
                    let class = ctx
                        .rt
                        .handle_of(ctx.rt.get_field(recv, "$declaring"))?
                        .2
                        .replace('/', ".");
                    ctx.rt.new_str(class)
                } else {
                    ctx.rt.get_field(recv, "$name")
                },
            ),
            ("getParameterTypes", "()[Ljava/lang/Class;") => {
                let descriptor = ctx
                    .rt
                    .str_of(ctx.rt.get_field(recv, "$descriptor"))?
                    .to_owned();
                let (parameters, _) = parse_method_descriptor(&descriptor).ok()?;
                let values = parameters
                    .iter()
                    .map(|typ| class_object(ctx.rt, &type_name(typ)))
                    .collect();
                Some(ctx.rt.alloc("[Ljava/lang/Class;", 0, Data::Array(values)))
            }
            ("getParameterCount", "()I") => {
                let descriptor = ctx.rt.str_of(ctx.rt.get_field(recv, "$descriptor"))?;
                Some(V::I(
                    parse_method_descriptor(descriptor).ok()?.0.len() as i32
                ))
            }
            _ => None,
        };
    }
    None
}

fn invoke_reflected(ctx: &mut ExecCtx<'_>, method: V, receiver: V, array: V) -> V {
    let Some(declaring) = ctx
        .rt
        .handle_of(ctx.rt.get_field(method, "$declaring"))
        .map(|h| h.2.to_owned())
    else {
        return V::NULL;
    };
    let Some(name) = ctx
        .rt
        .str_of(ctx.rt.get_field(method, "$name"))
        .map(str::to_owned)
    else {
        return V::NULL;
    };
    let Some(desc) = ctx
        .rt
        .str_of(ctx.rt.get_field(method, "$descriptor"))
        .map(str::to_owned)
    else {
        return V::NULL;
    };
    let flags = ctx.rt.get_field(method, "$modifiers").i() as u16;
    if flags & access::PUBLIC == 0 && ctx.rt.get_field(method, "$accessible") != V::TRUE {
        return ctx
            .rt
            .throw_exception("java/lang/IllegalAccessException", &name);
    }
    let (parameters, ret) =
        parse_method_descriptor(&desc).expect("reflected descriptor was parsed at load");
    let arguments = if array.is_null() {
        Vec::new()
    } else {
        match ctx.rt.get_obj(array).map(|o| &o.data) {
            Some(Data::Array(xs)) => xs.clone(),
            _ => {
                return ctx.rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "Method.invoke arguments are not an array",
                )
            }
        }
    };
    if arguments.len() != parameters.len() {
        return ctx.rt.throw_exception(
            "java/lang/IllegalArgumentException",
            "wrong number of arguments",
        );
    }
    let is_static = flags & access::STATIC != 0;
    let mut call_args = Vec::with_capacity(arguments.len() + 1);
    if !is_static {
        if receiver.is_null() {
            return ctx
                .rt
                .throw_exception("java/lang/NullPointerException", "Method.invoke receiver");
        }
        if !ctx.linked.is_subtype(
            ctx.rt.class_of(receiver).unwrap_or(""),
            &declaring,
            ctx.host,
        ) {
            return ctx.rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "object is not an instance of declaring class",
            );
        }
        call_args.push(receiver);
    }
    for (value, typ) in arguments.into_iter().zip(&parameters) {
        let converted = match typ {
            DescriptorType::Object(class) => {
                if value.is_null()
                    || ctx
                        .linked
                        .is_subtype(ctx.rt.class_of(value).unwrap_or(""), class, ctx.host)
                {
                    Some(value)
                } else {
                    None
                }
            }
            DescriptorType::Array(_) => {
                if value.is_null()
                    || ctx.linked.is_subtype(
                        ctx.rt.class_of(value).unwrap_or(""),
                        &typ.to_descriptor(),
                        ctx.host,
                    )
                {
                    Some(value)
                } else {
                    None
                }
            }
            _ => convert_primitive(ctx.rt, value, typ),
        };
        let Some(value) = converted else {
            return ctx.rt.throw_exception(
                "java/lang/IllegalArgumentException",
                "argument type mismatch",
            );
        };
        call_args.push(value);
    }
    if is_static {
        crate::ensure_class_init(ctx, &declaring);
        if ctx.rt.pending_exception.is_some() {
            return V::NULL;
        }
    }
    let opcode = if is_static {
        mmga_classfile::op::INVOKESTATIC
    } else if ctx
        .linked
        .bytecode_classes
        .get(&declaring)
        .is_some_and(|cf| cf.access & access::INTERFACE != 0)
    {
        mmga_classfile::op::INVOKEINTERFACE
    } else {
        mmga_classfile::op::INVOKEVIRTUAL
    };
    let result = crate::invoke_member(
        ctx,
        opcode,
        &MemberRef::new(declaring, name, desc),
        &call_args,
    );
    if let Some(cause) = ctx.rt.take_exception() {
        ctx.rt.throw_exception(
            "java/lang/reflect/InvocationTargetException",
            &ctx.rt.exception_summary(cause),
        );
        let exception = V::R(
            ctx.rt
                .pending_exception
                .expect("throw created an exception"),
        );
        ctx.rt.put_field(exception, "target", cause);
        ctx.rt.put_field(exception, "cause", cause);
        return V::NULL;
    }
    match ret {
        DescriptorType::Void => V::NULL,
        DescriptorType::Object(_) | DescriptorType::Array(_) => result,
        _ => crate::box_primitive(ctx.rt, primitive_wrapper(&ret), result),
    }
}

fn primitive_wrapper(typ: &DescriptorType) -> &'static str {
    match typ {
        DescriptorType::Boolean => "java/lang/Boolean",
        DescriptorType::Byte => "java/lang/Byte",
        DescriptorType::Char => "java/lang/Character",
        DescriptorType::Short => "java/lang/Short",
        DescriptorType::Int => "java/lang/Integer",
        DescriptorType::Long => "java/lang/Long",
        DescriptorType::Float => "java/lang/Float",
        DescriptorType::Double => "java/lang/Double",
        _ => unreachable!(),
    }
}

fn convert_primitive(rt: &Rt, value: V, typ: &DescriptorType) -> Option<V> {
    let class = rt.class_of(value)?;
    let raw = rt.get_field(value, "$value");
    let rank = match class {
        "java/lang/Byte" => 1,
        "java/lang/Short" => 2,
        "java/lang/Character" => 2,
        "java/lang/Integer" => 3,
        "java/lang/Long" => 4,
        "java/lang/Float" => 5,
        "java/lang/Double" => 6,
        "java/lang/Boolean" => 0,
        _ => return None,
    };
    Some(match typ {
        DescriptorType::Boolean if rank == 0 => raw,
        DescriptorType::Byte if rank == 1 => raw,
        DescriptorType::Char if class == "java/lang/Character" => raw,
        DescriptorType::Short if (1..=2).contains(&rank) && class != "java/lang/Character" => {
            V::I(raw.i())
        }
        DescriptorType::Int if (1..=3).contains(&rank) => V::I(raw.i()),
        DescriptorType::Long if (1..=4).contains(&rank) => {
            V::J(if rank == 4 { raw.j() } else { raw.i() as i64 })
        }
        DescriptorType::Float if (1..=5).contains(&rank) => V::F(match rank {
            5 => raw.f(),
            4 => raw.j() as f32,
            _ => raw.i() as f32,
        }),
        DescriptorType::Double if (1..=6).contains(&rank) => V::D(match rank {
            6 => raw.d(),
            5 => raw.f() as f64,
            4 => raw.j() as f64,
            _ => raw.i() as f64,
        }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NullHost};
    use mmga_classfile::{op, ClassFile};
    use std::collections::BTreeMap;

    fn method(name: &str, desc: &str, flags: u16) -> MethodInfo {
        MethodInfo {
            name: name.into(),
            desc: desc.into(),
            access: flags,
            signature: None,
            code: None,
            annotations: vec![],
            param_annotations: vec![],
            exceptions: vec![],
        }
    }
    fn class(
        name: &str,
        parent: Option<&str>,
        interfaces: &[&str],
        methods: Vec<MethodInfo>,
        flags: u16,
    ) -> ClassFile {
        ClassFile {
            major_version: 65,
            minor_version: 0,
            pool: vec![],
            access: flags,
            name: name.into(),
            superclass: parent.map(str::to_owned),
            interfaces: interfaces.iter().map(|s| s.to_string()).collect(),
            fields: vec![],
            methods,
            annotations: vec![],
            bootstrap_methods: vec![],
            signature: None,
            record_components: vec![],
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }
    #[test]
    fn ae2_model_methods_expose_real_parameters_and_invoke_the_original_target() {
        fn model(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
            ctx.rt.intern_str("ae2:part/model")
        }
        fn fail(ctx: &mut ExecCtx<'_>, _args: &[V]) -> V {
            ctx.rt
                .throw_exception("java/lang/IllegalStateException", "original failure")
        }
        let mut linked = LinkedSpace::new();
        linked.add_bytecode_classes(BTreeMap::from([(
            "test/Models".into(),
            class(
                "test/Models",
                Some("java/lang/Object"),
                &[],
                vec![
                    method(
                        "models",
                        "()Ljava/lang/String;",
                        access::PUBLIC | access::STATIC,
                    ),
                    method(
                        "typed",
                        "(ILjava/lang/String;)V",
                        access::PUBLIC | access::STATIC,
                    ),
                    method("fail", "()V", access::PUBLIC | access::STATIC),
                ],
                access::PUBLIC,
            ),
        )]));
        linked.register_native_module(crate::NativeModuleRegistry {
            classes: &[crate::NativeClassDef {
                name: "test/Models",
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[
                    ("models.()Ljava/lang/String;", model, true, false),
                    ("fail.()V", fail, true, false),
                ],
            }],
            lambdas: &[],
        });
        let mut rt = Rt::new();
        let cls = class_object(&mut rt, "test/Models");
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let methods = invoke(
            &mut ctx,
            "java/lang/Class",
            "getDeclaredMethods",
            "()[Ljava/lang/reflect/Method;",
            &[cls],
        );
        let methods = match &ctx.rt.get_obj(methods).unwrap().data {
            Data::Array(xs) => xs.clone(),
            _ => panic!(),
        };
        let parameters = invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "getParameters",
            "()[Ljava/lang/reflect/Parameter;",
            &[methods[0]],
        );
        assert!(
            ctx.rt.pending_exception.is_none(),
            "getParameters must use the actual descriptor"
        );
        assert!(
            matches!(&ctx.rt.get_obj(parameters).unwrap().data,Data::Array(xs) if xs.is_empty())
        );
        let parameters = invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "getParameters",
            "()[Ljava/lang/reflect/Parameter;",
            &[methods[1]],
        );
        let parameters = match &ctx.rt.get_obj(parameters).unwrap().data {
            Data::Array(xs) => xs.clone(),
            _ => panic!(),
        };
        assert_eq!(parameters.len(), 2);
        for (parameter, expected) in parameters.into_iter().zip(["int", "java/lang/String"]) {
            let typ = invoke(
                &mut ctx,
                "java/lang/reflect/Parameter",
                "getType",
                "()Ljava/lang/Class;",
                &[parameter],
            );
            assert_eq!(ctx.rt.handle_of(typ).unwrap().2, expected);
        }
        let typ = invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "getReturnType",
            "()Ljava/lang/Class;",
            &[methods[0]],
        );
        assert_eq!(ctx.rt.handle_of(typ).unwrap().2, "java/lang/String");
        let result = invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "invoke",
            "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
            &[methods[0], V::NULL, V::NULL],
        );
        assert_eq!(ctx.rt.str_of(result), Some("ae2:part/model"));
        assert!(ctx.rt.pending_exception.is_none());
        invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "invoke",
            "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
            &[methods[1], V::NULL, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("IllegalArgumentException"));
        invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "invoke",
            "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
            &[methods[2], V::NULL, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/reflect/InvocationTargetException")
        );
        let cause = ctx.rt.get_field(error, "target");
        assert!(ctx.rt.exception_summary(cause).contains("original failure"));
    }
    #[test]
    fn ae2_grid_service_reads_real_constructor_and_default_method_metadata() {
        let mut linked = LinkedSpace::new();
        linked.add_bytecode_classes(BTreeMap::from([
            (
                "test/Service".into(),
                class(
                    "test/Service",
                    Some("java/lang/Object"),
                    &["test/ServiceApi"],
                    vec![
                        method("<init>", "(Ltest/Grid;)V", access::PUBLIC),
                        method("<init>", "()V", access::PRIVATE),
                        method(
                            "onLevelStartTick",
                            "(Lnet/minecraft/world/level/Level;)V",
                            access::PUBLIC,
                        ),
                    ],
                    access::PUBLIC,
                ),
            ),
            (
                "test/ServiceApi".into(),
                class(
                    "test/ServiceApi",
                    Some("java/lang/Object"),
                    &[],
                    vec![
                        method("onServerStartTick", "()V", access::PUBLIC),
                        method(
                            "onLevelStartTick",
                            "(Lnet/minecraft/world/level/Level;)V",
                            access::PUBLIC,
                        ),
                    ],
                    access::PUBLIC | access::INTERFACE,
                ),
            ),
        ]));
        let mut rt = Rt::new();
        let cls = rt.new_pinned_handle("java/lang/Class", "class", 0, "test/Service");
        let api = rt.new_pinned_handle("java/lang/Class", "class", 0, "test/ServiceApi");
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let ctors = invoke(
            &mut ctx,
            "java/lang/Class",
            "getConstructors",
            "()[Ljava/lang/reflect/Constructor;",
            &[cls],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let constructors = match &ctx.rt.get_obj(ctors).unwrap().data {
            Data::Array(xs) => xs.clone(),
            _ => panic!(),
        };
        assert_eq!(constructors.len(), 1);
        let parameters = invoke(
            &mut ctx,
            "java/lang/reflect/Constructor",
            "getParameterTypes",
            "()[Ljava/lang/Class;",
            &[constructors[0]],
        );
        let types = match &ctx.rt.get_obj(parameters).unwrap().data {
            Data::Array(xs) => xs.clone(),
            _ => panic!(),
        };
        assert_eq!(ctx.rt.handle_of(types[0]).unwrap().2, "test/Grid");
        let name = ctx.rt.intern_str("onServerStartTick");
        let params = ctx.rt.alloc("[Ljava/lang/Class;", 0, Data::Array(vec![]));
        let inherited = invoke(
            &mut ctx,
            "java/lang/Class",
            "getMethod",
            "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
            &[cls, name, params],
        );
        let declaring = invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "getDeclaringClass",
            "()Ljava/lang/Class;",
            &[inherited],
        );
        assert_eq!(
            declaring, api,
            "Class references must have class-space identity"
        );
        let ctor_declaring = invoke(
            &mut ctx,
            "java/lang/reflect/Constructor",
            "getDeclaringClass",
            "()Ljava/lang/Class;",
            &[constructors[0]],
        );
        assert_eq!(ctor_declaring, cls);
        let missing = ctx.rt.intern_str("absent");
        invoke(
            &mut ctx,
            "java/lang/Class",
            "getMethod",
            "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
            &[cls, missing, params],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NoSuchMethodException"));
    }

    #[test]
    fn reflected_fields_preserve_annotations_access_control_and_original_values() {
        use mmga_classfile::{Annotation, FieldInfo};
        let mut cf = class(
            "test/Models",
            Some("java/lang/Object"),
            &[],
            vec![],
            access::PUBLIC,
        );
        cf.fields.push(FieldInfo {
            name: "MODELS".into(),
            desc: "Ljava/lang/String;".into(),
            access: access::PRIVATE | access::STATIC,
            signature: None,
            constant_value: None,
            annotations: vec![Annotation {
                desc: "Ltest/Marker;".into(),
                values: BTreeMap::new(),
                runtime_visible: true,
            }],
        });
        let mut linked = LinkedSpace::new();
        linked.add_bytecode_classes(BTreeMap::from([("test/Models".into(), cf)]));
        let mut rt = Rt::new();
        let cls = class_object(&mut rt, "test/Models");
        let ann = class_object(&mut rt, "test/Marker");
        let expected = rt.intern_str("original");
        rt.put_static("test/Models", "MODELS", expected);
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let fields = invoke(
            &mut ctx,
            "java/lang/Class",
            "getDeclaredFields",
            "()[Ljava/lang/reflect/Field;",
            &[cls],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let field = match &ctx.rt.get_obj(fields).unwrap().data {
            Data::Array(xs) => xs[0],
            _ => panic!(),
        };
        let annotation = invoke(
            &mut ctx,
            "java/lang/reflect/Field",
            "getAnnotation",
            "(Ljava/lang/Class;)Ljava/lang/annotation/Annotation;",
            &[field, ann],
        );
        assert!(!annotation.is_null());
        invoke(
            &mut ctx,
            "java/lang/reflect/Field",
            "get",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[field, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("IllegalAccessException"));
        invoke(
            &mut ctx,
            "java/lang/reflect/Field",
            "setAccessible",
            "(Z)V",
            &[field, V::TRUE],
        );
        assert_eq!(
            invoke(
                &mut ctx,
                "java/lang/reflect/Field",
                "get",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[field, V::NULL]
            ),
            expected
        );
        let wrong = class_object(ctx.rt, "test/Absent");
        assert_eq!(
            invoke(
                &mut ctx,
                "java/lang/reflect/Field",
                "getAnnotation",
                "(Ljava/lang/Class;)Ljava/lang/annotation/Annotation;",
                &[field, wrong]
            ),
            V::NULL
        );
    }

    #[test]
    fn public_method_lookup_distinguishes_known_object_methods_from_unmodeled_ancestors() {
        let mut linked = LinkedSpace::new();
        linked.add_bytecode_classes(BTreeMap::from([
            (
                "test/Plain".into(),
                class(
                    "test/Plain",
                    Some("java/lang/Object"),
                    &[],
                    vec![],
                    access::PUBLIC,
                ),
            ),
            (
                "test/ExternalChild".into(),
                class(
                    "test/ExternalChild",
                    Some("external/UnknownParent"),
                    &[],
                    vec![],
                    access::PUBLIC,
                ),
            ),
        ]));
        let mut rt = Rt::new();
        let plain = class_object(&mut rt, "test/Plain");
        let unknown = class_object(&mut rt, "test/ExternalChild");
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let tostring = ctx.rt.intern_str("toString");
        let reflected = invoke(
            &mut ctx,
            "java/lang/Class",
            "getMethod",
            "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
            &[plain, tostring, V::NULL],
        );
        assert!(ctx.rt.pending_exception.is_none());
        let declaring = invoke(
            &mut ctx,
            "java/lang/reflect/Method",
            "getDeclaringClass",
            "()Ljava/lang/Class;",
            &[reflected],
        );
        assert_eq!(ctx.rt.handle_of(declaring).unwrap().2, "java/lang/Object");
        let absent = ctx.rt.intern_str("unknownMethod");
        invoke(
            &mut ctx,
            "java/lang/Class",
            "getMethod",
            "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
            &[unknown, absent, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("external/UnknownParent"));
        let clone = ctx.rt.intern_str("clone");
        invoke(
            &mut ctx,
            "java/lang/Class",
            "getMethod",
            "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
            &[plain, clone, V::NULL],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("NoSuchMethodException"));
    }
}

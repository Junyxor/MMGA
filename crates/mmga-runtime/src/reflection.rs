//! Reflection metadata is read from the linked original classfiles. Unsupported
//! external classes are not represented as having an empty method list.
use crate::{Data, ExecCtx, Rt, V};
use mmga_classfile::{access, parse_method_descriptor, DescriptorType, MemberRef, MethodInfo};

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
    if is_static {
        return None;
    }
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    if owner == "java/lang/Class"
        && matches!(
            name,
            "getConstructors" | "getDeclaredConstructors" | "getMethod"
        )
    {
        let ("class", _, class, _) = ctx.rt.handle_of(args[0])? else {
            return None;
        };
        let class = class.to_owned();
        let Some(cf) = ctx.linked.bytecode_classes.get(&class) else {
            return Some(ctx.rt.throw_exception(
                "java/lang/UnsupportedOperationException",
                &format!("Reflection metadata for external class {class} is not implemented"),
            ));
        };
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
    ) {
        let recv = args[0];
        return match (name, desc) {
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

//! Tier-0 Bytecode Interpreter & Unified Dispatch Engine (`vm`).
//!
//! Executes any Java 8..25 method directly on `Rt` + `LinkedSpace` + `HostApi`,
//! seamlessly interoperating with Tier-1 AOT-compiled native Rust methods:
//! - Full JVMS Chapter 6 instruction set (arithmetic, bitwise, shifts, conversions,
//!   comparisons, `tableswitch`/`lookupswitch`, arrays, `checkcast`/`instanceof`,
//!   `athrow` + exception handler tables, `<clinit>` static initialization)
//! - `invokedynamic`:
//!   - `LambdaMetafactory.metafactory` / `altMetafactory` (method references, capturing lambdas, constructor refs)
//!   - `StringConcatFactory.makeConcatWithConstants` / `makeConcat`
//!   - `ObjectMethods.bootstrap` (`toString`, `hashCode`, `equals` for Java `record`s!)
//! - Live SpongePowered Mixin hook execution (`@Inject` HEAD/RETURN/TAIL/INVOKE,
//!   `@ModifyReturnValue`, `@Redirect`, `@ModifyArg`, `@Accessor`, `@Invoker`, and Mixin field initialisers)

use crate::heap::{ClosureData, ClosureTarget, Data, Rt};
use crate::linked::{ExecCtx, ResolvedMethod};
use crate::stdlib::{box_primitive, stdlib_call, stdlib_get_static, unbox_primitive};
use crate::value::V;
use mmga_classfile::{
    decode_instructions, op, parse_field_descriptor, parse_method_descriptor, BootstrapArg,
    ClassFile, Const, DescriptorType, InjectionPoint, InjectorKind, InvokeDynamicSite, MemberRef,
    MethodInfo, Operand, PlannedHook, SyntheticBridge,
};

const MAX_CALL_DEPTH: usize = 256;

/// Ensures `class_name`'s `<clinit>` has been executed once if loaded in `LinkedSpace`.
pub fn ensure_class_init(ctx: &mut ExecCtx<'_>, class_name: &str) {
    if class_name.is_empty()
        || class_name.starts_with('[')
        || ctx.rt.initialized_classes.contains(class_name)
    {
        return;
    }
    ctx.rt.initialized_classes.insert(class_name.to_owned());

    if let Some(sup) = ctx.linked.superclass_of(class_name).map(str::to_owned) {
        if sup != "java/lang/Object" {
            ensure_class_init(ctx, &sup);
        }
    }

    // Seed `ConstantValue` static fields from bytecode if present
    if let Some(cf) = ctx.linked.bytecode_classes.get(class_name) {
        let const_fields: Vec<(String, mmga_classfile::ConstantValue)> = cf
            .fields
            .iter()
            .filter(|f| f.is_static())
            .filter_map(|f| f.constant_value.clone().map(|cv| (f.name.clone(), cv)))
            .collect();
        for (fname, cv) in const_fields {
            let val = match cv {
                mmga_classfile::ConstantValue::Int(v) => V::I(v),
                mmga_classfile::ConstantValue::Long(v) => V::J(v),
                mmga_classfile::ConstantValue::Float(v) => V::F(v),
                mmga_classfile::ConstantValue::Double(v) => V::D(v),
                mmga_classfile::ConstantValue::String(s) => ctx.rt.intern_str(&s),
            };
            ctx.rt.put_static(class_name, &fname, val);
        }
    }

    if let Some(clinit) = ctx
        .linked
        .find_declared_method(class_name, "<clinit>", "()V")
    {
        let _ = invoke_resolved(ctx, clinit, &[]);
    }
}

/// Reads a static field (`GETSTATIC`), initializing the owner class if needed.
pub fn read_static_field(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str) -> V {
    ensure_class_init(ctx, owner);
    if let Some(v) = ctx.rt.get_static(owner, name) {
        return v;
    }
    if let Some(v) = stdlib_get_static(ctx.rt, owner, name, desc) {
        ctx.rt.put_static(owner, name, v);
        return v;
    }
    if let Some(v) = ctx.host.get_static_field(ctx.rt, owner, name, desc) {
        return v;
    }
    if refuses_unmapped(owner) {
        return reject_unmapped(ctx, &format!("GETSTATIC {owner}.{name}:{desc}"));
    }
    default_value_for_desc(desc)
}

/// Writes a static field (`PUTSTATIC`), initializing the owner class if needed.
pub fn write_static_field(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, val: V) {
    ensure_class_init(ctx, owner);
    if !ctx.host.put_static_field(ctx.rt, owner, name, desc, val) {
        if refuses_unmapped(owner) {
            let _ = reject_unmapped(ctx, &format!("PUTSTATIC {owner}.{name}:{desc}"));
            return;
        }
        ctx.rt.put_static(owner, name, val);
    }
}

/// Reads an instance field (`GETFIELD`).
pub fn read_instance_field(
    ctx: &mut ExecCtx<'_>,
    receiver: V,
    owner: &str,
    name: &str,
    desc: &str,
) -> V {
    if receiver.is_null() {
        return ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            &format!("read field `{name}` on null"),
        );
    }
    if let Some(obj) = ctx.rt.get_obj(receiver) {
        if let Data::Fields(map) = &obj.data {
            if let Some(&val) = map.get(name) {
                return val;
            }
        }
    }
    if let Some(v) = ctx
        .host
        .get_instance_field(ctx.rt, receiver, owner, name, desc)
    {
        return v;
    }
    if refuses_unmapped(owner) {
        return reject_unmapped(ctx, &format!("GETFIELD {owner}.{name}:{desc}"));
    }
    default_value_for_desc(desc)
}

/// Writes an instance field (`PUTFIELD`).
pub fn write_instance_field(ctx: &mut ExecCtx<'_>, receiver: V, name: &str, val: V) {
    if receiver.is_null() {
        let _ = ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            &format!("write field `{name}` on null"),
        );
        return;
    }
    ctx.rt.put_field(receiver, name, val);
}

pub fn default_value_for_desc(desc: &str) -> V {
    match desc.as_bytes().first().copied() {
        Some(b'B' | b'C' | b'I' | b'S' | b'Z') => V::I(0),
        Some(b'J') => V::J(0),
        Some(b'F') => V::F(0.0),
        Some(b'D') => V::D(0.0),
        Some(b'V') => V::Void,
        _ => V::NULL,
    }
}

/// Allocates an uninitialized object of `class_name`, pre-seeding default primitive field values.
pub fn alloc_new_object(ctx: &mut ExecCtx<'_>, class_name: &str) -> V {
    ensure_class_init(ctx, class_name);
    match class_name {
        "java/lang/StringBuilder" | "java/lang/StringBuffer" => {
            return ctx
                .rt
                .alloc(class_name, 0, Data::StringBuilder(String::new()));
        }
        "java/util/ArrayList"
        | "java/util/LinkedList"
        | "java/util/ArrayDeque"
        | "java/util/concurrent/CopyOnWriteArrayList" => {
            return ctx.rt.alloc(class_name, 0, Data::List(Vec::new()));
        }
        "java/util/HashSet" | "java/util/LinkedHashSet" | "java/util/TreeSet" => {
            return ctx.rt.alloc(class_name, 0, Data::Set(Vec::new()));
        }
        "java/util/HashMap"
        | "java/util/LinkedHashMap"
        | "java/util/IdentityHashMap"
        | "java/util/EnumMap"
        | "java/util/TreeMap"
        | "java/util/concurrent/ConcurrentHashMap" => {
            return ctx.rt.alloc(class_name, 0, Data::Map(Vec::new()));
        }
        _ => {}
    }

    let cid = ctx.linked.cid_of(class_name);
    let obj_v = ctx.rt.new_instance(class_name, cid);

    // Seed default primitive field values along superclass chain
    let mut chain = Vec::new();
    let mut cur = Some(class_name.to_owned());
    while let Some(c) = cur {
        chain.push(c.clone());
        cur = ctx.linked.superclass_of(&c).map(str::to_owned);
    }
    for cls in chain.into_iter().rev() {
        if let Some(cf) = ctx.linked.bytecode_classes.get(&cls) {
            let defaults: Vec<(String, V)> = cf
                .fields
                .iter()
                .filter(|f| !f.is_static())
                .map(|f| (f.name.clone(), default_value_for_desc(&f.desc)))
                .collect();
            for (fname, dval) in defaults {
                ctx.rt.put_field(obj_v, fname, dval);
            }
        }
    }
    obj_v
}

/// Unified method invocation entry point for `invokestatic`, `invokespecial`,
/// `invokevirtual`, and `invokeinterface`.
/// For non-static calls, `args[0]` is `receiver`.
pub fn invoke_member(ctx: &mut ExecCtx<'_>, opcode: u8, member: &MemberRef, args: &[V]) -> V {
    if ctx.depth >= MAX_CALL_DEPTH {
        return ctx.rt.throw_exception(
            "java/lang/StackOverflowError",
            "maximum call depth exceeded",
        );
    }
    ctx.rt.telemetry.total_calls += 1;
    let is_static = opcode == op::INVOKESTATIC;

    if is_static {
        ensure_class_init(ctx, &member.owner);
    } else if args.first().is_none_or(|v| v.is_null()) {
        return ctx.rt.throw_exception(
            "java/lang/NullPointerException",
            &format!("invoke `{}#{}` on null", member.owner, member.name),
        );
    }

    // 1. Determine dynamic or static target class in `LinkedSpace`
    let lookup_class: Option<String> = match opcode {
        op::INVOKESTATIC => Some(member.owner.clone()),
        op::INVOKESPECIAL => Some(member.owner.clone()),
        op::INVOKEVIRTUAL | op::INVOKEINTERFACE => ctx
            .rt
            .class_of(args[0])
            .map(str::to_owned)
            .or_else(|| Some(member.owner.clone())),
        _ => Some(member.owner.clone()),
    };

    if let Some(cls) = &lookup_class {
        let resolved = if opcode == op::INVOKESPECIAL && member.name == "<init>" {
            ctx.linked
                .find_declared_method(cls, &member.name, &member.desc)
        } else {
            ctx.linked.resolve_method(cls, &member.name, &member.desc)
        };
        if let Some(res) = resolved {
            return invoke_resolved(ctx, res, args);
        }
    }

    // 2. Built-in JDK stdlib / closures / Mixin CallbackInfo
    if let Some(v) = stdlib_call(ctx, member, is_static, args) {
        return v;
    }

    // 3. External Host API (NeoForge / Horizon engine)
    ctx.rt.telemetry.host_calls += 1;
    if let Some(v) = ctx
        .host
        .call_method(ctx.rt, ctx.linked, member, is_static, args)
    {
        return v;
    }

    // 4. Unknown Minecraft / NeoForge / Mojang / bundled-library calls fail.
    // Returning 0 or null here previously looked like a successful no-op.
    *ctx.rt
        .telemetry
        .unmapped_calls
        .entry(member.key())
        .or_insert(0) += 1;
    if refuses_unmapped(&member.owner) {
        return reject_unmapped(ctx, &member.key());
    }
    let ret_desc = member.desc.split_once(')').map_or("V", |(_, r)| r);
    default_value_for_desc(ret_desc)
}

fn refuses_unmapped(owner: &str) -> bool {
    let owner = owner.trim_start_matches('[');
    const PREFIXES: &[&str] = &[
        "net/minecraft/",
        "net/neoforged/",
        "net/minecraftforge/",
        "com/mojang/",
        "org/apache/commons/",
        "org/apache/maven/",
        "org/apache/logging/",
        "java/nio/",
        "java/io/File",
    ];
    PREFIXES.iter().any(|prefix| owner.starts_with(prefix))
}

fn reject_unmapped(ctx: &mut ExecCtx<'_>, detail: &str) -> V {
    ctx.rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("MMGA does not implement `{detail}`"),
    )
}

/// Invokes an already-resolved method (Native AOT function, Bytecode method, or Mixin SyntheticBridge).
pub fn invoke_resolved(ctx: &mut ExecCtx<'_>, resolved: ResolvedMethod, args: &[V]) -> V {
    ctx.depth += 1;
    let out = match resolved {
        ResolvedMethod::Native { func, .. } => func(ctx, args),
        ResolvedMethod::Bridge {
            declaring_class,
            bridge,
        } => execute_synthetic_bridge(ctx, &declaring_class, &bridge, args),
        ResolvedMethod::Bytecode {
            pool_owner,
            declaring_class,
            method,
        } => execute_bytecode_method(ctx, &pool_owner, &declaring_class, &method, args),
    };
    ctx.depth -= 1;
    out
}

fn execute_synthetic_bridge(
    ctx: &mut ExecCtx<'_>,
    declaring_class: &str,
    bridge: &SyntheticBridge,
    args: &[V],
) -> V {
    match bridge {
        SyntheticBridge::FieldGetter {
            field_name,
            field_desc,
            is_static,
            ..
        } => {
            if *is_static {
                read_static_field(ctx, declaring_class, field_name, field_desc)
            } else {
                read_instance_field(ctx, args[0], declaring_class, field_name, field_desc)
            }
        }
        SyntheticBridge::FieldSetter {
            field_name,
            field_desc,
            is_static,
            ..
        } => {
            if *is_static {
                write_static_field(ctx, declaring_class, field_name, field_desc, args[0]);
            } else {
                write_instance_field(ctx, args[0], field_name, args[1]);
            }
            V::Void
        }
        SyntheticBridge::MethodInvoker {
            target_name,
            target_desc,
            is_static,
            ..
        } => {
            let mref = MemberRef::new(declaring_class, target_name, target_desc);
            let op_code = if *is_static {
                op::INVOKESTATIC
            } else if target_name == "<init>" {
                op::INVOKESPECIAL
            } else {
                op::INVOKEVIRTUAL
            };
            invoke_member(ctx, op_code, &mref, args)
        }
    }
}

/// Invokes a Java lambda / method-reference `Closure` object (`Data::Closure`), performing
/// automatic primitive boxing/unboxing across erased SAM boundaries (`Consumer<T>`, `Supplier<T>`, `Function<T, R>`, etc.).
pub fn invoke_closure(ctx: &mut ExecCtx<'_>, closure_v: V, sam_args: &[V]) -> V {
    let Some(closure) = ctx.rt.get_obj(closure_v).and_then(|o| match &o.data {
        Data::Closure(c) => Some((**c).clone()),
        _ => None,
    }) else {
        // Might be a regular user class implementing `Consumer` / `Supplier` / `Runnable`!
        if !closure_v.is_null() {
            if let Some(cls) = ctx.rt.class_of(closure_v).map(str::to_owned) {
                for (mname, mdesc) in [
                    ("accept", "(Ljava/lang/Object;)V"),
                    ("get", "()Ljava/lang/Object;"),
                    ("run", "()V"),
                    ("apply", "(Ljava/lang/Object;)Ljava/lang/Object;"),
                    ("test", "(Ljava/lang/Object;)Z"),
                ] {
                    if let Some(res) = ctx.linked.resolve_method(&cls, mname, mdesc) {
                        let mut full = Vec::with_capacity(1 + sam_args.len());
                        full.push(closure_v);
                        full.extend_from_slice(sam_args);
                        return invoke_resolved(ctx, res, &full);
                    }
                }
            }
        }
        return V::NULL;
    };

    match &closure.target {
        ClosureTarget::NativeLambda(lid) => {
            if let Some(lfn) = ctx.linked.native_lambda(*lid) {
                let mut full = Vec::with_capacity(closure.captures.len() + sam_args.len());
                full.extend_from_slice(&closure.captures);
                full.extend_from_slice(sam_args);
                return lfn(ctx, &full);
            }
            V::NULL
        }
        ClosureTarget::MethodHandle {
            handle_kind,
            member,
        } => {
            // `REF_newInvokeSpecial` (kind == 8): `Foo::new` allocates a new `Foo` and runs `<init>`!
            if *handle_kind == 8 {
                let inst = alloc_new_object(ctx, &member.owner);
                let (expected_params, _) =
                    parse_method_descriptor(&member.desc).unwrap_or_default();
                let mut call_args = Vec::with_capacity(1 + closure.captures.len() + sam_args.len());
                call_args.push(inst);
                let mut raw_inputs = Vec::with_capacity(closure.captures.len() + sam_args.len());
                raw_inputs.extend_from_slice(&closure.captures);
                raw_inputs.extend_from_slice(sam_args);
                for (i, raw) in raw_inputs.into_iter().enumerate() {
                    call_args.push(adapt_arg(ctx.rt, raw, expected_params.get(i)));
                }
                let _ = invoke_member(ctx, op::INVOKESPECIAL, member, &call_args);
                return inst;
            }

            let op_code = match *handle_kind {
                6 => op::INVOKESTATIC,
                7 => op::INVOKESPECIAL,
                9 => op::INVOKEINTERFACE,
                _ => op::INVOKEVIRTUAL,
            };

            let (expected_params, expected_ret) =
                parse_method_descriptor(&member.desc).unwrap_or((Vec::new(), DescriptorType::Void));
            let mut raw_inputs = Vec::with_capacity(closure.captures.len() + sam_args.len());
            raw_inputs.extend_from_slice(&closure.captures);
            raw_inputs.extend_from_slice(sam_args);

            let mut call_args = Vec::with_capacity(raw_inputs.len());
            let has_receiver = op_code != op::INVOKESTATIC;
            for (idx, raw) in raw_inputs.into_iter().enumerate() {
                if has_receiver && idx == 0 {
                    call_args.push(raw);
                } else {
                    let p_idx = if has_receiver { idx - 1 } else { idx };
                    call_args.push(adapt_arg(ctx.rt, raw, expected_params.get(p_idx)));
                }
            }

            let result = invoke_member(ctx, op_code, member, &call_args);

            // Box primitive return value if SAM returns an Object (`()Ljava/lang/Object;`, etc.)
            let sam_ret_is_obj = closure
                .sam_desc
                .split_once(')')
                .is_some_and(|(_, r)| r.starts_with('L'));
            if sam_ret_is_obj {
                match (&expected_ret, result) {
                    (DescriptorType::Boolean, V::I(v)) => {
                        return box_primitive(ctx.rt, "java/lang/Boolean", V::I(v));
                    }
                    (
                        DescriptorType::Int | DescriptorType::Short | DescriptorType::Byte,
                        V::I(v),
                    ) => {
                        return box_primitive(ctx.rt, "java/lang/Integer", V::I(v));
                    }
                    (DescriptorType::Long, V::J(v)) => {
                        return box_primitive(ctx.rt, "java/lang/Long", V::J(v));
                    }
                    (DescriptorType::Float, V::F(v)) => {
                        return box_primitive(ctx.rt, "java/lang/Float", V::F(v));
                    }
                    (DescriptorType::Double, V::D(v)) => {
                        return box_primitive(ctx.rt, "java/lang/Double", V::D(v));
                    }
                    _ => {}
                }
            }
            result
        }
    }
}

fn adapt_arg(rt: &mut Rt, val: V, expected: Option<&DescriptorType>) -> V {
    let Some(ty) = expected else {
        return val;
    };
    match ty {
        DescriptorType::Boolean
        | DescriptorType::Byte
        | DescriptorType::Short
        | DescriptorType::Char
        | DescriptorType::Int => V::I(unbox_primitive(rt, val).i()),
        DescriptorType::Long => V::J(unbox_primitive(rt, val).j()),
        DescriptorType::Float => V::F(unbox_primitive(rt, val).f()),
        DescriptorType::Double => V::D(unbox_primitive(rt, val).d()),
        DescriptorType::Object(_) => match val {
            V::I(i) => box_primitive(rt, "java/lang/Integer", V::I(i)),
            V::J(j) => box_primitive(rt, "java/lang/Long", V::J(j)),
            V::F(f) => box_primitive(rt, "java/lang/Float", V::F(f)),
            V::D(d) => box_primitive(rt, "java/lang/Double", V::D(d)),
            other => other,
        },
        _ => val,
    }
}

/// Evaluates `invokedynamic` bootstrap sites (`LambdaMetafactory`, `StringConcatFactory`, `ObjectMethods`).
pub fn execute_invokedynamic(
    ctx: &mut ExecCtx<'_>,
    owner_class: &str,
    indy: &InvokeDynamicSite,
    dyn_args: &[V],
) -> V {
    let bsm_owner = indy
        .bootstrap_handle
        .as_ref()
        .map(|(_, m)| m.owner.as_str())
        .unwrap_or("");
    let bsm_name = indy
        .bootstrap_handle
        .as_ref()
        .map(|(_, m)| m.name.as_str())
        .unwrap_or("");

    // 1. `java/lang/invoke/LambdaMetafactory`
    if bsm_owner == "java/lang/invoke/LambdaMetafactory" {
        let sam_desc = match indy.bootstrap_args.first() {
            Some(BootstrapArg::MethodType(d)) => d.clone(),
            _ => "()V".into(),
        };
        let impl_handle = match indy.bootstrap_args.get(1) {
            Some(BootstrapArg::MethodHandle(k, m)) => (*k, m.clone()),
            _ => return V::NULL,
        };
        let instantiated_desc = match indy.bootstrap_args.get(2) {
            Some(BootstrapArg::MethodType(d)) => d.clone(),
            _ => sam_desc.clone(),
        };
        let iface_cls = indy
            .desc
            .split_once(')')
            .map(|(_, r)| {
                r.strip_prefix('L')
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or(r)
                    .to_owned()
            })
            .unwrap_or_else(|| "java/lang/Object".into());

        let closure = ClosureData {
            interface_class: iface_cls.clone(),
            sam_name: indy.name.clone(),
            sam_desc,
            instantiated_desc,
            captures: dyn_args.to_vec(),
            target: ClosureTarget::MethodHandle {
                handle_kind: impl_handle.0,
                member: impl_handle.1,
            },
        };
        return ctx.rt.alloc(iface_cls, 0, Data::Closure(Box::new(closure)));
    }

    // 2. `java/lang/invoke/StringConcatFactory`
    if bsm_owner == "java/lang/invoke/StringConcatFactory" {
        if bsm_name == "makeConcatWithConstants" {
            let recipe = match indy.bootstrap_args.first() {
                Some(BootstrapArg::String(s)) => s.as_str(),
                _ => "",
            };
            let mut out = String::with_capacity(recipe.len() + dyn_args.len() * 8);
            let mut dyn_idx = 0usize;
            let mut const_idx = 1usize;
            for ch in recipe.chars() {
                match ch {
                    '\u{0001}' => {
                        if let Some(&v) = dyn_args.get(dyn_idx) {
                            out.push_str(&ctx.rt.value_to_string(v));
                            dyn_idx += 1;
                        }
                    }
                    '\u{0002}' => {
                        if let Some(carg) = indy.bootstrap_args.get(const_idx) {
                            const_idx += 1;
                            match carg {
                                BootstrapArg::String(s) => out.push_str(s),
                                BootstrapArg::Int(i) => out.push_str(&i.to_string()),
                                BootstrapArg::Long(l) => out.push_str(&l.to_string()),
                                BootstrapArg::Float(f) => out.push_str(&f.to_string()),
                                BootstrapArg::Double(d) => out.push_str(&d.to_string()),
                                BootstrapArg::Class(c) => out.push_str(c),
                                _ => {}
                            }
                        }
                    }
                    other => out.push(other),
                }
            }
            return ctx.rt.new_str(out);
        }
        // `makeConcat`: concatenate all dynamic args in order
        let mut out = String::new();
        for &v in dyn_args {
            out.push_str(&ctx.rt.value_to_string(v));
        }
        return ctx.rt.new_str(out);
    }

    // 3. `java/lang/runtime/ObjectMethods.bootstrap` (Java 16+ `record` `toString`, `equals`, `hashCode`)
    if bsm_owner == "java/lang/runtime/ObjectMethods" {
        let field_names = match indy.bootstrap_args.get(1) {
            Some(BootstrapArg::String(s)) if !s.is_empty() => {
                s.split(';').map(str::to_owned).collect::<Vec<_>>()
            }
            _ => Vec::new(),
        };
        let self_v = dyn_args.first().copied().unwrap_or(V::NULL);
        match indy.name.as_str() {
            "toString" => {
                let simple = owner_class.rsplit(['/', '$']).next().unwrap_or(owner_class);
                let parts: Vec<String> = field_names
                    .iter()
                    .map(|fname| {
                        let fv = ctx.rt.get_field(self_v, fname);
                        format!("{fname}={}", ctx.rt.value_to_string(fv))
                    })
                    .collect();
                return ctx.rt.new_str(format!("{simple}[{}]", parts.join(", ")));
            }
            "hashCode" => {
                let mut h = 0i32;
                for fname in &field_names {
                    let fv = ctx.rt.get_field(self_v, fname);
                    let fh = if let Some(s) = ctx.rt.str_of(fv) {
                        s.encode_utf16().fold(0i32, |acc, u| {
                            acc.wrapping_mul(31).wrapping_add(i32::from(u))
                        })
                    } else {
                        fv.i() ^ (fv.j() as i32) ^ (fv.r() as i32)
                    };
                    h = h.wrapping_mul(31).wrapping_add(fh);
                }
                return V::I(h);
            }
            "equals" => {
                let other_v = dyn_args.get(1).copied().unwrap_or(V::NULL);
                if self_v == other_v {
                    return V::TRUE;
                }
                if ctx.rt.class_of(self_v) != ctx.rt.class_of(other_v) {
                    return V::FALSE;
                }
                for fname in &field_names {
                    let a = ctx.rt.get_field(self_v, fname);
                    let b = ctx.rt.get_field(other_v, fname);
                    if !ctx.rt.val_eq(a, b) {
                        return V::FALSE;
                    }
                }
                return V::TRUE;
            }
            _ => {}
        }
    }

    V::NULL
}

/// Executes a bytecode `MethodInfo` (with full Mixin hook weaving and exception table handling).
fn execute_bytecode_method(
    ctx: &mut ExecCtx<'_>,
    pool_owner: &str,
    declaring_class: &str,
    method: &MethodInfo,
    args: &[V],
) -> V {
    let Some(code) = &method.code else {
        return V::Void;
    };
    let Some(cf) = ctx.linked.bytecode_classes.get(pool_owner).cloned() else {
        return V::Void;
    };
    let Ok(insns) = decode_instructions(&code.bytes) else {
        return V::Void;
    };
    let (param_types, ret_type) =
        parse_method_descriptor(&method.desc).unwrap_or((Vec::new(), DescriptorType::Void));

    // Populate JVMS local-variable slots (remember `long` and `double` occupy 2 local slots!)
    let mut locals = vec![V::Void; (code.max_locals as usize).max(args.len() * 2 + 2)];
    let mut slot = 0usize;
    let is_static = method.is_static();
    for (idx, &arg) in args.iter().enumerate() {
        if slot >= locals.len() {
            locals.resize(slot + 2, V::Void);
        }
        locals[slot] = arg;
        if !is_static && idx == 0 {
            slot += 1;
        } else {
            let p_idx = if is_static { idx } else { idx - 1 };
            slot += param_types.get(p_idx).map_or(1, DescriptorType::jvm_slots);
        }
    }

    // Collect applicable Mixin hooks for `(declaring_class, method)`
    let applicable_hooks: Vec<PlannedHook> = ctx
        .linked
        .weave_plan(declaring_class)
        .map(|plan| {
            let full_key = format!("{}.{}", method.name, method.desc);
            plan.hooks
                .iter()
                .filter(|h| h.target_method == full_key || h.target_method == method.name)
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    // Run Mixin `@Inject(at = @At("HEAD"))` hooks before instruction 0
    for hook in &applicable_hooks {
        if hook.point == InjectionPoint::Head {
            if let InjectorKind::Inject { cancellable } = hook.kind {
                if let Some(early_ret) = run_mixin_inject_hook(
                    ctx,
                    declaring_class,
                    method,
                    &ret_type,
                    hook,
                    args,
                    None,
                    cancellable,
                ) {
                    return early_ret;
                }
            }
        }
    }

    // Map bytecode offset -> instruction index for O(1) jumps
    let mut pc_to_idx = vec![usize::MAX; code.bytes.len() + 1];
    for (idx, insn) in insns.iter().enumerate() {
        if insn.at < pc_to_idx.len() {
            pc_to_idx[insn.at] = idx;
        }
    }

    // Identify last return instruction index for `@At("TAIL")`
    let last_return_idx = insns.iter().rposition(mmga_classfile::Insn::is_return);

    // Count ordinals per `@At(value = "INVOKE", target = ...)`
    let mut invoke_ordinals: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    let mut stack: Vec<V> = Vec::with_capacity(code.max_stack as usize + 4);
    let mut idx = 0usize;

    while idx < insns.len() {
        let insn = &insns[idx];
        let cur_pc = insn.at;

        // If this is a `<init>` constructor on a Mixin target class and we are about to return,
        // run any Mixin field initialisers first!
        if method.name == "<init>" && insn.is_return() && pool_owner == declaring_class {
            if let Some(plan) = ctx.linked.weave_plan(declaring_class).cloned() {
                for (mixin_cls, init_m) in &plan.field_initializers {
                    // Execute the stripped mixin `<init>` skipping `super(...)`: run only field stores
                    run_mixin_field_initializer(ctx, mixin_cls, init_m, args[0]);
                }
            }
        }

        // Before `return` instructions: run `@Inject(at = @At("RETURN" | "TAIL"))` and `@ModifyReturnValue`
        if insn.is_return() && !applicable_hooks.is_empty() {
            let mut cur_ret = if insn.op == op::RETURN {
                None
            } else {
                stack.last().copied()
            };
            let is_tail = last_return_idx == Some(idx);
            for hook in &applicable_hooks {
                if let InjectionPoint::Return { tail_only } = hook.point {
                    if tail_only && !is_tail {
                        continue;
                    }
                    match hook.kind {
                        InjectorKind::Inject { cancellable } => {
                            if let Some(early) = run_mixin_inject_hook(
                                ctx,
                                declaring_class,
                                method,
                                &ret_type,
                                hook,
                                args,
                                cur_ret,
                                cancellable,
                            ) {
                                return early;
                            }
                        }
                        InjectorKind::ModifyReturnValue => {
                            if let Some(rv) = cur_ret {
                                let modified =
                                    run_mixin_modify_return(ctx, declaring_class, hook, args, rv);
                                cur_ret = Some(modified);
                                if let Some(top) = stack.last_mut() {
                                    *top = modified;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut next_idx = idx + 1;
        step_instruction(
            ctx,
            &cf,
            declaring_class,
            method,
            &ret_type,
            args,
            insn,
            &applicable_hooks,
            &mut invoke_ordinals,
            &mut locals,
            &mut stack,
            &pc_to_idx,
            &mut next_idx,
        );

        // Check for in-flight Java exception and route through `CodeAttr.handlers`!
        if let Some(exc_v) = ctx.rt.pending_exception.map(V::R) {
            let exc_cls = ctx
                .rt
                .class_of(exc_v)
                .unwrap_or("java/lang/Throwable")
                .to_owned();
            let mut caught_target = None;
            for h in &code.handlers {
                if cur_pc >= h.start_pc as usize && cur_pc < h.end_pc as usize {
                    let matches = match &h.catch_type {
                        None => true,
                        Some(catch_cls) => ctx.linked.is_subtype(&exc_cls, catch_cls, ctx.host),
                    };
                    if matches {
                        caught_target = Some(h.handler_pc as usize);
                        break;
                    }
                }
            }
            if let Some(handler_pc) = caught_target {
                ctx.rt.pending_exception = None;
                stack.clear();
                stack.push(exc_v);
                next_idx = pc_to_idx[handler_pc];
            } else {
                return default_value_for_desc(method.desc.split_once(')').map_or("V", |(_, r)| r));
            }
        }

        // Check if instruction returned from the method (`next_idx == usize::MAX`)
        if next_idx == usize::MAX {
            return stack.pop().unwrap_or(V::Void);
        }
        idx = next_idx;
    }

    V::Void
}

fn run_mixin_field_initializer(
    ctx: &mut ExecCtx<'_>,
    mixin_cls: &str,
    init_m: &MethodInfo,
    self_v: V,
) {
    let Some(code) = &init_m.code else {
        return;
    };
    let Some(cf) = ctx.linked.bytecode_classes.get(mixin_cls).cloned() else {
        return;
    };
    let Ok(insns) = decode_instructions(&code.bytes) else {
        return;
    };
    // Find the first `invokespecial <init>` (the `super(...)` call) and start executing right after it!
    let Some(super_pos) = insns.iter().position(|i| {
        i.op == op::INVOKESPECIAL
            && i.cp_index()
                .and_then(|cp| cf.cp_member(cp))
                .is_some_and(|m| m.name == "<init>")
    }) else {
        return;
    };

    let mut locals = vec![V::Void; (code.max_locals as usize).max(4)];
    locals[0] = self_v;
    let mut stack = Vec::new();
    let mut pc_to_idx = vec![usize::MAX; code.bytes.len() + 1];
    for (k, insn) in insns.iter().enumerate() {
        pc_to_idx[insn.at] = k;
    }
    let mut idx = super_pos + 1;
    let mut dummy_ord = std::collections::HashMap::new();
    while idx < insns.len() {
        let insn = &insns[idx];
        if insn.is_return() {
            break;
        }
        let mut next_idx = idx + 1;
        step_instruction(
            ctx,
            &cf,
            mixin_cls,
            init_m,
            &DescriptorType::Void,
            &[self_v],
            insn,
            &[],
            &mut dummy_ord,
            &mut locals,
            &mut stack,
            &pc_to_idx,
            &mut next_idx,
        );
        if next_idx == usize::MAX || ctx.rt.pending_exception.is_some() {
            break;
        }
        idx = next_idx;
    }
}

fn run_mixin_inject_hook(
    ctx: &mut ExecCtx<'_>,
    declaring_class: &str,
    target_method: &MethodInfo,
    ret_type: &DescriptorType,
    hook: &PlannedHook,
    method_args: &[V],
    current_return: Option<V>,
    cancellable: bool,
) -> Option<V> {
    let ci_cls = if *ret_type == DescriptorType::Void {
        "org/spongepowered/asm/mixin/injection/callback/CallbackInfo"
    } else {
        "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable"
    };
    let ci_v = ctx.rt.alloc(
        ci_cls,
        0,
        Data::CallbackInfo {
            name: target_method.name.clone(),
            cancellable,
            cancelled: false,
            return_value: current_return,
        },
    );

    let (h_params, _) = parse_method_descriptor(&hook.handler_desc).unwrap_or_default();
    // Build handler call args: `[this?]` + `[captured target params if handler expects them]` + `[ci]`
    let mut h_args = Vec::new();
    let target_is_static = target_method.is_static();
    if !hook.handler_is_static && !target_is_static {
        h_args.push(method_args[0]);
    }
    let non_recv_method_args = if target_is_static {
        method_args
    } else {
        &method_args[1..]
    };
    // `h_params` ends with `CallbackInfo*`; any preceding parameters are target method parameters
    let wanted_target_params = h_params.len().saturating_sub(1);
    for &arg in non_recv_method_args.iter().take(wanted_target_params) {
        h_args.push(arg);
    }
    h_args.push(ci_v);

    let mref = MemberRef::new(declaring_class, &hook.handler_name, &hook.handler_desc);
    let op_code = if hook.handler_is_static {
        op::INVOKESTATIC
    } else {
        op::INVOKEVIRTUAL
    };
    let _ = invoke_member(ctx, op_code, &mref, &h_args);

    if let Some(Data::CallbackInfo {
        cancelled: true,
        return_value,
        ..
    }) = ctx.rt.get_obj(ci_v).map(|o| &o.data)
    {
        if *ret_type == DescriptorType::Void {
            return Some(V::Void);
        }
        return Some(
            return_value
                .map(|rv| adapt_arg(ctx.rt, rv, Some(ret_type)))
                .unwrap_or_else(|| default_value_for_desc(&ret_type.to_descriptor())),
        );
    }
    None
}

fn run_mixin_modify_return(
    ctx: &mut ExecCtx<'_>,
    declaring_class: &str,
    hook: &PlannedHook,
    method_args: &[V],
    original: V,
) -> V {
    let mut h_args = Vec::new();
    if !hook.handler_is_static {
        h_args.push(method_args[0]);
    }
    h_args.push(original);
    let mref = MemberRef::new(declaring_class, &hook.handler_name, &hook.handler_desc);
    let op_code = if hook.handler_is_static {
        op::INVOKESTATIC
    } else {
        op::INVOKEVIRTUAL
    };
    invoke_member(ctx, op_code, &mref, &h_args)
}

#[allow(clippy::cognitive_complexity)]
fn step_instruction(
    ctx: &mut ExecCtx<'_>,
    cf: &ClassFile,
    declaring_class: &str,
    target_method: &MethodInfo,
    ret_type: &DescriptorType,
    method_args: &[V],
    insn: &mmga_classfile::Insn,
    hooks: &[PlannedHook],
    invoke_ordinals: &mut std::collections::HashMap<String, usize>,
    locals: &mut Vec<V>,
    stack: &mut Vec<V>,
    pc_to_idx: &[usize],
    next_idx: &mut usize,
) {
    let pop = |s: &mut Vec<V>| s.pop().unwrap_or(V::Void);
    let ensure_local = |l: &mut Vec<V>, slot: usize| {
        if slot >= l.len() {
            l.resize(slot + 2, V::Void);
        }
    };

    match insn.op {
        op::NOP | op::MONITORENTER | op::MONITOREXIT => {}
        op::ACONST_NULL => stack.push(V::NULL),
        op::ICONST_M1..=op::ICONST_5 => stack.push(V::I(i32::from(insn.op) - 3)),
        op::LCONST_0 | op::LCONST_1 => stack.push(V::J(i64::from(insn.op - op::LCONST_0))),
        op::FCONST_0..=op::FCONST_2 => stack.push(V::F(f32::from(insn.op - op::FCONST_0))),
        op::DCONST_0 | op::DCONST_1 => stack.push(V::D(f64::from(insn.op - op::DCONST_0))),
        op::BIPUSH | op::SIPUSH => {
            if let Operand::Int(v) = insn.arg {
                stack.push(V::I(v));
            }
        }
        op::LDC | op::LDC_W | op::LDC2_W => {
            let Operand::Int(cp) = insn.arg else {
                return;
            };
            let val = match cf.pool.get(cp as usize) {
                Some(Const::Int(v)) => V::I(*v),
                Some(Const::Long(v)) => V::J(*v),
                Some(Const::Float(v)) => V::F(*v),
                Some(Const::Double(v)) => V::D(*v),
                Some(Const::String(u)) => {
                    let s = cf.cp_utf8(*u).unwrap_or_default();
                    ctx.rt.intern_str(&s)
                }
                Some(Const::Class(u)) => {
                    let cname = cf.cp_utf8(*u).unwrap_or_default();
                    ctx.rt
                        .new_pinned_handle("java/lang/Class", "class", 0, cname)
                }
                _ => V::NULL,
            };
            stack.push(val);
        }

        // Local loads
        op::ILOAD | op::LLOAD | op::FLOAD | op::DLOAD | op::ALOAD => {
            if let Operand::Int(slot) = insn.arg {
                let s = slot as usize;
                stack.push(locals.get(s).copied().unwrap_or(V::Void));
            }
        }
        op::ILOAD_0..=op::ILOAD_3 => {
            stack.push(locals[(insn.op - op::ILOAD_0) as usize]);
        }
        op::LLOAD_0..=op::LLOAD_3 => {
            stack.push(locals[(insn.op - op::LLOAD_0) as usize]);
        }
        op::FLOAD_0..=op::FLOAD_3 => {
            stack.push(locals[(insn.op - op::FLOAD_0) as usize]);
        }
        op::DLOAD_0..=op::DLOAD_3 => {
            stack.push(locals[(insn.op - op::DLOAD_0) as usize]);
        }
        op::ALOAD_0..=op::ALOAD_3 => {
            stack.push(locals[(insn.op - op::ALOAD_0) as usize]);
        }

        // Local stores
        op::ISTORE | op::LSTORE | op::FSTORE | op::DSTORE | op::ASTORE => {
            if let Operand::Int(slot) = insn.arg {
                let s = slot as usize;
                ensure_local(locals, s);
                locals[s] = pop(stack);
            }
        }
        op::ISTORE_0..=op::ISTORE_3 => {
            let s = (insn.op - op::ISTORE_0) as usize;
            ensure_local(locals, s);
            locals[s] = pop(stack);
        }
        op::LSTORE_0..=op::LSTORE_3 => {
            let s = (insn.op - op::LSTORE_0) as usize;
            ensure_local(locals, s);
            locals[s] = pop(stack);
        }
        op::FSTORE_0..=op::FSTORE_3 => {
            let s = (insn.op - op::FSTORE_0) as usize;
            ensure_local(locals, s);
            locals[s] = pop(stack);
        }
        op::DSTORE_0..=op::DSTORE_3 => {
            let s = (insn.op - op::DSTORE_0) as usize;
            ensure_local(locals, s);
            locals[s] = pop(stack);
        }
        op::ASTORE_0..=op::ASTORE_3 => {
            let s = (insn.op - op::ASTORE_0) as usize;
            ensure_local(locals, s);
            locals[s] = pop(stack);
        }

        // Array loads & stores
        op::IALOAD..=op::SALOAD => {
            let index = pop(stack).i();
            let arr = pop(stack);
            if arr.is_null() {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/NullPointerException", "array load on null");
                return;
            }
            let elem = match ctx.rt.get_obj(arr).map(|o| &o.data) {
                Some(Data::Array(items)) => {
                    if index < 0 || (index as usize) >= items.len() {
                        None
                    } else {
                        Some(items[index as usize])
                    }
                }
                _ => Some(V::NULL),
            };
            match elem {
                Some(v) => stack.push(v),
                None => {
                    let _ = ctx.rt.throw_exception(
                        "java/lang/ArrayIndexOutOfBoundsException",
                        &format!("Index {index}"),
                    );
                }
            }
        }
        op::IASTORE..=op::SASTORE => {
            let val = pop(stack);
            let index = pop(stack).i();
            let arr = pop(stack);
            if arr.is_null() {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/NullPointerException", "array store on null");
                return;
            }
            let mut oob = false;
            if let Some(Data::Array(items)) = ctx.rt.get_obj_mut(arr).map(|o| &mut o.data) {
                if index < 0 || (index as usize) >= items.len() {
                    oob = true;
                } else {
                    items[index as usize] = val;
                }
            }
            if oob {
                let _ = ctx.rt.throw_exception(
                    "java/lang/ArrayIndexOutOfBoundsException",
                    &format!("Index {index}"),
                );
            }
        }

        // Stack manipulation
        op::POP => {
            let _ = pop(stack);
        }
        op::POP2 => {
            let v1 = pop(stack);
            if !v1.is_cat2() {
                let _ = pop(stack);
            }
        }
        op::DUP => {
            if let Some(&top) = stack.last() {
                stack.push(top);
            }
        }
        op::DUP_X1 => {
            let v1 = pop(stack);
            let v2 = pop(stack);
            stack.push(v1);
            stack.push(v2);
            stack.push(v1);
        }
        op::DUP_X2 => {
            let v1 = pop(stack);
            let v2 = pop(stack);
            if v2.is_cat2() {
                stack.push(v1);
                stack.push(v2);
                stack.push(v1);
            } else {
                let v3 = pop(stack);
                stack.push(v1);
                stack.push(v3);
                stack.push(v2);
                stack.push(v1);
            }
        }
        op::DUP2 => {
            let v1 = pop(stack);
            if v1.is_cat2() {
                stack.push(v1);
                stack.push(v1);
            } else {
                let v2 = pop(stack);
                stack.push(v2);
                stack.push(v1);
                stack.push(v2);
                stack.push(v1);
            }
        }
        op::DUP2_X1 => {
            let v1 = pop(stack);
            if v1.is_cat2() {
                let v2 = pop(stack);
                stack.push(v1);
                stack.push(v2);
                stack.push(v1);
            } else {
                let v2 = pop(stack);
                let v3 = pop(stack);
                stack.push(v2);
                stack.push(v1);
                stack.push(v3);
                stack.push(v2);
                stack.push(v1);
            }
        }
        op::SWAP => {
            let v1 = pop(stack);
            let v2 = pop(stack);
            stack.push(v1);
            stack.push(v2);
        }

        // Arithmetic & Bitwise
        op::IADD => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a.wrapping_add(b)));
        }
        op::LADD => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::J(a.wrapping_add(b)));
        }
        op::FADD => {
            let (b, a) = (pop(stack).f(), pop(stack).f());
            stack.push(V::F(a + b));
        }
        op::DADD => {
            let (b, a) = (pop(stack).d(), pop(stack).d());
            stack.push(V::D(a + b));
        }
        op::ISUB => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a.wrapping_sub(b)));
        }
        op::LSUB => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::J(a.wrapping_sub(b)));
        }
        op::FSUB => {
            let (b, a) = (pop(stack).f(), pop(stack).f());
            stack.push(V::F(a - b));
        }
        op::DSUB => {
            let (b, a) = (pop(stack).d(), pop(stack).d());
            stack.push(V::D(a - b));
        }
        op::IMUL => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a.wrapping_mul(b)));
        }
        op::LMUL => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::J(a.wrapping_mul(b)));
        }
        op::FMUL => {
            let (b, a) = (pop(stack).f(), pop(stack).f());
            stack.push(V::F(a * b));
        }
        op::DMUL => {
            let (b, a) = (pop(stack).d(), pop(stack).d());
            stack.push(V::D(a * b));
        }
        op::IDIV => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            if b == 0 {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/ArithmeticException", "/ by zero");
            } else {
                stack.push(V::I(a.wrapping_div(b)));
            }
        }
        op::LDIV => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            if b == 0 {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/ArithmeticException", "/ by zero");
            } else {
                stack.push(V::J(a.wrapping_div(b)));
            }
        }
        op::FDIV => {
            let (b, a) = (pop(stack).f(), pop(stack).f());
            stack.push(V::F(a / b));
        }
        op::DDIV => {
            let (b, a) = (pop(stack).d(), pop(stack).d());
            stack.push(V::D(a / b));
        }
        op::IREM => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            if b == 0 {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/ArithmeticException", "/ by zero");
            } else {
                stack.push(V::I(a.wrapping_rem(b)));
            }
        }
        op::LREM => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            if b == 0 {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/ArithmeticException", "/ by zero");
            } else {
                stack.push(V::J(a.wrapping_rem(b)));
            }
        }
        op::FREM => {
            let (b, a) = (pop(stack).f(), pop(stack).f());
            stack.push(V::F(a % b));
        }
        op::DREM => {
            let (b, a) = (pop(stack).d(), pop(stack).d());
            stack.push(V::D(a % b));
        }
        op::INEG => {
            let a = pop(stack).i();
            stack.push(V::I(a.wrapping_neg()));
        }
        op::LNEG => {
            let a = pop(stack).j();
            stack.push(V::J(a.wrapping_neg()));
        }
        op::FNEG => {
            let a = pop(stack).f();
            stack.push(V::F(-a));
        }
        op::DNEG => {
            let a = pop(stack).d();
            stack.push(V::D(-a));
        }
        op::ISHL => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a.wrapping_shl((b & 0x1f) as u32)));
        }
        op::LSHL => {
            let (b, a) = (pop(stack).i(), pop(stack).j());
            stack.push(V::J(a.wrapping_shl((b & 0x3f) as u32)));
        }
        op::ISHR => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a.wrapping_shr((b & 0x1f) as u32)));
        }
        op::LSHR => {
            let (b, a) = (pop(stack).i(), pop(stack).j());
            stack.push(V::J(a.wrapping_shr((b & 0x3f) as u32)));
        }
        op::IUSHR => {
            let (b, a) = (pop(stack).i(), pop(stack).i() as u32);
            stack.push(V::I(a.wrapping_shr((b & 0x1f) as u32) as i32));
        }
        op::LUSHR => {
            let (b, a) = (pop(stack).i(), pop(stack).j() as u64);
            stack.push(V::J(a.wrapping_shr((b & 0x3f) as u32) as i64));
        }
        op::IAND => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a & b));
        }
        op::LAND => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::J(a & b));
        }
        op::IOR => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a | b));
        }
        op::LOR => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::J(a | b));
        }
        op::IXOR => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            stack.push(V::I(a ^ b));
        }
        op::LXOR => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::J(a ^ b));
        }
        op::IINC => {
            if let Operand::Iinc(slot, delta) = insn.arg {
                let s = slot as usize;
                ensure_local(locals, s);
                locals[s] = V::I(locals[s].i().wrapping_add(delta));
            }
        }

        // Numeric conversions
        op::I2L => {
            let v = pop(stack).i();
            stack.push(V::J(i64::from(v)));
        }
        op::I2F => {
            let v = pop(stack).i();
            stack.push(V::F(v as f32));
        }
        op::I2D => {
            let v = pop(stack).i();
            stack.push(V::D(f64::from(v)));
        }
        op::L2I => {
            let v = pop(stack).j();
            stack.push(V::I(v as i32));
        }
        op::L2F => {
            let v = pop(stack).j();
            stack.push(V::F(v as f32));
        }
        op::L2D => {
            let v = pop(stack).j();
            stack.push(V::D(v as f64));
        }
        op::F2I => {
            let v = pop(stack).f();
            stack.push(V::I(v as i32));
        }
        op::F2L => {
            let v = pop(stack).f();
            stack.push(V::J(v as i64));
        }
        op::F2D => {
            let v = pop(stack).f();
            stack.push(V::D(f64::from(v)));
        }
        op::D2I => {
            let v = pop(stack).d();
            stack.push(V::I(v as i32));
        }
        op::D2L => {
            let v = pop(stack).d();
            stack.push(V::J(v as i64));
        }
        op::D2F => {
            let v = pop(stack).d();
            stack.push(V::F(v as f32));
        }
        op::I2B => {
            let v = pop(stack).i();
            stack.push(V::I(i32::from(v as i8)));
        }
        op::I2C => {
            let v = pop(stack).i();
            stack.push(V::I(v & 0xFFFF));
        }
        op::I2S => {
            let v = pop(stack).i();
            stack.push(V::I(i32::from(v as i16)));
        }

        // Comparisons
        op::LCMP => {
            let (b, a) = (pop(stack).j(), pop(stack).j());
            stack.push(V::I(a.cmp(&b) as i32));
        }
        op::FCMPL | op::FCMPG => {
            let (b, a) = (pop(stack).f(), pop(stack).f());
            let r = if a.is_nan() || b.is_nan() {
                if insn.op == op::FCMPG {
                    1
                } else {
                    -1
                }
            } else {
                a.partial_cmp(&b).unwrap() as i32
            };
            stack.push(V::I(r));
        }
        op::DCMPL | op::DCMPG => {
            let (b, a) = (pop(stack).d(), pop(stack).d());
            let r = if a.is_nan() || b.is_nan() {
                if insn.op == op::DCMPG {
                    1
                } else {
                    -1
                }
            } else {
                a.partial_cmp(&b).unwrap() as i32
            };
            stack.push(V::I(r));
        }

        // Branches & Switches
        op::IFEQ..=op::IFLE => {
            let v = pop(stack).i();
            let take = match insn.op {
                op::IFEQ => v == 0,
                op::IFNE => v != 0,
                op::IFLT => v < 0,
                op::IFGE => v >= 0,
                op::IFGT => v > 0,
                op::IFLE => v <= 0,
                _ => false,
            };
            if take {
                if let Operand::Jump(tgt) = insn.arg {
                    *next_idx = pc_to_idx[tgt];
                }
            }
        }
        op::IF_ICMPEQ..=op::IF_ICMPLE => {
            let (b, a) = (pop(stack).i(), pop(stack).i());
            let take = match insn.op {
                op::IF_ICMPEQ => a == b,
                op::IF_ICMPNE => a != b,
                op::IF_ICMPLT => a < b,
                op::IF_ICMPGE => a >= b,
                op::IF_ICMPGT => a > b,
                op::IF_ICMPLE => a <= b,
                _ => false,
            };
            if take {
                if let Operand::Jump(tgt) = insn.arg {
                    *next_idx = pc_to_idx[tgt];
                }
            }
        }
        op::IF_ACMPEQ | op::IF_ACMPNE => {
            let (b, a) = (pop(stack), pop(stack));
            let eq = a.r() == b.r();
            let take = if insn.op == op::IF_ACMPEQ { eq } else { !eq };
            if take {
                if let Operand::Jump(tgt) = insn.arg {
                    *next_idx = pc_to_idx[tgt];
                }
            }
        }
        op::IFNULL | op::IFNONNULL => {
            let a = pop(stack);
            let take = if insn.op == op::IFNULL {
                a.is_null()
            } else {
                !a.is_null()
            };
            if take {
                if let Operand::Jump(tgt) = insn.arg {
                    *next_idx = pc_to_idx[tgt];
                }
            }
        }
        op::GOTO | op::GOTO_W => {
            if let Operand::Jump(tgt) = insn.arg {
                *next_idx = pc_to_idx[tgt];
            }
        }
        op::TABLESWITCH | op::LOOKUPSWITCH => {
            let key = pop(stack).i();
            if let Operand::Switch { default, pairs } = &insn.arg {
                let target = pairs
                    .iter()
                    .find(|&&(k, _)| k == key)
                    .map_or(*default, |&(_, t)| t);
                *next_idx = pc_to_idx[target];
            }
        }

        // Returns
        op::IRETURN..=op::ARETURN => {
            *next_idx = usize::MAX;
        }
        op::RETURN => {
            stack.push(V::Void);
            *next_idx = usize::MAX;
        }

        // Field accesses
        op::GETSTATIC => {
            if let Some(m) = insn.cp_index().and_then(|i| cf.cp_member(i)) {
                let v = read_static_field(ctx, &m.owner, &m.name, &m.desc);
                stack.push(v);
            }
        }
        op::PUTSTATIC => {
            let val = pop(stack);
            if let Some(m) = insn.cp_index().and_then(|i| cf.cp_member(i)) {
                write_static_field(ctx, &m.owner, &m.name, &m.desc, val);
            }
        }
        op::GETFIELD => {
            let recv = pop(stack);
            if let Some(m) = insn.cp_index().and_then(|i| cf.cp_member(i)) {
                let v = read_instance_field(ctx, recv, &m.owner, &m.name, &m.desc);
                stack.push(v);
            }
        }
        op::PUTFIELD => {
            let val = pop(stack);
            let recv = pop(stack);
            if let Some(m) = insn.cp_index().and_then(|i| cf.cp_member(i)) {
                write_instance_field(ctx, recv, &m.name, val);
            }
        }

        // Method invocations (with Mixin `@Inject(INVOKE)`, `@Redirect`, `@ModifyArg` weaving!)
        op::INVOKEVIRTUAL | op::INVOKESPECIAL | op::INVOKESTATIC | op::INVOKEINTERFACE => {
            let Some(m) = insn.cp_index().and_then(|i| cf.cp_member(i)) else {
                return;
            };
            let (p_types, call_ret) =
                parse_method_descriptor(&m.desc).unwrap_or((Vec::new(), DescriptorType::Void));
            let argc = p_types.len() + usize::from(insn.op != op::INVOKESTATIC);
            let mut call_args = vec![V::Void; argc];
            for k in (0..argc).rev() {
                call_args[k] = pop(stack);
            }

            let cur_ordinal = {
                let e = invoke_ordinals.entry(m.key()).or_insert(0);
                let o = *e;
                *e += 1;
                o
            };

            let mut redirected: Option<V> = None;
            let mut after_inject_hooks: Vec<&PlannedHook> = Vec::new();

            if !hooks.is_empty() {
                for hook in hooks {
                    let InjectionPoint::Invoke {
                        target,
                        after,
                        ordinal,
                    } = &hook.point
                    else {
                        continue;
                    };
                    if target != &m || ordinal.is_some_and(|want| want != cur_ordinal) {
                        continue;
                    }
                    match hook.kind {
                        InjectorKind::Inject { cancellable } if !*after => {
                            if let Some(early) = run_mixin_inject_hook(
                                ctx,
                                declaring_class,
                                target_method,
                                ret_type,
                                hook,
                                method_args,
                                None,
                                cancellable,
                            ) {
                                stack.clear();
                                stack.push(early);
                                *next_idx = usize::MAX;
                                return;
                            }
                        }
                        InjectorKind::Inject { .. } if *after => {
                            after_inject_hooks.push(hook);
                        }
                        InjectorKind::ModifyArg { index } => {
                            let arg_slot = index + usize::from(insn.op != op::INVOKESTATIC);
                            if arg_slot < call_args.len() {
                                let mut h_args = Vec::new();
                                if !hook.handler_is_static {
                                    h_args.push(method_args[0]);
                                }
                                h_args.push(call_args[arg_slot]);
                                let hm = MemberRef::new(
                                    declaring_class,
                                    &hook.handler_name,
                                    &hook.handler_desc,
                                );
                                let hop = if hook.handler_is_static {
                                    op::INVOKESTATIC
                                } else {
                                    op::INVOKEVIRTUAL
                                };
                                call_args[arg_slot] = invoke_member(ctx, hop, &hm, &h_args);
                            }
                        }
                        InjectorKind::Redirect => {
                            let mut h_args = Vec::new();
                            if !hook.handler_is_static {
                                h_args.push(method_args[0]);
                            }
                            h_args.extend_from_slice(&call_args);
                            let hm = MemberRef::new(
                                declaring_class,
                                &hook.handler_name,
                                &hook.handler_desc,
                            );
                            let hop = if hook.handler_is_static {
                                op::INVOKESTATIC
                            } else {
                                op::INVOKEVIRTUAL
                            };
                            redirected = Some(invoke_member(ctx, hop, &hm, &h_args));
                        }
                        _ => {}
                    }
                }
            }

            let res = match redirected {
                Some(r) => r,
                None => invoke_member(ctx, insn.op, &m, &call_args),
            };
            if call_ret != DescriptorType::Void {
                stack.push(res);
            }

            for hook in after_inject_hooks {
                if let InjectorKind::Inject { cancellable } = hook.kind {
                    if let Some(early) = run_mixin_inject_hook(
                        ctx,
                        declaring_class,
                        target_method,
                        ret_type,
                        hook,
                        method_args,
                        None,
                        cancellable,
                    ) {
                        stack.clear();
                        stack.push(early);
                        *next_idx = usize::MAX;
                        return;
                    }
                }
            }
        }

        op::INVOKEDYNAMIC => {
            let Some(cp_idx) = insn.cp_index() else {
                return;
            };
            let Some(indy) = cf.resolve_indy(cp_idx) else {
                return;
            };
            let (p_types, call_ret) =
                parse_method_descriptor(&indy.desc).unwrap_or((Vec::new(), DescriptorType::Void));
            let argc = p_types.len();
            let mut dyn_args = vec![V::Void; argc];
            for k in (0..argc).rev() {
                dyn_args[k] = pop(stack);
            }
            let res = execute_invokedynamic(ctx, &cf.name, &indy, &dyn_args);
            if call_ret != DescriptorType::Void {
                stack.push(res);
            }
        }

        // Object & Array allocation, casts, `athrow`
        op::NEW => {
            if let Some(cp) = insn.cp_index() {
                if let Ok(cname) = cf.cp_class_name(cp) {
                    let obj = alloc_new_object(ctx, &cname);
                    stack.push(obj);
                }
            }
        }
        op::NEWARRAY => {
            let count = pop(stack).i().max(0) as usize;
            let (arr_cls, def_val) = match insn.arg {
                Operand::Int(4) => ("[Z", V::I(0)),
                Operand::Int(5) => ("[C", V::I(0)),
                Operand::Int(6) => ("[F", V::F(0.0)),
                Operand::Int(7) => ("[D", V::D(0.0)),
                Operand::Int(8) => ("[B", V::I(0)),
                Operand::Int(9) => ("[S", V::I(0)),
                Operand::Int(10) => ("[I", V::I(0)),
                Operand::Int(11) => ("[J", V::J(0)),
                _ => ("[I", V::I(0)),
            };
            let arr = ctx.rt.alloc(arr_cls, 0, Data::Array(vec![def_val; count]));
            stack.push(arr);
        }
        op::ANEWARRAY => {
            let count = pop(stack).i().max(0) as usize;
            let elem_cls = insn
                .cp_index()
                .and_then(|cp| cf.cp_class_name(cp).ok())
                .unwrap_or_else(|| "java/lang/Object".into());
            let arr_cls = if elem_cls.starts_with('[') {
                format!("[{elem_cls}")
            } else {
                format!("[L{elem_cls};")
            };
            let arr = ctx.rt.alloc(arr_cls, 0, Data::Array(vec![V::NULL; count]));
            stack.push(arr);
        }
        op::MULTIANEWARRAY => {
            if let Operand::IndexCount(cp, dims) = insn.arg {
                let arr_cls = cf
                    .cp_class_name(cp)
                    .unwrap_or_else(|_| "[[Ljava/lang/Object;".into());
                let mut sizes = vec![0usize; dims as usize];
                for d in (0..dims as usize).rev() {
                    sizes[d] = pop(stack).i().max(0) as usize;
                }
                let arr = alloc_multi_array(ctx.rt, &arr_cls, &sizes);
                stack.push(arr);
            }
        }
        op::ARRAYLENGTH => {
            let arr = pop(stack);
            if arr.is_null() {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/NullPointerException", "arraylength on null");
            } else {
                let len = match ctx.rt.get_obj(arr).map(|o| &o.data) {
                    Some(Data::Array(v)) => v.len() as i32,
                    _ => 0,
                };
                stack.push(V::I(len));
            }
        }
        op::ATHROW => {
            let exc = pop(stack);
            if exc.is_null() {
                let _ = ctx
                    .rt
                    .throw_exception("java/lang/NullPointerException", "Cannot throw null");
            } else {
                ctx.rt.pending_exception = Some(exc.r());
            }
        }
        op::CHECKCAST => {
            let top = stack.last().copied().unwrap_or(V::NULL);
            if !top.is_null() {
                if let Some(target_cls) = insn.cp_index().and_then(|cp| cf.cp_class_name(cp).ok()) {
                    let clean_target = target_cls
                        .strip_prefix('L')
                        .and_then(|s| s.strip_suffix(';'))
                        .unwrap_or(&target_cls);
                    // Unbox if casting boxed primitive or keep reference; verify subtype when known
                    if let Some(actual_cls) = ctx.rt.class_of(top).map(str::to_owned) {
                        // Closures always satisfy functional interfaces; unknown external classes pass if host matches
                        let is_closure =
                            matches!(ctx.rt.get_obj(top).map(|o| &o.data), Some(Data::Closure(_)));
                        if !is_closure
                            && ctx.linked.has_class(&actual_cls)
                            && ctx.linked.has_class(clean_target)
                            && !ctx.linked.is_subtype(&actual_cls, clean_target, ctx.host)
                        {
                            let _ = ctx.rt.throw_exception(
                                "java/lang/ClassCastException",
                                &format!("{actual_cls} cannot be cast to {clean_target}"),
                            );
                        }
                    }
                }
            }
        }
        op::INSTANCEOF => {
            let obj = pop(stack);
            if obj.is_null() {
                stack.push(V::FALSE);
            } else if let Some(target_cls) =
                insn.cp_index().and_then(|cp| cf.cp_class_name(cp).ok())
            {
                let clean_target = target_cls
                    .strip_prefix('L')
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or(&target_cls);
                let actual = ctx.rt.class_of(obj).unwrap_or("java/lang/Object");
                let ok = ctx.linked.is_subtype(actual, clean_target, ctx.host);
                stack.push(V::bool(ok));
            } else {
                stack.push(V::FALSE);
            }
        }
        _ => {
            let _ = parse_field_descriptor("V");
        }
    }
}

fn alloc_multi_array(rt: &mut Rt, desc: &str, sizes: &[usize]) -> V {
    let n = sizes.first().copied().unwrap_or(0);
    if sizes.len() <= 1 {
        let elem_desc = desc.strip_prefix('[').unwrap_or("I");
        let def = default_value_for_desc(elem_desc);
        return rt.alloc(desc, 0, Data::Array(vec![def; n]));
    }
    let sub_desc = desc.strip_prefix('[').unwrap_or("[I");
    let mut outer = Vec::with_capacity(n);
    for _ in 0..n {
        outer.push(alloc_multi_array(rt, sub_desc, &sizes[1..]));
    }
    rt.alloc(desc, 0, Data::Array(outer))
}

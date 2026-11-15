//! Complete JVMS Chapter 4 `.class` file parser: constant pool, fields, methods,
//! `Code` attributes, exception handlers, generic signatures, `BootstrapMethods`,
//! runtime visible & invisible annotations, and Mixin constructor initialiser slicing.

use crate::insn::{decode_instructions, op, Operand};
use crate::mutf8::decode_mutf8;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// JVM access flags (JVMS Table 4.1-B, 4.5-A, 4.6-A).
pub mod access {
    pub const PUBLIC: u16 = 0x0001;
    pub const PRIVATE: u16 = 0x0002;
    pub const PROTECTED: u16 = 0x0004;
    pub const STATIC: u16 = 0x0008;
    pub const FINAL: u16 = 0x0010;
    pub const SYNCHRONIZED: u16 = 0x0020;
    pub const BRIDGE: u16 = 0x0040;
    pub const VARARGS: u16 = 0x0080;
    pub const NATIVE: u16 = 0x0100;
    pub const INTERFACE: u16 = 0x0200;
    pub const ABSTRACT: u16 = 0x0400;
    pub const SYNTHETIC: u16 = 0x1000;
    pub const ANNOTATION: u16 = 0x2000;
    pub const ENUM: u16 = 0x4000;
    pub const RECORD: u16 = 0x0010; // paired with `Record` attribute or `java/lang/Record` superclass
}

/// Constant pool entry (JVMS §4.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Const {
    Unused,
    Utf8(String),
    Int(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class(u16),
    String(u16),
    FieldRef(u16, u16),
    MethodRef(u16, u16),
    InterfaceMethodRef(u16, u16),
    NameAndType(u16, u16),
    MethodHandle(u8, u16),
    MethodType(u16),
    Dynamic(u16, u16),
    InvokeDynamic(u16, u16),
    Module(u16),
    Package(u16),
}

/// Resolved field, method, or interface-method reference `(owner, name, descriptor)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemberRef {
    pub owner: String,
    pub name: String,
    pub desc: String,
}

impl MemberRef {
    pub fn new(owner: impl Into<String>, name: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
            desc: desc.into(),
        }
    }

    /// Canonical lookup key `"owner.name.desc"`.
    pub fn key(&self) -> String {
        format!("{}.{}.{}", self.owner, self.name, self.desc)
    }

    /// Method key within a class `"name.desc"`.
    pub fn member_key(&self) -> String {
        format!("{}.{}", self.name, self.desc)
    }
}

/// Decoded `BootstrapMethods` entry for `invokedynamic` (lambdas, method refs, string concat, ObjectMethods).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapMethod {
    pub handle_index: u16,
    pub args: Vec<u16>,
}

/// Decoded `invokedynamic` call site with resolved bootstrap handle and static arguments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvokeDynamicSite {
    pub name: String,
    pub desc: String,
    pub bootstrap_handle: Option<(u8, MemberRef)>,
    pub bootstrap_args: Vec<BootstrapArg>,
}

/// Resolved static argument to a bootstrap method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BootstrapArg {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    Class(String),
    MethodType(String),
    MethodHandle(u8, MemberRef),
    Unknown(u16),
}

/// Typed annotation element value (JVMS §4.7.16.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ElementValue {
    Byte(i8),
    Char(u16),
    Double(f64),
    Float(f32),
    Int(i32),
    Long(i64),
    Short(i16),
    Boolean(bool),
    String(String),
    Enum { desc: String, const_name: String },
    Class(String),
    Annotation(Annotation),
    Array(Vec<ElementValue>),
}

impl ElementValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) | Self::Class(s) => Some(s.as_str()),
            Self::Enum { const_name, .. } => Some(const_name.as_str()),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            Self::Int(i) => Some(*i != 0),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Short(v) => Some(i32::from(*v)),
            Self::Byte(v) => Some(i32::from(*v)),
            _ => None,
        }
    }

    /// Collects string/class/enum items whether stored as a single value or an array.
    pub fn to_string_list(&self) -> Vec<String> {
        match self {
            Self::String(s) | Self::Class(s) => vec![s.clone()],
            Self::Enum { const_name, .. } => vec![const_name.clone()],
            Self::Array(items) => items.iter().flat_map(Self::to_string_list).collect(),
            _ => Vec::new(),
        }
    }

    /// Collects nested annotations whether stored as a single annotation or an array.
    pub fn to_annotations(&self) -> Vec<&Annotation> {
        match self {
            Self::Annotation(a) => vec![a],
            Self::Array(items) => items.iter().flat_map(Self::to_annotations).collect(),
            _ => Vec::new(),
        }
    }
}

/// Parsed class, field, method, or parameter annotation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Annotation {
    /// Descriptor, e.g. `"Lnet/neoforged/fml/common/Mod;"`.
    pub desc: String,
    pub values: BTreeMap<String, ElementValue>,
    pub runtime_visible: bool,
}

impl Annotation {
    pub fn get(&self, name: &str) -> Option<&ElementValue> {
        self.values.get(name)
    }

    pub fn get_str(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(ElementValue::as_str)
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.values.get(name).and_then(ElementValue::as_bool)
    }

    pub fn get_i32(&self, name: &str) -> Option<i32> {
        self.values.get(name).and_then(ElementValue::as_i32)
    }
}

/// Exception handler entry in a `Code` attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionHandler {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    /// Internal class name (`"java/lang/Exception"`), or `None` for catch-all (`finally`).
    pub catch_type: Option<String>,
}

/// Local variable debug info from `LocalVariableTable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalVarEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name: String,
    pub desc: String,
    pub slot: u16,
}

/// Method `Code` attribute.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CodeAttr {
    pub max_stack: u16,
    pub max_locals: u16,
    pub bytes: Vec<u8>,
    pub handlers: Vec<ExceptionHandler>,
    pub locals: Vec<LocalVarEntry>,
    pub lines: Vec<(u16, u16)>,
    pub stack_map_offsets: Vec<u16>,
}

/// Constant value attached to a `static final` field (`ConstantValue` attribute).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConstantValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
}

/// Parsed field declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldInfo {
    pub access: u16,
    pub name: String,
    pub desc: String,
    pub signature: Option<String>,
    pub constant_value: Option<ConstantValue>,
    pub annotations: Vec<Annotation>,
}

impl FieldInfo {
    pub fn is_static(&self) -> bool {
        self.access & access::STATIC != 0
    }

    pub fn has_annotation(&self, desc: &str) -> bool {
        self.annotations.iter().any(|a| a.desc == desc)
    }
}

/// Parsed method declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodInfo {
    pub access: u16,
    pub name: String,
    pub desc: String,
    pub signature: Option<String>,
    pub code: Option<CodeAttr>,
    pub annotations: Vec<Annotation>,
    pub param_annotations: Vec<Vec<Annotation>>,
    pub exceptions: Vec<String>,
}

impl MethodInfo {
    pub fn is_static(&self) -> bool {
        self.access & access::STATIC != 0
    }

    pub fn is_private(&self) -> bool {
        self.access & access::PRIVATE != 0
    }

    pub fn is_abstract(&self) -> bool {
        self.access & access::ABSTRACT != 0
    }

    pub fn is_native(&self) -> bool {
        self.access & access::NATIVE != 0
    }

    pub fn member_key(&self) -> String {
        format!("{}.{}", self.name, self.desc)
    }

    pub fn annotation(&self, desc: &str) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.desc == desc)
    }
}

/// Complete parsed JVM `.class` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassFile {
    pub major_version: u16,
    pub minor_version: u16,
    pub pool: Vec<Const>,
    pub access: u16,
    pub name: String,
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub annotations: Vec<Annotation>,
    pub bootstrap_methods: Vec<BootstrapMethod>,
    pub signature: Option<String>,
    pub record_components: Vec<(String, String)>,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self
            .bytes
            .get(self.pos..self.pos + n)
            .ok_or_else(|| format!("truncated class file at byte {}", self.pos))?;
        self.pos += n;
        Ok(s)
    }

    fn u1(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u2(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }

    fn u4(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
}

fn parse_stack_map_offsets(body: &[u8]) -> Result<Vec<u16>, String> {
    let mut r = Cursor {
        bytes: body,
        pos: 0,
    };
    let skip_vtype = |r: &mut Cursor| -> Result<(), String> {
        if matches!(r.u1()?, 7 | 8) {
            r.u2()?;
        }
        Ok(())
    };
    let count = r.u2()?;
    let mut out = Vec::with_capacity(count as usize);
    let mut prev: Option<u16> = None;
    for _ in 0..count {
        let frame_type = r.u1()?;
        let delta = match frame_type {
            0..=63 => u16::from(frame_type),
            64..=127 => {
                skip_vtype(&mut r)?;
                u16::from(frame_type - 64)
            }
            247 => {
                let d = r.u2()?;
                skip_vtype(&mut r)?;
                d
            }
            248..=251 => r.u2()?,
            252..=254 => {
                let d = r.u2()?;
                for _ in 0..(frame_type - 251) {
                    skip_vtype(&mut r)?;
                }
                d
            }
            255 => {
                let d = r.u2()?;
                let nlocals = r.u2()?;
                for _ in 0..nlocals {
                    skip_vtype(&mut r)?;
                }
                let nstack = r.u2()?;
                for _ in 0..nstack {
                    skip_vtype(&mut r)?;
                }
                d
            }
            other => return Err(format!("reserved StackMapTable frame type {other}")),
        };
        let pc = prev.map_or(delta, |p| p.wrapping_add(delta).wrapping_add(1));
        out.push(pc);
        prev = Some(pc);
    }
    Ok(out)
}

impl ClassFile {
    /// Parses a binary `.class` file.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Cursor { bytes, pos: 0 };
        if r.u4()? != 0xCAFE_BABE {
            return Err("invalid classfile magic (expected 0xCAFEBABE)".into());
        }
        let minor_version = r.u2()?;
        let major_version = r.u2()?;
        let cp_count = r.u2()? as usize;
        let mut pool = Vec::with_capacity(cp_count);
        pool.push(Const::Unused);

        while pool.len() < cp_count {
            let tag = r.u1()?;
            let entry = match tag {
                1 => {
                    let len = r.u2()? as usize;
                    Const::Utf8(decode_mutf8(r.take(len)?))
                }
                3 => Const::Int(r.u4()? as i32),
                4 => Const::Float(f32::from_bits(r.u4()?)),
                5 => {
                    let hi = u64::from(r.u4()?);
                    let lo = u64::from(r.u4()?);
                    Const::Long(((hi << 32) | lo) as i64)
                }
                6 => {
                    let hi = u64::from(r.u4()?);
                    let lo = u64::from(r.u4()?);
                    Const::Double(f64::from_bits((hi << 32) | lo))
                }
                7 => Const::Class(r.u2()?),
                8 => Const::String(r.u2()?),
                9 => Const::FieldRef(r.u2()?, r.u2()?),
                10 => Const::MethodRef(r.u2()?, r.u2()?),
                11 => Const::InterfaceMethodRef(r.u2()?, r.u2()?),
                12 => Const::NameAndType(r.u2()?, r.u2()?),
                15 => Const::MethodHandle(r.u1()?, r.u2()?),
                16 => Const::MethodType(r.u2()?),
                17 => Const::Dynamic(r.u2()?, r.u2()?),
                18 => Const::InvokeDynamic(r.u2()?, r.u2()?),
                19 => Const::Module(r.u2()?),
                20 => Const::Package(r.u2()?),
                other => return Err(format!("unknown constant-pool tag {other}")),
            };
            let wide = matches!(entry, Const::Long(_) | Const::Double(_));
            pool.push(entry);
            if wide {
                pool.push(Const::Unused);
            }
        }

        let mut cf = ClassFile {
            major_version,
            minor_version,
            pool,
            access: 0,
            name: String::new(),
            superclass: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
            annotations: Vec::new(),
            bootstrap_methods: Vec::new(),
            signature: None,
            record_components: Vec::new(),
        };

        cf.access = r.u2()?;
        cf.name = cf.cp_class_name(r.u2()?)?;
        let super_idx = r.u2()?;
        cf.superclass = if super_idx == 0 {
            None
        } else {
            Some(cf.cp_class_name(super_idx)?)
        };

        let iface_count = r.u2()?;
        for _ in 0..iface_count {
            let idx = r.u2()?;
            cf.interfaces.push(cf.cp_class_name(idx)?);
        }

        let field_count = r.u2()?;
        for _ in 0..field_count {
            let access = r.u2()?;
            let name = cf.cp_utf8(r.u2()?)?;
            let desc = cf.cp_utf8(r.u2()?)?;
            let attr_count = r.u2()?;
            let mut signature = None;
            let mut constant_value = None;
            let mut annotations = Vec::new();
            for _ in 0..attr_count {
                let attr_name = cf.cp_utf8(r.u2()?)?;
                let attr_len = r.u4()? as usize;
                let body = r.take(attr_len)?;
                match attr_name.as_str() {
                    "RuntimeVisibleAnnotations" => {
                        annotations.extend(cf.parse_annotations(body, true)?);
                    }
                    "RuntimeInvisibleAnnotations" => {
                        annotations.extend(cf.parse_annotations(body, false)?);
                    }
                    "Signature" if body.len() == 2 => {
                        signature = Some(cf.cp_utf8(u16::from_be_bytes([body[0], body[1]]))?);
                    }
                    "ConstantValue" if body.len() == 2 => {
                        let cv_idx = u16::from_be_bytes([body[0], body[1]]) as usize;
                        constant_value = match cf.pool.get(cv_idx) {
                            Some(Const::Int(v)) => Some(ConstantValue::Int(*v)),
                            Some(Const::Long(v)) => Some(ConstantValue::Long(*v)),
                            Some(Const::Float(v)) => Some(ConstantValue::Float(*v)),
                            Some(Const::Double(v)) => Some(ConstantValue::Double(*v)),
                            Some(Const::String(s)) => {
                                Some(ConstantValue::String(cf.cp_utf8(*s)?))
                            }
                            _ => None,
                        };
                    }
                    _ => {}
                }
            }
            cf.fields.push(FieldInfo {
                access,
                name,
                desc,
                signature,
                constant_value,
                annotations,
            });
        }

        let method_count = r.u2()?;
        for _ in 0..method_count {
            let access = r.u2()?;
            let name = cf.cp_utf8(r.u2()?)?;
            let desc = cf.cp_utf8(r.u2()?)?;
            let attr_count = r.u2()?;
            let mut signature = None;
            let mut code = None;
            let mut annotations = Vec::new();
            let mut param_annotations: Vec<Vec<Annotation>> = Vec::new();
            let mut exceptions = Vec::new();

            for _ in 0..attr_count {
                let attr_name = cf.cp_utf8(r.u2()?)?;
                let attr_len = r.u4()? as usize;
                let body = r.take(attr_len)?;
                match attr_name.as_str() {
                    "RuntimeVisibleAnnotations" => {
                        annotations.extend(cf.parse_annotations(body, true)?);
                    }
                    "RuntimeInvisibleAnnotations" => {
                        annotations.extend(cf.parse_annotations(body, false)?);
                    }
                    "RuntimeVisibleParameterAnnotations"
                    | "RuntimeInvisibleParameterAnnotations" => {
                        let visible = attr_name == "RuntimeVisibleParameterAnnotations";
                        let mut pr = Cursor {
                            bytes: body,
                            pos: 0,
                        };
                        let num_params = pr.u1()? as usize;
                        if param_annotations.len() < num_params {
                            param_annotations.resize(num_params, Vec::new());
                        }
                        for slot in param_annotations.iter_mut().take(num_params) {
                            let n = pr.u2()?;
                            for _ in 0..n {
                                slot.push(cf.parse_annotation(&mut pr, visible)?);
                            }
                        }
                    }
                    "Signature" if body.len() == 2 => {
                        signature = Some(cf.cp_utf8(u16::from_be_bytes([body[0], body[1]]))?);
                    }
                    "Exceptions" => {
                        let mut er = Cursor {
                            bytes: body,
                            pos: 0,
                        };
                        let n = er.u2()?;
                        for _ in 0..n {
                            exceptions.push(cf.cp_class_name(er.u2()?)?);
                        }
                    }
                    "Code" => {
                        let mut cr = Cursor {
                            bytes: body,
                            pos: 0,
                        };
                        let max_stack = cr.u2()?;
                        let max_locals = cr.u2()?;
                        let code_len = cr.u4()? as usize;
                        let code_bytes = cr.take(code_len)?.to_vec();
                        let exc_count = cr.u2()?;
                        let mut handlers = Vec::with_capacity(exc_count as usize);
                        for _ in 0..exc_count {
                            let start_pc = cr.u2()?;
                            let end_pc = cr.u2()?;
                            let handler_pc = cr.u2()?;
                            let catch_idx = cr.u2()?;
                            let catch_type = if catch_idx == 0 {
                                None
                            } else {
                                Some(cf.cp_class_name(catch_idx)?)
                            };
                            handlers.push(ExceptionHandler {
                                start_pc,
                                end_pc,
                                handler_pc,
                                catch_type,
                            });
                        }
                        let sub_attrs = cr.u2()?;
                        let mut locals = Vec::new();
                        let mut lines = Vec::new();
                        let mut stack_map_offsets = Vec::new();
                        for _ in 0..sub_attrs {
                            let sub_name = cf.cp_utf8(cr.u2()?)?;
                            let sub_len = cr.u4()? as usize;
                            let sub_body = cr.take(sub_len)?;
                            match sub_name.as_str() {
                                "LocalVariableTable" => {
                                    let mut lr = Cursor {
                                        bytes: sub_body,
                                        pos: 0,
                                    };
                                    let n = lr.u2()?;
                                    for _ in 0..n {
                                        let start_pc = lr.u2()?;
                                        let length = lr.u2()?;
                                        let lv_name = cf.cp_utf8(lr.u2()?)?;
                                        let lv_desc = cf.cp_utf8(lr.u2()?)?;
                                        let slot = lr.u2()?;
                                        locals.push(LocalVarEntry {
                                            start_pc,
                                            length,
                                            name: lv_name,
                                            desc: lv_desc,
                                            slot,
                                        });
                                    }
                                }
                                "LineNumberTable" => {
                                    let mut lr = Cursor {
                                        bytes: sub_body,
                                        pos: 0,
                                    };
                                    let n = lr.u2()?;
                                    for _ in 0..n {
                                        lines.push((lr.u2()?, lr.u2()?));
                                    }
                                }
                                "StackMapTable" => {
                                    stack_map_offsets =
                                        parse_stack_map_offsets(sub_body).unwrap_or_default();
                                }
                                _ => {}
                            }
                        }
                        code = Some(CodeAttr {
                            max_stack,
                            max_locals,
                            bytes: code_bytes,
                            handlers,
                            locals,
                            lines,
                            stack_map_offsets,
                        });
                    }
                    _ => {}
                }
            }

            cf.methods.push(MethodInfo {
                access,
                name,
                desc,
                signature,
                code,
                annotations,
                param_annotations,
                exceptions,
            });
        }

        let class_attrs = r.u2()?;
        for _ in 0..class_attrs {
            let attr_name = cf.cp_utf8(r.u2()?)?;
            let attr_len = r.u4()? as usize;
            let body = r.take(attr_len)?;
            match attr_name.as_str() {
                "RuntimeVisibleAnnotations" => {
                    cf.annotations.extend(cf.parse_annotations(body, true)?);
                }
                "RuntimeInvisibleAnnotations" => {
                    cf.annotations.extend(cf.parse_annotations(body, false)?);
                }
                "Signature" if body.len() == 2 => {
                    cf.signature = Some(cf.cp_utf8(u16::from_be_bytes([body[0], body[1]]))?);
                }
                "BootstrapMethods" => {
                    let mut br = Cursor {
                        bytes: body,
                        pos: 0,
                    };
                    let n = br.u2()?;
                    for _ in 0..n {
                        let handle_index = br.u2()?;
                        let argc = br.u2()?;
                        let mut args = Vec::with_capacity(argc as usize);
                        for _ in 0..argc {
                            args.push(br.u2()?);
                        }
                        cf.bootstrap_methods.push(BootstrapMethod {
                            handle_index,
                            args,
                        });
                    }
                }
                "Record" => {
                    let mut rr = Cursor {
                        bytes: body,
                        pos: 0,
                    };
                    let n = rr.u2()?;
                    for _ in 0..n {
                        let rc_name = cf.cp_utf8(rr.u2()?)?;
                        let rc_desc = cf.cp_utf8(rr.u2()?)?;
                        let sub_n = rr.u2()?;
                        for _ in 0..sub_n {
                            let _ = rr.u2()?;
                            let sub_len = rr.u4()? as usize;
                            let _ = rr.take(sub_len)?;
                        }
                        cf.record_components.push((rc_name, rc_desc));
                    }
                }
                _ => {}
            }
        }

        if cf.is_mixin() {
            cf.strip_mixin_constructor_bodies();
        }

        Ok(cf)
    }

    fn parse_annotations(&self, body: &[u8], visible: bool) -> Result<Vec<Annotation>, String> {
        let mut r = Cursor {
            bytes: body,
            pos: 0,
        };
        let n = r.u2()?;
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            out.push(self.parse_annotation(&mut r, visible)?);
        }
        Ok(out)
    }

    fn parse_annotation(&self, r: &mut Cursor<'_>, visible: bool) -> Result<Annotation, String> {
        let desc = self.cp_utf8(r.u2()?)?;
        let pairs = r.u2()?;
        let mut values = BTreeMap::new();
        for _ in 0..pairs {
            let name = self.cp_utf8(r.u2()?)?;
            let val = self.parse_element_value(r)?;
            values.insert(name, val);
        }
        Ok(Annotation {
            desc,
            values,
            runtime_visible: visible,
        })
    }

    fn parse_element_value(&self, r: &mut Cursor<'_>) -> Result<ElementValue, String> {
        let tag = r.u1()?;
        Ok(match tag {
            b'B' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Int(v)) => ElementValue::Byte(*v as i8),
                _ => ElementValue::Byte(0),
            },
            b'C' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Int(v)) => ElementValue::Char(*v as u16),
                _ => ElementValue::Char(0),
            },
            b'D' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Double(v)) => ElementValue::Double(*v),
                _ => ElementValue::Double(0.0),
            },
            b'F' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Float(v)) => ElementValue::Float(*v),
                _ => ElementValue::Float(0.0),
            },
            b'I' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Int(v)) => ElementValue::Int(*v),
                _ => ElementValue::Int(0),
            },
            b'J' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Long(v)) => ElementValue::Long(*v),
                _ => ElementValue::Long(0),
            },
            b'S' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Int(v)) => ElementValue::Short(*v as i16),
                _ => ElementValue::Short(0),
            },
            b'Z' => match self.pool.get(r.u2()? as usize) {
                Some(Const::Int(v)) => ElementValue::Boolean(*v != 0),
                _ => ElementValue::Boolean(false),
            },
            b's' => ElementValue::String(self.cp_utf8(r.u2()?)?),
            b'e' => ElementValue::Enum {
                desc: self.cp_utf8(r.u2()?)?,
                const_name: self.cp_utf8(r.u2()?)?,
            },
            b'c' => ElementValue::Class(self.cp_utf8(r.u2()?)?),
            b'@' => ElementValue::Annotation(self.parse_annotation(r, false)?),
            b'[' => {
                let n = r.u2()?;
                let mut arr = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    arr.push(self.parse_element_value(r)?);
                }
                ElementValue::Array(arr)
            }
            other => {
                return Err(format!(
                    "unknown annotation element tag `{}` ({other})",
                    other as char
                ))
            }
        })
    }

    pub fn cp_utf8(&self, idx: u16) -> Result<String, String> {
        match self.pool.get(idx as usize) {
            Some(Const::Utf8(s)) => Ok(s.clone()),
            other => Err(format!("constant pool #{idx} is not Utf8: {other:?}")),
        }
    }

    pub fn cp_class_name(&self, idx: u16) -> Result<String, String> {
        match self.pool.get(idx as usize) {
            Some(Const::Class(u)) => self.cp_utf8(*u),
            other => Err(format!("constant pool #{idx} is not Class: {other:?}")),
        }
    }

    pub fn cp_member(&self, idx: u16) -> Option<MemberRef> {
        let (c, nt) = match self.pool.get(idx as usize)? {
            Const::FieldRef(c, nt)
            | Const::MethodRef(c, nt)
            | Const::InterfaceMethodRef(c, nt) => (*c, *nt),
            _ => return None,
        };
        let Const::NameAndType(n, d) = self.pool.get(nt as usize)? else {
            return None;
        };
        Some(MemberRef {
            owner: self.cp_class_name(c).ok()?,
            name: self.cp_utf8(*n).ok()?,
            desc: self.cp_utf8(*d).ok()?,
        })
    }

    pub fn cp_method_handle(&self, idx: u16) -> Option<(u8, MemberRef)> {
        match self.pool.get(idx as usize)? {
            Const::MethodHandle(kind, target) => Some((*kind, self.cp_member(*target)?)),
            _ => None,
        }
    }

    pub fn resolve_indy(&self, idx: u16) -> Option<InvokeDynamicSite> {
        let Const::InvokeDynamic(bsm_idx, nt_idx) = self.pool.get(idx as usize)? else {
            return None;
        };
        let Const::NameAndType(n, d) = self.pool.get(*nt_idx as usize)? else {
            return None;
        };
        let name = self.cp_utf8(*n).ok()?;
        let desc = self.cp_utf8(*d).ok()?;
        let bsm = self.bootstrap_methods.get(*bsm_idx as usize)?;
        let bootstrap_handle = self.cp_method_handle(bsm.handle_index);
        let bootstrap_args = bsm
            .args
            .iter()
            .map(|&arg_idx| match self.pool.get(arg_idx as usize) {
                Some(Const::Int(v)) => BootstrapArg::Int(*v),
                Some(Const::Long(v)) => BootstrapArg::Long(*v),
                Some(Const::Float(v)) => BootstrapArg::Float(*v),
                Some(Const::Double(v)) => BootstrapArg::Double(*v),
                Some(Const::String(s)) => self
                    .cp_utf8(*s)
                    .map_or(BootstrapArg::Unknown(arg_idx), BootstrapArg::String),
                Some(Const::Class(c)) => self
                    .cp_utf8(*c)
                    .map_or(BootstrapArg::Unknown(arg_idx), BootstrapArg::Class),
                Some(Const::MethodType(mt)) => self
                    .cp_utf8(*mt)
                    .map_or(BootstrapArg::Unknown(arg_idx), BootstrapArg::MethodType),
                Some(Const::MethodHandle(kind, mr)) => self.cp_member(*mr).map_or(
                    BootstrapArg::Unknown(arg_idx),
                    |m| BootstrapArg::MethodHandle(*kind, m),
                ),
                _ => BootstrapArg::Unknown(arg_idx),
            })
            .collect();

        Some(InvokeDynamicSite {
            name,
            desc,
            bootstrap_handle,
            bootstrap_args,
        })
    }

    pub fn annotation(&self, desc: &str) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.desc == desc)
    }

    pub fn is_interface(&self) -> bool {
        self.access & access::INTERFACE != 0
    }

    pub fn is_abstract(&self) -> bool {
        self.access & access::ABSTRACT != 0
    }

    pub fn is_enum(&self) -> bool {
        self.access & access::ENUM != 0
            || self.superclass.as_deref() == Some("java/lang/Enum")
    }

    pub fn is_record(&self) -> bool {
        !self.record_components.is_empty()
            || self.superclass.as_deref() == Some("java/lang/Record")
    }

    /// Returns the `@Mod("modid")` ID if annotated with NeoForge or legacy Forge `@Mod`.
    pub fn neoforge_mod_id(&self) -> Option<&str> {
        self.annotation("Lnet/neoforged/fml/common/Mod;")
            .or_else(|| self.annotation("Lnet/minecraftforge/fml/common/Mod;"))
            .and_then(|a| a.get_str("value"))
            .filter(|id| !id.is_empty())
    }

    /// Returns true if annotated with `@org.spongepowered.asm.mixin.Mixin`.
    pub fn is_mixin(&self) -> bool {
        self.annotation("Lorg/spongepowered/asm/mixin/Mixin;")
            .is_some()
    }

    /// Internal names of all classes targeted by `@Mixin(value = ..., targets = ...)`.
    pub fn mixin_targets(&self) -> Vec<String> {
        let Some(ann) = self.annotation("Lorg/spongepowered/asm/mixin/Mixin;") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(val) = ann.get("value") {
            for raw in val.to_string_list() {
                let cleaned = raw
                    .strip_prefix('L')
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or(&raw)
                    .replace('.', "/");
                if !cleaned.is_empty() && !out.contains(&cleaned) {
                    out.push(cleaned);
                }
            }
        }
        if let Some(targets) = ann.get("targets") {
            for raw in targets.to_string_list() {
                let cleaned = raw
                    .strip_prefix('L')
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or(&raw)
                    .replace('.', "/");
                if !cleaned.is_empty() && !out.contains(&cleaned) {
                    out.push(cleaned);
                }
            }
        }
        out
    }

    pub fn method(&self, name: &str, desc: &str) -> Option<&MethodInfo> {
        self.methods
            .iter()
            .find(|m| m.name == name && m.desc == desc)
    }

    /// Follows SpongePowered Mixin's `MixinApplicatorStandard.getInitialiser` semantics:
    /// retains only field initialiser bytecode emitted outside the constructor's body line range,
    /// discarding dummy `super(...)` / `throw new AssertionError()` bodies.
    fn strip_mixin_constructor_bodies(&mut self) {
        let pool = self.pool.clone();
        let is_ctor = |cp_idx: u16| -> bool {
            let Some(Const::MethodRef(_, nt)) = pool.get(cp_idx as usize) else {
                return false;
            };
            let Some(Const::NameAndType(n, _)) = pool.get(*nt as usize) else {
                return false;
            };
            matches!(pool.get(*n as usize), Some(Const::Utf8(s)) if s == "<init>")
        };

        for method in &mut self.methods {
            if method.name != "<init>" {
                continue;
            }
            let Some(code) = method.code.as_mut() else {
                continue;
            };
            let Ok(insns) = decode_instructions(&code.bytes) else {
                continue;
            };
            let line_map: BTreeMap<usize, u16> = code
                .lines
                .iter()
                .map(|&(pc, line)| (pc as usize, line))
                .collect();

            let (mut cur_line, mut start_line, mut end_line) = (0u16, 0u16, 0u16);
            let (mut super_pos, mut fresh_line, mut synth_end_pc) = (None, false, None);
            for (idx, insn) in insns.iter().enumerate() {
                if let Some(&l) = line_map.get(&insn.at) {
                    cur_line = l;
                    fresh_line = true;
                }
                match (insn.op, &insn.arg) {
                    (op::INVOKESPECIAL, Operand::Int(cp))
                        if super_pos.is_none() && is_ctor(*cp as u16) =>
                    {
                        super_pos = Some(idx);
                        start_line = cur_line;
                    }
                    (op::PUTFIELD, _) => fresh_line = false,
                    (op::RETURN, _) if fresh_line => end_line = cur_line,
                    (op::RETURN, _) => {
                        end_line = start_line;
                        synth_end_pc = Some(insn.at);
                    }
                    _ => {}
                }
            }

            let Some(marker) = super_pos else {
                continue;
            };
            let mut keep = vec![false; insns.len()];
            if start_line != 0 && end_line >= start_line {
                let mut gather = false;
                for (idx, insn) in insns.iter().enumerate().skip(marker + 1) {
                    let begins = if synth_end_pc == Some(insn.at) {
                        Some(start_line)
                    } else {
                        line_map.get(&insn.at).copied()
                    };
                    if let Some(l) = begins {
                        gather =
                            (l == end_line && insn.op != op::RETURN) || l < start_line || l > end_line;
                    }
                    keep[idx] = gather && insn.op != op::RETURN;
                }
            }

            let len = code.bytes.len();
            for (idx, insn) in insns.iter().enumerate().skip(marker + 1) {
                if !keep[idx] {
                    let next_pc = insns.get(idx + 1).map_or(len, |n| n.at);
                    code.bytes[insn.at..next_pc].fill(op::NOP);
                }
            }
            if insns.len().saturating_sub(1) > marker && !keep[insns.len() - 1] && len > 0 {
                code.bytes[len - 1] = op::RETURN;
            }
            code.handlers.retain(|h| {
                insns.iter().enumerate().any(|(i, insn)| {
                    (i <= marker || keep[i])
                        && insn.at >= h.start_pc as usize
                        && insn.at < h.end_pc as usize
                })
            });
        }
    }
}

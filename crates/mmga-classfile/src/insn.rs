//! JVM bytecode instruction decoder (JVMS Chapter 6) and descriptor parser (JVMS §4.3).

use serde::{Deserialize, Serialize};

/// Decoded operand of a JVM bytecode instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operand {
    None,
    /// Local slot index, constant-pool index, `bipush` / `sipush` immediate, or `newarray` atype.
    Int(i32),
    /// `iinc`: `(local_slot, signed_delta)`.
    Iinc(u16, i32),
    /// Branch target as absolute bytecode offset within the method body.
    Jump(usize),
    /// `invokeinterface` `(cp_index, count)` or `multianewarray` `(cp_index, dimensions)`.
    IndexCount(u16, u8),
    /// `tableswitch` / `lookupswitch`: `(default_target, vec![(match_key, target)])` with absolute offsets.
    Switch {
        default: usize,
        pairs: Vec<(i32, usize)>,
    },
}

/// One decoded JVM instruction at byte offset `at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insn {
    pub at: usize,
    pub op: u8,
    pub arg: Operand,
}

/// Complete JVM opcode table.
pub mod op {
    pub const NOP: u8 = 0x00;
    pub const ACONST_NULL: u8 = 0x01;
    pub const ICONST_M1: u8 = 0x02;
    pub const ICONST_0: u8 = 0x03;
    pub const ICONST_1: u8 = 0x04;
    pub const ICONST_2: u8 = 0x05;
    pub const ICONST_3: u8 = 0x06;
    pub const ICONST_4: u8 = 0x07;
    pub const ICONST_5: u8 = 0x08;
    pub const LCONST_0: u8 = 0x09;
    pub const LCONST_1: u8 = 0x0a;
    pub const FCONST_0: u8 = 0x0b;
    pub const FCONST_1: u8 = 0x0c;
    pub const FCONST_2: u8 = 0x0d;
    pub const DCONST_0: u8 = 0x0e;
    pub const DCONST_1: u8 = 0x0f;
    pub const BIPUSH: u8 = 0x10;
    pub const SIPUSH: u8 = 0x11;
    pub const LDC: u8 = 0x12;
    pub const LDC_W: u8 = 0x13;
    pub const LDC2_W: u8 = 0x14;

    pub const ILOAD: u8 = 0x15;
    pub const LLOAD: u8 = 0x16;
    pub const FLOAD: u8 = 0x17;
    pub const DLOAD: u8 = 0x18;
    pub const ALOAD: u8 = 0x19;
    pub const ILOAD_0: u8 = 0x1a;
    pub const ILOAD_1: u8 = 0x1b;
    pub const ILOAD_2: u8 = 0x1c;
    pub const ILOAD_3: u8 = 0x1d;
    pub const LLOAD_0: u8 = 0x1e;
    pub const LLOAD_1: u8 = 0x1f;
    pub const LLOAD_2: u8 = 0x20;
    pub const LLOAD_3: u8 = 0x21;
    pub const FLOAD_0: u8 = 0x22;
    pub const FLOAD_1: u8 = 0x23;
    pub const FLOAD_2: u8 = 0x24;
    pub const FLOAD_3: u8 = 0x25;
    pub const DLOAD_0: u8 = 0x26;
    pub const DLOAD_1: u8 = 0x27;
    pub const DLOAD_2: u8 = 0x28;
    pub const DLOAD_3: u8 = 0x29;
    pub const ALOAD_0: u8 = 0x2a;
    pub const ALOAD_1: u8 = 0x2b;
    pub const ALOAD_2: u8 = 0x2c;
    pub const ALOAD_3: u8 = 0x2d;

    pub const IALOAD: u8 = 0x2e;
    pub const LALOAD: u8 = 0x2f;
    pub const FALOAD: u8 = 0x30;
    pub const DALOAD: u8 = 0x31;
    pub const AALOAD: u8 = 0x32;
    pub const BALOAD: u8 = 0x33;
    pub const CALOAD: u8 = 0x34;
    pub const SALOAD: u8 = 0x35;

    pub const ISTORE: u8 = 0x36;
    pub const LSTORE: u8 = 0x37;
    pub const FSTORE: u8 = 0x38;
    pub const DSTORE: u8 = 0x39;
    pub const ASTORE: u8 = 0x3a;
    pub const ISTORE_0: u8 = 0x3b;
    pub const ISTORE_1: u8 = 0x3c;
    pub const ISTORE_2: u8 = 0x3d;
    pub const ISTORE_3: u8 = 0x3e;
    pub const LSTORE_0: u8 = 0x3f;
    pub const LSTORE_1: u8 = 0x40;
    pub const LSTORE_2: u8 = 0x41;
    pub const LSTORE_3: u8 = 0x42;
    pub const FSTORE_0: u8 = 0x43;
    pub const FSTORE_1: u8 = 0x44;
    pub const FSTORE_2: u8 = 0x45;
    pub const FSTORE_3: u8 = 0x46;
    pub const DSTORE_0: u8 = 0x47;
    pub const DSTORE_1: u8 = 0x48;
    pub const DSTORE_2: u8 = 0x49;
    pub const DSTORE_3: u8 = 0x4a;
    pub const ASTORE_0: u8 = 0x4b;
    pub const ASTORE_1: u8 = 0x4c;
    pub const ASTORE_2: u8 = 0x4d;
    pub const ASTORE_3: u8 = 0x4e;

    pub const IASTORE: u8 = 0x4f;
    pub const LASTORE: u8 = 0x50;
    pub const FASTORE: u8 = 0x51;
    pub const DASTORE: u8 = 0x52;
    pub const AASTORE: u8 = 0x53;
    pub const BASTORE: u8 = 0x54;
    pub const CASTORE: u8 = 0x55;
    pub const SASTORE: u8 = 0x56;

    pub const POP: u8 = 0x57;
    pub const POP2: u8 = 0x58;
    pub const DUP: u8 = 0x59;
    pub const DUP_X1: u8 = 0x5a;
    pub const DUP_X2: u8 = 0x5b;
    pub const DUP2: u8 = 0x5c;
    pub const DUP2_X1: u8 = 0x5d;
    pub const DUP2_X2: u8 = 0x5e;
    pub const SWAP: u8 = 0x5f;

    pub const IADD: u8 = 0x60;
    pub const LADD: u8 = 0x61;
    pub const FADD: u8 = 0x62;
    pub const DADD: u8 = 0x63;
    pub const ISUB: u8 = 0x64;
    pub const LSUB: u8 = 0x65;
    pub const FSUB: u8 = 0x66;
    pub const DSUB: u8 = 0x67;
    pub const IMUL: u8 = 0x68;
    pub const LMUL: u8 = 0x69;
    pub const FMUL: u8 = 0x6a;
    pub const DMUL: u8 = 0x6b;
    pub const IDIV: u8 = 0x6c;
    pub const LDIV: u8 = 0x6d;
    pub const FDIV: u8 = 0x6e;
    pub const DDIV: u8 = 0x6f;
    pub const IREM: u8 = 0x70;
    pub const LREM: u8 = 0x71;
    pub const FREM: u8 = 0x72;
    pub const DREM: u8 = 0x73;
    pub const INEG: u8 = 0x74;
    pub const LNEG: u8 = 0x75;
    pub const FNEG: u8 = 0x76;
    pub const DNEG: u8 = 0x77;

    pub const ISHL: u8 = 0x78;
    pub const LSHL: u8 = 0x79;
    pub const ISHR: u8 = 0x7a;
    pub const LSHR: u8 = 0x7b;
    pub const IUSHR: u8 = 0x7c;
    pub const LUSHR: u8 = 0x7d;
    pub const IAND: u8 = 0x7e;
    pub const LAND: u8 = 0x7f;
    pub const IOR: u8 = 0x80;
    pub const LOR: u8 = 0x81;
    pub const IXOR: u8 = 0x82;
    pub const LXOR: u8 = 0x83;
    pub const IINC: u8 = 0x84;

    pub const I2L: u8 = 0x85;
    pub const I2F: u8 = 0x86;
    pub const I2D: u8 = 0x87;
    pub const L2I: u8 = 0x88;
    pub const L2F: u8 = 0x89;
    pub const L2D: u8 = 0x8a;
    pub const F2I: u8 = 0x8b;
    pub const F2L: u8 = 0x8c;
    pub const F2D: u8 = 0x8d;
    pub const D2I: u8 = 0x8e;
    pub const D2L: u8 = 0x8f;
    pub const D2F: u8 = 0x90;
    pub const I2B: u8 = 0x91;
    pub const I2C: u8 = 0x92;
    pub const I2S: u8 = 0x93;

    pub const LCMP: u8 = 0x94;
    pub const FCMPL: u8 = 0x95;
    pub const FCMPG: u8 = 0x96;
    pub const DCMPL: u8 = 0x97;
    pub const DCMPG: u8 = 0x98;

    pub const IFEQ: u8 = 0x99;
    pub const IFNE: u8 = 0x9a;
    pub const IFLT: u8 = 0x9b;
    pub const IFGE: u8 = 0x9c;
    pub const IFGT: u8 = 0x9d;
    pub const IFLE: u8 = 0x9e;
    pub const IF_ICMPEQ: u8 = 0x9f;
    pub const IF_ICMPNE: u8 = 0xa0;
    pub const IF_ICMPLT: u8 = 0xa1;
    pub const IF_ICMPGE: u8 = 0xa2;
    pub const IF_ICMPGT: u8 = 0xa3;
    pub const IF_ICMPLE: u8 = 0xa4;
    pub const IF_ACMPEQ: u8 = 0xa5;
    pub const IF_ACMPNE: u8 = 0xa6;
    pub const GOTO: u8 = 0xa7;
    pub const JSR: u8 = 0xa8;
    pub const RET: u8 = 0xa9;
    pub const TABLESWITCH: u8 = 0xaa;
    pub const LOOKUPSWITCH: u8 = 0xab;

    pub const IRETURN: u8 = 0xac;
    pub const LRETURN: u8 = 0xad;
    pub const FRETURN: u8 = 0xae;
    pub const DRETURN: u8 = 0xaf;
    pub const ARETURN: u8 = 0xb0;
    pub const RETURN: u8 = 0xb1;

    pub const GETSTATIC: u8 = 0xb2;
    pub const PUTSTATIC: u8 = 0xb3;
    pub const GETFIELD: u8 = 0xb4;
    pub const PUTFIELD: u8 = 0xb5;
    pub const INVOKEVIRTUAL: u8 = 0xb6;
    pub const INVOKESPECIAL: u8 = 0xb7;
    pub const INVOKESTATIC: u8 = 0xb8;
    pub const INVOKEINTERFACE: u8 = 0xb9;
    pub const INVOKEDYNAMIC: u8 = 0xba;

    pub const NEW: u8 = 0xbb;
    pub const NEWARRAY: u8 = 0xbc;
    pub const ANEWARRAY: u8 = 0xbd;
    pub const ARRAYLENGTH: u8 = 0xbe;
    pub const ATHROW: u8 = 0xbf;
    pub const CHECKCAST: u8 = 0xc0;
    pub const INSTANCEOF: u8 = 0xc1;
    pub const MONITORENTER: u8 = 0xc2;
    pub const MONITOREXIT: u8 = 0xc3;
    pub const WIDE: u8 = 0xc4;
    pub const MULTIANEWARRAY: u8 = 0xc5;
    pub const IFNULL: u8 = 0xc6;
    pub const IFNONNULL: u8 = 0xc7;
    pub const GOTO_W: u8 = 0xc8;
    pub const JSR_W: u8 = 0xc9;
}

fn read_u8(bytes: &[u8], i: usize) -> Result<u8, String> {
    bytes
        .get(i)
        .copied()
        .ok_or_else(|| format!("truncated bytecode at offset {i}"))
}

fn read_i16(bytes: &[u8], i: usize) -> Result<i32, String> {
    let s = bytes
        .get(i..i + 2)
        .ok_or_else(|| format!("truncated u2 operand at offset {i}"))?;
    Ok(i16::from_be_bytes([s[0], s[1]]) as i32)
}

fn read_i32(bytes: &[u8], i: usize) -> Result<i32, String> {
    let s = bytes
        .get(i..i + 4)
        .ok_or_else(|| format!("truncated u4 operand at offset {i}"))?;
    Ok(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// Decodes a JVM method's bytecode stream into instructions with absolute jump targets.
pub fn decode_instructions(bytes: &[u8]) -> Result<Vec<Insn>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let at = i;
        let opcode = bytes[i];
        i += 1;
        let resolve_jump = |rel: i32| -> Result<usize, String> {
            let target = (at as i64) + (rel as i64);
            if target < 0 || (target as usize) > bytes.len() {
                Err(format!(
                    "branch target {target} out of bounds (code len {}) at pc {at}",
                    bytes.len()
                ))
            } else {
                Ok(target as usize)
            }
        };

        if opcode == op::WIDE {
            let inner = read_u8(bytes, i)?;
            let slot = read_i16(bytes, i + 1)? as u16;
            if inner == op::IINC {
                let delta = read_i16(bytes, i + 3)?;
                i += 5;
                out.push(Insn {
                    at,
                    op: inner,
                    arg: Operand::Iinc(slot, delta),
                });
            } else {
                i += 3;
                out.push(Insn {
                    at,
                    op: inner,
                    arg: Operand::Int(i32::from(slot)),
                });
            }
            continue;
        }

        let arg = match opcode {
            // u1 local index / ldc / newarray
            0x15..=0x19 | 0x36..=0x3a | op::RET | op::NEWARRAY | op::LDC => {
                let v = read_u8(bytes, i)?;
                i += 1;
                Operand::Int(i32::from(v))
            }
            op::BIPUSH => {
                let v = read_u8(bytes, i)? as i8;
                i += 1;
                Operand::Int(i32::from(v))
            }
            op::SIPUSH => {
                let v = read_i16(bytes, i)?;
                i += 2;
                Operand::Int(v)
            }
            // u2 constant-pool index
            op::LDC_W
            | op::LDC2_W
            | op::GETSTATIC..=op::INVOKESTATIC
            | op::NEW
            | op::ANEWARRAY
            | op::CHECKCAST
            | op::INSTANCEOF => {
                let idx = read_i16(bytes, i)? as u16;
                i += 2;
                Operand::Int(i32::from(idx))
            }
            op::INVOKEINTERFACE => {
                let idx = read_i16(bytes, i)? as u16;
                let count = read_u8(bytes, i + 2)?;
                let _zero = read_u8(bytes, i + 3)?;
                i += 4;
                Operand::IndexCount(idx, count)
            }
            op::INVOKEDYNAMIC => {
                let idx = read_i16(bytes, i)? as u16;
                let _zero = read_i16(bytes, i + 2)?;
                i += 4;
                Operand::Int(i32::from(idx))
            }
            op::MULTIANEWARRAY => {
                let idx = read_i16(bytes, i)? as u16;
                let dims = read_u8(bytes, i + 2)?;
                i += 3;
                Operand::IndexCount(idx, dims)
            }
            op::IINC => {
                let slot = u16::from(read_u8(bytes, i)?);
                let delta = i32::from(read_u8(bytes, i + 1)? as i8);
                i += 2;
                Operand::Iinc(slot, delta)
            }
            // s2 branch offset
            op::IFEQ..=op::JSR | op::IFNULL | op::IFNONNULL => {
                let rel = read_i16(bytes, i)?;
                i += 2;
                Operand::Jump(resolve_jump(rel)?)
            }
            op::GOTO_W | op::JSR_W => {
                let rel = read_i32(bytes, i)?;
                i += 4;
                Operand::Jump(resolve_jump(rel)?)
            }
            op::TABLESWITCH => {
                i = (i + 3) & !3;
                let default = resolve_jump(read_i32(bytes, i)?)?;
                let low = read_i32(bytes, i + 4)?;
                let high = read_i32(bytes, i + 8)?;
                if high < low {
                    return Err(format!("invalid tableswitch range {low}..={high} at pc {at}"));
                }
                i += 12;
                let count = ((high as i64) - (low as i64) + 1) as usize;
                let mut pairs = Vec::with_capacity(count);
                for step in 0..count {
                    let key = (low as i64 + step as i64) as i32;
                    let target = resolve_jump(read_i32(bytes, i)?)?;
                    i += 4;
                    pairs.push((key, target));
                }
                Operand::Switch { default, pairs }
            }
            op::LOOKUPSWITCH => {
                i = (i + 3) & !3;
                let default = resolve_jump(read_i32(bytes, i)?)?;
                let npairs = read_i32(bytes, i + 4)?;
                if npairs < 0 {
                    return Err(format!("negative lookupswitch count {npairs} at pc {at}"));
                }
                i += 8;
                let mut pairs = Vec::with_capacity(npairs as usize);
                for _ in 0..npairs {
                    let key = read_i32(bytes, i)?;
                    let target = resolve_jump(read_i32(bytes, i + 4)?)?;
                    i += 8;
                    pairs.push((key, target));
                }
                Operand::Switch { default, pairs }
            }
            _ => Operand::None,
        };

        out.push(Insn {
            at,
            op: opcode,
            arg,
        });
    }
    Ok(out)
}

impl Insn {
    /// Returns true if this instruction is any return (`ireturn`..`return`).
    pub fn is_return(&self) -> bool {
        matches!(self.op, op::IRETURN..=op::RETURN)
    }

    /// Returns true if this instruction is a method invocation (`invokevirtual`..`invokedynamic`).
    pub fn is_invoke(&self) -> bool {
        matches!(self.op, op::INVOKEVIRTUAL..=op::INVOKEDYNAMIC)
    }

    /// Extracts the constant pool index if this instruction carries one.
    pub fn cp_index(&self) -> Option<u16> {
        match self.arg {
            Operand::Int(v) if v >= 0 => Some(v as u16),
            Operand::IndexCount(idx, _) => Some(idx),
            _ => None,
        }
    }
}

/// Parsed JVM field or parameter type descriptor (JVMS §4.3.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DescriptorType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
    #[default]
    Void,
    Object(String),
    Array(Box<DescriptorType>),
}

impl DescriptorType {
    /// Number of local-variable / operand-stack slots in standard JVMS (2 for `long` and `double`, 0 for `void`, 1 otherwise).
    pub fn jvm_slots(&self) -> usize {
        match self {
            Self::Long | Self::Double => 2,
            Self::Void => 0,
            _ => 1,
        }
    }

    /// Internal class name if `Object("pkg/Name")` or array descriptor `"[..."`.
    pub fn internal_name(&self) -> Option<String> {
        match self {
            Self::Object(c) => Some(c.clone()),
            Self::Array(_) => Some(self.to_descriptor()),
            _ => None,
        }
    }

    /// Formats back to canonical JVMS descriptor string.
    pub fn to_descriptor(&self) -> String {
        match self {
            Self::Byte => "B".into(),
            Self::Char => "C".into(),
            Self::Double => "D".into(),
            Self::Float => "F".into(),
            Self::Int => "I".into(),
            Self::Long => "J".into(),
            Self::Short => "S".into(),
            Self::Boolean => "Z".into(),
            Self::Void => "V".into(),
            Self::Object(c) => format!("L{c};"),
            Self::Array(inner) => format!("[{}", inner.to_descriptor()),
        }
    }
}

fn parse_one_type(desc: &str, pos: &mut usize) -> Result<DescriptorType, String> {
    let bytes = desc.as_bytes();
    let b = *bytes
        .get(*pos)
        .ok_or_else(|| format!("unexpected end of descriptor `{desc}`"))?;
    *pos += 1;
    Ok(match b {
        b'B' => DescriptorType::Byte,
        b'C' => DescriptorType::Char,
        b'D' => DescriptorType::Double,
        b'F' => DescriptorType::Float,
        b'I' => DescriptorType::Int,
        b'J' => DescriptorType::Long,
        b'S' => DescriptorType::Short,
        b'Z' => DescriptorType::Boolean,
        b'V' => DescriptorType::Void,
        b'[' => DescriptorType::Array(Box::new(parse_one_type(desc, pos)?)),
        b'L' => {
            let rest = &desc[*pos..];
            let semi = rest
                .find(';')
                .ok_or_else(|| format!("unterminated object descriptor in `{desc}`"))?;
            let cls = rest[..semi].to_owned();
            *pos += semi + 1;
            DescriptorType::Object(cls)
        }
        other => {
            return Err(format!(
                "invalid descriptor character `{}` in `{desc}`",
                other as char
            ))
        }
    })
}

/// Parses a single field descriptor (e.g. `"Ljava/lang/String;"`, `"[I"`, `"J"`).
pub fn parse_field_descriptor(desc: &str) -> Result<DescriptorType, String> {
    let mut pos = 0;
    let ty = parse_one_type(desc, &mut pos)?;
    if pos != desc.len() {
        return Err(format!("trailing characters in field descriptor `{desc}`"));
    }
    Ok(ty)
}

/// Parses a method descriptor `(params...)return` into `(param_types, return_type)`.
pub fn parse_method_descriptor(
    desc: &str,
) -> Result<(Vec<DescriptorType>, DescriptorType), String> {
    if !desc.starts_with('(') {
        return Err(format!("method descriptor `{desc}` missing `(`"));
    }
    let mut pos = 1;
    let mut params = Vec::new();
    let bytes = desc.as_bytes();
    while pos < bytes.len() && bytes[pos] != b')' {
        params.push(parse_one_type(desc, &mut pos)?);
    }
    if bytes.get(pos) != Some(&b')') {
        return Err(format!("method descriptor `{desc}` missing `)`"));
    }
    pos += 1;
    let ret = parse_one_type(desc, &mut pos)?;
    if pos != desc.len() {
        return Err(format!("trailing characters in method descriptor `{desc}`"));
    }
    Ok((params, ret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_branches_iinc_and_tableswitch() {
        let code = [
            op::ICONST_0,
            op::ISTORE_1,
            op::ILOAD_1,
            op::BIPUSH,
            0xfd,
            op::IF_ICMPGE,
            0x00,
            0x09,
            op::IINC,
            0x01,
            0x01,
            op::GOTO,
            0xff,
            0xf7,
            op::RETURN,
        ];
        let insns = decode_instructions(&code).unwrap();
        assert_eq!(insns.len(), 8);
        assert_eq!(insns[3].arg, Operand::Int(-3));
        assert_eq!(insns[4].arg, Operand::Jump(14));
        assert_eq!(insns[5].arg, Operand::Iinc(1, 1));
        assert_eq!(insns[6].arg, Operand::Jump(2));
    }

    #[test]
    fn parses_complex_neoforge_method_descriptors() {
        let (params, ret) = parse_method_descriptor(
            "(Lnet/neoforged/bus/api/IEventBus;Lnet/neoforged/fml/ModContainer;J[Ljava/lang/String;)Z",
        )
        .unwrap();
        assert_eq!(
            params,
            vec![
                DescriptorType::Object("net/neoforged/bus/api/IEventBus".into()),
                DescriptorType::Object("net/neoforged/fml/ModContainer".into()),
                DescriptorType::Long,
                DescriptorType::Array(Box::new(DescriptorType::Object(
                    "java/lang/String".into()
                ))),
            ]
        );
        assert_eq!(ret, DescriptorType::Boolean);
    }
}

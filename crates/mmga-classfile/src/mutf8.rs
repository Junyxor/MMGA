//! Java Modified UTF-8 (`MUTF-8`) decoding and encoding per JVMS §4.4.7.
//!
//! In class files:
//! - U+0000 is encoded as the two-byte sequence `0xC0 0x80` (never `0x00`).
//! - Code points U+0001..=U+007F use 1 byte.
//! - Code points U+0080..=U+07FF use 2 bytes.
//! - Code points U+0800..=U+FFFF use 3 bytes.
//! - Supplementary characters (U+10000..=U+10FFFF) are encoded as two 3-byte UTF-16 surrogates.

/// Decodes JVMS Modified UTF-8 bytes into a Rust `String`.
pub fn decode_mutf8(bytes: &[u8]) -> String {
    if !bytes.contains(&0xC0) && !bytes.contains(&0xED) {
        if let Ok(s) = std::str::from_utf8(bytes) {
            return s.to_owned();
        }
    }

    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let cont = |offset: usize| bytes.get(i + offset).is_some_and(|b| (b & 0xC0) == 0x80);
        match b0 {
            0x00..=0x7F => {
                units.push(u16::from(b0));
                i += 1;
            }
            0xC0..=0xDF if cont(1) => {
                let u = (u16::from(b0 & 0x1F) << 6) | u16::from(bytes[i + 1] & 0x3F);
                units.push(u);
                i += 2;
            }
            0xE0..=0xEF if cont(1) && cont(2) => {
                let u = (u16::from(b0 & 0x0F) << 12)
                    | (u16::from(bytes[i + 1] & 0x3F) << 6)
                    | u16::from(bytes[i + 2] & 0x3F);
                units.push(u);
                i += 3;
            }
            _ => {
                units.push(0xFFFD);
                i += 1;
            }
        }
    }
    String::from_utf16_lossy(&units)
}

/// Encodes a Rust `&str` into JVMS Modified UTF-8 bytes.
pub fn encode_mutf8(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for unit in text.encode_utf16() {
        match unit {
            0x0000 => {
                out.push(0xC0);
                out.push(0x80);
            }
            0x0001..=0x007F => {
                out.push(unit as u8);
            }
            0x0080..=0x07FF => {
                out.push(0xC0 | ((unit >> 6) as u8 & 0x1F));
                out.push(0x80 | (unit as u8 & 0x3F));
            }
            0x0800..=0xFFFF => {
                out.push(0xE0 | ((unit >> 12) as u8 & 0x0F));
                out.push(0x80 | ((unit >> 6) as u8 & 0x3F));
                out.push(0x80 | (unit as u8 & 0x3F));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutf8_roundtrips_ascii_unicode_nul_and_astral() {
        let cases = [
            "hello_neoforge",
            "中文测试 αβγ",
            "embedded\0nul\0bytes",
            "emoji \u{1F680} and CJK",
        ];
        for s in cases {
            let encoded = encode_mutf8(s);
            if s.contains('\0') {
                assert!(!encoded.contains(&0x00));
                assert!(encoded.windows(2).any(|w| w == [0xC0, 0x80]));
            }
            assert_eq!(decode_mutf8(&encoded), s);
        }
    }

    #[test]
    fn mutf8_handles_lone_surrogates_gracefully() {
        // High surrogate 0xD83D without trailing low surrogate: ED A0 BD + 'x'
        let raw = [0xED, 0xA0, 0xBD, b'x'];
        assert_eq!(decode_mutf8(&raw), "\u{FFFD}x");
    }
}

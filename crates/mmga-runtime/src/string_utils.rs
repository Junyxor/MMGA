//! Commons Lang 3.14.0 StringUtils.capitalize (source blob
//! 7d924eb5e89499bada34edb50aca62072a4d39ae). Java 21 simple titlecase is
//! generated from official OpenJDK jdk-21+35 UnicodeData, not Rust's uppercase.
//! Rt strings contain Unicode scalars; existing lone-UTF16-surrogate loss is
//! outside this helper and is not represented as newly supported here.

use crate::{Data, ExecCtx, V};
use mmga_classfile::MemberRef;
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    if !is_static
        || member.owner != "org/apache/commons/lang3/StringUtils"
        || member.name != "capitalize"
        || member.desc != "(Ljava/lang/String;)Ljava/lang/String;"
    {
        return None;
    }
    let value = *args.first()?;
    if value.is_null() {
        return Some(V::NULL);
    }
    let Some(Data::Str(input)) = ctx.rt.get_obj(value).map(|object| &object.data) else {
        return Some(ctx.rt.throw_exception(
            "java/lang/ClassCastException",
            "StringUtils.capitalize requires String",
        ));
    };
    let Some(first) = input.chars().next() else {
        return Some(value);
    };
    let mapped = title_case(first as u32);
    if mapped == first as u32 {
        return Some(value);
    }
    let Some(mapped) = char::from_u32(mapped) else {
        return Some(ctx.rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "Titlecase result is not a representable Unicode scalar",
        ));
    };
    let output = format!("{mapped}{}", &input[first.len_utf8()..]);
    Some(ctx.rt.new_str(output))
}
fn title_case(value: u32) -> u32 {
    let index = match TITLECASE_RANGES.binary_search_by_key(&value, |range| range.0) {
        Ok(index) => index,
        Err(0) => return value,
        Err(index) => index - 1,
    };
    let (start, end, step, offset) = TITLECASE_RANGES[index];
    if value <= end && (value - start) % step == 0 {
        (value as i64 + offset as i64) as u32
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{invoke_member, LinkedSpace, NullHost, Rt};
    use mmga_classfile::op;
    fn capitalize(ctx: &mut ExecCtx<'_>, value: V) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                "org/apache/commons/lang3/StringUtils",
                "capitalize",
                "(Ljava/lang/String;)Ljava/lang/String;",
            ),
            &[value],
        )
    }
    #[test]
    fn capitalization_uses_java_simple_titlecase_and_preserves_unchanged_identity() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert_eq!(capitalize(&mut ctx, V::NULL), V::NULL);
        assert!(ctx.rt.pending_exception.is_none());
        for (input, expected) in [
            ("create brass", "Create brass"),
            ("", ""),
            ("Create", "Create"),
            ("ßeta", "ßeta"),
            ("ǳungarian", "ǲungarian"),
            ("Ǳabc", "ǲabc"),
            ("ǆuro", "ǅuro"),
            ("ǲabc", "ǲabc"),
            ("\u{1FB3}abc", "\u{1FBC}abc"),
            ("😀create", "😀create"),
            ("𐐨𐐨tail", "𐐀𐐨tail"),
        ] {
            let original = ctx.rt.new_str(input);
            let value = capitalize(&mut ctx, original);
            assert!(ctx.rt.pending_exception.is_none(), "capitalize {input}");
            assert_eq!(ctx.rt.str_of(value), Some(expected));
            if input == expected {
                assert_eq!(
                    value, original,
                    "unchanged/null/empty source returns original reference"
                );
            } else {
                assert_ne!(value, original);
            }
        }
    }
    #[test]
    fn supplementary_prefix_boundaries_and_wrong_argument_types_are_not_faked() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        // Both supplementary characters occupy two UTF-16 code units. Only
        // the leading scalar changes; lone surrogates cannot exist in Data::Str.
        let input = ctx.rt.new_str("𐐨😀zinc");
        let result = capitalize(&mut ctx, input);
        assert_eq!(ctx.rt.str_of(result), Some("𐐀😀zinc"));
        let wrong = ctx.rt.new_instance("test/NotString", 0);
        capitalize(&mut ctx, wrong);
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(ctx.rt.class_of(error), Some("java/lang/ClassCastException"));
    }
}

// Generated from OpenJDK jdk-21+35 UnicodeData.txt, blob ea963a7162ce6e913a9f5a98e940b52181ac68dd.
const TITLECASE_RANGES: &[(u32, u32, u32, i32)] = &[
    (0x61, 0x7A, 1, -32),
    (0xB5, 0xB5, 1, 743),
    (0xE0, 0xF6, 1, -32),
    (0xF8, 0xFE, 1, -32),
    (0xFF, 0xFF, 1, 121),
    (0x101, 0x12F, 2, -1),
    (0x131, 0x131, 1, -232),
    (0x133, 0x137, 2, -1),
    (0x13A, 0x148, 2, -1),
    (0x14B, 0x177, 2, -1),
    (0x17A, 0x17E, 2, -1),
    (0x17F, 0x17F, 1, -300),
    (0x180, 0x180, 1, 195),
    (0x183, 0x185, 2, -1),
    (0x188, 0x188, 1, -1),
    (0x18C, 0x18C, 1, -1),
    (0x192, 0x192, 1, -1),
    (0x195, 0x195, 1, 97),
    (0x199, 0x199, 1, -1),
    (0x19A, 0x19A, 1, 163),
    (0x19E, 0x19E, 1, 130),
    (0x1A1, 0x1A5, 2, -1),
    (0x1A8, 0x1A8, 1, -1),
    (0x1AD, 0x1AD, 1, -1),
    (0x1B0, 0x1B0, 1, -1),
    (0x1B4, 0x1B6, 2, -1),
    (0x1B9, 0x1B9, 1, -1),
    (0x1BD, 0x1BD, 1, -1),
    (0x1BF, 0x1BF, 1, 56),
    (0x1C4, 0x1C4, 1, 1),
    (0x1C6, 0x1C6, 1, -1),
    (0x1C7, 0x1C7, 1, 1),
    (0x1C9, 0x1C9, 1, -1),
    (0x1CA, 0x1CA, 1, 1),
    (0x1CC, 0x1DC, 2, -1),
    (0x1DD, 0x1DD, 1, -79),
    (0x1DF, 0x1EF, 2, -1),
    (0x1F1, 0x1F1, 1, 1),
    (0x1F3, 0x1F5, 2, -1),
    (0x1F9, 0x21F, 2, -1),
    (0x223, 0x233, 2, -1),
    (0x23C, 0x23C, 1, -1),
    (0x23F, 0x240, 1, 10815),
    (0x242, 0x242, 1, -1),
    (0x247, 0x24F, 2, -1),
    (0x250, 0x250, 1, 10783),
    (0x251, 0x251, 1, 10780),
    (0x252, 0x252, 1, 10782),
    (0x253, 0x253, 1, -210),
    (0x254, 0x254, 1, -206),
    (0x256, 0x257, 1, -205),
    (0x259, 0x259, 1, -202),
    (0x25B, 0x25B, 1, -203),
    (0x25C, 0x25C, 1, 42319),
    (0x260, 0x260, 1, -205),
    (0x261, 0x261, 1, 42315),
    (0x263, 0x263, 1, -207),
    (0x265, 0x265, 1, 42280),
    (0x266, 0x266, 1, 42308),
    (0x268, 0x268, 1, -209),
    (0x269, 0x269, 1, -211),
    (0x26A, 0x26A, 1, 42308),
    (0x26B, 0x26B, 1, 10743),
    (0x26C, 0x26C, 1, 42305),
    (0x26F, 0x26F, 1, -211),
    (0x271, 0x271, 1, 10749),
    (0x272, 0x272, 1, -213),
    (0x275, 0x275, 1, -214),
    (0x27D, 0x27D, 1, 10727),
    (0x280, 0x280, 1, -218),
    (0x282, 0x282, 1, 42307),
    (0x283, 0x283, 1, -218),
    (0x287, 0x287, 1, 42282),
    (0x288, 0x288, 1, -218),
    (0x289, 0x289, 1, -69),
    (0x28A, 0x28B, 1, -217),
    (0x28C, 0x28C, 1, -71),
    (0x292, 0x292, 1, -219),
    (0x29D, 0x29D, 1, 42261),
    (0x29E, 0x29E, 1, 42258),
    (0x345, 0x345, 1, 84),
    (0x371, 0x373, 2, -1),
    (0x377, 0x377, 1, -1),
    (0x37B, 0x37D, 1, 130),
    (0x3AC, 0x3AC, 1, -38),
    (0x3AD, 0x3AF, 1, -37),
    (0x3B1, 0x3C1, 1, -32),
    (0x3C2, 0x3C2, 1, -31),
    (0x3C3, 0x3CB, 1, -32),
    (0x3CC, 0x3CC, 1, -64),
    (0x3CD, 0x3CE, 1, -63),
    (0x3D0, 0x3D0, 1, -62),
    (0x3D1, 0x3D1, 1, -57),
    (0x3D5, 0x3D5, 1, -47),
    (0x3D6, 0x3D6, 1, -54),
    (0x3D7, 0x3D7, 1, -8),
    (0x3D9, 0x3EF, 2, -1),
    (0x3F0, 0x3F0, 1, -86),
    (0x3F1, 0x3F1, 1, -80),
    (0x3F2, 0x3F2, 1, 7),
    (0x3F3, 0x3F3, 1, -116),
    (0x3F5, 0x3F5, 1, -96),
    (0x3F8, 0x3F8, 1, -1),
    (0x3FB, 0x3FB, 1, -1),
    (0x430, 0x44F, 1, -32),
    (0x450, 0x45F, 1, -80),
    (0x461, 0x481, 2, -1),
    (0x48B, 0x4BF, 2, -1),
    (0x4C2, 0x4CE, 2, -1),
    (0x4CF, 0x4CF, 1, -15),
    (0x4D1, 0x52F, 2, -1),
    (0x561, 0x586, 1, -48),
    (0x13F8, 0x13FD, 1, -8),
    (0x1C80, 0x1C80, 1, -6254),
    (0x1C81, 0x1C81, 1, -6253),
    (0x1C82, 0x1C82, 1, -6244),
    (0x1C83, 0x1C84, 1, -6242),
    (0x1C85, 0x1C85, 1, -6243),
    (0x1C86, 0x1C86, 1, -6236),
    (0x1C87, 0x1C87, 1, -6181),
    (0x1C88, 0x1C88, 1, 35266),
    (0x1D79, 0x1D79, 1, 35332),
    (0x1D7D, 0x1D7D, 1, 3814),
    (0x1D8E, 0x1D8E, 1, 35384),
    (0x1E01, 0x1E95, 2, -1),
    (0x1E9B, 0x1E9B, 1, -59),
    (0x1EA1, 0x1EFF, 2, -1),
    (0x1F00, 0x1F07, 1, 8),
    (0x1F10, 0x1F15, 1, 8),
    (0x1F20, 0x1F27, 1, 8),
    (0x1F30, 0x1F37, 1, 8),
    (0x1F40, 0x1F45, 1, 8),
    (0x1F51, 0x1F57, 2, 8),
    (0x1F60, 0x1F67, 1, 8),
    (0x1F70, 0x1F71, 1, 74),
    (0x1F72, 0x1F75, 1, 86),
    (0x1F76, 0x1F77, 1, 100),
    (0x1F78, 0x1F79, 1, 128),
    (0x1F7A, 0x1F7B, 1, 112),
    (0x1F7C, 0x1F7D, 1, 126),
    (0x1F80, 0x1F87, 1, 8),
    (0x1F90, 0x1F97, 1, 8),
    (0x1FA0, 0x1FA7, 1, 8),
    (0x1FB0, 0x1FB1, 1, 8),
    (0x1FB3, 0x1FB3, 1, 9),
    (0x1FBE, 0x1FBE, 1, -7205),
    (0x1FC3, 0x1FC3, 1, 9),
    (0x1FD0, 0x1FD1, 1, 8),
    (0x1FE0, 0x1FE1, 1, 8),
    (0x1FE5, 0x1FE5, 1, 7),
    (0x1FF3, 0x1FF3, 1, 9),
    (0x214E, 0x214E, 1, -28),
    (0x2170, 0x217F, 1, -16),
    (0x2184, 0x2184, 1, -1),
    (0x24D0, 0x24E9, 1, -26),
    (0x2C30, 0x2C5F, 1, -48),
    (0x2C61, 0x2C61, 1, -1),
    (0x2C65, 0x2C65, 1, -10795),
    (0x2C66, 0x2C66, 1, -10792),
    (0x2C68, 0x2C6C, 2, -1),
    (0x2C73, 0x2C73, 1, -1),
    (0x2C76, 0x2C76, 1, -1),
    (0x2C81, 0x2CE3, 2, -1),
    (0x2CEC, 0x2CEE, 2, -1),
    (0x2CF3, 0x2CF3, 1, -1),
    (0x2D00, 0x2D25, 1, -7264),
    (0x2D27, 0x2D27, 1, -7264),
    (0x2D2D, 0x2D2D, 1, -7264),
    (0xA641, 0xA66D, 2, -1),
    (0xA681, 0xA69B, 2, -1),
    (0xA723, 0xA72F, 2, -1),
    (0xA733, 0xA76F, 2, -1),
    (0xA77A, 0xA77C, 2, -1),
    (0xA77F, 0xA787, 2, -1),
    (0xA78C, 0xA78C, 1, -1),
    (0xA791, 0xA793, 2, -1),
    (0xA794, 0xA794, 1, 48),
    (0xA797, 0xA7A9, 2, -1),
    (0xA7B5, 0xA7C3, 2, -1),
    (0xA7C8, 0xA7CA, 2, -1),
    (0xA7D1, 0xA7D1, 1, -1),
    (0xA7D7, 0xA7D9, 2, -1),
    (0xA7F6, 0xA7F6, 1, -1),
    (0xAB53, 0xAB53, 1, -928),
    (0xAB70, 0xABBF, 1, -38864),
    (0xFF41, 0xFF5A, 1, -32),
    (0x10428, 0x1044F, 1, -40),
    (0x104D8, 0x104FB, 1, -40),
    (0x10597, 0x105A1, 1, -39),
    (0x105A3, 0x105B1, 1, -39),
    (0x105B3, 0x105B9, 1, -39),
    (0x105BB, 0x105BC, 1, -39),
    (0x10CC0, 0x10CF2, 1, -64),
    (0x118C0, 0x118DF, 1, -32),
    (0x16E60, 0x16E7F, 1, -32),
    (0x1E922, 0x1E943, 1, -34),
];

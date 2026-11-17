//! Minecraft 1.21.1 global dimension height bounds used in mod configuration.
//!
//! Official `dwy.<clinit>` derives these from `jd.PACKED_Y_LENGTH`: BlockPos uses
//! 26 bits each for X/Z and 12 for Y, then reserves 32 vertical positions.
//! This is not a loaded dimension's minY/height or a world simulation.

use mmga_runtime::{Rt, V};
const DIMENSION: &str = "net/minecraft/world/level/dimension/DimensionType";

pub fn get_static_field(_rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner != DIMENSION || desc != "I" {
        return None;
    }
    const BITS_FOR_Y: i32 = 64 - 26 - 26;
    const Y_SIZE: i32 = (1 << BITS_FOR_Y) - 32;
    const MAX_Y: i32 = (Y_SIZE >> 1) - 1;
    const MIN_Y: i32 = MAX_Y - Y_SIZE + 1;
    Some(V::I(match name {
        "BITS_FOR_Y" => BITS_FOR_Y,
        "MIN_HEIGHT" => 16,
        "Y_SIZE" => Y_SIZE,
        "MAX_Y" => MAX_Y,
        "MIN_Y" => MIN_Y,
        "WAY_ABOVE_MAX_Y" => MAX_Y << 4,
        "WAY_BELOW_MIN_Y" => MIN_Y << 4,
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::MemberRef;
    use mmga_runtime::{read_static_field, ExecCtx, HostApi, LinkedSpace};
    struct Host;
    impl HostApi for Host {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, owner, name, desc)
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
    }
    #[test]
    fn mekanism_salt_configuration_reads_global_height_limits() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = Host;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, expected) in [
            ("BITS_FOR_Y", 12),
            ("MIN_HEIGHT", 16),
            ("Y_SIZE", 4064),
            ("MAX_Y", 2031),
            ("MIN_Y", -2032),
            ("WAY_ABOVE_MAX_Y", 32496),
            ("WAY_BELOW_MIN_Y", -32512),
        ] {
            assert_eq!(
                read_static_field(&mut ctx, DIMENSION, name, "I"),
                V::I(expected)
            );
            assert!(ctx.rt.pending_exception.is_none());
        }
        let max = read_static_field(&mut ctx, DIMENSION, "MAX_Y", "I").i();
        let min = read_static_field(&mut ctx, DIMENSION, "MIN_Y", "I").i();
        assert_eq!(max - min - 1, 4062);
        assert!(get_static_field(ctx.rt, DIMENSION, "MAX_Y", "J").is_none());
        assert!(get_static_field(ctx.rt, DIMENSION, "CURRENT_WORLD_HEIGHT", "I").is_none());
    }
}

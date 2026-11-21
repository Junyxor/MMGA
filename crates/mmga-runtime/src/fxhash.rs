//! High-speed non-cryptographic hashing (`FxHasher`) for object fields, static tables,
//! and interned strings, plus `const fn` 64-bit FNV-1a for pre-hashed virtual method keys.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

const FX_SEED: u64 = 0x517c_c1b7_2722_0a95;

/// Word-at-a-time rotate-xor-multiply hasher (matching `rustc`'s `FxHasher`).
#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn mix(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(FX_SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.mix(u64::from_le_bytes([
                c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
            ]));
        }
        let rem = chunks.remainder();
        let n = rem.len();
        if n >= 4 {
            let lo = u32::from_le_bytes([rem[0], rem[1], rem[2], rem[3]]);
            let hi = u32::from_le_bytes([rem[n - 4], rem[n - 3], rem[n - 2], rem[n - 1]]);
            self.mix(u64::from(lo) | (u64::from(hi) << (8 * (n - 4))));
        } else if n > 0 {
            let packed = u64::from(rem[0])
                | (u64::from(rem[n / 2]) << (8 * (n / 2)))
                | (u64::from(rem[n - 1]) << (8 * (n - 1)));
            self.mix(packed);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.mix(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.mix(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.mix(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.mix(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FxHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxHashSet<K> = HashSet<K, BuildHasherDefault<FxHasher>>;

/// Compile-time and runtime 64-bit FNV-1a hash for `"methodName.descriptor"` dispatch keys.
#[inline]
pub const fn fnv1a_64(text: &str) -> u64 {
    let bytes = text.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

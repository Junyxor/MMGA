//! Unboxed JVM runtime value (`V`) and stack-allocated operand stack (`OperandStack<N>`).

use serde::{Deserialize, Serialize};

/// Unboxed JVM value used uniformly by both the Tier-0 bytecode interpreter and
/// Tier-1 AOT-compiled native Rust code.
///
/// Heap references (`V::R(id)`) index `Rt::heap`; `V::R(0)` is Java `null`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum V {
    #[default]
    Void,
    I(i32),
    J(i64),
    F(f32),
    D(f64),
    R(u32),
}

impl V {
    pub const NULL: V = V::R(0);
    pub const TRUE: V = V::I(1);
    pub const FALSE: V = V::I(0);

    #[inline]
    pub const fn bool(b: bool) -> Self {
        if b {
            Self::I(1)
        } else {
            Self::I(0)
        }
    }

    #[inline]
    pub const fn i(self) -> i32 {
        match self {
            V::I(v) => v,
            _ => 0,
        }
    }

    #[inline]
    pub const fn j(self) -> i64 {
        match self {
            V::J(v) => v,
            V::I(v) => v as i64,
            _ => 0,
        }
    }

    #[inline]
    pub const fn f(self) -> f32 {
        match self {
            V::F(v) => v,
            _ => 0.0,
        }
    }

    #[inline]
    pub const fn d(self) -> f64 {
        match self {
            V::D(v) => v,
            _ => 0.0,
        }
    }

    #[inline]
    pub const fn r(self) -> u32 {
        match self {
            V::R(id) => id,
            _ => 0,
        }
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        matches!(self, V::R(0) | V::Void)
    }

    /// JVMS Category-2 computational type (`long` and `double` occupy 2 stack words for `dup2`/`pop2`).
    #[inline]
    pub const fn is_cat2(self) -> bool {
        matches!(self, V::J(_) | V::D(_))
    }
}

/// Fixed-size, zero-heap-allocation operand stack used by AOT-generated Rust methods.
#[derive(Debug, Clone)]
pub struct OperandStack<const N: usize> {
    buf: [V; N],
    sp: usize,
}

impl<const N: usize> Default for OperandStack<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> OperandStack<N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            buf: [V::Void; N],
            sp: 0,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.sp = 0;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.sp
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.sp == 0
    }

    #[inline]
    pub fn push(&mut self, v: V) {
        if self.sp < N {
            self.buf[self.sp] = v;
            self.sp += 1;
        }
    }

    #[inline]
    pub fn pop(&mut self) -> V {
        if self.sp > 0 {
            self.sp -= 1;
            self.buf[self.sp]
        } else {
            V::Void
        }
    }

    #[inline]
    pub fn peek(&self) -> V {
        if self.sp > 0 {
            self.buf[self.sp - 1]
        } else {
            V::Void
        }
    }

    #[inline]
    pub fn pop_args(&mut self, count: usize) -> Vec<V> {
        let mut args = vec![V::Void; count];
        for idx in (0..count).rev() {
            args[idx] = self.pop();
        }
        args
    }

    #[inline]
    pub fn dup(&mut self) {
        let top = self.peek();
        self.push(top);
    }

    #[inline]
    pub fn dup_x1(&mut self) {
        let v1 = self.pop();
        let v2 = self.pop();
        self.push(v1);
        self.push(v2);
        self.push(v1);
    }

    #[inline]
    pub fn dup_x2(&mut self) {
        let v1 = self.pop();
        let v2 = self.pop();
        if v2.is_cat2() {
            self.push(v1);
            self.push(v2);
            self.push(v1);
        } else {
            let v3 = self.pop();
            self.push(v1);
            self.push(v3);
            self.push(v2);
            self.push(v1);
        }
    }

    #[inline]
    pub fn dup2(&mut self) {
        let v1 = self.pop();
        if v1.is_cat2() {
            self.push(v1);
            self.push(v1);
        } else {
            let v2 = self.pop();
            self.push(v2);
            self.push(v1);
            self.push(v2);
            self.push(v1);
        }
    }

    #[inline]
    pub fn pop2(&mut self) {
        let v1 = self.pop();
        if !v1.is_cat2() {
            let _ = self.pop();
        }
    }

    #[inline]
    pub fn swap(&mut self) {
        let v1 = self.pop();
        let v2 = self.pop();
        self.push(v1);
        self.push(v2);
    }
}

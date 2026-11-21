//! Value geometry from Minecraft 1.21.1's Shapes, VoxelShape and AABB.
//! Coordinates and occupied cells are owned JVM data, never a host collision
//! registry. Unknown shape methods and world collision queries remain refused.
use mmga_classfile::MemberRef;
use mmga_runtime::{
    invoke_member, Data, ExecCtx, LinkedSpace, NativeClassDef, NativeModuleRegistry, Rt, V,
};

const BLOCK: &str = "net/minecraft/world/level/block/Block";
const SHAPES: &str = "net/minecraft/world/phys/shapes/Shapes";
const SHAPE: &str = "net/minecraft/world/phys/shapes/VoxelShape";
const ARRAY: &str = "net/minecraft/world/phys/shapes/ArrayVoxelShape";
const CUBE: &str = "net/minecraft/world/phys/shapes/CubeVoxelShape";
const OP: &str = "net/minecraft/world/phys/shapes/BooleanOp";
const AABB: &str = "net/minecraft/world/phys/AABB";
const VEC: &str = "net/minecraft/world/phys/Vec3";
const AXIS: &str = "net/minecraft/core/Direction$Axis";
const EPSILON: f64 = 1.0e-7;
const FIELDS: [&str; 6] = ["minX", "minY", "minZ", "maxX", "maxY", "maxZ"];
const OP_NAMES: [&str; 16] = [
    "FALSE",
    "NOT_OR",
    "ONLY_SECOND",
    "NOT_FIRST",
    "ONLY_FIRST",
    "NOT_SECOND",
    "NOT_SAME",
    "NOT_AND",
    "AND",
    "SAME",
    "SECOND",
    "CAUSES",
    "FIRST",
    "CAUSED_BY",
    "OR",
    "TRUE",
];
const MAX_CELLS: usize = 2_000_000;

#[derive(Clone, Debug)]
struct Shape {
    coords: [Vec<f64>; 3],
    filled: Vec<bool>,
    cube: bool,
    canonical: bool,
}

fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if a <= b {
        a
    } else {
        b
    }
}
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else if a >= b {
        a
    } else {
        b
    }
}
fn aabb(values: [f64; 6]) -> [f64; 6] {
    let mut out = values;
    for axis in 0..3 {
        out[axis] = java_min(values[axis], values[axis + 3]);
        out[axis + 3] = java_max(values[axis], values[axis + 3]);
    }
    out
}
fn find_bits(min: f64, max: f64) -> Option<usize> {
    if min < -EPSILON || max > 1.0 + EPSILON {
        return None;
    }
    (0..=3).find(|bits| {
        let size = (1usize << bits) as f64;
        (min * size - (min * size + 0.5).floor()).abs() < EPSILON * size
            && (max * size - (max * size + 0.5).floor()).abs() < EPSILON * size
    })
}
fn op(mask: u8, first: bool, second: bool) -> bool {
    mask & (1 << ((usize::from(first) << 1) | usize::from(second))) != 0
}
impl Shape {
    fn empty() -> Self {
        Self {
            coords: std::array::from_fn(|_| vec![0.0]),
            filled: Vec::new(),
            cube: false,
            canonical: true,
        }
    }
    fn create(values: [f64; 6]) -> Self {
        if (0..3).any(|a| values[a + 3] - values[a] < EPSILON) {
            return Self::empty();
        }
        let bits = std::array::from_fn::<_, 3, _>(|a| find_bits(values[a], values[a + 3]));
        if bits.iter().any(Option::is_none) {
            return Self {
                coords: std::array::from_fn(|a| vec![values[a], values[a + 3]]),
                filled: vec![true],
                cube: false,
                canonical: false,
            };
        }
        let sizes = bits.map(|b| 1usize << b.unwrap());
        let coords =
            std::array::from_fn(|a| (0..=sizes[a]).map(|i| i as f64 / sizes[a] as f64).collect());
        let mut shape = Self {
            coords,
            filled: vec![false; sizes.iter().product()],
            cube: true,
            canonical: sizes == [1, 1, 1],
        };
        let min = std::array::from_fn::<_, 3, _>(|a| {
            (values[a] * sizes[a] as f64 + 0.5).floor() as usize
        });
        let max = std::array::from_fn::<_, 3, _>(|a| {
            (values[a + 3] * sizes[a] as f64 + 0.5).floor() as usize
        });
        for x in min[0]..max[0] {
            for y in min[1]..max[1] {
                for z in min[2]..max[2] {
                    shape.set(x, y, z, true);
                }
            }
        }
        shape
    }
    fn sizes(&self) -> [usize; 3] {
        self.coords.each_ref().map(|a| a.len() - 1)
    }
    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        let s = self.sizes();
        (x * s[1] + y) * s[2] + z
    }
    fn full(&self, x: isize, y: isize, z: isize) -> bool {
        let s = self.sizes();
        x >= 0
            && y >= 0
            && z >= 0
            && (x as usize) < s[0]
            && (y as usize) < s[1]
            && (z as usize) < s[2]
            && self.filled[self.index(x as usize, y as usize, z as usize)]
    }
    fn set(&mut self, x: usize, y: usize, z: usize, value: bool) {
        let i = self.index(x, y, z);
        self.filled[i] = value;
    }
    fn is_empty(&self) -> bool {
        !self.filled.iter().any(|v| *v)
    }
    fn extreme(&self, axis: usize, max: bool) -> f64 {
        let s = self.sizes();
        let mut edge = if max { 0 } else { s[axis] };
        for x in 0..s[0] {
            for y in 0..s[1] {
                for z in 0..s[2] {
                    if self.full(x as isize, y as isize, z as isize) {
                        let i = [x, y, z][axis];
                        edge = if max { edge.max(i + 1) } else { edge.min(i) };
                    }
                }
            }
        }
        if max && edge == 0 {
            f64::NEG_INFINITY
        } else if !max && edge >= s[axis] {
            f64::INFINITY
        } else {
            self.coords[axis][edge]
        }
    }
    fn bounds(&self) -> Option<[f64; 6]> {
        (!self.is_empty()).then(|| aabb(std::array::from_fn(|i| self.extreme(i % 3, i >= 3))))
    }
    fn boxes(&self) -> Vec<[f64; 6]> {
        let mut work = self.clone();
        let s = self.sizes();
        let mut out = Vec::new();
        // BitSetDiscreteVoxelShape.forAllBoxes merges Z strips, then X,
        // then Y, visiting Y/X/Z. Keep that observable decomposition order.
        for y in 0..s[1] {
            for x in 0..s[0] {
                let mut z = 0;
                while z < s[2] {
                    if !work.full(x as isize, y as isize, z as isize) {
                        z += 1;
                        continue;
                    }
                    let start = z;
                    while z < s[2] && work.full(x as isize, y as isize, z as isize) {
                        z += 1;
                    }
                    let strip = |shape: &Shape, xx: usize, yy: usize| {
                        (start..z).all(|zz| shape.full(xx as isize, yy as isize, zz as isize))
                    };
                    for zz in start..z {
                        work.set(x, y, zz, false);
                    }
                    let mut endx = x + 1;
                    while endx < s[0] && strip(&work, endx, y) {
                        for zz in start..z {
                            work.set(endx, y, zz, false);
                        }
                        endx += 1;
                    }
                    let mut endy = y + 1;
                    while endy < s[1] && (x..endx).all(|xx| strip(&work, xx, endy)) {
                        for xx in x..endx {
                            for zz in start..z {
                                work.set(xx, endy, zz, false);
                            }
                        }
                        endy += 1;
                    }
                    out.push(aabb([
                        self.coords[0][x],
                        self.coords[1][y],
                        self.coords[2][start],
                        self.coords[0][endx],
                        self.coords[1][endy],
                        self.coords[2][z],
                    ]));
                }
            }
        }
        out
    }
    fn moved(&self, delta: [f64; 3]) -> Self {
        if self.is_empty() {
            return Self::empty();
        }
        Self {
            coords: std::array::from_fn(|a| self.coords[a].iter().map(|v| v + delta[a]).collect()),
            filled: self.filled.clone(),
            cube: false,
            canonical: false,
        }
    }
    fn join(&self, other: &Self, mask: u8) -> Result<Self, &'static str> {
        if op(mask, false, false) {
            return Err("unbounded BooleanOp");
        }
        if self.is_empty() {
            return Ok(if op(mask, false, true) {
                other.clone()
            } else {
                Self::empty()
            });
        }
        if other.is_empty() {
            return Ok(if op(mask, true, false) {
                self.clone()
            } else {
                Self::empty()
            });
        }
        let mut axes = Vec::new();
        let mut product = 1usize;
        let mut cube = true;
        for a in 0..3 {
            let merged = merge_axis(
                &self.coords[a],
                &other.coords[a],
                op(mask, true, false),
                op(mask, false, true),
                self.cube && other.cube,
                product,
            );
            product = product.saturating_mul(merged.coords.len().saturating_sub(1));
            cube &= merged.cube;
            axes.push(merged);
        }
        if product > MAX_CELLS {
            return Err("shape grid exceeds supported memory bound");
        }
        let coords = std::array::from_fn(|a| axes[a].coords.clone());
        let mut joined = Self {
            coords,
            filled: vec![false; product],
            cube,
            canonical: false,
        };
        let s = joined.sizes();
        for x in 0..s[0] {
            for y in 0..s[1] {
                for z in 0..s[2] {
                    let p = [x, y, z];
                    let first = std::array::from_fn::<_, 3, _>(|a| axes[a].first[p[a]]);
                    let second = std::array::from_fn::<_, 3, _>(|a| axes[a].second[p[a]]);
                    let value = op(
                        mask,
                        self.full(first[0], first[1], first[2]),
                        other.full(second[0], second[1], second[2]),
                    );
                    joined.set(x, y, z, value);
                }
            }
        }
        Ok(joined)
    }
    fn optimized(&self) -> Result<Self, &'static str> {
        let mut result = Self::empty();
        for b in self.boxes() {
            result = result.join(&Self::create(b), 14)?;
        }
        Ok(result)
    }
    fn find_index(&self, axis: usize, value: f64) -> isize {
        // Mth.binarySearch on value < coordinate, including Java's NaN
        // predicate semantics. Cube and array forms agree for these grids.
        let mut low = 0;
        let mut high = self.coords[axis].len();
        while low < high {
            let mid = (low + high) / 2;
            if value < self.coords[axis][mid] {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        low as isize - 1
    }
    fn collide(&self, axis: usize, b: [f64; 6], mut distance: f64) -> f64 {
        if self.is_empty() {
            return distance;
        }
        if distance.abs() < EPSILON {
            return 0.0;
        }
        let cross = [(axis + 1) % 3, (axis + 2) % 3];
        let size = self.sizes();
        let lo = cross.map(|a| self.find_index(a, b[a] + EPSILON).max(0));
        let hi = cross.map(|a| (self.find_index(a, b[a + 3] - EPSILON) + 1).min(size[a] as isize));
        let hits = |i: isize| {
            for j in lo[0]..hi[0] {
                for k in lo[1]..hi[1] {
                    let mut p = [0; 3];
                    p[axis] = i;
                    p[cross[0]] = j;
                    p[cross[1]] = k;
                    if self.full(p[0], p[1], p[2]) {
                        return true;
                    }
                }
            }
            false
        };
        if distance > 0.0 {
            let mut i = self.find_index(axis, b[axis + 3] - EPSILON) + 1;
            while i < size[axis] as isize {
                if hits(i) {
                    let gap = self.coords[axis][i as usize] - b[axis + 3];
                    if gap >= -EPSILON {
                        distance = java_min(distance, gap);
                    }
                    return distance;
                }
                i += 1;
            }
        } else if distance < 0.0 {
            let mut i = self.find_index(axis, b[axis] + EPSILON) - 1;
            while i >= 0 {
                if hits(i) {
                    let gap = self.coords[axis][i as usize + 1] - b[axis];
                    if gap <= EPSILON {
                        distance = java_max(distance, gap);
                    }
                    return distance;
                }
                i -= 1;
            }
        }
        distance
    }
}
struct Merger {
    coords: Vec<f64>,
    first: Vec<isize>,
    second: Vec<isize>,
    cube: bool,
}
fn merge_axis(
    first: &[f64],
    second: &[f64],
    include_first: bool,
    include_second: bool,
    cubes: bool,
    product: usize,
) -> Merger {
    let (n, m) = (first.len() - 1, second.len() - 1);
    if cubes {
        fn gcd(mut a: usize, mut b: usize) -> usize {
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        }
        let length = n / gcd(n, m) * m;
        if product.saturating_mul(length) <= 256 {
            return Merger {
                coords: (0..=length).map(|i| i as f64 / length as f64).collect(),
                first: (0..=length).map(|i| (i * n / length) as isize).collect(),
                second: (0..=length).map(|i| (i * m / length) as isize).collect(),
                cube: true,
            };
        }
    }
    if first == second {
        return Merger {
            coords: first.to_vec(),
            first: (0..first.len()).map(|i| i as isize).collect(),
            second: (0..second.len()).map(|i| i as isize).collect(),
            cube: false,
        };
    }
    let mut coords = Vec::new();
    let mut a = Vec::new();
    let mut b = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut last = f64::NAN;
    while i < first.len() || j < second.len() {
        let end_first = i >= first.len();
        let end_second = j >= second.len();
        let choose_first = !end_first && (end_second || first[i] < second[j] + EPSILON);
        let v = if choose_first {
            i += 1;
            if !include_first && (j == 0 || end_second) {
                continue;
            }
            first[i - 1]
        } else {
            j += 1;
            if !include_second && (i == 0 || end_first) {
                continue;
            }
            second[j - 1]
        };
        if !(last >= v - EPSILON) {
            coords.push(v);
            a.push(i as isize - 1);
            b.push(j as isize - 1);
            last = v;
        } else {
            *a.last_mut().unwrap() = i as isize - 1;
            *b.last_mut().unwrap() = j as isize - 1;
        }
    }
    if coords.is_empty() {
        coords.push(0.0);
        a.push(-1);
        b.push(-1);
    }
    Merger {
        coords,
        first: a,
        second: b,
        cube: false,
    }
}

fn store_shape(rt: &mut Rt, shape: Shape) -> V {
    if shape.canonical {
        return canonical(rt, !shape.is_empty());
    }
    let value = rt.new_instance(if shape.cube { CUBE } else { ARRAY }, 0);
    for (axis, name) in ["$shape_x", "$shape_y", "$shape_z"].iter().enumerate() {
        let data = rt.alloc(
            "[D",
            0,
            Data::Array(shape.coords[axis].iter().copied().map(V::D).collect()),
        );
        rt.put_field(value, *name, data);
    }
    let occupied = rt.alloc(
        "[Z",
        0,
        Data::Array(shape.filled.into_iter().map(V::bool).collect()),
    );
    rt.put_field(value, "$shape_cells", occupied);
    value
}
fn load_shape(rt: &Rt, value: V) -> Option<Shape> {
    if !matches!(rt.class_of(value), Some(SHAPE | ARRAY | CUBE)) {
        return None;
    }
    let mut coords = Vec::new();
    for name in ["$shape_x", "$shape_y", "$shape_z"] {
        let Data::Array(values) = &rt.get_obj(rt.get_field(value, name))?.data else {
            return None;
        };
        coords.push(
            values
                .iter()
                .map(|v| if let V::D(v) = v { Some(*v) } else { None })
                .collect::<Option<Vec<_>>>()?,
        );
    }
    let Data::Array(values) = &rt.get_obj(rt.get_field(value, "$shape_cells"))?.data else {
        return None;
    };
    let shape = Shape {
        coords: coords.try_into().ok()?,
        filled: values.iter().map(|v| v.i() != 0).collect(),
        cube: rt.class_of(value) == Some(CUBE),
        canonical: ["EMPTY", "BLOCK"]
            .iter()
            .any(|name| rt.get_static(SHAPES, name) == Some(value)),
    };
    (shape.coords.iter().all(|v| !v.is_empty())
        && shape.filled.len() == shape.sizes().iter().product::<usize>())
    .then_some(shape)
}
fn canonical(rt: &mut Rt, full: bool) -> V {
    let name = if full { "BLOCK" } else { "EMPTY" };
    if let Some(v) = rt.get_static(SHAPES, name) {
        return v;
    }
    let shape = if full {
        Shape::create([0., 0., 0., 1., 1., 1.])
    } else {
        Shape::empty()
    };
    let value = rt.new_instance(if full { CUBE } else { ARRAY }, 0);
    for (a, name) in ["$shape_x", "$shape_y", "$shape_z"].iter().enumerate() {
        let array = rt.alloc(
            "[D",
            0,
            Data::Array(shape.coords[a].iter().copied().map(V::D).collect()),
        );
        rt.put_field(value, *name, array);
    }
    let cells = rt.alloc(
        "[Z",
        0,
        Data::Array(shape.filled.into_iter().map(V::bool).collect()),
    );
    rt.put_field(value, "$shape_cells", cells);
    rt.put_static(SHAPES, name, value);
    value
}
fn numbers(args: &[V]) -> Option<[f64; 6]> {
    args.get(..6)?
        .iter()
        .map(|v| if let V::D(n) = v { Some(*n) } else { None })
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
}
fn load_box(rt: &Rt, value: V) -> Option<[f64; 6]> {
    if rt.class_of(value) != Some(AABB) {
        return None;
    }
    FIELDS
        .map(|f| rt.get_field(value, f))
        .map(|v| if let V::D(n) = v { Some(n) } else { None })
        .into_iter()
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
}
fn store_box(rt: &mut Rt, bounds: [f64; 6]) -> V {
    let value = rt.new_instance(AABB, 0);
    for (name, v) in FIELDS.into_iter().zip(aabb(bounds)) {
        rt.put_field(value, name, V::D(v));
    }
    value
}
fn axis(rt: &Rt, value: V) -> Option<usize> {
    let v = rt.get_field(value, "$ordinal");
    matches!(
        rt.class_of(value),
        Some(
            AXIS | "net/minecraft/core/Direction$Axis$1"
                | "net/minecraft/core/Direction$Axis$2"
                | "net/minecraft/core/Direction$Axis$3"
        )
    )
    .then_some(v.i() as usize)
    .filter(|a| *a < 3)
}
fn shape_error(rt: &mut Rt, message: &str) -> V {
    rt.throw_exception("java/lang/UnsupportedOperationException", message)
}
fn invalid_shape(rt: &mut Rt) -> V {
    rt.throw_exception(
        "java/lang/IllegalArgumentException",
        "The min values need to be smaller or equals to the max values",
    )
}
fn result(rt: &mut Rt, value: Result<Shape, &'static str>) -> V {
    match value {
        Ok(value) => store_shape(rt, value),
        Err("unbounded BooleanOp") => rt.throw_exception("java/lang/IllegalArgumentException", ""),
        Err(e) => shape_error(rt, e),
    }
}
fn load_op(rt: &Rt, value: V) -> Option<u8> {
    let (kind, id, _, _) = rt.handle_of(value)?;
    (kind == "boolean_op").then_some(id as u8)
}

fn join_values(rt: &mut Rt, first: V, second: V, mask: u8, method: &str) -> Option<V> {
    let a = load_shape(rt, first)?;
    let b = load_shape(rt, second)?;
    if op(mask, false, false) {
        return Some(result(rt, Err("unbounded BooleanOp")));
    }
    if method == "joinIsNotEmpty" {
        if a.is_empty() || b.is_empty() {
            return Some(V::bool(op(mask, !a.is_empty(), !b.is_empty())));
        }
        if first == second {
            return Some(V::bool(op(mask, true, true)));
        }
        return Some(match a.join(&b, mask) {
            Ok(shape) => V::bool(!shape.is_empty()),
            Err(error) => result(rt, Err(error)),
        });
    }
    // These are reference-preserving fast paths in Shapes.joinUnoptimized.
    // join() still optimizes the returned value, so it does not generally keep
    // a noncanonical input's identity.
    let unchanged = if first == second {
        Some(if op(mask, true, true) {
            first
        } else {
            canonical(rt, false)
        })
    } else if a.is_empty() {
        Some(if op(mask, false, true) {
            second
        } else {
            canonical(rt, false)
        })
    } else if b.is_empty() {
        Some(if op(mask, true, false) {
            first
        } else {
            canonical(rt, false)
        })
    } else {
        None
    };
    if method == "joinUnoptimized" {
        return Some(unchanged.unwrap_or_else(|| result(rt, a.join(&b, mask))));
    }
    let joined = match unchanged {
        Some(value) => load_shape(rt, value)?.optimized(),
        None => a.join(&b, mask).and_then(|shape| shape.optimized()),
    };
    Some(result(rt, joined))
}

pub fn get_static_field(rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
    if owner == SHAPES {
        return match (name, desc) {
            ("EPSILON", "D") => Some(V::D(EPSILON)),
            ("BIG_EPSILON", "D") => Some(V::D(1e-6)),
            ("INFINITY", "Lnet/minecraft/world/phys/shapes/VoxelShape;") => {
                if let Some(v) = rt.get_static(SHAPES, name) {
                    return Some(v);
                }
                let v = store_shape(
                    rt,
                    Shape::create(
                        [f64::NEG_INFINITY; 3]
                            .into_iter()
                            .chain([f64::INFINITY; 3])
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap(),
                    ),
                );
                rt.put_static(SHAPES, name, v);
                Some(v)
            }
            _ => None,
        };
    }
    if owner == OP && desc == "Lnet/minecraft/world/phys/shapes/BooleanOp;" {
        let mask = OP_NAMES.iter().position(|s| *s == name)?;
        if let Some(v) = rt.get_static(OP, name) {
            return Some(v);
        }
        let v = rt.new_handle(OP, "boolean_op", mask as u64, name);
        rt.put_static(OP, name, v);
        return Some(v);
    }
    if owner == VEC && name == "ZERO" && desc == "Lnet/minecraft/world/phys/Vec3;" {
        if let Some(v) = rt.get_static(VEC, name) {
            return Some(v);
        }
        let v = rt.new_instance(VEC, 0);
        for f in ["x", "y", "z"] {
            rt.put_field(v, f, V::D(0.));
        }
        rt.put_static(VEC, name, v);
        return Some(v);
    }
    None
}
pub fn call(ctx: &mut ExecCtx<'_>, member: &MemberRef, is_static: bool, args: &[V]) -> Option<V> {
    let (owner, name, desc) = (
        member.owner.as_str(),
        member.name.as_str(),
        member.desc.as_str(),
    );
    let rt = &mut *ctx.rt;
    if is_static {
        if owner == BLOCK
            && name == "box"
            && desc == "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;"
        {
            let b = numbers(args)?.map(|v| v / 16.0);
            return Some(if (0..3).any(|a| b[a] > b[a + 3]) {
                invalid_shape(rt)
            } else {
                store_shape(rt, Shape::create(b))
            });
        }
        if owner != SHAPES {
            return None;
        }
        return match (name,desc){
            ("empty","()Lnet/minecraft/world/phys/shapes/VoxelShape;")=>Some(canonical(rt,false)),
            ("block","()Lnet/minecraft/world/phys/shapes/VoxelShape;")=>Some(canonical(rt,true)),
            ("box"|"create","(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;")=>{let b=numbers(args)?;Some(if name=="box" && (0..3).any(|a|b[a]>b[a+3]){invalid_shape(rt)}else{store_shape(rt,Shape::create(b))})},
            ("create","(Lnet/minecraft/world/phys/AABB;)Lnet/minecraft/world/phys/shapes/VoxelShape;")=>Some(store_shape(rt,Shape::create(load_box(rt,*args.first()?)?))),
            ("or","(Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/VoxelShape;)Lnet/minecraft/world/phys/shapes/VoxelShape;")=>join_values(rt,*args.first()?,*args.get(1)?,14,"join"),
            ("or","(Lnet/minecraft/world/phys/shapes/VoxelShape;[Lnet/minecraft/world/phys/shapes/VoxelShape;)Lnet/minecraft/world/phys/shapes/VoxelShape;")=>{let mut a=*args.first()?;load_shape(rt,a)?;let Data::Array(values)=&rt.get_obj(*args.get(1)?)?.data else{return None;};let values=values.clone();for value in values {a=join_values(rt,a,value,14,"join")?;if rt.pending_exception.is_some(){return Some(a);}}Some(a)},
            ("join"|"joinUnoptimized","(Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/BooleanOp;)Lnet/minecraft/world/phys/shapes/VoxelShape;")|
            ("joinIsNotEmpty","(Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/BooleanOp;)Z")=>join_values(rt,*args.first()?,*args.get(1)?,load_op(rt,*args.get(2)?)?,name),
            _=>None,
        };
    }
    let receiver = *args.first()?;
    if owner == OP && name == "apply" && desc == "(ZZ)Z" {
        return Some(V::bool(op(
            load_op(rt, receiver)?,
            args.get(1)?.i() != 0,
            args.get(2)?.i() != 0,
        )));
    }
    if owner == AABB {
        let b = if name == "<init>" {
            [0.; 6]
        } else {
            load_box(rt, receiver)?
        };
        return match (name, desc) {
            ("<init>", "(DDDDDD)V") => {
                for (f, v) in FIELDS.into_iter().zip(aabb(numbers(args.get(1..)?)?)) {
                    rt.put_field(receiver, f, V::D(v));
                }
                Some(V::Void)
            }
            ("move", "(DDD)Lnet/minecraft/world/phys/AABB;") => {
                let d = [args.get(1)?.d(), args.get(2)?.d(), args.get(3)?.d()];
                Some(store_box(rt, std::array::from_fn(|i| b[i] + d[i % 3])))
            }
            ("min" | "max", "(Lnet/minecraft/core/Direction$Axis;)D") => Some(V::D(
                b[axis(rt, *args.get(1)?)? + if name == "max" { 3 } else { 0 }],
            )),
            ("getXsize" | "getYsize" | "getZsize", "()D") => {
                let a = match name {
                    "getXsize" => 0,
                    "getYsize" => 1,
                    _ => 2,
                };
                Some(V::D(b[a + 3] - b[a]))
            }
            ("getSize", "()D") => Some(V::D((b[3] - b[0] + b[4] - b[1] + b[5] - b[2]) / 3.0)),
            ("hasNaN", "()Z") => Some(V::bool(b.iter().any(|v| v.is_nan()))),
            ("intersects", "(Lnet/minecraft/world/phys/AABB;)Z") => {
                let c = load_box(rt, *args.get(1)?)?;
                Some(V::bool((0..3).all(|a| b[a] < c[a + 3] && b[a + 3] > c[a])))
            }
            ("contains", "(DDD)Z") => {
                Some(V::bool((0..3).all(|a| {
                    args[a + 1].d() >= b[a] && args[a + 1].d() < b[a + 3]
                })))
            }
            _ => None,
        };
    }
    if owner == VEC {
        return match (name, desc) {
            ("<init>", "(DDD)V") => {
                for (f, v) in ["x", "y", "z"].into_iter().zip(args.get(1..4)?) {
                    rt.put_field(receiver, f, *v);
                }
                Some(V::Void)
            }
            _ => None,
        };
    }
    if !matches!(owner, SHAPE | ARRAY | CUBE) {
        return None;
    }
    let shape = load_shape(rt, receiver)?;
    match (name, desc) {
        ("isEmpty", "()Z") => Some(V::bool(shape.is_empty())),
        ("min" | "max", "(Lnet/minecraft/core/Direction$Axis;)D") => {
            Some(V::D(shape.extreme(axis(rt, *args.get(1)?)?, name == "max")))
        }
        ("bounds", "()Lnet/minecraft/world/phys/AABB;") => Some(match shape.bounds() {
            Some(b) => store_box(rt, b),
            None => shape_error(rt, "No bounds for empty shape."),
        }),
        ("singleEncompassing", "()Lnet/minecraft/world/phys/shapes/VoxelShape;") => {
            Some(match shape.bounds() {
                Some(b) => store_shape(rt, Shape::create(b)),
                None => canonical(rt, false),
            })
        }
        ("move", "(DDD)Lnet/minecraft/world/phys/shapes/VoxelShape;") => Some(store_shape(
            rt,
            shape.moved([args.get(1)?.d(), args.get(2)?.d(), args.get(3)?.d()]),
        )),
        ("optimize", "()Lnet/minecraft/world/phys/shapes/VoxelShape;") => {
            Some(result(rt, shape.optimized()))
        }
        ("toAabbs", "()Ljava/util/List;") => {
            let boxes = shape
                .boxes()
                .into_iter()
                .map(|b| store_box(rt, b))
                .collect();
            Some(rt.alloc("java/util/ArrayList", 0, Data::List(boxes)))
        }
        ("collide", "(Lnet/minecraft/core/Direction$Axis;Lnet/minecraft/world/phys/AABB;D)D") => {
            Some(V::D(shape.collide(
                axis(rt, *args.get(1)?)?,
                load_box(rt, *args.get(2)?)?,
                args.get(3)?.d(),
            )))
        }
        ("forAllBoxes", "(Lnet/minecraft/world/phys/shapes/Shapes$DoubleLineConsumer;)V") => {
            let consumer = *args.get(1)?;
            for b in shape.boxes() {
                let mut values = vec![consumer];
                values.extend(b.map(V::D));
                invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEINTERFACE,
                    &MemberRef::new(
                        "net/minecraft/world/phys/shapes/Shapes$DoubleLineConsumer",
                        "consume",
                        "(DDDDDD)V",
                    ),
                    &values,
                );
                if ctx.rt.pending_exception.is_some() {
                    break;
                }
            }
            Some(V::Void)
        }
        _ => None,
    }
}
pub fn register_types(linked: &mut LinkedSpace) {
    linked.register_native_module(NativeModuleRegistry {
        classes: &[
            NativeClassDef {
                name: SHAPE,
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: ARRAY,
                superclass: Some(SHAPE),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: CUBE,
                superclass: Some(SHAPE),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: AABB,
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: VEC,
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: false,
                methods: &[],
            },
            NativeClassDef {
                name: OP,
                superclass: Some("java/lang/Object"),
                interfaces: &[],
                is_interface: true,
                methods: &[],
            },
        ],
        lambdas: &[],
    });
}
pub fn is_subtype(sub: &str, sup: &str) -> bool {
    matches!((sub, sup), (ARRAY | CUBE, SHAPE))
}

#[cfg(test)]
#[path = "shapes_tests.rs"]
mod tests;

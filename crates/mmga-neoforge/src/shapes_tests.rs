use super::*;
use crate::PreparedLoad;
use mmga_classfile::op;
use mmga_runtime::{invoke_member, read_static_field, ExecCtx};

fn call_method(
    ctx: &mut ExecCtx<'_>,
    owner: &str,
    name: &str,
    desc: &str,
    is_static: bool,
    args: &[V],
) -> V {
    invoke_member(
        ctx,
        if is_static {
            op::INVOKESTATIC
        } else {
            op::INVOKEVIRTUAL
        },
        &MemberRef::new(owner, name, desc),
        args,
    )
}
#[test]
fn box_values_union_empty_and_collision_keep_the_1_21_1_boundaries() {
    let mut rt = Rt::new();
    let mut linked = LinkedSpace::new();
    register_types(&mut linked);
    crate::direction::register_types(&mut linked);
    let mut host = PreparedLoad::from_jars(Vec::new(), true).unwrap().loader;
    let mut ctx = ExecCtx {
        rt: &mut rt,
        linked: &linked,
        host: &mut host,
        depth: 0,
    };
    let block = call_method(
        &mut ctx,
        BLOCK,
        "box",
        "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;",
        true,
        &[0., 0., 0., 16., 16., 16.].map(V::D),
    );
    assert_eq!(
        load_shape(ctx.rt, block).unwrap().boxes(),
        vec![[0., 0., 0., 1., 1., 1.]]
    );
    let left = call_method(
        &mut ctx,
        BLOCK,
        "box",
        "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;",
        true,
        &[0., 0., 0., 8., 16., 16.].map(V::D),
    );
    let right = call_method(
        &mut ctx,
        BLOCK,
        "box",
        "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;",
        true,
        &[8., 0., 0., 16., 16., 16.].map(V::D),
    );
    let combined=call_method(&mut ctx,SHAPES,"or","(Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/VoxelShape;)Lnet/minecraft/world/phys/shapes/VoxelShape;",true,&[left,right]);
    assert_eq!(
        load_shape(ctx.rt, combined).unwrap().boxes(),
        vec![[0., 0., 0., 1., 1., 1.]]
    );
    let x = read_static_field(&mut ctx, AXIS, "X", "Lnet/minecraft/core/Direction$Axis;");
    let moving = store_box(ctx.rt, [-1., 0.25, 0.25, -0.25, 0.75, 0.75]);
    assert_eq!(
        call_method(
            &mut ctx,
            SHAPE,
            "collide",
            "(Lnet/minecraft/core/Direction$Axis;Lnet/minecraft/world/phys/AABB;D)D",
            false,
            &[combined, x, moving, V::D(2.)]
        ),
        V::D(0.25)
    );
    let empty = call_method(
        &mut ctx,
        SHAPES,
        "create",
        "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;",
        true,
        &[0., 0., 0., 1e-8, 1., 1.].map(V::D),
    );
    assert!(load_shape(ctx.rt, empty).unwrap().is_empty());
    call_method(
        &mut ctx,
        SHAPE,
        "bounds",
        "()Lnet/minecraft/world/phys/AABB;",
        false,
        &[empty],
    );
    let exception = ctx.rt.take_exception().unwrap();
    assert!(ctx
        .rt
        .exception_summary(exception)
        .contains("No bounds for empty shape"));
    call_method(
        &mut ctx,
        BLOCK,
        "box",
        "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;",
        true,
        &[12., 0., 0., 4., 16., 16.].map(V::D),
    );
    let exception = ctx.rt.take_exception().unwrap();
    assert!(ctx
        .rt
        .exception_summary(exception)
        .contains("IllegalArgumentException"));
    let bad = ctx.rt.new_instance(SHAPE, 0);
    assert!(
        super::call(
            &mut ctx,
            &MemberRef::new(SHAPE, "isEmpty", "()Z"),
            false,
            &[bad]
        )
        .is_none(),
        "uninitialized or foreign shape cannot become an empty shape"
    );
    assert!(super::call(
        &mut ctx,
        &MemberRef::new(
            SHAPES,
            "unknown",
            "()Lnet/minecraft/world/phys/shapes/VoxelShape;"
        ),
        true,
        &[]
    )
    .is_none());
}

fn number(value: &serde_json::Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn raw(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn identity_snapshot() -> serde_json::Value {
    let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
    let mut rt = Rt::new();
    let mut ctx = ExecCtx {
        rt: &mut rt,
        linked: &prepared.linked,
        host: &mut prepared.loader,
        depth: 0,
    };
    let descriptor = "(DDDDDD)Lnet/minecraft/world/phys/shapes/VoxelShape;";
    let block = canonical(ctx.rt, true);
    let created = call_method(
        &mut ctx,
        SHAPES,
        "create",
        descriptor,
        true,
        &[0., 0., 0., 1., 1., 1.].map(V::D),
    );
    let left = call_method(
        &mut ctx,
        SHAPES,
        "box",
        descriptor,
        true,
        &[0., 0., 0., 0.5, 1., 1.].map(V::D),
    );
    let right = call_method(
        &mut ctx,
        SHAPES,
        "box",
        descriptor,
        true,
        &[0.5, 0., 0., 1., 1., 1.].map(V::D),
    );
    let empty = canonical(ctx.rt, false);
    let or = read_static_field(
        &mut ctx,
        OP,
        "OR",
        "Lnet/minecraft/world/phys/shapes/BooleanOp;",
    );
    let and = read_static_field(
        &mut ctx,
        OP,
        "AND",
        "Lnet/minecraft/world/phys/shapes/BooleanOp;",
    );
    let join_desc = "(Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/VoxelShape;Lnet/minecraft/world/phys/shapes/BooleanOp;)Lnet/minecraft/world/phys/shapes/VoxelShape;";
    let mut same = Vec::new();
    for inputs in [[left, left, or], [left, empty, or], [empty, left, or]] {
        same.push(
            call_method(
                &mut ctx,
                SHAPES,
                "joinUnoptimized",
                join_desc,
                true,
                &inputs,
            ) == left,
        );
    }
    let full = call_method(
        &mut ctx,
        SHAPES,
        "joinUnoptimized",
        join_desc,
        true,
        &[left, right, or],
    );
    let optimized = call_method(
        &mut ctx,
        SHAPE,
        "optimize",
        "()Lnet/minecraft/world/phys/shapes/VoxelShape;",
        false,
        &[full],
    );
    let disjoint = call_method(
        &mut ctx,
        SHAPES,
        "joinUnoptimized",
        join_desc,
        true,
        &[left, right, and],
    );
    let no_more = ctx.rt.alloc(
        "[Lnet/minecraft/world/phys/shapes/VoxelShape;",
        0,
        Data::Array(vec![]),
    );
    let varargs = call_method(&mut ctx, SHAPES, "or", "(Lnet/minecraft/world/phys/shapes/VoxelShape;[Lnet/minecraft/world/phys/shapes/VoxelShape;)Lnet/minecraft/world/phys/shapes/VoxelShape;", true, &[left, no_more]);
    assert!(
        ctx.rt.pending_exception.is_none(),
        "{:?}",
        ctx.rt.pending_exception
    );
    serde_json::json!({
        "create_is_block": created == block,
        "self_or_is_self": same[0],
        "or_empty_is_self": same[1],
        "empty_or_is_self": same[2],
        "joined_full_is_block": full == block,
        "optimized_full_is_block": optimized == block,
        "joined_empty_is_empty": load_shape(ctx.rt, disjoint).unwrap().is_empty(),
        "joined_empty_is_canonical": disjoint == empty,
        "empty_varargs_is_self": varargs == left,
    })
}

#[test]
fn created_shapes_and_unoptimized_fast_paths_keep_vanilla_reference_identity() {
    assert_eq!(
        identity_snapshot(),
        serde_json::json!({
            "create_is_block": true,
            "self_or_is_self": true,
            "or_empty_is_self": true,
            "empty_or_is_self": true,
            "joined_full_is_block": false,
            "optimized_full_is_block": true,
            "joined_empty_is_empty": true,
            "joined_empty_is_canonical": false,
            "empty_varargs_is_self": true,
        })
    );
}

fn snapshot(shape: &Shape) -> serde_json::Value {
    let mut out = serde_json::json!({
        "empty":shape.is_empty(),
        "boxes":shape.boxes().into_iter().map(|b|b.map(raw)).collect::<Vec<_>>(),
        "min":std::array::from_fn::<_,3,_>(|a|raw(shape.extreme(a,false))),
        "max":std::array::from_fn::<_,3,_>(|a|raw(shape.extreme(a,true))),
    });
    if let Some(b) = shape.bounds() {
        out["bounds"] = serde_json::json!(b.map(raw));
    } else {
        out["bounds_error"] = "UnsupportedOperationException: No bounds for empty shape.".into();
    }
    out
}
fn without_class(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("class");
    }
    value
}
#[test]
#[ignore = "needs MMGA_SHAPE_ORACLE_JSON from the actual Minecraft 1.21.1 JAR"]
fn installed_1_21_1_shape_oracle() {
    let path = std::env::var_os("MMGA_SHAPE_ORACLE_JSON").expect("MMGA_SHAPE_ORACLE_JSON");
    let oracle: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(oracle["minecraft"], "1.21.1");
    assert_eq!(identity_snapshot(), oracle["identities"]);
    for (i, case) in oracle["cases"].as_array().unwrap().iter().enumerate() {
        let coords = std::array::from_fn(|a| number(&case["input"][a]));
        for kind in ["create", "box", "block_box"] {
            let b = if kind == "block_box" {
                coords.map(|v| v / 16.)
            } else {
                coords
            };
            let actual = if kind != "create" && (0..3).any(|a| b[a] > b[a + 3]) {
                serde_json::json!({"error":"IllegalArgumentException: The min values need to be smaller or equals to the max values"})
            } else {
                snapshot(&Shape::create(b))
            };
            assert_eq!(actual, without_class(case[kind].clone()), "case {i}/{kind}");
        }
    }
    let values = [
        Shape::empty(),
        Shape::create([0., 0., 0., 1., 1., 1.]),
        Shape::create([0., 0., 0., 0.5, 1., 1.]),
        Shape::create([0.5, 0., 0., 1., 1., 1.]),
        Shape::create([0.25, 0.25, 0.25, 0.75, 0.75, 0.75]),
        Shape::create([0.5, 1e-8, 0., 1., 1., 1.]),
        Shape::create([0.5000001, 0., 0., 1., 1., 1.]),
        Shape::create([-1., -1., -1., 2., 2., 2.]),
    ];
    for item in oracle["joins"].as_array().unwrap() {
        let a = item["a"].as_u64().unwrap() as usize;
        let b = item["b"].as_u64().unwrap() as usize;
        let mask = OP_NAMES
            .iter()
            .position(|name| *name == item["op"].as_str().unwrap())
            .unwrap() as u8;
        let joined = values[a].join(&values[b], mask).and_then(|v| v.optimized());
        if op(mask, false, false) {
            assert!(joined.is_err());
            continue;
        }
        let joined = joined.unwrap();
        assert_eq!(
            snapshot(&joined),
            without_class(item["shape"].clone()),
            "join {a}/{b}/{}",
            item["op"]
        );
        assert_eq!(!joined.is_empty(), item["not_empty"].as_bool().unwrap());
    }
    let boxes = [
        [-1., 0.25, 0.25, 0., 0.75, 0.75],
        [-1., 0.25, 0.25, -0.25, 0.75, 0.75],
        [1., 0.25, 0.25, 2., 0.75, 0.75],
        [0.25, 0.25, 0.25, 0.75, 0.75, 0.75],
        [-1., 1., 0., 0., 2., 1.],
        [-1., 1. - 1e-8, 0., 0., 2., 1.],
        [-1., 1. - 2e-7, 0., 0., 2., 1.],
    ];
    for item in oracle["collisions"].as_array().unwrap() {
        let s = item["shape"].as_u64().unwrap() as usize;
        let a = item["axis"].as_u64().unwrap() as usize;
        let b = item["box"].as_u64().unwrap() as usize;
        let result = values[s].collide(a, boxes[b], number(&item["distance"]));
        assert_eq!(
            raw(result),
            item["result"].as_str().unwrap(),
            "collision {item}"
        );
    }
    for item in oracle["moves"].as_array().unwrap() {
        let s = item["shape"].as_u64().unwrap() as usize;
        assert_eq!(
            snapshot(&values[s].moved([-0.5, 2., 0.125])),
            without_class(item["moved"].clone())
        );
        assert_eq!(
            snapshot(&values[s].optimized().unwrap()),
            without_class(item["optimized"].clone())
        );
    }
}

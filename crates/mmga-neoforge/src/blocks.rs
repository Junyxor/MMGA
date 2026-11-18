//! Verified local block definitions for Create's vanilla display associations.
//!
//! Minecraft 1.21.1 official bytecode: Blocks=dga, RespawnAnchorBlock=dmh,
//! TargetBlock=dog, DispenserBlock=did, DropperBlock=dij, StateDefinition=dtd,
//! StateHolder=dte, IntegerProperty=duc, BooleanProperty=dtt, DirectionProperty=dtw. Client SHA-1
//! 30c73b1c5da787909b2f73340419fdf13b9def88; mappings SHA-1
//! 2244b6f072256667bcd9a73df124d6c58de77992. This is not a complete vanilla
//! block/state registry: global IDs, enumeration, shapes and world actions fail.

use crate::{ContentProperties, RegistryStore};
use mmga_classfile::MemberRef;
use mmga_runtime::{box_primitive, Data, Rt, V};

const BLOCK: &str = "net/minecraft/world/level/block/Block";
const BLOCKS: &str = "net/minecraft/world/level/block/Blocks";
const STATE: &str = "net/minecraft/world/level/block/state/BlockState";
const DEFINITION: &str = "net/minecraft/world/level/block/state/StateDefinition";
const PROPERTY: &str = "net/minecraft/world/level/block/state/properties/Property";
const INTEGER_PROPERTY: &str = "net/minecraft/world/level/block/state/properties/IntegerProperty";
const STATE_PROPERTIES: &str =
    "net/minecraft/world/level/block/state/properties/BlockStateProperties";
const ANCHOR: &str = "net/minecraft/world/level/block/RespawnAnchorBlock";
const TARGET: &str = "net/minecraft/world/level/block/TargetBlock";
const DISPENSER: &str = "net/minecraft/world/level/block/DispenserBlock";
const DROPPER: &str = "net/minecraft/world/level/block/DropperBlock";
const DIRECTIONAL: &str = "net/minecraft/world/level/block/DirectionalBlock";
const BOOLEAN_PROPERTY: &str = "net/minecraft/world/level/block/state/properties/BooleanProperty";
const DIRECTION_PROPERTY: &str =
    "net/minecraft/world/level/block/state/properties/DirectionProperty";
const PISTON_TYPE: &str = "net/minecraft/world/level/block/state/properties/PistonType";
const BEHAVIOUR: &str = "net/minecraft/world/level/block/state/BlockBehaviour";
const STATE_BASE: &str = "net/minecraft/world/level/block/state/BlockBehaviour$BlockStateBase";
const STATE_HOLDER: &str = "net/minecraft/world/level/block/state/StateHolder";
const BUILDER: &str = "net/minecraft/world/level/block/state/StateDefinition$Builder";
const CREATE_BLOCK_STATES_DESC: &str =
    "(Lnet/minecraft/world/level/block/state/StateDefinition$Builder;)V";
const BLOCK_DESC: &str = "Lnet/minecraft/world/level/block/Block;";
const PROPERTY_DESC: &str = "Lnet/minecraft/world/level/block/state/properties/IntegerProperty;";
const LOCAL_LIST: &str = "com/google/common/collect/RegularImmutableList";
const LOCAL_SET: &str = "com/google/common/collect/RegularImmutableSet";
const LOCAL_COLLECTION: &str = "java/util/Collections$UnmodifiableCollection";

struct BlockDefinition {
    name: &'static str,
    id: &'static str,
    class: &'static str,
    property: &'static str,
    max: i32,
    hardness: f32,
    resistance: f32,
    correct_tool: bool,
    map_color: &'static str,
}

const DEFINITIONS: [BlockDefinition; 4] = [
    BlockDefinition {
        name: "RESPAWN_ANCHOR",
        id: "minecraft:respawn_anchor",
        class: ANCHOR,
        property: "charges",
        max: 4,
        hardness: 50.0,
        resistance: 1200.0,
        correct_tool: true,
        map_color: "COLOR_BLACK",
    },
    BlockDefinition {
        name: "TARGET",
        id: "minecraft:target",
        class: TARGET,
        property: "power",
        max: 15,
        hardness: 0.5,
        resistance: 0.5,
        correct_tool: false,
        map_color: "QUARTZ",
    },
    BlockDefinition {
        name: "DISPENSER",
        id: "minecraft:dispenser",
        class: DISPENSER,
        property: "",
        max: 0,
        hardness: 3.5,
        resistance: 3.5,
        correct_tool: true,
        map_color: "STONE",
    },
    BlockDefinition {
        name: "DROPPER",
        id: "minecraft:dropper",
        class: DROPPER,
        property: "",
        max: 0,
        hardness: 3.5,
        resistance: 3.5,
        correct_tool: true,
        map_color: "STONE",
    },
];

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
    if owner == PISTON_TYPE
        && desc == "Lnet/minecraft/world/level/block/state/properties/PistonType;"
    {
        return piston_type(rt, name);
    }
    if owner == BLOCK && name == "BLOCK_STATE_REGISTRY" && desc == "Lnet/minecraft/core/IdMapper;" {
        return Some(unsupported(
            rt,
            "complete global Block.BLOCK_STATE_REGISTRY and protocol IDs",
        ));
    }
    if owner == BLOCKS && desc == BLOCK_DESC {
        return DEFINITIONS
            .iter()
            .find(|definition| definition.name == name)
            .map(|definition| block_constant(rt, registries, definition));
    }
    if desc == PROPERTY_DESC {
        return match (owner, name) {
            (ANCHOR, "CHARGE") | (STATE_PROPERTIES, "RESPAWN_ANCHOR_CHARGES") => {
                Some(property(rt, "charges", 4))
            }
            (TARGET, "OUTPUT_POWER") | (STATE_PROPERTIES, "POWER") => {
                Some(property(rt, "power", 15))
            }
            _ => None,
        };
    }
    if desc == "Lnet/minecraft/world/level/block/state/properties/DirectionProperty;"
        && name == "FACING"
        && matches!(owner, DISPENSER | DROPPER | DIRECTIONAL | STATE_PROPERTIES)
    {
        return Some(dispenser_property(rt, "facing"));
    }
    if owner == STATE_PROPERTIES
        && name == "HORIZONTAL_FACING"
        && desc == "Lnet/minecraft/world/level/block/state/properties/DirectionProperty;"
    {
        return Some(horizontal_facing_property(rt));
    }
    if desc == "Lnet/minecraft/world/level/block/state/properties/BooleanProperty;" {
        return match (owner, name) {
            (DISPENSER | DROPPER | STATE_PROPERTIES, "TRIGGERED") => {
                Some(dispenser_property(rt, "triggered"))
            }
            (STATE_PROPERTIES, "OPEN") => Some(state_boolean_property(rt, "OPEN", "open")),
            (STATE_PROPERTIES, "WATERLOGGED") => {
                Some(state_boolean_property(rt, "WATERLOGGED", "waterlogged"))
            }
            _ => None,
        };
    }
    if owner == ANCHOR && desc == "I" {
        return match name {
            "MIN_CHARGES" => Some(V::I(0)),
            "MAX_CHARGES" => Some(V::I(4)),
            _ => None,
        };
    }
    None
}

pub fn call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    let name = member.name.as_str();
    let desc = member.desc.as_str();
    let owner = member.owner.as_str();
    if owner == PISTON_TYPE {
        return match (name, desc, is_static) {
            (
                "values",
                "()[Lnet/minecraft/world/level/block/state/properties/PistonType;",
                true,
            ) => {
                let values = ["DEFAULT", "STICKY"]
                    .into_iter()
                    .map(|name| piston_type(rt, name).unwrap())
                    .collect();
                Some(rt.alloc(
                    "[Lnet/minecraft/world/level/block/state/properties/PistonType;",
                    0,
                    Data::Array(values),
                ))
            }
            (
                "valueOf",
                "(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/PistonType;",
                true,
            ) => {
                let name = *args.first()?;
                if name.is_null() {
                    return Some(
                        rt.throw_exception("java/lang/NullPointerException", "Name is null"),
                    );
                }
                let name = rt.str_of(name)?.to_owned();
                Some(piston_type(rt,&name).unwrap_or_else(||rt.throw_exception("java/lang/IllegalArgumentException",&format!("No enum constant net.minecraft.world.level.block.state.properties.PistonType.{name}"))))
            }
            ("getSerializedName" | "toString", "()Ljava/lang/String;", false) => {
                Some(rt.get_field(*args.first()?, "name"))
            }
            _ => None,
        };
    }
    if let Some(value) = registry_call(rt, registries, member, is_static, args) {
        return Some(value);
    }
    if is_static {
        return match (owner, name, desc) {
            (
                BOOLEAN_PROPERTY,
                "create",
                "(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/BooleanProperty;",
            ) => Some(create_boolean_property(rt, *args.first()?)),
            (BLOCK, "getId", "(Lnet/minecraft/world/level/block/state/BlockState;)I")
            | (BLOCK, "stateById", "(I)Lnet/minecraft/world/level/block/state/BlockState;") => {
                Some(unsupported(rt, "global vanilla block-state protocol IDs"))
            }
            (
                ANCHOR,
                "getScaledChargeLevel",
                "(Lnet/minecraft/world/level/block/state/BlockState;I)I",
            ) => {
                let state = *args.first()?;
                let charge = property(rt, "charges", 4);
                if rt.get_field(state, "$property") != charge {
                    return Some(rt.throw_exception(
                        "java/lang/IllegalArgumentException",
                        "state has no charges property",
                    ));
                }
                Some(V::I(
                    (rt.get_field(state, "$propertyValue").i() as f32 / 4.0
                        * args.get(1)?.i() as f32)
                        .floor() as i32,
                ))
            }
            _ => None,
        };
    }
    let recv = *args.first()?;
    if !rt.get_field(recv, "$blockCollection").is_null() {
        return collection_call(rt, recv, name, desc, &args[1..]);
    }
    if matches!(
        rt.class_of(recv),
        Some(ANCHOR | TARGET | DISPENSER | DROPPER)
    ) {
        return match (name, desc) {
            ("defaultBlockState", "()Lnet/minecraft/world/level/block/state/BlockState;") => {
                Some(rt.get_field(recv, "defaultBlockState"))
            }
            ("getStateDefinition", "()Lnet/minecraft/world/level/block/state/StateDefinition;") => {
                Some(rt.get_field(recv, "stateDefinition"))
            }
            ("builtInRegistryHolder", "()Lnet/minecraft/core/Holder$Reference;") => {
                Some(rt.get_field(recv, "builtInRegistryHolder"))
            }
            ("asBlock", "()Lnet/minecraft/world/level/block/Block;") => Some(recv),
            ("defaultDestroyTime", "()F") => Some(rt.get_field(recv, "$destroyTime")),
            ("getExplosionResistance", "()F") => Some(rt.get_field(recv, "explosionResistance")),
            ("getFriction", "()F") => Some(rt.get_field(recv, "friction")),
            ("getSpeedFactor", "()F") => Some(rt.get_field(recv, "speedFactor")),
            ("getJumpFactor", "()F") => Some(rt.get_field(recv, "jumpFactor")),
            ("hasDynamicShape", "()Z") => Some(V::FALSE),
            ("defaultMapColor", "()Lnet/minecraft/world/level/material/MapColor;") => {
                Some(rt.get_field(recv, "$mapColor"))
            }
            ("getDescriptionId", "()Ljava/lang/String;") => {
                Some(rt.get_field(recv, "descriptionId"))
            }
            ("toString", "()Ljava/lang/String;") => {
                let id = rt.str_of(rt.get_field(recv, "$blockId"))?;
                let text = format!("Block{{{id}}}");
                Some(rt.new_str(text))
            }
            _ => None,
        };
    }
    if rt.class_of(recv) == Some(STATE)
        && (!rt.get_field(recv, "$property").is_null()
            || !rt.get_field(recv, "$stateValues").is_null())
    {
        return state_call(rt, recv, name, desc, &args[1..]);
    }
    if rt.class_of(recv) == Some(DEFINITION) && !rt.get_field(recv, "$properties").is_null() {
        return match (name, desc) {
            ("getPossibleStates", "()Lcom/google/common/collect/ImmutableList;") => {
                Some(rt.get_field(recv, "states"))
            }
            ("getOwner", "()Ljava/lang/Object;") => Some(rt.get_field(recv, "owner")),
            ("any", "()Lnet/minecraft/world/level/block/state/StateHolder;") => {
                Some(rt.get_field(recv, "$first"))
            }
            ("getProperties", "()Ljava/util/Collection;") => {
                Some(rt.get_field(recv, "$properties"))
            }
            (
                "getProperty",
                "(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/Property;",
            ) => {
                let wanted = rt.str_of(*args.get(1)?);
                Some(
                    collection_items(rt, rt.get_field(recv, "$properties"))
                        .into_iter()
                        .find(|property| wanted == rt.str_of(rt.get_field(*property, "name")))
                        .unwrap_or(V::NULL),
                )
            }
            _ => None,
        };
    }
    if rt.class_of(recv) == Some(INTEGER_PROPERTY)
        && !rt.get_field(recv, "$possibleValues").is_null()
    {
        return property_call(rt, recv, name, desc, &args[1..]);
    }
    if matches!(
        rt.class_of(recv),
        Some(BOOLEAN_PROPERTY | DIRECTION_PROPERTY)
    ) && !rt.get_field(recv, "$possibleValues").is_null()
    {
        return dispenser_property_call(rt, recv, name, desc, &args[1..]);
    }
    None
}

/// Handles the verified Block constructor path with access to the linked mod bytecode.
/// The official Block constructor invokes the overridable state-definition hook before
/// storing the resulting definition and its first state as the default.
pub fn call_context(
    ctx: &mut mmga_runtime::ExecCtx<'_>,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static {
        return None;
    }
    let recv = *args.first()?;
    if ctx.rt.class_of(recv) == Some(STATE)
        && ctx.rt.get_field(recv, "$genericBlockState") == V::TRUE
        && ctx.linked.is_subtype(&member.owner, STATE_BASE, ctx.host)
        && member.name == "getSoundType"
        && member.desc == "()Lnet/minecraft/world/level/block/SoundType;"
    {
        let block = ctx.rt.get_field(recv, "owner");
        return Some(mmga_runtime::invoke_member(
            ctx,
            mmga_classfile::op::INVOKEVIRTUAL,
            &MemberRef::new(
                BLOCK,
                "getSoundType",
                "(Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/SoundType;",
            ),
            &[block, recv],
        ));
    }
    match (member.owner.as_str(), member.name.as_str(), member.desc.as_str()) {
        (
            BLOCK,
            "<init>",
            "(Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;)V",
        ) => Some(initialize_block(ctx, recv, *args.get(1)?)),
        (BUILDER, "<init>", "(Ljava/lang/Object;)V") => {
            ctx.rt.put_field(recv, "owner", *args.get(1)?);
            put_builder_properties(ctx.rt, recv, Vec::new());
            Some(V::Void)
        }
        (BUILDER, "add", "([Lnet/minecraft/world/level/block/state/properties/Property;)Lnet/minecraft/world/level/block/state/StateDefinition$Builder;") => {
            let builder = recv;
            let property_array = *args.get(1)?;
            let Some(array) = ctx.rt.get_obj(property_array) else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/NullPointerException",
                    "StateDefinition.Builder.add properties",
                ));
            };
            let Data::Array(new_properties) = &array.data else {
                return Some(ctx.rt.throw_exception(
                    "java/lang/ClassCastException",
                    "StateDefinition.Builder.add requires Property[]",
                ));
            };
            let new_properties = new_properties.clone();
            for property in new_properties {
                if let Some(error) = add_builder_property(ctx, builder, property) {
                    return Some(error);
                }
            }
            Some(builder)
        }
        (BLOCK, "createBlockStateDefinition", CREATE_BLOCK_STATES_DESC) => Some(V::Void),
        _ => inherited_block_call(ctx, member, recv, &args[1..]),
    }
}

fn initialize_block(ctx: &mut mmga_runtime::ExecCtx<'_>, block: V, properties: V) -> V {
    if properties.is_null() {
        return ctx.rt.throw_exception("java/lang/NullPointerException", "Block properties");
    }
    for field in [
        "hasCollision", "drops", "explosionResistance", "isRandomlyTicking", "soundType",
        "friction", "speedFactor", "jumpFactor", "dynamicShape", "requiredFeatures",
    ] {
        let value = ctx.rt.get_field(properties, field);
        ctx.rt.put_field(block, field, value);
    }
    ctx.rt.put_field(block, "properties", properties);
    let registry = mmga_runtime::read_static_field(
        ctx,
        "net/minecraft/core/registries/BuiltInRegistries",
        "BLOCK",
        "Lnet/minecraft/core/DefaultedRegistry;",
    );
    if ctx.rt.pending_exception.is_some() { return V::NULL; }
    let holder = mmga_runtime::invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new("net/minecraft/core/Registry", "createIntrusiveHolder", "(Ljava/lang/Object;)Lnet/minecraft/core/Holder$Reference;"),
        &[registry, block],
    );
    if ctx.rt.pending_exception.is_some() { return V::NULL; }
    ctx.rt.put_field(block, "builtInRegistryHolder", holder);
    let builder = ctx.rt.new_instance(BUILDER, 0);
    ctx.rt.put_field(builder, "owner", block);
    put_builder_properties(ctx.rt, builder, Vec::new());
    let hook = MemberRef::new(BLOCK, "createBlockStateDefinition", CREATE_BLOCK_STATES_DESC);
    let _ = mmga_runtime::invoke_member(ctx, mmga_classfile::op::INVOKEVIRTUAL, &hook, &[block, builder]);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let definition = build_state_definition(ctx, block, builder);
    if ctx.rt.pending_exception.is_some() {
        return V::NULL;
    }
    let default_state = ctx.rt.get_field(definition, "$first");
    ctx.rt.put_field(block, "stateDefinition", definition);
    ctx.rt.put_field(block, "defaultBlockState", default_state);
    ctx.rt.put_field(block, "$constructedBlock", V::TRUE);
    V::Void
}

fn inherited_block_call(
    ctx: &mut mmga_runtime::ExecCtx<'_>,
    member: &MemberRef,
    recv: V,
    args: &[V],
) -> Option<V> {
    if !ctx.linked.is_subtype(&member.owner, BLOCK, ctx.host) {
        return None;
    }
    match (member.name.as_str(), member.desc.as_str()) {
        ("registerDefaultState", "(Lnet/minecraft/world/level/block/state/BlockState;)V") => {
            ctx.rt.put_field(recv, "defaultBlockState", *args.first()?);
            Some(V::Void)
        }
        ("defaultBlockState", "()Lnet/minecraft/world/level/block/state/BlockState;") => Some(ctx.rt.get_field(recv, "defaultBlockState")),
        ("getStateDefinition", "()Lnet/minecraft/world/level/block/state/StateDefinition;") => Some(ctx.rt.get_field(recv, "stateDefinition")),
        ("builtInRegistryHolder", "()Lnet/minecraft/core/Holder$Reference;") => Some(ctx.rt.get_field(recv, "builtInRegistryHolder")),
        ("properties", "()Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;") => Some(ctx.rt.get_field(recv, "properties")),
        ("defaultDestroyTime", "()F") => Some(ctx.rt.get_field(ctx.rt.get_field(recv, "properties"), "destroyTime")),
        ("useShapeForLightOcclusion", "(Lnet/minecraft/world/level/block/state/BlockState;)Z") => Some(V::FALSE),
        ("getSoundType", "(Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/SoundType;") => Some(ctx.rt.get_field(recv, "soundType")),
        _ => None,
    }
}

fn builder_properties(rt: &Rt, builder: V) -> Vec<V> {
    match rt.get_obj(rt.get_field(builder, "$properties")).map(|object| &object.data) {
        Some(Data::Array(properties)) => properties.clone(),
        _ => Vec::new(),
    }
}

fn put_builder_properties(rt: &mut Rt, builder: V, properties: Vec<V>) {
    let properties = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(properties));
    rt.put_field(builder, "$properties", properties);
}

fn add_builder_property(
    ctx: &mut mmga_runtime::ExecCtx<'_>,
    builder: V,
    property: V,
) -> Option<V> {
    let name = mmga_runtime::invoke_member(
        ctx,
        mmga_classfile::op::INVOKEVIRTUAL,
        &MemberRef::new(PROPERTY, "getName", "()Ljava/lang/String;"),
        &[property],
    );
    if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
    let Some(name) = ctx.rt.str_of(name).map(str::to_owned) else {
        return Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Property name"));
    };
    if !valid_property_name(&name) {
        return Some(ctx.rt.throw_exception(
            "java/lang/IllegalArgumentException",
            &format!("Block {0} has invalidly named property: {name}", ctx.rt.value_to_string(ctx.rt.get_field(builder, "owner"))),
        ));
    }
    let values = mmga_runtime::invoke_member(
        ctx,
        mmga_classfile::op::INVOKEVIRTUAL,
        &MemberRef::new(PROPERTY, "getPossibleValues", "()Ljava/util/Collection;"),
        &[property],
    );
    if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
    if ctx.rt.get_field(values, "$blockCollection").is_null() {
        return Some(unsupported(ctx.rt, "state properties with unmodeled possible-value collections"));
    }
    let values = collection_items(ctx.rt, values);
    if values.len() <= 1 {
        return Some(ctx.rt.throw_exception(
            "java/lang/IllegalArgumentException",
            &format!("Block {0} attempted use property {name} with <= 1 possible values", ctx.rt.value_to_string(ctx.rt.get_field(builder, "owner"))),
        ));
    }
    for value in values {
        let value_name = mmga_runtime::invoke_member(
            ctx,
            mmga_classfile::op::INVOKEVIRTUAL,
            &MemberRef::new(PROPERTY, "getName", "(Ljava/lang/Comparable;)Ljava/lang/String;"),
            &[property, value],
        );
        if ctx.rt.pending_exception.is_some() { return Some(V::NULL); }
        let Some(value_name) = ctx.rt.str_of(value_name) else {
            return Some(ctx.rt.throw_exception("java/lang/NullPointerException", "Property value name"));
        };
        if !valid_property_name(value_name) {
            return Some(ctx.rt.throw_exception(
                "java/lang/IllegalArgumentException",
                &format!("Block {0} has property {name} with invalidly named value: {value_name}", ctx.rt.value_to_string(ctx.rt.get_field(builder, "owner"))),
            ));
        }
    }
    let mut properties = builder_properties(ctx.rt, builder);
    if properties
        .iter()
        .any(|existing| ctx.rt.str_of(ctx.rt.get_field(*existing, "name")) == Some(name.as_str()))
    {
        return Some(ctx.rt.throw_exception(
            "java/lang/IllegalArgumentException",
            &format!("Block {0} has duplicate property: {name}", ctx.rt.value_to_string(ctx.rt.get_field(builder, "owner"))),
        ));
    }
    properties.push(property);
    put_builder_properties(ctx.rt, builder, properties);
    None
}

fn valid_property_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn build_state_definition(ctx: &mut mmga_runtime::ExecCtx<'_>, owner: V, builder: V) -> V {
    let mut properties = builder_properties(ctx.rt, builder);
    properties.sort_by(|left, right| {
        ctx.rt.str_of(ctx.rt.get_field(*left, "name"))
            .cmp(&ctx.rt.str_of(ctx.rt.get_field(*right, "name")))
    });
    let definition = ctx.rt.new_instance(DEFINITION, 0);
    let property_collection = immutable_collection(ctx.rt, LOCAL_COLLECTION, properties.clone());
    ctx.rt.put_field(definition, "owner", owner);
    ctx.rt.put_field(definition, "$properties", property_collection);
    ctx.rt.put_field(definition, "$property", properties.first().copied().unwrap_or(V::NULL));
    let mut states = Vec::new();
    build_state_combinations(ctx, owner, definition, &properties, 0, &mut Vec::new(), &mut states);
    if ctx.rt.pending_exception.is_some() { return V::NULL; }
    let first = states[0];
    let states = immutable_collection(ctx.rt, LOCAL_LIST, states);
    ctx.rt.put_field(definition, "$first", first);
    ctx.rt.put_field(definition, "states", states);
    definition
}

fn build_state_combinations(
    ctx: &mut mmga_runtime::ExecCtx<'_>,
    owner: V,
    definition: V,
    properties: &[V],
    index: usize,
    selected: &mut Vec<(V, V)>,
    out: &mut Vec<V>,
) {
    if index == properties.len() {
        out.push(new_block_state(ctx, owner, definition, selected));
        return;
    }
    let property = properties[index];
    let values = collection_items(ctx.rt, ctx.rt.get_field(property, "$possibleValues"));
    for value in values {
        selected.push((property, value));
        build_state_combinations(ctx, owner, definition, properties, index + 1, selected, out);
        if ctx.rt.pending_exception.is_some() { return; }
        selected.pop();
    }
}

fn new_block_state(ctx: &mut mmga_runtime::ExecCtx<'_>, owner: V, definition: V, values: &[(V, V)]) -> V {
    let state = ctx.rt.new_instance(STATE, 0);
    // Seed JVM primitive defaults before invoking callbacks. Assignments below
    // retain the observable dtb$a.<init> callback/field-write order.
    for field in [
        "useShapeForLightOcclusion", "isAir", "ignitedByLava", "liquid", "legacySolid",
        "requiresCorrectToolForDrops", "canOcclude", "spawnTerrainParticles", "replaceable",
        "isRandomlyTicking",
    ] {
        ctx.rt.put_field(state, field, V::FALSE);
    }
    ctx.rt.put_field(state, "lightEmission", V::I(0));
    ctx.rt.put_field(state, "destroySpeed", V::F(0.0));
    let property_values = ctx.rt.alloc("java/util/Map", 0, Data::Map(values.to_vec()));
    ctx.rt.put_field(state, "owner", owner);
    ctx.rt.put_field(state, "$definition", definition);
    ctx.rt.put_field(
        state,
        "$property",
        values.first().map_or(V::NULL, |(property, _)| *property),
    );
    ctx.rt.put_field(state, "$stateValues", property_values);
    ctx.rt.put_field(state, "$genericBlockState", V::TRUE);
    if let Some((_, value)) = values.first() {
        ctx.rt.put_field(state, "$boxedValue", *value);
        ctx.rt.put_field(state, "$propertyValue", ctx.rt.get_field(*value, "$value"));
    }
    let properties = ctx.rt.get_field(owner, "properties");
    let light = ctx.rt.get_field(properties, "lightEmission");
    let light_value = mmga_runtime::invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new("java/util/function/ToIntFunction", "applyAsInt", "(Ljava/lang/Object;)I"),
        &[light, state],
    );
    if ctx.rt.pending_exception.is_some() { return V::NULL; }
    ctx.rt.put_field(state, "lightEmission", light_value);
    let occlusion = mmga_runtime::invoke_member(
        ctx,
        mmga_classfile::op::INVOKEVIRTUAL,
        &MemberRef::new(BLOCK, "useShapeForLightOcclusion", "(Lnet/minecraft/world/level/block/state/BlockState;)Z"),
        &[owner, state],
    );
    if ctx.rt.pending_exception.is_some() { return V::NULL; }
    ctx.rt.put_field(state, "useShapeForLightOcclusion", occlusion);
    for field in ["isAir", "ignitedByLava", "liquid", "pushReaction"] {
        let value = ctx.rt.get_field(properties, field);
        ctx.rt.put_field(state, field, value);
    }
    let color = ctx.rt.get_field(properties, "mapColor");
    let color_value = mmga_runtime::invoke_member(
        ctx,
        mmga_classfile::op::INVOKEINTERFACE,
        &MemberRef::new("java/util/function/Function", "apply", "(Ljava/lang/Object;)Ljava/lang/Object;"),
        &[color, state],
    );
    if ctx.rt.pending_exception.is_some() { return V::NULL; }
    ctx.rt.put_field(state, "mapColor", color_value);
    ctx.rt.put_field(state, "destroySpeed", ctx.rt.get_field(properties, "destroyTime"));
    for field in [
        "requiresCorrectToolForDrops", "canOcclude", "isRedstoneConductor", "isSuffocating",
        "isViewBlocking", "hasPostProcess", "emissiveRendering", "offsetFunction",
        "spawnTerrainParticles", "instrument", "replaceable",
    ] {
        let value = ctx.rt.get_field(properties, field);
        ctx.rt.put_field(state, field, value);
    }
    state
}


fn block_constant(rt: &mut Rt, registries: &mut RegistryStore, definition: &BlockDefinition) -> V {
    if let Some(value) = rt.get_static(BLOCKS, definition.name) {
        return value;
    }
    let block = rt.new_instance(definition.class, 0);
    registries.ensure_builtin_block_registry(rt);
    let holder = registries.create_intrusive_reference(rt, "minecraft:block", block);
    if rt.pending_exception.is_some() { return V::NULL; }
    rt.put_field(block, "builtInRegistryHolder", holder);
    let id = rt.intern_str(definition.id);
    let description = rt.intern_str(&format!("block.{}", definition.id.replace(':', ".")));
    let map_color = rt
        .get_static(
            "net/minecraft/world/level/material/MapColor",
            definition.map_color,
        )
        .unwrap_or_else(|| {
            let value = crate::minecraft::get_static_field(
                rt,
                "net/minecraft/world/level/material/MapColor",
                definition.map_color,
            )
            .expect("verified map color");
            rt.put_static(
                "net/minecraft/world/level/material/MapColor",
                definition.map_color,
                value,
            );
            value
        });
    for (field, value) in [
        ("$blockId", id),
        ("descriptionId", description),
        ("$destroyTime", V::F(definition.hardness)),
        ("explosionResistance", V::F(definition.resistance)),
        ("friction", V::F(0.6)),
        ("speedFactor", V::F(1.0)),
        ("jumpFactor", V::F(1.0)),
        ("hasCollision", V::TRUE),
        ("dynamicShape", V::FALSE),
        ("isRandomlyTicking", V::FALSE),
        ("$mapColor", map_color),
    ] {
        rt.put_field(block, field, value);
    }
    let (state_definition, first) = if matches!(definition.class, DISPENSER | DROPPER) {
        dispenser_states(rt, block, definition, map_color)
    } else {
        let property = property(rt, definition.property, definition.max);
        let state_definition = rt.new_instance(DEFINITION, 0);
        let properties = immutable_collection(rt, LOCAL_COLLECTION, vec![property]);
        rt.put_field(state_definition, "owner", block);
        rt.put_field(state_definition, "$property", property);
        rt.put_field(state_definition, "$properties", properties);
        let states: Vec<V> = (0..=definition.max)
            .map(|value| {
                let state = rt.new_instance(STATE, 0);
                let boxed = box_primitive(rt, "java/lang/Integer", V::I(value));
                rt.put_field(state, "owner", block);
                rt.put_field(state, "$definition", state_definition);
                rt.put_field(state, "$property", property);
                rt.put_field(state, "$propertyValue", V::I(value));
                rt.put_field(state, "$boxedValue", boxed);
                rt.put_field(
                    state,
                    "lightEmission",
                    V::I(if definition.class == ANCHOR {
                        value * 15 / 4
                    } else {
                        0
                    }),
                );
                rt.put_field(state, "destroySpeed", V::F(definition.hardness));
                rt.put_field(
                    state,
                    "requiresCorrectToolForDrops",
                    V::bool(definition.correct_tool),
                );
                rt.put_field(state, "mapColor", map_color);
                state
            })
            .collect();
        let first = states[0];
        let state_list = immutable_collection(rt, LOCAL_LIST, states);
        rt.put_field(state_definition, "states", state_list);
        rt.put_field(state_definition, "$first", first);
        (state_definition, first)
    };
    rt.put_field(block, "stateDefinition", state_definition);
    rt.put_field(block, "defaultBlockState", first);
    registries.register_direct(
        rt,
        "minecraft:block",
        definition.id,
        "minecraft",
        block,
        ContentProperties {
            kind: "block".into(),
            template: definition.id.into(),
            hardness: Some(definition.hardness),
            resistance: Some(definition.resistance),
            light_level: Some(0),
            friction: Some(0.6),
            requires_correct_tool: definition.correct_tool,
            ..ContentProperties::default()
        },
    );
    if rt.pending_exception.is_some() { return V::NULL; }
    let Some(bound_holder) = registries.canonical_reference("minecraft:block", definition.id) else { return V::NULL; };
    debug_assert_eq!(holder, bound_holder);
    rt.put_static(BLOCKS, definition.name, block);
    block
}

// Minecraft 1.21.1 official `due`: DEFAULT serializes as "normal", not "default".
fn piston_type(rt: &mut Rt, name: &str) -> Option<V> {
    let (index, label) = match name {
        "DEFAULT" => (0, "normal"),
        "STICKY" => (1, "sticky"),
        _ => return None,
    };
    if let Some(value) = rt.get_static(PISTON_TYPE, name) {
        return Some(value);
    }
    let value = rt.new_instance(PISTON_TYPE, 0);
    let enum_name = rt.intern_str(name);
    let label = rt.intern_str(label);
    rt.put_field(value, "$name", enum_name);
    rt.put_field(value, "$ordinal", V::I(index));
    rt.put_field(value, "name", label);
    rt.put_static(PISTON_TYPE, name, value);
    Some(value)
}

fn property(rt: &mut Rt, name: &str, max: i32) -> V {
    let field = if name == "charges" {
        "RESPAWN_ANCHOR_CHARGES"
    } else {
        "POWER"
    };
    if let Some(value) = rt.get_static(STATE_PROPERTIES, field) {
        return value;
    }
    let value = rt.new_instance(INTEGER_PROPERTY, 0);
    let label = rt.intern_str(name);
    let values = (0..=max)
        .map(|n| box_primitive(rt, "java/lang/Integer", V::I(n)))
        .collect();
    let values = immutable_collection(rt, LOCAL_SET, values);
    let clazz = rt.new_pinned_handle("java/lang/Class", "class", 0, "java/lang/Integer");
    rt.put_field(value, "name", label);
    rt.put_field(value, "clazz", clazz);
    rt.put_field(value, "min", V::I(0));
    rt.put_field(value, "max", V::I(max));
    rt.put_field(value, "$possibleValues", values);
    rt.put_static(STATE_PROPERTIES, field, value);
    rt.put_static(
        if name == "charges" { ANCHOR } else { TARGET },
        if name == "charges" {
            "CHARGE"
        } else {
            "OUTPUT_POWER"
        },
        value,
    );
    value
}

fn dispenser_property(rt: &mut Rt, name: &str) -> V {
    let (field, class, value_class) = if name == "facing" {
        ("FACING", DIRECTION_PROPERTY, "net/minecraft/core/Direction")
    } else {
        ("TRIGGERED", BOOLEAN_PROPERTY, "java/lang/Boolean")
    };
    if let Some(value) = rt.get_static(STATE_PROPERTIES, field) {
        return value;
    }
    let values = if name == "facing" {
        ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"]
            .into_iter()
            .map(|direction| {
                crate::direction::get_static_field(
                    rt,
                    "net/minecraft/core/Direction",
                    direction,
                    "Lnet/minecraft/core/Direction;",
                )
                .expect("verified Direction constant")
            })
            .collect()
    } else {
        // BooleanProperty's ImmutableSet is true then false in the official bytecode.
        vec![
            box_primitive(rt, "java/lang/Boolean", V::TRUE),
            box_primitive(rt, "java/lang/Boolean", V::FALSE),
        ]
    };
    let values = immutable_collection(rt, LOCAL_SET, values);
    let property = rt.new_instance(class, 0);
    let label = rt.intern_str(name);
    let clazz = rt.new_pinned_handle("java/lang/Class", "class", 0, value_class);
    rt.put_field(property, "name", label);
    rt.put_field(property, "clazz", clazz);
    rt.put_field(property, "$possibleValues", values);
    rt.put_static(STATE_PROPERTIES, field, property);
    property
}

fn create_boolean_property(rt: &mut Rt, name: V) -> V {
    let values = vec![
        box_primitive(rt, "java/lang/Boolean", V::TRUE),
        box_primitive(rt, "java/lang/Boolean", V::FALSE),
    ];
    let property = rt.new_instance(BOOLEAN_PROPERTY, 0);
    let clazz = rt.new_pinned_handle("java/lang/Class", "class", 0, "java/lang/Boolean");
    rt.put_field(property, "name", name);
    rt.put_field(property, "clazz", clazz);
    let values = immutable_collection(rt, LOCAL_SET, values);
    rt.put_field(property, "$possibleValues", values);
    property
}

fn state_boolean_property(rt: &mut Rt, field: &str, name: &str) -> V {
    if let Some(property) = rt.get_static(STATE_PROPERTIES, field) {
        return property;
    }
    let name = rt.intern_str(name);
    let property = create_boolean_property(rt, name);
    rt.put_static(STATE_PROPERTIES, field, property);
    property
}

fn horizontal_facing_property(rt: &mut Rt) -> V {
    if let Some(property) = rt.get_static(STATE_PROPERTIES, "HORIZONTAL_FACING") {
        return property;
    }
    // dts passes Plane.HORIZONTAL as a Predicate; dtw filters Direction.values(),
    // retaining enum order rather than the Plane iterator or 2D-data order.
    let values = ["NORTH", "SOUTH", "WEST", "EAST"]
        .into_iter()
        .map(|name| {
            crate::direction::get_static_field(
                rt,
                "net/minecraft/core/Direction",
                name,
                "Lnet/minecraft/core/Direction;",
            )
            .expect("verified Direction constant")
        })
        .collect();
    let values = immutable_collection(rt, LOCAL_SET, values);
    let property = rt.new_instance(DIRECTION_PROPERTY, 0);
    let name = rt.intern_str("facing");
    let clazz = rt.new_pinned_handle(
        "java/lang/Class",
        "class",
        0,
        "net/minecraft/core/Direction",
    );
    rt.put_field(property, "name", name);
    rt.put_field(property, "clazz", clazz);
    rt.put_field(property, "$possibleValues", values);
    rt.put_static(STATE_PROPERTIES, "HORIZONTAL_FACING", property);
    property
}

fn dispenser_states(rt: &mut Rt, block: V, definition: &BlockDefinition, color: V) -> (V, V) {
    let facing = dispenser_property(rt, "facing");
    let triggered = dispenser_property(rt, "triggered");
    let states_definition = rt.new_instance(DEFINITION, 0);
    let properties = immutable_collection(rt, LOCAL_COLLECTION, vec![facing, triggered]);
    rt.put_field(states_definition, "owner", block);
    rt.put_field(states_definition, "$property", facing);
    rt.put_field(states_definition, "$properties", properties);
    let mut states = Vec::new();
    let mut default = V::NULL;
    let directions = collection_items(rt, rt.get_field(facing, "$possibleValues"));
    let triggers = collection_items(rt, rt.get_field(triggered, "$possibleValues"));
    for direction in directions {
        for trigger in &triggers {
            let state = rt.new_instance(STATE, 0);
            let values = rt.alloc(
                "java/util/Map",
                0,
                Data::Map(vec![(facing, direction), (triggered, *trigger)]),
            );
            rt.put_field(state, "owner", block);
            rt.put_field(state, "$definition", states_definition);
            rt.put_field(state, "$property", facing);
            rt.put_field(state, "$stateValues", values);
            rt.put_field(state, "lightEmission", V::I(0));
            rt.put_field(state, "destroySpeed", V::F(definition.hardness));
            rt.put_field(
                state,
                "requiresCorrectToolForDrops",
                V::bool(definition.correct_tool),
            );
            rt.put_field(state, "mapColor", color);
            if rt.str_of(rt.get_field(direction, "$name")) == Some("NORTH")
                && rt.get_field(*trigger, "$value") == V::FALSE
            {
                default = state;
            }
            states.push(state);
        }
    }
    rt.put_field(states_definition, "$first", states[0]);
    let states = immutable_collection(rt, LOCAL_LIST, states);
    rt.put_field(states_definition, "states", states);
    (states_definition, default)
}

fn dispenser_property_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    match (name, desc) {
        ("getName", "()Ljava/lang/String;") => Some(rt.get_field(recv, "name")),
        ("getValueClass", "()Ljava/lang/Class;") => Some(rt.get_field(recv, "clazz")),
        ("getPossibleValues", "()Ljava/util/Collection;") => {
            Some(rt.get_field(recv, "$possibleValues"))
        }
        ("equals", "(Ljava/lang/Object;)Z") if rt.class_of(recv) == Some(BOOLEAN_PROPERTY) => {
            let other = *args.first()?;
            if recv == other {
                return Some(V::TRUE);
            }
            if rt.class_of(other) != Some(BOOLEAN_PROPERTY) {
                return Some(V::FALSE);
            }
            let name = rt.get_field(recv, "name");
            let other_name = rt.get_field(other, "name");
            let same_name = rt.val_eq(name, other_name);
            let values = collection_items(rt, rt.get_field(recv, "$possibleValues"));
            let other_values = collection_items(rt, rt.get_field(other, "$possibleValues"));
            Some(V::bool(
                same_name
                    && values.len() == other_values.len()
                    && values
                        .iter()
                        .all(|value| other_values.iter().any(|other| rt.val_eq(*value, *other))),
            ))
        }
        ("hashCode", "()I") if rt.class_of(recv) == Some(BOOLEAN_PROPERTY) => {
            let clazz = rt.get_field(recv, "clazz");
            let name_hash = java_string_hash(rt.str_of(rt.get_field(recv, "name"))?);
            let property_hash = 31i32
                .wrapping_mul(31i32.wrapping_mul(clazz.r() as i32).wrapping_add(name_hash))
                .wrapping_add(1231 + 1237);
            Some(V::I(property_hash))
        }
        ("getValue", "(Ljava/lang/String;)Ljava/util/Optional;") => {
            let text = rt.str_of(*args.first()?);
            let values = collection_items(rt, rt.get_field(recv, "$possibleValues"));
            let value = values.into_iter().find(|value| {
                if rt.class_of(recv) == Some(DIRECTION_PROPERTY) {
                    text == rt.str_of(rt.get_field(*value, "name"))
                } else {
                    text == Some(if rt.get_field(*value, "$value").i() != 0 {
                        "true"
                    } else {
                        "false"
                    })
                }
            });
            Some(rt.alloc("java/util/Optional", 0, Data::Optional(value)))
        }
        (
            "getName",
            "(Ljava/lang/Comparable;)Ljava/lang/String;"
            | "(Ljava/lang/Boolean;)Ljava/lang/String;"
            | "(Ljava/lang/Enum;)Ljava/lang/String;",
        ) => {
            let value = *args.first()?;
            if value.is_null() {
                return Some(
                    rt.throw_exception("java/lang/NullPointerException", "Property.getName(null)"),
                );
            }
            let expected = if rt.class_of(recv) == Some(DIRECTION_PROPERTY) {
                "net/minecraft/core/Direction"
            } else {
                "java/lang/Boolean"
            };
            if rt.class_of(value) != Some(expected) {
                return Some(rt.throw_exception(
                    "java/lang/ClassCastException",
                    "wrong property value class",
                ));
            }
            Some(if expected == "java/lang/Boolean" {
                rt.intern_str(if rt.get_field(value, "$value").i() != 0 {
                    "true"
                } else {
                    "false"
                })
            } else {
                rt.get_field(value, "name")
            })
        }
        _ => None,
    }
}

fn java_string_hash(text: &str) -> i32 {
    text.encode_utf16()
        .fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(i32::from(unit)))
}

fn multistate_values(rt: &Rt, state: V) -> Vec<(V, V)> {
    match rt
        .get_obj(rt.get_field(state, "$stateValues"))
        .map(|object| &object.data)
    {
        Some(Data::Map(values)) => values.clone(),
        _ => Vec::new(),
    }
}

fn multi_property_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    let values = multistate_values(rt, recv);
    let property = *args.first()?;
    let current = values
        .iter()
        .find(|(key, _)| *key == property)
        .map(|(_, value)| *value);
    match (name,desc) {
        ("hasProperty","(Lnet/minecraft/world/level/block/state/properties/Property;)Z") => Some(V::bool(current.is_some())),
        ("getValue","(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;") => Some(current.unwrap_or_else(||rt.throw_exception("java/lang/IllegalArgumentException","property does not exist in block state"))),
        ("getOptionalValue","(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/util/Optional;") => Some(rt.alloc("java/util/Optional",0,Data::Optional(current))),
        ("setValue"|"trySetValue","(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;")
        | ("cycle","(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Object;") => {
            let Some(current)=current else {
                return Some(if name=="trySetValue" {recv} else {rt.throw_exception("java/lang/IllegalArgumentException","property does not exist in block state")});
            };
            let possibilities = collection_items(rt,rt.get_field(property,"$possibleValues"));
            let candidate = if name=="cycle" {
                let position=possibilities.iter().position(|value|rt.val_eq(*value,current)).expect("verified property state");
                possibilities[(position+1)%possibilities.len()]
            } else { *args.get(1)? };
            let Some(candidate)=possibilities.into_iter().find(|value|rt.val_eq(*value,candidate)) else {
                return Some(rt.throw_exception("java/lang/IllegalArgumentException","property value is not allowed in block state"));
            };
            if rt.val_eq(candidate,current) { return Some(recv); }
            let states = collection_items(rt,rt.get_field(rt.get_field(recv,"$definition"),"states"));
            Some(states.into_iter().find(|state|{
                let other = multistate_values(rt,*state);
                values.iter().all(|(key,value)| {
                    let wanted=if *key==property {candidate} else {*value};
                    other.iter().any(|(other_key,other_value)|other_key==key&&rt.val_eq(*other_value,wanted))
                })
            }).expect("complete local block-state Cartesian product"))
        }
        _ => None,
    }
}

fn state_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    if rt.get_field(recv, "$genericBlockState") == V::TRUE {
        return generic_state_call(rt, recv, name, desc, args);
    }
    if !rt.get_field(recv, "$stateValues").is_null() {
        if let Some(value) = multi_property_call(rt, recv, name, desc, args) {
            return Some(value);
        }
    }
    let block = rt.get_field(recv, "owner");
    let property = rt.get_field(recv, "$property");
    let current = rt.get_field(recv, "$propertyValue").i();
    match (name,desc) {
        ("getBlock","()Lnet/minecraft/world/level/block/Block;") => Some(block),
        ("getBlockHolder","()Lnet/minecraft/core/Holder;") => Some(rt.get_field(block,"builtInRegistryHolder")),
        ("getLightEmission","()I") => Some(rt.get_field(recv,"lightEmission")),
        ("requiresCorrectToolForDrops","()Z") => Some(rt.get_field(recv,"requiresCorrectToolForDrops")),
        ("getDestroySpeed","(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)F") => Some(rt.get_field(recv,"destroySpeed")),
        ("getMapColor","(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/material/MapColor;") => Some(rt.get_field(recv,"mapColor")),
        ("isAir"|"liquid"|"ignitedByLava"|"isRandomlyTicking","()Z") => Some(V::FALSE),
        ("hasBlockEntity","()Z") => Some(V::bool(matches!(rt.class_of(block),Some(DISPENSER|DROPPER)))),
        ("canOcclude"|"shouldSpawnTerrainParticles","()Z") => Some(V::TRUE),
        ("is","(Lnet/minecraft/world/level/block/Block;)Z") => Some(V::bool(block==*args.first()?)),
        ("hasProperty","(Lnet/minecraft/world/level/block/state/properties/Property;)Z") => Some(V::bool(property==*args.first()?)),
        ("getProperties","()Ljava/util/Collection;") => Some(rt.get_field(rt.get_field(recv,"$definition"),"$properties")),
        ("getValue","(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;") => {
            Some(if property==*args.first()? { rt.get_field(recv,"$boxedValue") } else { rt.throw_exception("java/lang/IllegalArgumentException","property does not exist in block state") })
        }
        ("getOptionalValue","(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/util/Optional;") => {
            let value=if property==*args.first()? { Some(rt.get_field(recv,"$boxedValue")) } else { None };
            Some(rt.alloc("java/util/Optional",0,Data::Optional(value)))
        }
        ("setValue"|"trySetValue","(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;") => {
            if property!=*args.first()? { return Some(if name=="trySetValue" { recv } else { rt.throw_exception("java/lang/IllegalArgumentException","property does not exist in block state") }); }
            let candidate=*args.get(1)?;
            let raw=rt.get_field(candidate,"$value");
            if rt.class_of(candidate)!=Some("java/lang/Integer") { return Some(rt.throw_exception("java/lang/IllegalArgumentException","invalid integer property value")); }
            Some(state_at(rt,recv,raw.i()))
        }
        ("cycle","(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Object;") => {
            if property!=*args.first()? { return Some(rt.throw_exception("java/lang/IllegalArgumentException","property does not exist in block state")); }
            let max=rt.get_field(property,"max").i(); Some(state_at(rt,recv,(current+1)%(max+1)))
        }
        ("isSignalSource","()Z") => Some(V::bool(rt.class_of(block)==Some(TARGET))),
        ("hasAnalogOutputSignal","()Z") => Some(V::bool(matches!(rt.class_of(block),Some(ANCHOR|DISPENSER|DROPPER)))),
        ("getAnalogOutputSignal","(Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;)I") => Some(if matches!(rt.class_of(block),Some(DISPENSER|DROPPER)) { unsupported(rt,"dispenser/dropper inventory and world comparator signal") } else {V::I(if rt.class_of(block)==Some(ANCHOR) { current*15/4 } else { 0 })}),
        ("getSignal","(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;Lnet/minecraft/core/Direction;)I") => Some(V::I(if rt.class_of(block)==Some(TARGET) { current } else { 0 })),
        _=>None,
    }
}

fn generic_state_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    if let Some(value) = multi_property_call(rt, recv, name, desc, args) {
        return Some(value);
    }
    let block = rt.get_field(recv, "owner");
    match (name, desc) {
        ("getBlock", "()Lnet/minecraft/world/level/block/Block;") => Some(block),
        ("getBlockHolder", "()Lnet/minecraft/core/Holder;") => Some(rt.get_field(block, "builtInRegistryHolder")),
        ("getProperties", "()Ljava/util/Collection;") => Some(rt.get_field(rt.get_field(recv, "$definition"), "$properties")),
        ("getValues", "()Ljava/util/Map;") => Some(rt.get_field(recv, "$stateValues")),
        ("getLightEmission", "()I") => Some(rt.get_field(recv, "lightEmission")),
        ("isAir" | "ignitedByLava" | "liquid" | "canOcclude" | "requiresCorrectToolForDrops" | "useShapeForLightOcclusion", "()Z") => Some(rt.get_field(recv, name)),
        ("getDestroySpeed", "(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)F") => Some(rt.get_field(recv, "destroySpeed")),
        ("getMapColor", "(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/material/MapColor;") => Some(rt.get_field(recv, "mapColor")),
        ("getPistonPushReaction", "()Lnet/minecraft/world/level/material/PushReaction;") => Some(rt.get_field(recv, "pushReaction")),
        ("is", "(Lnet/minecraft/world/level/block/Block;)Z") => Some(V::bool(block == *args.first()?)),
        _ => None,
    }
}

fn state_at(rt: &mut Rt, state: V, value: i32) -> V {
    let property = rt.get_field(state, "$property");
    let max = rt.get_field(property, "max").i();
    if !(0..=max).contains(&value) {
        return rt.throw_exception(
            "java/lang/IllegalArgumentException",
            &format!("property value {value} outside 0..{max}"),
        );
    }
    let list = rt.get_field(rt.get_field(state, "$definition"), "states");
    collection_items(rt, list)[value as usize]
}

fn property_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    match (name, desc) {
        ("getName", "()Ljava/lang/String;") => Some(rt.get_field(recv, "name")),
        ("getValueClass", "()Ljava/lang/Class;") => Some(rt.get_field(recv, "clazz")),
        ("getPossibleValues", "()Ljava/util/Collection;") => {
            Some(rt.get_field(recv, "$possibleValues"))
        }
        (
            "getName",
            "(Ljava/lang/Integer;)Ljava/lang/String;"
            | "(Ljava/lang/Comparable;)Ljava/lang/String;",
        ) => {
            let value = *args.first()?;
            if value.is_null() {
                return Some(rt.throw_exception(
                    "java/lang/NullPointerException",
                    "IntegerProperty.getName(null)",
                ));
            }
            if rt.class_of(value) != Some("java/lang/Integer") {
                return Some(rt.throw_exception(
                    "java/lang/ClassCastException",
                    "IntegerProperty requires Integer",
                ));
            }
            Some(rt.new_str(rt.get_field(value, "$value").i().to_string()))
        }
        ("getValue", "(Ljava/lang/String;)Ljava/util/Optional;") => {
            let text = rt.str_of(*args.first()?).unwrap_or("");
            let parsed = if text.trim() == text {
                text.parse::<i32>().ok()
            } else {
                None
            };
            let max = rt.get_field(recv, "max").i();
            let value = parsed
                .filter(|v| (0..=max).contains(v))
                .map(|v| box_primitive(rt, "java/lang/Integer", V::I(v)));
            Some(rt.alloc("java/util/Optional", 0, Data::Optional(value)))
        }
        _ => None,
    }
}

fn immutable_collection(rt: &mut Rt, class: &str, values: Vec<V>) -> V {
    let collection = rt.new_instance(class, 0);
    let array = rt.alloc("[Ljava/lang/Object;", 0, Data::Array(values));
    rt.put_field(collection, "$blockCollection", array);
    collection
}
fn collection_items(rt: &Rt, collection: V) -> Vec<V> {
    match rt
        .get_obj(rt.get_field(collection, "$blockCollection"))
        .map(|o| &o.data)
    {
        Some(Data::Array(values)) => values.clone(),
        _ => Vec::new(),
    }
}
fn collection_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
    let values = collection_items(rt, recv);
    match (name, desc) {
        ("size", "()I") => Some(V::I(values.len() as i32)),
        ("isEmpty", "()Z") => Some(V::bool(values.is_empty())),
        ("contains", "(Ljava/lang/Object;)Z") => {
            Some(V::bool(values.iter().any(|v| rt.val_eq(*v, args[0]))))
        }
        ("get", "(I)Ljava/lang/Object;") if rt.class_of(recv) == Some(LOCAL_LIST) => Some(
            values
                .get(args[0].i() as usize)
                .copied()
                .unwrap_or_else(|| {
                    rt.throw_exception(
                        "java/lang/IndexOutOfBoundsException",
                        "immutable block-state list index",
                    )
                }),
        ),
        (
            "iterator",
            "()Ljava/util/Iterator;" | "()Lcom/google/common/collect/UnmodifiableIterator;",
        ) => Some(rt.alloc(
            "java/util/Iterator",
            0,
            Data::Iter {
                items: values,
                pos: 0,
            },
        )),
        ("stream", "()Ljava/util/stream/Stream;") => {
            Some(rt.alloc("java/util/stream/Stream", 0, Data::Stream(values)))
        }
        ("toArray", "()[Ljava/lang/Object;") => {
            Some(rt.alloc("[Ljava/lang/Object;", 0, Data::Array(values)))
        }
        ("clear", "()V")
        | ("add" | "remove", "(Ljava/lang/Object;)Z")
        | ("addAll" | "removeAll" | "retainAll", "(Ljava/util/Collection;)Z")
        | ("removeIf", "(Ljava/util/function/Predicate;)Z")
        | ("replaceAll", "(Ljava/util/function/UnaryOperator;)V")
        | ("sort", "(Ljava/util/Comparator;)V")
        | ("set", "(ILjava/lang/Object;)Ljava/lang/Object;")
        | ("add", "(ILjava/lang/Object;)V")
        | ("remove", "(I)Ljava/lang/Object;") => Some(rt.throw_exception(
            "java/lang/UnsupportedOperationException",
            "immutable block state collection",
        )),
        _ => None,
    }
}

fn registry_call(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    member: &MemberRef,
    is_static: bool,
    args: &[V],
) -> Option<V> {
    if is_static
        || !matches!(
            member.owner.as_str(),
            "net/minecraft/core/Registry"
                | "net/minecraft/core/DefaultedRegistry"
                | "net/minecraft/core/WritableRegistry"
                | "net/minecraft/core/IdMap"
        )
    {
        return None;
    }
    let ("registry", _, key, _) = rt.handle_of(*args.first()?)? else {
        return None;
    };
    if key != "minecraft:block" {
        return None;
    }
    let name = member.name.as_str();
    if matches!(
        name,
        "size"
            | "stream"
            | "iterator"
            | "keySet"
            | "entrySet"
            | "holders"
            | "getId"
            | "byId"
            | "byIdOrThrow"
            | "getRandom"
    ) {
        return Some(unsupported(
            rt,
            "enumeration or numeric IDs of the incomplete vanilla block registry",
        ));
    }
    if matches!(
        name,
        "get" | "getOptional" | "containsKey" | "getHolder" | "getHolderOrThrow"
    ) {
        let target = *args.get(1)?;
        let id = rt
            .handle_of(target)
            .and_then(|(kind, _, tag, _)| match kind {
                "resource_location" => Some(tag.to_owned()),
                "resource_key" => tag.split_once('|').map(|(_, id)| id.to_owned()),
                _ => None,
            })?;
        if let Some(definition) = DEFINITIONS.iter().find(|definition| definition.id == id) {
            block_constant(rt, registries, definition);
        }
        if registries
            .lookup("minecraft:block", &id)
            .is_none_or(|entry| entry.value.is_null())
        {
            return Some(unsupported(
                rt,
                &format!("vanilla block definition for {id}; registry is incomplete"),
            ));
        }
    }
    None
}
fn unsupported(rt: &mut Rt, reason: &str) -> V {
    rt.throw_exception(
        "java/lang/UnsupportedOperationException",
        &format!("MMGA does not implement {reason}"),
    )
}

pub fn is_subtype(sub_class: &str, super_class: &str) -> bool {
    match sub_class {
        PISTON_TYPE => matches!(
            super_class,
            "java/lang/Enum"
                | "java/lang/Comparable"
                | "java/io/Serializable"
                | "net/minecraft/util/StringRepresentable"
        ),
        ANCHOR | TARGET => matches!(
            super_class,
            BLOCK | BEHAVIOUR | "net/minecraft/world/level/ItemLike"
        ),
        DISPENSER | DROPPER => {
            matches!(
                super_class,
                BLOCK
                    | BEHAVIOUR
                    | "net/minecraft/world/level/block/BaseEntityBlock"
                    | "net/minecraft/world/level/block/EntityBlock"
                    | "net/minecraft/world/level/ItemLike"
            ) || (sub_class == DROPPER && super_class == DISPENSER)
        }
        STATE => matches!(super_class, STATE_BASE | STATE_HOLDER),
        INTEGER_PROPERTY | BOOLEAN_PROPERTY => super_class == PROPERTY,
        DIRECTION_PROPERTY => matches!(
            super_class,
            PROPERTY | "net/minecraft/world/level/block/state/properties/EnumProperty"
        ),
        LOCAL_LIST => matches!(
            super_class,
            "com/google/common/collect/ImmutableList"
                | "com/google/common/collect/ImmutableCollection"
                | "java/util/List"
                | "java/util/Collection"
                | "java/lang/Iterable"
        ),
        LOCAL_SET => matches!(
            super_class,
            "com/google/common/collect/ImmutableSet"
                | "com/google/common/collect/ImmutableCollection"
                | "java/util/Set"
                | "java/util/Collection"
                | "java/lang/Iterable"
        ),
        LOCAL_COLLECTION => matches!(super_class, "java/util/Collection" | "java/lang/Iterable"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_classfile::op;
    use mmga_runtime::{
        alloc_new_object, box_primitive, invoke_member, read_static_field, ExecCtx, HostApi,
        LinkedSpace,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::process::Command;

    #[derive(Default)]
    struct BlocksHost(RegistryStore);
    impl HostApi for BlocksHost {
        fn get_static_field(
            &mut self,
            rt: &mut Rt,
            owner: &str,
            name: &str,
            desc: &str,
        ) -> Option<V> {
            super::get_static_field(rt, &mut self.0, owner, name, desc)
        }
        fn call_method(
            &mut self,
            rt: &mut Rt,
            linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            if let Some(value) = super::call(rt, &mut self.0, member, is_static, args) {
                return Some(value);
            }
            {
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                };
                if let Some(value) = super::call_context(&mut ctx, member, is_static, args) {
                    return Some(value);
                }
            }
            crate::minecraft::call_keys_authlib_and_libs(
                rt,
                &mut self.0,
                member,
                is_static,
                args,
            )
        }
        fn is_api_subtype(&self, sub: &str, sup: &str) -> bool {
            is_subtype(sub, sup)
        }
    }
    fn invoke(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    fn invoke_static(ctx: &mut ExecCtx<'_>, owner: &str, name: &str, desc: &str, args: &[V]) -> V {
        invoke_member(
            ctx,
            op::INVOKESTATIC,
            &MemberRef::new(owner, name, desc),
            args,
        )
    }

    fn compile_block_hook_fixture() -> Option<(std::path::PathBuf, mmga_classfile::ClassFile)> {
        if Command::new("javac").arg("-version").output().is_err() {
            return None;
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mmga-block-state-hook-{nonce}"));
        let src = root.join("src");
        let classes = root.join("classes");
        fs::create_dir_all(&src).ok()?;
        fs::create_dir_all(&classes).ok()?;
        let files = [
            (
                "net/minecraft/world/level/block/state/properties/Property.java",
                r#"package net.minecraft.world.level.block.state.properties;
                public abstract class Property<T extends Comparable<T>> { }"#,
            ),
            (
                "net/minecraft/world/level/block/state/properties/BooleanProperty.java",
                r#"package net.minecraft.world.level.block.state.properties;
                public final class BooleanProperty extends Property<Boolean> {
                    public static BooleanProperty create(String name) { return null; }
                }"#,
            ),
            (
                "net/minecraft/world/level/block/state/BlockState.java",
                r#"package net.minecraft.world.level.block.state;
                import net.minecraft.world.level.block.Block;
                public class BlockState extends StateHolder<Block, BlockState> { }"#,
            ),
            (
                "net/minecraft/world/level/block/state/StateHolder.java",
                r#"package net.minecraft.world.level.block.state;
                import net.minecraft.world.level.block.state.properties.Property;
                public class StateHolder<O, S> {
                    public <T extends Comparable<T>, V extends T> S setValue(Property<T> property, V value) { return null; }
                    public <T extends Comparable<T>> T getValue(Property<T> property) { return null; }
                }"#,
            ),
            (
                "net/minecraft/world/level/block/state/StateDefinition.java",
                r#"package net.minecraft.world.level.block.state;
                import net.minecraft.world.level.block.state.properties.Property;
                public class StateDefinition<O, S extends StateHolder<O, S>> {
                    public S any() { return null; }
                    public static class Builder<O, S> {
                        public Builder(O owner) { }
                        public Builder<O, S> add(Property<?>... properties) { return this; }
                    }
                }"#,
            ),
            (
                "net/minecraft/core/Holder.java",
                r#"package net.minecraft.core;
                public class Holder<T> { public static class Reference<T> extends Holder<T> { } }"#,
            ),
            (
                "net/minecraft/world/level/block/state/BlockBehaviour.java",
                r#"package net.minecraft.world.level.block.state;
                public class BlockBehaviour {
                    public static class Properties { }
                }"#,
            ),
            (
                "net/minecraft/world/level/block/SoundType.java",
                r#"package net.minecraft.world.level.block;
                public class SoundType { }"#,
            ),
            (
                "net/minecraft/world/level/block/Block.java",
                r#"package net.minecraft.world.level.block;
                import net.minecraft.world.level.block.state.BlockBehaviour;
                import net.minecraft.world.level.block.state.StateDefinition;
                import net.minecraft.world.level.block.state.BlockState;
                import net.minecraft.core.Holder;
                public class Block {
                    protected final StateDefinition<Block, BlockState> stateDefinition = null;
                    public Block(BlockBehaviour.Properties properties) { }
                    protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) { }
                    public StateDefinition<Block, BlockState> getStateDefinition() { return null; }
                    public Holder.Reference<Block> builtInRegistryHolder() { return null; }
                    protected final void registerDefaultState(BlockState state) { }
                    public final BlockState defaultBlockState() { return null; }
                    protected boolean useShapeForLightOcclusion(BlockState state) { return false; }
                    protected SoundType getSoundType(BlockState state) { return null; }
                }"#,
            ),
            (
                "test/HookBlock.java",
                r#"package test;
                import net.minecraft.world.level.block.Block;
                import net.minecraft.world.level.block.SoundType;
                import net.minecraft.world.level.block.state.BlockBehaviour;
                import net.minecraft.world.level.block.state.BlockState;
                import net.minecraft.world.level.block.state.StateDefinition;
                import net.minecraft.world.level.block.state.properties.BooleanProperty;
                public final class HookBlock extends Block {
                    public static final BooleanProperty ACTIVE = BooleanProperty.create("active");
                    public static final BooleanProperty INVERTED = BooleanProperty.create("inverted");
                    public static int hookCalls;
                    public static boolean holderSeenInHook;
                    public static boolean definitionSeenInHook;
                    public static int fieldSeenInHook;
                    public static SoundType activeSound;
                    public static SoundType inactiveSound;
                    public static BlockState lastSoundState;
                    public static int soundCalls;
                    public int initializedAfterSuper = 42;
                    public HookBlock(BlockBehaviour.Properties properties) {
                        super(properties);
                        registerDefaultState(stateDefinition.any().setValue(ACTIVE, false).setValue(INVERTED, true));
                    }
                    @Override protected void createBlockStateDefinition(
                        StateDefinition.Builder<Block, BlockState> builder) {
                        super.createBlockStateDefinition(builder);
                        holderSeenInHook = builtInRegistryHolder() != null;
                        definitionSeenInHook = getStateDefinition() != null;
                        fieldSeenInHook = initializedAfterSuper;
                        builder.add(INVERTED, ACTIVE);
                        hookCalls++;
                    }
                    @Override protected SoundType getSoundType(BlockState state) {
                        soundCalls++;
                        lastSoundState = state;
                        return state.getValue(ACTIVE) ? activeSound : inactiveSound;
                    }
                    public SoundType baseSound(BlockState state) { return super.getSoundType(state); }
                }"#,
            ),
        ];
        let mut paths = Vec::new();
        for (relative, source) in files {
            let path = src.join(relative);
            fs::create_dir_all(path.parent()?).ok()?;
            fs::write(&path, source).ok()?;
            paths.push(path);
        }
        let output = Command::new("javac")
            .arg("-d")
            .arg(&classes)
            .args(paths)
            .output()
            .ok()?;
        assert!(
            output.status.success(),
            "javac failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let hook_bytes = fs::read(classes.join("test/HookBlock.class")).ok()?;
        let hook = mmga_classfile::ClassFile::parse(&hook_bytes).ok()?;
        Some((root, hook))
    }

    fn fixture_light(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        let active = ctx.rt.get_static("test/HookBlock", "ACTIVE").unwrap();
        let value = invoke(
            ctx,
            STATE_HOLDER,
            "getValue",
            "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;",
            &[args[0], active],
        );
        if ctx.rt.pending_exception.is_some() { return V::NULL; }
        V::I(if ctx.rt.get_field(value, "$value").i() != 0 { 11 } else { 3 })
    }

    fn fixture_map_color(ctx: &mut ExecCtx<'_>, args: &[V]) -> V {
        for method in ["canOcclude", "requiresCorrectToolForDrops"] {
            let observed = invoke(ctx, STATE_BASE, method, "()Z", &[args[0]]);
            assert_eq!(observed, V::FALSE, "{method} must retain its JVM default during mapColor");
        }
        let destroy_speed = invoke(
            ctx,
            STATE_BASE,
            "getDestroySpeed",
            "(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)F",
            &[args[0], V::NULL, V::NULL],
        );
        assert_eq!(destroy_speed, V::F(0.0));
        let is_air = invoke(ctx, STATE_BASE, "isAir", "()Z", &[args[0]]);
        assert_eq!(is_air, V::TRUE, "isAir is copied before the mapColor callback");
        assert!(ctx.rt.get_field(args[0], "isRedstoneConductor").is_null());
        let inverted = ctx.rt.get_static("test/HookBlock", "INVERTED").unwrap();
        let value = invoke(
            ctx,
            STATE_HOLDER,
            "getValue",
            "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;",
            &[args[0], inverted],
        );
        if ctx.rt.pending_exception.is_some() { return V::NULL; }
        ctx.rt.get_static(
            "test/HookBlock",
            if ctx.rt.get_field(value, "$value").i() != 0 { "$colorTrue" } else { "$colorFalse" },
        ).unwrap()
    }

    #[test]
    fn block_constructor_executes_subclass_hook_before_initializers_and_builds_real_states() {
        let Some((root, hook)) = compile_block_hook_fixture() else {
            eprintln!("javac unavailable; skipping compiled Block subclass fixture");
            return;
        };
        let mut prepared = crate::PreparedLoad::from_jars(Vec::new(), true).unwrap();
        prepared.linked.add_bytecode_classes(BTreeMap::from([(hook.name.clone(), hook)]));
        prepared.linked.register_native_module(mmga_runtime::NativeModuleRegistry {
            classes: &[],
            lambdas: &[(61501, fixture_light), (61502, fixture_map_color)],
        });
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        let properties_class = "net/minecraft/world/level/block/state/BlockBehaviour$Properties";
        let properties = invoke_static(
            &mut ctx,
            properties_class,
            "of",
            "()Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;",
            &[],
        );
        let light = ctx.rt.alloc(
            "java/util/function/ToIntFunction",
            0,
            Data::Closure(Box::new(mmga_runtime::ClosureData {
                interface_class: "java/util/function/ToIntFunction".into(),
                sam_name: "applyAsInt".into(),
                sam_desc: "(Ljava/lang/Object;)I".into(),
                instantiated_desc: "(Lnet/minecraft/world/level/block/state/BlockState;)I".into(),
                captures: Vec::new(),
                target: mmga_runtime::ClosureTarget::NativeLambda(61501),
            })),
        );
        let color_true = ctx.rt.new_instance("net/minecraft/world/level/material/MapColor", 0);
        let color_false = ctx.rt.new_instance("net/minecraft/world/level/material/MapColor", 0);
        ctx.rt.put_static("test/HookBlock", "$colorTrue", color_true);
        ctx.rt.put_static("test/HookBlock", "$colorFalse", color_false);
        let map_color = ctx.rt.alloc(
            "java/util/function/Function",
            0,
            Data::Closure(Box::new(mmga_runtime::ClosureData {
                interface_class: "java/util/function/Function".into(),
                sam_name: "apply".into(),
                sam_desc: "(Ljava/lang/Object;)Ljava/lang/Object;".into(),
                instantiated_desc: "(Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/material/MapColor;".into(),
                captures: Vec::new(),
                target: mmga_runtime::ClosureTarget::NativeLambda(61502),
            })),
        );
        ctx.rt.put_field(properties, "lightEmission", light);
        ctx.rt.put_field(properties, "mapColor", map_color);
        ctx.rt.put_field(properties, "destroyTime", V::F(2.5));
        ctx.rt.put_field(properties, "explosionResistance", V::F(7.0));
        ctx.rt.put_field(properties, "requiresCorrectToolForDrops", V::TRUE);
        ctx.rt.put_field(properties, "isAir", V::TRUE);
        let block = alloc_new_object(&mut ctx, "test/HookBlock");
        assert!(ctx.rt.pending_exception.is_none());
        let active_sound = ctx.rt.new_instance("net/minecraft/world/level/block/SoundType", 0);
        let inactive_sound = ctx.rt.new_instance("net/minecraft/world/level/block/SoundType", 0);
        ctx.rt.put_static("test/HookBlock", "activeSound", active_sound);
        ctx.rt.put_static("test/HookBlock", "inactiveSound", inactive_sound);
        invoke_member(
            &mut ctx,
            op::INVOKESPECIAL,
            &MemberRef::new("test/HookBlock", "<init>", "(Lnet/minecraft/world/level/block/state/BlockBehaviour$Properties;)V"),
            &[block, properties],
        );
        let error = ctx.rt.take_exception();
        assert!(error.is_none(), "Block constructor failed: {:?}", error.map(|error| ctx.rt.exception_summary(error)));
        assert_eq!(ctx.rt.get_static("test/HookBlock", "hookCalls"), Some(V::I(1)));
        assert_eq!(ctx.rt.get_static("test/HookBlock", "holderSeenInHook"), Some(V::TRUE));
        assert_eq!(ctx.rt.get_static("test/HookBlock", "definitionSeenInHook"), Some(V::FALSE));
        assert_eq!(ctx.rt.get_static("test/HookBlock", "fieldSeenInHook"), Some(V::I(0)));
        assert_eq!(ctx.rt.get_field(block, "initializedAfterSuper"), V::I(42));
        assert_eq!(ctx.rt.get_field(block, "properties"), properties);
        assert_eq!(ctx.rt.get_field(block, "explosionResistance"), V::F(7.0));

        let active = ctx.rt.get_static("test/HookBlock", "ACTIVE").unwrap();
        let inverted = ctx.rt.get_static("test/HookBlock", "INVERTED").unwrap();
        let definition = ctx.rt.get_field(block, "stateDefinition");
        assert!(!definition.is_null());
        let properties = collection_items(ctx.rt, ctx.rt.get_field(definition, "$properties"));
        assert_eq!(properties, [active, inverted]);
        let states = collection_items(ctx.rt, ctx.rt.get_field(definition, "states"));
        assert_eq!(states.len(), 4);
        let values: Vec<_> = states.iter().map(|state| {
            multistate_values(ctx.rt, *state).into_iter().map(|(_, value)| ctx.rt.get_field(value, "$value")).collect::<Vec<_>>()
        }).collect();
        assert_eq!(values, [vec![V::TRUE, V::TRUE], vec![V::TRUE, V::FALSE], vec![V::FALSE, V::TRUE], vec![V::FALSE, V::FALSE]]);
        assert_eq!(ctx.rt.get_field(block, "defaultBlockState"), states[2]);
        for (index, state) in states.iter().copied().enumerate() {
            assert_eq!(ctx.rt.get_field(state, "lightEmission"), V::I(if index < 2 { 11 } else { 3 }));
            assert_eq!(ctx.rt.get_field(state, "destroySpeed"), V::F(2.5));
            assert_eq!(ctx.rt.get_field(state, "owner"), block);
            assert_eq!(ctx.rt.get_field(state, "mapColor"), if index % 2 == 0 { color_true } else { color_false });
            assert_eq!(ctx.rt.get_field(state, "canOcclude"), V::TRUE);
            assert_eq!(ctx.rt.get_field(state, "requiresCorrectToolForDrops"), V::TRUE);
            assert!(!ctx.rt.get_field(state, "isRedstoneConductor").is_null());
            let sound = invoke(
                &mut ctx,
                if index % 2 == 0 { STATE_BASE } else { STATE },
                "getSoundType",
                "()Lnet/minecraft/world/level/block/SoundType;",
                &[state],
            );
            assert_eq!(sound, if index < 2 { active_sound } else { inactive_sound });
            assert_eq!(ctx.rt.get_static("test/HookBlock", "lastSoundState"), Some(state));
        }
        assert_eq!(ctx.rt.get_static("test/HookBlock", "soundCalls"), Some(V::I(4)));
        let expected_base_sound = ctx.rt.get_field(block, "soundType");
        let base_sound = invoke(
            &mut ctx,
            "test/HookBlock",
            "baseSound",
            "(Lnet/minecraft/world/level/block/state/BlockState;)Lnet/minecraft/world/level/block/SoundType;",
            &[block, states[0]],
        );
        assert_eq!(base_sound, expected_base_sound);
        assert_eq!(ctx.rt.get_static("test/HookBlock", "soundCalls"), Some(V::I(4)));
        let set_false = box_primitive(ctx.rt, "java/lang/Boolean", V::FALSE);
        let next = invoke(
            &mut ctx,
            STATE_HOLDER,
            "setValue",
            "(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;",
            &[states[0], active, set_false],
        );
        assert_eq!(next, states[2]);
        assert!(ctx.rt.pending_exception.is_none());
        invoke(
            &mut ctx,
            "test/HookBlock",
            "registerDefaultState",
            "(Lnet/minecraft/world/level/block/state/BlockState;)V",
            &[block, V::NULL],
        );
        assert_eq!(ctx.rt.get_field(block, "defaultBlockState"), V::NULL);
        let foreign_state = ctx.rt.new_instance(STATE, 0);
        invoke(
            &mut ctx,
            "test/HookBlock",
            "registerDefaultState",
            "(Lnet/minecraft/world/level/block/state/BlockState;)V",
            &[block, foreign_state],
        );
        assert_eq!(ctx.rt.get_field(block, "defaultBlockState"), foreign_state);
        assert!(ctx.rt.pending_exception.is_none());
        drop(ctx);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn boolean_property_factory_and_value_semantics_match_vanilla() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let factory_desc = "(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/BooleanProperty;";
        let first_name = ctx.rt.intern_str("active");
        let same_name = ctx.rt.new_str("active");
        let first = invoke_static(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "create",
            factory_desc,
            &[first_name],
        );
        let second = invoke_static(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "create",
            factory_desc,
            &[same_name],
        );
        assert!(ctx.rt.pending_exception.is_none());
        assert_ne!(first, second, "factory allocates a fresh property each call");
        assert_eq!(ctx.rt.class_of(first), Some(BOOLEAN_PROPERTY));
        assert_eq!(ctx.rt.get_field(first, "name"), first_name);
        assert_eq!(
            invoke(
                &mut ctx,
                BOOLEAN_PROPERTY,
                "getValueClass",
                "()Ljava/lang/Class;",
                &[first],
            ),
            ctx.rt.new_pinned_handle("java/lang/Class", "class", 0, "java/lang/Boolean")
        );

        let values = invoke(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "getPossibleValues",
            "()Ljava/util/Collection;",
            &[first],
        );
        let values = collection_items(ctx.rt, values);
        assert_eq!(values.len(), 2);
        assert_eq!(ctx.rt.get_field(values[0], "$value"), V::TRUE);
        assert_eq!(ctx.rt.get_field(values[1], "$value"), V::FALSE);

        for (text, expected) in [("true", Some(V::TRUE)), ("false", Some(V::FALSE)), ("TRUE", None), (" true", None), ("yes", None)] {
            let text = ctx.rt.new_str(text);
            let optional = invoke(
                &mut ctx,
                BOOLEAN_PROPERTY,
                "getValue",
                "(Ljava/lang/String;)Ljava/util/Optional;",
                &[first, text],
            );
            let actual = match &ctx.rt.get_obj(optional).unwrap().data {
                Data::Optional(value) => *value,
                other => panic!("unexpected Optional representation: {other:?}"),
            };
            assert_eq!(actual.map(|boxed| ctx.rt.get_field(boxed, "$value")), expected);
        }
        let true_value = box_primitive(ctx.rt, "java/lang/Boolean", V::TRUE);
        let false_value = box_primitive(ctx.rt, "java/lang/Boolean", V::FALSE);
        let true_name = invoke(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "getName",
            "(Ljava/lang/Boolean;)Ljava/lang/String;",
            &[first, true_value],
        );
        assert_eq!(ctx.rt.str_of(true_name), Some("true"));
        let false_name = invoke(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "getName",
            "(Ljava/lang/Boolean;)Ljava/lang/String;",
            &[first, false_value],
        );
        assert_eq!(ctx.rt.str_of(false_name), Some("false"));

        assert_eq!(
            invoke(&mut ctx, BOOLEAN_PROPERTY, "equals", "(Ljava/lang/Object;)Z", &[first, second]),
            V::TRUE
        );
        assert_eq!(
            invoke(&mut ctx, BOOLEAN_PROPERTY, "hashCode", "()I", &[first]),
            invoke(&mut ctx, BOOLEAN_PROPERTY, "hashCode", "()I", &[second])
        );
        let different_name = ctx.rt.intern_str("powered");
        let different = invoke_static(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "create",
            factory_desc,
            &[different_name],
        );
        assert_eq!(
            invoke(&mut ctx, BOOLEAN_PROPERTY, "equals", "(Ljava/lang/Object;)Z", &[first, different]),
            V::FALSE
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn block_state_properties_open_is_a_stable_static_property() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let desc = "Lnet/minecraft/world/level/block/state/properties/BooleanProperty;";
        let open = read_static_field(&mut ctx, STATE_PROPERTIES, "OPEN", desc);
        let open_again = read_static_field(&mut ctx, STATE_PROPERTIES, "OPEN", desc);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(open, open_again);
        assert_eq!(ctx.rt.str_of(ctx.rt.get_field(open, "name")), Some("open"));

        let factory_desc = "(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/BooleanProperty;";
        let name = ctx.rt.new_str("open");
        let separately_created = invoke_static(
            &mut ctx,
            BOOLEAN_PROPERTY,
            "create",
            factory_desc,
            &[name],
        );
        assert_ne!(open, separately_created);
        assert_eq!(
            invoke(
                &mut ctx,
                BOOLEAN_PROPERTY,
                "equals",
                "(Ljava/lang/Object;)Z",
                &[open, separately_created],
            ),
            V::TRUE
        );
        assert_eq!(
            invoke(&mut ctx, BOOLEAN_PROPERTY, "hashCode", "()I", &[open]),
            invoke(&mut ctx, BOOLEAN_PROPERTY, "hashCode", "()I", &[separately_created])
        );
        let triggered = read_static_field(&mut ctx, STATE_PROPERTIES, "TRIGGERED", desc);
        assert_ne!(open, triggered);
        assert_eq!(
            invoke(
                &mut ctx,
                BOOLEAN_PROPERTY,
                "equals",
                "(Ljava/lang/Object;)Z",
                &[open, triggered],
            ),
            V::FALSE
        );
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn block_state_properties_waterlogged_is_a_stable_static_property() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let desc = "Lnet/minecraft/world/level/block/state/properties/BooleanProperty;";
        let property = read_static_field(&mut ctx, STATE_PROPERTIES, "WATERLOGGED", desc);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(property, read_static_field(&mut ctx, STATE_PROPERTIES, "WATERLOGGED", desc));
        assert_eq!(ctx.rt.str_of(ctx.rt.get_field(property, "name")), Some("waterlogged"));
        let possible_values = invoke(&mut ctx, BOOLEAN_PROPERTY, "getPossibleValues", "()Ljava/util/Collection;", &[property]);
        let values = collection_items(ctx.rt, possible_values);
        assert_eq!(values.len(), 2);
        assert_eq!(ctx.rt.get_field(values[0], "$value"), V::TRUE);
        assert_eq!(ctx.rt.get_field(values[1], "$value"), V::FALSE);
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn horizontal_facing_keeps_direction_values_order_when_filtered_by_plane_predicate() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let owner = STATE_PROPERTIES;
        let desc = "Lnet/minecraft/world/level/block/state/properties/DirectionProperty;";
        let facing = read_static_field(&mut ctx, owner, "HORIZONTAL_FACING", desc);
        let facing_again = read_static_field(&mut ctx, owner, "HORIZONTAL_FACING", desc);
        assert!(ctx.rt.pending_exception.is_none());
        assert_eq!(facing, facing_again);
        assert_eq!(ctx.rt.str_of(ctx.rt.get_field(facing, "name")), Some("facing"));

        let values = invoke(
            &mut ctx,
            DIRECTION_PROPERTY,
            "getPossibleValues",
            "()Ljava/util/Collection;",
            &[facing],
        );
        let values = collection_items(ctx.rt, values);
        assert_eq!(values.len(), 4);
        let names: Vec<_> = values
            .iter()
            .map(|value| ctx.rt.str_of(ctx.rt.get_field(*value, "name")).unwrap())
            .collect();
        assert_eq!(names, ["north", "south", "west", "east"]);
        let directions = crate::direction::call(
            ctx.rt,
            &MemberRef::new("net/minecraft/core/Direction", "values", "()[Lnet/minecraft/core/Direction;"),
            true,
            &[],
        ).unwrap();
        let Data::Array(directions) = &ctx.rt.get_obj(directions).unwrap().data else {
            panic!("Direction.values must return Direction[]");
        };
        let filtered: Vec<_> = directions.iter().copied().filter(|direction| {
            matches!(ctx.rt.str_of(ctx.rt.get_field(*direction, "$name")), Some("NORTH" | "SOUTH" | "WEST" | "EAST"))
        }).collect();
        assert_eq!(values, filtered);
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn create_piston_types_keep_serialized_names_canonical_values_and_enum_errors() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let owner = "net/minecraft/world/level/block/state/properties/PistonType";
        let desc = "Lnet/minecraft/world/level/block/state/properties/PistonType;";
        for (index, name, label) in [(0, "DEFAULT", "normal"), (1, "STICKY", "sticky")] {
            let value = read_static_field(&mut ctx, owner, name, desc);
            assert!(ctx.rt.pending_exception.is_none(), "{name} missing");
            assert_eq!(
                invoke(&mut ctx, owner, "ordinal", "()I", &[value]),
                V::I(index)
            );
            let serialized = invoke(
                &mut ctx,
                owner,
                "getSerializedName",
                "()Ljava/lang/String;",
                &[value],
            );
            assert_eq!(ctx.rt.str_of(serialized), Some(label));
            let printed = invoke(
                &mut ctx,
                owner,
                "toString",
                "()Ljava/lang/String;",
                &[value],
            );
            assert_eq!(ctx.rt.str_of(printed), Some(label));
        }
        let values = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                owner,
                "values",
                "()[Lnet/minecraft/world/level/block/state/properties/PistonType;",
            ),
            &[],
        );
        assert!(
            matches!(&ctx.rt.get_obj(values).unwrap().data,Data::Array(values) if values.len()==2)
        );
        if let Data::Array(values) = &mut ctx.rt.get_obj_mut(values).unwrap().data {
            values[0] = V::NULL;
        }
        let fresh = invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                owner,
                "values",
                "()[Lnet/minecraft/world/level/block/state/properties/PistonType;",
            ),
            &[],
        );
        assert!(
            matches!(&ctx.rt.get_obj(fresh).unwrap().data,Data::Array(values) if !values[0].is_null())
        );
        let lowercase = ctx.rt.intern_str("normal");
        invoke_member(
            &mut ctx,
            op::INVOKESTATIC,
            &MemberRef::new(
                owner,
                "valueOf",
                "(Ljava/lang/String;)Lnet/minecraft/world/level/block/state/properties/PistonType;",
            ),
            &[lowercase],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert_eq!(
            ctx.rt.class_of(error),
            Some("java/lang/IllegalArgumentException")
        );
    }

    #[test]
    fn dispenser_and_dropper_keep_real_default_states_and_canonical_property_transitions() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (field, class) in [
            (
                "DISPENSER",
                "net/minecraft/world/level/block/DispenserBlock",
            ),
            ("DROPPER", "net/minecraft/world/level/block/DropperBlock"),
        ] {
            let block = read_static_field(&mut ctx, BLOCKS, field, BLOCK_DESC);
            assert!(
                ctx.rt.pending_exception.is_none(),
                "{field} constant missing"
            );
            assert_eq!(ctx.rt.class_of(block), Some(class));
            assert_eq!(
                block,
                read_static_field(&mut ctx, BLOCKS, field, BLOCK_DESC)
            );
            assert_eq!(
                invoke(&mut ctx, BLOCK, "defaultDestroyTime", "()F", &[block]),
                V::F(3.5)
            );
            assert_eq!(
                invoke(&mut ctx, BLOCK, "getExplosionResistance", "()F", &[block]),
                V::F(3.5)
            );
            let state = invoke(
                &mut ctx,
                BLOCK,
                "defaultBlockState",
                "()Lnet/minecraft/world/level/block/state/BlockState;",
                &[block],
            );
            let facing = read_static_field(
                &mut ctx,
                class,
                "FACING",
                "Lnet/minecraft/world/level/block/state/properties/DirectionProperty;",
            );
            let triggered = read_static_field(
                &mut ctx,
                class,
                "TRIGGERED",
                "Lnet/minecraft/world/level/block/state/properties/BooleanProperty;",
            );
            let direction = invoke(&mut ctx, STATE, "getValue", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;", &[state, facing]);
            assert_eq!(
                ctx.rt.str_of(ctx.rt.get_field(direction, "$name")),
                Some("NORTH")
            );
            let initial_triggered = invoke(&mut ctx, STATE, "getValue", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;", &[state, triggered]);
            assert_eq!(ctx.rt.get_field(initial_triggered, "$value"), V::FALSE);
            assert_eq!(
                invoke(&mut ctx, STATE, "hasBlockEntity", "()Z", &[state]),
                V::TRUE
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    STATE,
                    "requiresCorrectToolForDrops",
                    "()Z",
                    &[state]
                ),
                V::TRUE
            );
            let definition = invoke(
                &mut ctx,
                BLOCK,
                "getStateDefinition",
                "()Lnet/minecraft/world/level/block/state/StateDefinition;",
                &[block],
            );
            let states = invoke(
                &mut ctx,
                DEFINITION,
                "getPossibleStates",
                "()Lcom/google/common/collect/ImmutableList;",
                &[definition],
            );
            let all = collection_items(ctx.rt, states);
            assert_eq!(all.len(), 12);
            assert_eq!(
                all.iter()
                    .map(|value| value.r())
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                12
            );
            let first = invoke(
                &mut ctx,
                DEFINITION,
                "any",
                "()Lnet/minecraft/world/level/block/state/StateHolder;",
                &[definition],
            );
            let first_dir = invoke(&mut ctx, STATE, "getValue", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;", &[first, facing]);
            assert_eq!(
                ctx.rt.str_of(ctx.rt.get_field(first_dir, "$name")),
                Some("DOWN")
            );
            for original in all {
                let changed = invoke(&mut ctx, STATE, "cycle", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Object;", &[original, triggered]);
                assert_ne!(changed, original);
                assert_eq!(invoke(&mut ctx, STATE, "cycle", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Object;", &[changed, triggered]), original);
                let mut rotated = original;
                for _ in 0..6 {
                    rotated = invoke(&mut ctx, STATE, "cycle", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Object;", &[rotated, facing]);
                }
                assert_eq!(rotated, original);
            }
            let bad_value = box_primitive(ctx.rt, "java/lang/Integer", V::I(1));
            invoke(&mut ctx, STATE, "setValue", "(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;", &[state, triggered, bad_value]);
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/IllegalArgumentException")
            );
            invoke(
                &mut ctx,
                STATE,
                "getAnalogOutputSignal",
                "(Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;)I",
                &[state, V::NULL, V::NULL],
            );
            let error = ctx.rt.take_exception().unwrap();
            assert_eq!(
                ctx.rt.class_of(error),
                Some("java/lang/UnsupportedOperationException")
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn vanilla_display_blocks_have_registered_identity_and_official_defaults() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for (name, id, class, property_field, max, hardness, resistance, tool) in [
            (
                "RESPAWN_ANCHOR",
                "minecraft:respawn_anchor",
                ANCHOR,
                "RESPAWN_ANCHOR_CHARGES",
                4,
                50.0,
                1200.0,
                true,
            ),
            (
                "TARGET",
                "minecraft:target",
                TARGET,
                "POWER",
                15,
                0.5,
                0.5,
                false,
            ),
        ] {
            let block = read_static_field(
                &mut ctx,
                BLOCKS,
                name,
                "Lnet/minecraft/world/level/block/Block;",
            );
            assert!(ctx.rt.pending_exception.is_none(), "{name}");
            assert_eq!(ctx.rt.class_of(block), Some(class));
            assert_eq!(
                block,
                read_static_field(
                    &mut ctx,
                    BLOCKS,
                    name,
                    "Lnet/minecraft/world/level/block/Block;"
                )
            );
            let registry = ctx.rt.new_pinned_handle(
                "net/minecraft/core/DefaultedRegistry",
                "registry",
                0,
                "minecraft:block",
            );
            let key = invoke(
                &mut ctx,
                "net/minecraft/core/Registry",
                "getKey",
                "(Ljava/lang/Object;)Lnet/minecraft/resources/ResourceLocation;",
                &[registry, block],
            );
            assert_eq!(ctx.rt.handle_of(key).unwrap().2, id);
            assert_eq!(
                invoke(
                    &mut ctx,
                    "net/minecraft/core/Registry",
                    "get",
                    "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
                    &[registry, key]
                ),
                block
            );
            assert_eq!(
                invoke(&mut ctx, BLOCK, "defaultDestroyTime", "()F", &[block]),
                V::F(hardness)
            );
            assert_eq!(
                invoke(&mut ctx, BLOCK, "getExplosionResistance", "()F", &[block]),
                V::F(resistance)
            );
            let state = invoke(
                &mut ctx,
                BLOCK,
                "defaultBlockState",
                "()Lnet/minecraft/world/level/block/state/BlockState;",
                &[block],
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    STATE,
                    "getBlock",
                    "()Lnet/minecraft/world/level/block/Block;",
                    &[state]
                ),
                block
            );
            assert_eq!(
                invoke(
                    &mut ctx,
                    STATE,
                    "requiresCorrectToolForDrops",
                    "()Z",
                    &[state]
                ),
                V::bool(tool)
            );
            let property = read_static_field(
                &mut ctx,
                STATE_PROPERTIES,
                property_field,
                "Lnet/minecraft/world/level/block/state/properties/IntegerProperty;",
            );
            let zero = invoke(&mut ctx, STATE, "getValue", "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;", &[state, property]);
            assert_eq!(ctx.rt.get_field(zero, "$value"), V::I(0));
            let definition = invoke(
                &mut ctx,
                BLOCK,
                "getStateDefinition",
                "()Lnet/minecraft/world/level/block/state/StateDefinition;",
                &[block],
            );
            let states = invoke(
                &mut ctx,
                DEFINITION,
                "getPossibleStates",
                "()Lcom/google/common/collect/ImmutableList;",
                &[definition],
            );
            assert_eq!(
                invoke(&mut ctx, "java/util/List", "size", "()I", &[states]),
                V::I(max + 1)
            );
        }
        assert!(ctx.rt.pending_exception.is_none());
    }

    #[test]
    fn charge_states_are_canonical_and_validate_property_and_range() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let block = read_static_field(
            &mut ctx,
            BLOCKS,
            "RESPAWN_ANCHOR",
            "Lnet/minecraft/world/level/block/Block;",
        );
        let property = read_static_field(
            &mut ctx,
            ANCHOR,
            "CHARGE",
            "Lnet/minecraft/world/level/block/state/properties/IntegerProperty;",
        );
        let initial = invoke(
            &mut ctx,
            BLOCK,
            "defaultBlockState",
            "()Lnet/minecraft/world/level/block/state/BlockState;",
            &[block],
        );
        assert!(ctx.rt.pending_exception.is_none());
        for (value, light) in [0, 3, 7, 11, 15].into_iter().enumerate() {
            let number = box_primitive(ctx.rt, "java/lang/Integer", V::I(value as i32));
            let state=invoke(&mut ctx,STATE,"setValue","(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;",&[initial,property,number]);
            assert_eq!(
                invoke(&mut ctx, STATE, "getLightEmission", "()I", &[state]),
                V::I(light)
            );
            assert_eq!(invoke(&mut ctx,STATE,"setValue","(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;",&[state,property,number]),state);
        }
        let invalid = box_primitive(ctx.rt, "java/lang/Integer", V::I(5));
        invoke(&mut ctx,STATE,"setValue","(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;",&[initial,property,invalid]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("IllegalArgumentException"));
        let power = read_static_field(
            &mut ctx,
            STATE_PROPERTIES,
            "POWER",
            "Lnet/minecraft/world/level/block/state/properties/IntegerProperty;",
        );
        invoke(
            &mut ctx,
            STATE,
            "getValue",
            "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Comparable;",
            &[initial, power],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("IllegalArgumentException"));
        assert_eq!(
            invoke(&mut ctx, STATE, "getLightEmission", "()I", &[initial]),
            V::I(0)
        );
    }

    #[test]
    fn partial_registry_never_supplies_global_ids_or_fake_complete_enumeration() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        read_static_field(
            &mut ctx,
            BLOCKS,
            "TARGET",
            "Lnet/minecraft/world/level/block/Block;",
        );
        assert!(ctx.rt.pending_exception.is_none());
        read_static_field(
            &mut ctx,
            BLOCK,
            "BLOCK_STATE_REGISTRY",
            "Lnet/minecraft/core/IdMapper;",
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        let registry = ctx.rt.new_pinned_handle(
            "net/minecraft/core/DefaultedRegistry",
            "registry",
            0,
            "minecraft:block",
        );
        invoke(
            &mut ctx,
            "net/minecraft/core/Registry",
            "size",
            "()I",
            &[registry],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        let unknown = ctx.rt.new_pinned_handle(
            "net/minecraft/resources/ResourceLocation",
            "resource_location",
            0,
            "minecraft:stone",
        );
        invoke(
            &mut ctx,
            "net/minecraft/core/Registry",
            "get",
            "(Lnet/minecraft/resources/ResourceLocation;)Ljava/lang/Object;",
            &[registry, unknown],
        );
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }

    #[test]
    fn local_state_lists_are_immutable_and_world_operations_stay_unsupported() {
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = BlocksHost::default();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        let block = read_static_field(&mut ctx, BLOCKS, "TARGET", BLOCK_DESC);
        let state = invoke(
            &mut ctx,
            BLOCK,
            "defaultBlockState",
            "()Lnet/minecraft/world/level/block/state/BlockState;",
            &[block],
        );
        let definition = invoke(
            &mut ctx,
            BLOCK,
            "getStateDefinition",
            "()Lnet/minecraft/world/level/block/state/StateDefinition;",
            &[block],
        );
        let states = invoke(
            &mut ctx,
            DEFINITION,
            "getPossibleStates",
            "()Lcom/google/common/collect/ImmutableList;",
            &[definition],
        );
        invoke(&mut ctx, "java/util/List", "clear", "()V", &[states]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert_eq!(
            invoke(&mut ctx, "java/util/List", "size", "()I", &[states]),
            V::I(16)
        );
        let power = read_static_field(&mut ctx, STATE_PROPERTIES, "POWER", PROPERTY_DESC);
        let values = invoke(
            &mut ctx,
            INTEGER_PROPERTY,
            "getPossibleValues",
            "()Ljava/util/Collection;",
            &[power],
        );
        invoke(&mut ctx, "java/util/Collection", "clear", "()V", &[values]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        assert_eq!(
            invoke(&mut ctx, "java/util/Collection", "size", "()I", &[values]),
            V::I(16)
        );
        let maximum = box_primitive(ctx.rt, "java/lang/Integer", V::I(15));
        let powered = invoke(&mut ctx, STATE, "setValue", "(Lnet/minecraft/world/level/block/state/properties/Property;Ljava/lang/Comparable;)Ljava/lang/Object;", &[state, power, maximum]);
        assert_eq!(invoke(&mut ctx, STATE, "getSignal", "(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;Lnet/minecraft/core/Direction;)I", &[powered,V::NULL,V::NULL,V::NULL]), V::I(15));
        assert_eq!(
            invoke(
                &mut ctx,
                STATE,
                "cycle",
                "(Lnet/minecraft/world/level/block/state/properties/Property;)Ljava/lang/Object;",
                &[powered, power]
            ),
            state
        );
        invoke(&mut ctx, STATE, "getCollisionShape", "(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/phys/shapes/VoxelShape;", &[state,V::NULL,V::NULL]);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
        read_static_field(&mut ctx, BLOCKS, "STONE", BLOCK_DESC);
        let error = ctx.rt.take_exception().unwrap();
        assert!(ctx
            .rt
            .exception_summary(error)
            .contains("UnsupportedOperationException"));
    }
}

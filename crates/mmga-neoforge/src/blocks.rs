//! Verified local block definitions for Create's vanilla display associations.
//!
//! Minecraft 1.21.1 official bytecode: Blocks=dga, RespawnAnchorBlock=dmh,
//! TargetBlock=dog, StateHolder=dte, IntegerProperty=duc. Client SHA-1
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
const BEHAVIOUR: &str = "net/minecraft/world/level/block/state/BlockBehaviour";
const STATE_BASE: &str = "net/minecraft/world/level/block/state/BlockBehaviour$BlockStateBase";
const STATE_HOLDER: &str = "net/minecraft/world/level/block/state/StateHolder";
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

const DEFINITIONS: [BlockDefinition; 2] = [
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
];

pub fn get_static_field(
    rt: &mut Rt,
    registries: &mut RegistryStore,
    owner: &str,
    name: &str,
    desc: &str,
) -> Option<V> {
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
    if let Some(value) = registry_call(rt, registries, member, is_static, args) {
        return Some(value);
    }
    if is_static {
        return match (owner, name, desc) {
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
    if matches!(rt.class_of(recv), Some(ANCHOR | TARGET)) {
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
    if rt.class_of(recv) == Some(STATE) && !rt.get_field(recv, "$property").is_null() {
        return state_call(rt, recv, name, desc, &args[1..]);
    }
    if rt.class_of(recv) == Some(DEFINITION) && !rt.get_field(recv, "$property").is_null() {
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
                let property = rt.get_field(recv, "$property");
                Some(
                    if rt.str_of(*args.get(1)?) == rt.str_of(rt.get_field(property, "name")) {
                        property
                    } else {
                        V::NULL
                    },
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
    None
}

fn block_constant(rt: &mut Rt, registries: &mut RegistryStore, definition: &BlockDefinition) -> V {
    if let Some(value) = rt.get_static(BLOCKS, definition.name) {
        return value;
    }
    let block = rt.new_instance(definition.class, 0);
    let property = property(rt, definition.property, definition.max);
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
    rt.put_field(block, "stateDefinition", state_definition);
    rt.put_field(block, "defaultBlockState", first);
    let holder = rt.new_instance("net/minecraft/core/Holder$Reference", 0);
    let key = crate::minecraft::new_resource_key(rt, "minecraft:block", definition.id);
    rt.put_field(holder, "$key", key);
    rt.put_field(holder, "$value", block);
    rt.put_field(block, "builtInRegistryHolder", holder);
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
    rt.put_static(BLOCKS, definition.name, block);
    block
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

fn state_call(rt: &mut Rt, recv: V, name: &str, desc: &str, args: &[V]) -> Option<V> {
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
        ("isAir"|"liquid"|"ignitedByLava"|"isRandomlyTicking"|"hasBlockEntity","()Z") => Some(V::FALSE),
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
        ("hasAnalogOutputSignal","()Z") => Some(V::bool(rt.class_of(block)==Some(ANCHOR))),
        ("getAnalogOutputSignal","(Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;)I") => Some(V::I(if rt.class_of(block)==Some(ANCHOR) { current*15/4 } else { 0 })),
        ("getSignal","(Lnet/minecraft/world/level/BlockGetter;Lnet/minecraft/core/BlockPos;Lnet/minecraft/core/Direction;)I") => Some(V::I(if rt.class_of(block)==Some(TARGET) { current } else { 0 })),
        _=>None,
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
        ANCHOR | TARGET => matches!(
            super_class,
            BLOCK | BEHAVIOUR | "net/minecraft/world/level/ItemLike"
        ),
        STATE => matches!(super_class, STATE_BASE | STATE_HOLDER),
        INTEGER_PROPERTY => super_class == PROPERTY,
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
        box_primitive, invoke_member, read_static_field, ExecCtx, HostApi, LinkedSpace,
    };

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
            _linked: &LinkedSpace,
            member: &MemberRef,
            is_static: bool,
            args: &[V],
        ) -> Option<V> {
            super::call(rt, &mut self.0, member, is_static, args).or_else(|| {
                crate::minecraft::call_keys_authlib_and_libs(
                    rt,
                    &mut self.0,
                    member,
                    is_static,
                    args,
                )
            })
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

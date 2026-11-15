//! Authoritative Rust World, BlockState, Entity, Player, and ItemStack Simulation State
//! exposed natively to NeoForge mods (and synchronized with Horizon's server/engine hooks).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BlockCoord {
    pub dimension: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockCoord {
    pub fn overworld(x: i32, y: i32, z: i32) -> Self {
        Self {
            dimension: "minecraft:overworld".into(),
            x,
            y,
            z,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimEntity {
    pub uuid: String,
    pub entity_type: String,
    pub name: String,
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub health: f32,
    pub max_health: f32,
    pub is_player: bool,
    pub main_hand_item: String,
    pub main_hand_count: i32,
}

#[derive(Debug, Clone, Default)]
pub struct WorldState {
    pub game_time: i64,
    pub day_time: i64,
    pub blocks: BTreeMap<BlockCoord, String>,
    pub entities: BTreeMap<String, SimEntity>,
    pub command_feedback: Vec<String>,
}

impl WorldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_block(&mut self, coord: BlockCoord, block_id: impl Into<String>) {
        self.blocks.insert(coord, block_id.into());
    }

    pub fn get_block(&self, coord: &BlockCoord) -> &str {
        self.blocks
            .get(coord)
            .map(String::as_str)
            .unwrap_or("minecraft:air")
    }

    pub fn spawn_player(&mut self, uuid: &str, name: &str, x: f64, y: f64, z: f64) {
        self.entities.insert(
            uuid.to_owned(),
            SimEntity {
                uuid: uuid.to_owned(),
                entity_type: "minecraft:player".into(),
                name: name.to_owned(),
                dimension: "minecraft:overworld".into(),
                x,
                y,
                z,
                health: 20.0,
                max_health: 20.0,
                is_player: true,
                main_hand_item: "minecraft:air".into(),
                main_hand_count: 0,
            },
        );
    }
}

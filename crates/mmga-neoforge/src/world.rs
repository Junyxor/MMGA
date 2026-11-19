//! Diagnostic world model used by the standalone probe.
//!
//! Native Horizon callbacks use `host_world::HostWorld` instead. This model is
//! not a Minecraft world engine and its counters do not prove real gameplay.

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

    pub fn entity(&self, uuid: &str) -> Option<&SimEntity> {
        self.entities.get(uuid)
    }

    pub fn entity_mut(&mut self, uuid: &str) -> Option<&mut SimEntity> {
        self.entities.get_mut(uuid)
    }

    pub fn give_item(&mut self, uuid: &str, item_id: &str, count: i32) {
        if let Some(entity) = self.entities.get_mut(uuid) {
            entity.main_hand_item = item_id.to_owned();
            entity.main_hand_count = count;
        }
    }

    /// Applies incoming damage. Returns whether the entity existed.
    pub fn hurt(&mut self, uuid: &str, amount: f32) -> bool {
        let Some(entity) = self.entities.get_mut(uuid) else {
            return false;
        };
        if amount > 0.0 {
            entity.health = (entity.health - amount).max(0.0);
        }
        true
    }

    /// Advances one server tick (`game_time` and `day_time`).
    pub fn tick(&mut self) {
        self.game_time = self.game_time.saturating_add(1);
        self.day_time = self.day_time.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_players_and_ticks() {
        let mut world = WorldState::new();
        let origin = BlockCoord::overworld(0, 64, 0);
        assert_eq!(world.get_block(&origin), "minecraft:air");
        world.set_block(origin.clone(), "examplemod:example_block");
        assert_eq!(world.get_block(&origin), "examplemod:example_block");

        world.spawn_player("steve", "Steve", 0.5, 65.0, 0.5);
        world.give_item("steve", "minecraft:stick", 4);
        assert!(world.hurt("steve", 5.0));
        assert!(!world.hurt("missing", 1.0));
        let player = world.entity("steve").unwrap();
        assert_eq!(player.main_hand_item, "minecraft:stick");
        assert_eq!(player.main_hand_count, 4);
        assert!((player.health - 15.0).abs() < f32::EPSILON);
        world.tick();
        assert_eq!(world.game_time, 1);
        assert_eq!(world.day_time, 1);
    }
}

//! NeoForge & Vanilla Registries (`Registries`, `BuiltInRegistries`, `NeoForgeRegistries`,
//! `DeferredRegister`, `DeferredHolder`, `DeferredBlock`, `DeferredItem`, `RegisterEvent`).

use mmga_runtime::{Rt, V};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// NeoForge canonical `RegisterEvent` dispatch order (matching `GameData.getRegistrationOrder`).
pub const REGISTRATION_ORDER: &[&str] = &[
    "minecraft:attribute",
    "minecraft:data_component_type",
    "minecraft:armor_material",
    "minecraft:sound_event",
    "minecraft:fluid",
    "minecraft:mob_effect",
    "minecraft:block",
    "minecraft:entity_type",
    "minecraft:item",
    "minecraft:potion",
    "minecraft:particle_type",
    "minecraft:block_entity_type",
    "minecraft:menu",
    "minecraft:recipe_type",
    "minecraft:recipe_serializer",
    "minecraft:creative_mode_tab",
    "neoforge:attachment_types",
    "neoforge:fluid_type",
    "neoforge:biome_modifier_serializers",
    "neoforge:ingredient_serializer",
];

/// Normalizes a registry field or name (e.g. `"BLOCK"`, `"item"`, `"ATTACHMENT_TYPES"`)
/// into its canonical `"namespace:path"` registry key.
pub fn canonical_registry_key(raw: &str) -> String {
    let cleaned = raw.trim();
    if cleaned.contains(':') {
        return cleaned.to_ascii_lowercase();
    }
    match cleaned.to_ascii_uppercase().as_str() {
        "ATTACHMENT_TYPES" => "neoforge:attachment_types".into(),
        "FLUID_TYPES" | "FLUID_TYPE" => "neoforge:fluid_type".into(),
        "BIOME_MODIFIER_SERIALIZERS" => "neoforge:biome_modifier_serializers".into(),
        "INGREDIENT_TYPES" | "INGREDIENT_SERIALIZER" => "neoforge:ingredient_serializer".into(),
        other => format!("minecraft:{}", other.to_ascii_lowercase()),
    }
}

/// Concrete properties extracted when a mod constructs a `Block`, `Item`, `BlockItem`, or `CreativeModeTab`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentProperties {
    pub kind: String,
    pub template: String,
    pub hardness: Option<f32>,
    pub resistance: Option<f32>,
    pub light_level: Option<i32>,
    pub friction: Option<f32>,
    pub no_occlusion: bool,
    pub requires_correct_tool: bool,
    pub random_ticks: bool,
    pub max_stack_size: Option<i32>,
    pub max_damage: Option<i32>,
    pub rarity: Option<String>,
    pub fire_resistant: bool,
    pub block_for_item: Option<String>,
}

/// One registered object inside a frozen or active registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredEntry {
    pub registry: String,
    pub id: String,
    pub mod_id: String,
    #[serde(skip)]
    pub value: V,
    pub properties: ContentProperties,
}

/// Pending supplier entry added to a `DeferredRegister<T>` via `.register(name, supplier)`.
#[derive(Debug, Clone)]
pub struct PendingDeferredItem {
    pub name: String,
    pub full_id: String,
    pub supplier: V,
    pub holder_handle: V,
}

/// One `DeferredRegister` instance tracked by the host.
#[derive(Debug, Clone)]
pub struct DeferredRegisterRecord {
    pub handle_id: u64,
    pub registry_key: String,
    pub namespace: String,
    pub bound_mod_bus: Option<String>,
    pub entries: Vec<PendingDeferredItem>,
}

/// Central Rust store for all vanilla + NeoForge + custom mod registries.
#[derive(Debug, Default, Clone)]
pub struct RegistryStore {
    next_dr_id: u64,
    next_holder_id: u64,
    pub deferred_registers: BTreeMap<u64, DeferredRegisterRecord>,
    /// `holder_id -> (registry_key, full_id, resolved_value)`
    pub holders: BTreeMap<u64, (String, String, Option<V>)>,
    /// `registry_key -> (entry_id -> RegisteredEntry)`
    pub entries: BTreeMap<String, BTreeMap<String, RegisteredEntry>>,
    pub custom_registries: Vec<String>,
    pub frozen: bool,
}

impl RegistryStore {
    pub fn new() -> Self {
        let mut store = Self::default();
        store.seed_vanilla_defaults();
        store
    }

    fn seed_vanilla_defaults(&mut self) {
        for (reg, ids, kind) in [
            (
                "minecraft:block",
                &[
                    "minecraft:air",
                    "minecraft:stone",
                    "minecraft:dirt",
                    "minecraft:grass_block",
                    "minecraft:iron_block",
                    "minecraft:oak_planks",
                    "minecraft:glass",
                ][..],
                "block",
            ),
            (
                "minecraft:item",
                &[
                    "minecraft:air",
                    "minecraft:stick",
                    "minecraft:iron_ingot",
                    "minecraft:diamond",
                    "minecraft:apple",
                    "minecraft:stone",
                ][..],
                "item",
            ),
        ] {
            let map = self.entries.entry(reg.to_owned()).or_default();
            for &id in ids {
                map.insert(
                    id.to_owned(),
                    RegisteredEntry {
                        registry: reg.to_owned(),
                        id: id.to_owned(),
                        mod_id: "minecraft".into(),
                        value: V::NULL,
                        properties: ContentProperties {
                            kind: kind.into(),
                            template: id.into(),
                            ..Default::default()
                        },
                    },
                );
            }
        }
    }

    pub fn create_deferred_register(&mut self, registry_key: &str, namespace: &str) -> u64 {
        self.next_dr_id += 1;
        let id = self.next_dr_id;
        self.deferred_registers.insert(
            id,
            DeferredRegisterRecord {
                handle_id: id,
                registry_key: canonical_registry_key(registry_key),
                namespace: namespace.to_owned(),
                bound_mod_bus: None,
                entries: Vec::new(),
            },
        );
        id
    }

    pub fn enqueue_deferred_entry(
        &mut self,
        rt: &mut Rt,
        dr_id: u64,
        holder_class: &str,
        entry_name: &str,
        supplier: V,
    ) -> Option<V> {
        rt.pin(supplier);
        let dr = self.deferred_registers.get_mut(&dr_id)?;
        let full_id = format!("{}:{entry_name}", dr.namespace);
        self.next_holder_id += 1;
        let hid = self.next_holder_id;
        let holder_v = rt.new_pinned_handle(holder_class, "deferred_holder", hid, &full_id);
        self.holders
            .insert(hid, (dr.registry_key.clone(), full_id.clone(), None));
        dr.entries.push(PendingDeferredItem {
            name: entry_name.to_owned(),
            full_id,
            supplier,
            holder_handle: holder_v,
        });
        Some(holder_v)
    }

    pub fn register_direct(
        &mut self,
        rt: &mut Rt,
        registry_key: &str,
        full_id: &str,
        mod_id: &str,
        val: V,
        props: ContentProperties,
    ) {
        rt.pin(val);
        let reg = canonical_registry_key(registry_key);
        self.entries.entry(reg.clone()).or_default().insert(
            full_id.to_owned(),
            RegisteredEntry {
                registry: reg.clone(),
                id: full_id.to_owned(),
                mod_id: mod_id.to_owned(),
                value: val,
                properties: props,
            },
        );
        // Resolve any DeferredHolders pointing at `(reg, full_id)`
        for (_, (h_reg, h_id, slot)) in self.holders.iter_mut() {
            if h_reg == &reg && h_id == full_id {
                *slot = Some(val);
            }
        }
    }

    pub fn lookup(&self, registry_key: &str, full_id: &str) -> Option<&RegisteredEntry> {
        let reg = canonical_registry_key(registry_key);
        self.entries.get(&reg)?.get(full_id)
    }

    pub fn holder_value(&self, holder_id: u64) -> Option<V> {
        let (reg, full_id, cached) = self.holders.get(&holder_id)?;
        if let Some(v) = cached {
            return Some(*v);
        }
        self.entries
            .get(reg)
            .and_then(|m| m.get(full_id))
            .map(|e| e.value)
    }

    pub fn total_mod_entries(&self) -> usize {
        self.entries
            .values()
            .flat_map(|m| m.values())
            .filter(|e| e.mod_id != "minecraft")
            .count()
    }
}

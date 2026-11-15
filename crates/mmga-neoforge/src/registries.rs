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

/// One supplier still waiting for its registry's `RegisterEvent`.
#[derive(Debug, Clone)]
pub struct DeferredSupplier {
    pub dr_id: u64,
    pub registry_key: String,
    pub namespace: String,
    pub name: String,
    pub full_id: String,
    pub supplier: V,
}

/// Central Rust store for all vanilla + NeoForge + custom mod registries.
#[derive(Debug, Clone)]
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

impl Default for RegistryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryStore {
    pub fn new() -> Self {
        let mut store = Self {
            next_dr_id: 0,
            next_holder_id: 0,
            deferred_registers: BTreeMap::new(),
            holders: BTreeMap::new(),
            entries: BTreeMap::new(),
            custom_registries: Vec::new(),
            frozen: false,
        };
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

    /// Binds a `DeferredRegister` to the mod bus that will receive its `RegisterEvent`.
    pub fn bind_mod_bus(&mut self, dr_id: u64, mod_id: &str) -> bool {
        match self.deferred_registers.get_mut(&dr_id) {
            Some(dr) => {
                dr.bound_mod_bus = Some(mod_id.to_owned());
                true
            }
            None => false,
        }
    }

    /// Freezes registries the way `Registry#freeze` does after the register phase.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Suppliers in `RegisterEvent` order: `REGISTRATION_ORDER`, then any custom registry keys.
    pub fn pending_suppliers(&self) -> Vec<DeferredSupplier> {
        let mut by_reg: BTreeMap<String, Vec<DeferredSupplier>> = BTreeMap::new();
        let mut dr_ids: Vec<u64> = self.deferred_registers.keys().copied().collect();
        dr_ids.sort_unstable();
        for dr_id in dr_ids {
            let Some(dr) = self.deferred_registers.get(&dr_id) else {
                continue;
            };
            let bucket = by_reg.entry(dr.registry_key.clone()).or_default();
            for entry in &dr.entries {
                bucket.push(DeferredSupplier {
                    dr_id,
                    registry_key: dr.registry_key.clone(),
                    namespace: dr.namespace.clone(),
                    name: entry.name.clone(),
                    full_id: entry.full_id.clone(),
                    supplier: entry.supplier,
                });
            }
        }
        let mut out = Vec::new();
        for key in REGISTRATION_ORDER {
            if let Some(items) = by_reg.remove(*key) {
                out.extend(items);
            }
        }
        for items in by_reg.into_values() {
            out.extend(items);
        }
        out
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

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::Rt;

    #[test]
    fn canonicalizes_registry_names() {
        assert_eq!(canonical_registry_key("BLOCK"), "minecraft:block");
        assert_eq!(canonical_registry_key("minecraft:Item"), "minecraft:item");
        assert_eq!(
            canonical_registry_key("ATTACHMENT_TYPES"),
            "neoforge:attachment_types"
        );
        assert_eq!(canonical_registry_key("FLUID_TYPE"), "neoforge:fluid_type");
    }

    #[test]
    fn deferred_holders_resolve_in_registration_order() {
        let mut store = RegistryStore::new();
        assert!(store.lookup("block", "minecraft:stone").is_some());
        assert_eq!(
            store.lookup("item", "minecraft:air").unwrap().mod_id,
            "minecraft"
        );

        let mut rt = Rt::new();
        let items = store.create_deferred_register("ITEM", "examplemod");
        let blocks = store.create_deferred_register("BLOCK", "examplemod");
        assert!(store.bind_mod_bus(blocks, "examplemod"));
        let item_supplier = rt.intern_str("item-supplier");
        let block_supplier = rt.intern_str("block-supplier");
        let item_holder = store
            .enqueue_deferred_entry(
                &mut rt,
                items,
                "net/neoforged/neoforge/registries/DeferredItem",
                "example_block",
                item_supplier,
            )
            .unwrap();
        let block_holder = store
            .enqueue_deferred_entry(
                &mut rt,
                blocks,
                "net/neoforged/neoforge/registries/DeferredBlock",
                "example_block",
                block_supplier,
            )
            .unwrap();

        let plan = store.pending_suppliers();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].registry_key, "minecraft:block");
        assert_eq!(plan[1].registry_key, "minecraft:item");
        assert_eq!(plan[0].full_id, "examplemod:example_block");

        let block_holder_id = rt.handle_of(block_holder).unwrap().1;
        let item_holder_id = rt.handle_of(item_holder).unwrap().1;
        assert!(store.holder_value(block_holder_id).is_none());

        let block = rt.new_instance("net/minecraft/world/level/block/Block", 0);
        store.register_direct(
            &mut rt,
            "minecraft:block",
            "examplemod:example_block",
            "examplemod",
            block,
            ContentProperties {
                kind: "block".into(),
                hardness: Some(2.0),
                ..ContentProperties::default()
            },
        );
        assert_eq!(store.holder_value(block_holder_id), Some(block));
        assert!(store.holder_value(item_holder_id).is_none());
        assert_eq!(store.total_mod_entries(), 1);

        store.freeze();
        assert!(store.frozen);
    }
}

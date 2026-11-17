//! NeoForge & Vanilla Registries (`Registries`, `BuiltInRegistries`, `NeoForgeRegistries`,
//! `DeferredRegister`, `DeferredHolder`, `DeferredBlock`, `DeferredItem`, `RegisterEvent`).

use mmga_runtime::{Rt, V};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    /// DeferredRegister applies these aliases when its registry event begins.
    pub aliases: BTreeMap<String, String>,
    /// Matches DeferredRegister.seenRegisterEvent: set before its suppliers run.
    pub seen_register_event: bool,
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
    pub aliases: BTreeMap<String, BTreeMap<String, String>>,
    pub custom_registries: Vec<String>,
    /// Registry event passes already completed, even before the global freeze.
    pub completed_registries: BTreeSet<String>,
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
            aliases: BTreeMap::new(),
            custom_registries: Vec::new(),
            completed_registries: BTreeSet::new(),
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
        let registry_key = canonical_registry_key(registry_key);
        let seen_register_event = self.completed_registries.contains(&registry_key);
        self.deferred_registers.insert(
            id,
            DeferredRegisterRecord {
                handle_id: id,
                registry_key,
                namespace: namespace.to_owned(),
                bound_mod_bus: None,
                entries: Vec::new(),
                aliases: BTreeMap::new(),
                seen_register_event,
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

    /// The native loader invokes these deferred-register listeners during its
    /// flush before explicit RegisterEvent callbacks. NeoForge closes each
    /// DeferredRegister at listener entry, not after its factory returns.
    pub fn begin_deferred_registration(&mut self, registry_key: &str) {
        let key = canonical_registry_key(registry_key);
        for register in self.deferred_registers.values_mut() {
            if register.registry_key == key {
                register.seen_register_event = true;
            }
        }
    }

    pub fn add_deferred_alias(
        &mut self,
        register_id: u64,
        from: &str,
        to: &str,
    ) -> Result<(), String> {
        if self.deferred_registration_closed(register_id) {
            return Err(
                "Cannot add aliases to DeferredRegister after RegisterEvent has been fired.".into(),
            );
        }
        let register = self
            .deferred_registers
            .get_mut(&register_id)
            .ok_or_else(|| "Unknown DeferredRegister".to_owned())?;
        register.aliases.insert(from.to_owned(), to.to_owned());
        Ok(())
    }

    pub fn apply_deferred_aliases(&mut self, registry_key: &str) -> Result<(), String> {
        let registry = canonical_registry_key(registry_key);
        let aliases: Vec<_> = self
            .deferred_registers
            .values()
            .filter(|register| register.registry_key == registry)
            .flat_map(|register| {
                register
                    .aliases
                    .iter()
                    .map(|(from, to)| (from.clone(), to.clone()))
            })
            .collect();
        for (from, to) in aliases {
            self.add_alias(&registry, &from, &to)?;
        }
        Ok(())
    }

    /// Matches BaseMappedRegistry.addAlias; a directly registered name shadows
    /// an alias, identical aliases are idempotent, and active remaps are errors.
    pub fn add_alias(&mut self, registry_key: &str, from: &str, to: &str) -> Result<(), String> {
        if from == to {
            return Ok(());
        }
        let registry = canonical_registry_key(registry_key);
        if let Some(old) = self
            .aliases
            .get(&registry)
            .and_then(|aliases| aliases.get(from))
        {
            if old != to {
                return Err(format!("Duplicate alias with key \"{from}\" attempting to map to \"{to}\", found existing mapping \"{old}\""));
            }
        }
        if self.resolve_alias(&registry, to) == from {
            return Err(format!("Infinite alias loop detected: from {from} to {to}"));
        }
        self.aliases
            .entry(registry)
            .or_default()
            .insert(from.to_owned(), to.to_owned());
        Ok(())
    }

    pub fn contains_entry(&self, registry_key: &str, full_id: &str) -> bool {
        let registry = canonical_registry_key(registry_key);
        self.entries
            .get(&registry)
            .is_some_and(|entries| entries.contains_key(full_id))
    }

    pub fn resolve_alias(&self, registry_key: &str, full_id: &str) -> String {
        let registry = canonical_registry_key(registry_key);
        let mut current = full_id;
        while !self.contains_entry(&registry, current) {
            let Some(next) = self
                .aliases
                .get(&registry)
                .and_then(|aliases| aliases.get(current))
            else {
                break;
            };
            current = next;
        }
        current.to_owned()
    }

    pub fn finish_registry_event(&mut self, registry_key: &str) {
        self.completed_registries
            .insert(canonical_registry_key(registry_key));
    }

    pub fn deferred_registration_closed(&self, dr_id: u64) -> bool {
        self.frozen
            || self.deferred_registers.get(&dr_id).is_some_and(|register| {
                register.seen_register_event
                    || self.completed_registries.contains(&register.registry_key)
            })
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
        self.deferred_registers.get(&dr_id)?;
        if self.deferred_registration_closed(dr_id) {
            return Some(rt.throw_exception(
                "java/lang/IllegalStateException",
                "Cannot register new entries to DeferredRegister after RegisterEvent has been fired.",
            ));
        }
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

    /// Creates a reference to an entry without registering an entry or supplier.
    /// Like `DeferredHolder.create`, the reference can be created before its
    /// registry or value exists and resolves when registration supplies a value.
    pub fn bind_holder(
        &mut self,
        rt: &mut Rt,
        registry_key: &str,
        full_id: &str,
        holder_class: &str,
    ) -> V {
        let registry_key = canonical_registry_key(registry_key);
        let resolved = self
            .lookup(&registry_key, full_id)
            .map(|entry| entry.value)
            .filter(|value| !value.is_null());
        self.next_holder_id += 1;
        let id = self.next_holder_id;
        let holder = rt.new_pinned_handle(holder_class, "deferred_holder", id, full_id);
        self.holders
            .insert(id, (registry_key, full_id.to_owned(), resolved));
        holder
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
        self.entries
            .get(&reg)?
            .get(&self.resolve_alias(&reg, full_id))
    }

    pub fn holder_value(&self, holder_id: u64) -> Option<V> {
        let (reg, full_id, cached) = self.holders.get(&holder_id)?;
        if let Some(v) = cached.filter(|value| !value.is_null()) {
            return Some(v);
        }
        self.lookup(reg, full_id)
            .map(|e| e.value)
            .filter(|value| !value.is_null())
    }

    /// `DeferredHolder.value/get`: absence is observable as a Java exception,
    /// never as a fabricated value. NeoForge's `bind(true)` distinguishes a
    /// missing registry from an existing registry with an unbound entry.
    pub fn require_holder_value(&self, rt: &mut Rt, holder_id: u64) -> V {
        if let Some(value) = self.holder_value(holder_id) {
            return value;
        }
        let Some((registry, location, _)) = self.holders.get(&holder_id) else {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                "DeferredHolder has no resource key",
            );
        };
        let key = format!("ResourceKey[{registry} / {location}]");
        if !self.registry_is_present(registry) {
            return rt.throw_exception(
                "java/lang/IllegalStateException",
                &format!("Registry not present for DeferredHolder{{{key}}}: {registry}"),
            );
        }
        rt.throw_exception(
            "java/lang/NullPointerException",
            &format!("Trying to access unbound value: {key}"),
        )
    }

    fn registry_is_present(&self, registry: &str) -> bool {
        if self.entries.contains_key(registry)
            || self.custom_registries.iter().any(|key| key == registry)
        {
            return true;
        }
        // Built-in registries exist even when none of their values have been
        // materialized in this host. Reuse the explicit known field mapping.
        let field = match registry {
            "minecraft:root" => "REGISTRY".to_owned(),
            "minecraft:trigger_type" => "TRIGGER_TYPES".to_owned(),
            "minecraft:worldgen/block_state_provider_type" => "BLOCKSTATE_PROVIDER_TYPE".to_owned(),
            "neoforge:fluid_type" => "FLUID_TYPES".to_owned(),
            "neoforge:ingredient_serializer" => "INGREDIENT_TYPES".to_owned(),
            "neoforge:fluid_ingredient_type" => "FLUID_INGREDIENT_TYPES".to_owned(),
            "neoforge:holder_set_type" => "HOLDER_SET_TYPES".to_owned(),
            _ => registry
                .rsplit([':', '/'])
                .next()
                .unwrap_or("")
                .to_ascii_uppercase(),
        };
        [
            "net/minecraft/core/registries/BuiltInRegistries",
            "net/neoforged/neoforge/registries/NeoForgeRegistries",
        ]
        .iter()
        .any(|owner| crate::minecraft::lookup_builtin_registry_key(owner, &field) == Some(registry))
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
    fn deferred_aliases_activate_at_registration_and_resolve_real_values() {
        let mut rt = Rt::new();
        let mut store = RegistryStore::new();
        let id = store.create_deferred_register("data_component_type", "example");
        let value = rt.new_instance("example/Component", 0);
        store.register_direct(
            &mut rt,
            "data_component_type",
            "example:chemicals",
            "example",
            value,
            ContentProperties::default(),
        );
        store
            .add_deferred_alias(id, "example:gases", "example:wrong")
            .unwrap();
        store
            .add_deferred_alias(id, "example:gases", "example:chemicals")
            .unwrap();
        assert!(store
            .lookup("data_component_type", "example:gases")
            .is_none());
        store.apply_deferred_aliases("data_component_type").unwrap();
        assert_eq!(
            store
                .lookup("data_component_type", "example:gases")
                .unwrap()
                .value,
            value
        );
        assert!(!store.contains_entry("data_component_type", "example:gases"));
        store.begin_deferred_registration("data_component_type");
        assert!(store
            .add_deferred_alias(id, "example:new", "example:chemicals")
            .is_err());
        assert!(store
            .add_alias("data_component_type", "example:gases", "example:elsewhere")
            .is_err());
        store
            .add_alias("data_component_type", "example:legacy", "example:gases")
            .unwrap();
        assert_eq!(
            store
                .lookup("data_component_type", "example:legacy")
                .unwrap()
                .value,
            value
        );
    }

    #[test]
    fn alias_cycles_are_rejected_and_direct_entries_shadow_aliases() {
        let mut rt = Rt::new();
        let mut store = RegistryStore::new();
        store.add_alias("item", "example:a", "example:b").unwrap();
        store.add_alias("item", "example:b", "example:c").unwrap();
        assert!(store.add_alias("item", "example:c", "example:a").is_err());
        assert!(store.lookup("item", "example:a").is_none());
        store
            .add_alias("item", "example:self", "example:self")
            .unwrap();
        assert!(store.lookup("item", "example:self").is_none());
        let direct = rt.new_instance("example/Item", 0);
        store.register_direct(
            &mut rt,
            "item",
            "example:a",
            "example",
            direct,
            ContentProperties::default(),
        );
        assert_eq!(store.lookup("item", "example:a").unwrap().value, direct);
    }

    #[test]
    fn standalone_holder_throws_until_actual_value_is_registered() {
        let mut rt = Rt::new();
        let mut store = RegistryStore::new();
        let holder = store.bind_holder(
            &mut rt,
            "SOUND_EVENT",
            "example:bell",
            "net/neoforged/neoforge/registries/DeferredHolder",
        );
        let id = rt.handle_of(holder).unwrap().1;
        assert!(store.pending_suppliers().is_empty());
        assert!(store.lookup("SOUND_EVENT", "example:bell").is_none());
        assert!(store.holder_value(id).is_none());
        store.require_holder_value(&mut rt, id);
        let exception = rt.take_exception().expect("unbound access must throw");
        assert_eq!(
            rt.class_of(exception),
            Some("java/lang/NullPointerException")
        );
        assert!(rt.exception_summary(exception).contains("example:bell"));

        let sound = rt.new_instance("net/minecraft/sounds/SoundEvent", 0);
        store.register_direct(
            &mut rt,
            "SOUND_EVENT",
            "example:bell",
            "example",
            sound,
            ContentProperties::default(),
        );
        assert_eq!(store.require_holder_value(&mut rt, id), sound);
        assert!(rt.pending_exception.is_none());
        let later = store.bind_holder(
            &mut rt,
            "SOUND_EVENT",
            "example:bell",
            "net/neoforged/neoforge/registries/DeferredHolder",
        );
        let later_id = rt.handle_of(later).unwrap().1;
        assert_ne!(id, later_id);
        assert_eq!(store.require_holder_value(&mut rt, later_id), sound);
    }

    #[test]
    fn missing_registry_and_null_seed_are_not_bound_values() {
        let mut rt = Rt::new();
        let mut store = RegistryStore::new();
        let missing = store.bind_holder(
            &mut rt,
            "example:custom",
            "example:value",
            "net/neoforged/neoforge/registries/DeferredHolder",
        );
        let missing_id = rt.handle_of(missing).unwrap().1;
        store.require_holder_value(&mut rt, missing_id);
        let exception = rt.take_exception().expect("missing registry must throw");
        assert_eq!(
            rt.class_of(exception),
            Some("java/lang/IllegalStateException")
        );
        store.custom_registries.push("example:custom".into());
        store.require_holder_value(&mut rt, missing_id);
        let exception = rt
            .take_exception()
            .expect("existing registry has no value yet");
        assert_eq!(
            rt.class_of(exception),
            Some("java/lang/NullPointerException")
        );

        let seed = store.bind_holder(
            &mut rt,
            "ITEM",
            "minecraft:stone",
            "net/neoforged/neoforge/registries/DeferredHolder",
        );
        let seed_id = rt.handle_of(seed).unwrap().1;
        assert!(store.holder_value(seed_id).is_none());
        store.require_holder_value(&mut rt, seed_id);
        let exception = rt.take_exception().expect("unmaterialized seed must throw");
        assert_eq!(
            rt.class_of(exception),
            Some("java/lang/NullPointerException")
        );
    }

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
    fn deferred_register_closes_before_factory_and_after_registry_pass() {
        let mut store = RegistryStore::new();
        let mut rt = Rt::new();
        let items = store.create_deferred_register("item", "test");
        let other = store.create_deferred_register("block_entity_type", "test");
        store.begin_deferred_registration("item");
        assert!(store.deferred_registration_closed(items));
        assert!(!store.deferred_registration_closed(other));
        assert!(!store.frozen);
        store.enqueue_deferred_entry(
            &mut rt,
            items,
            "net/neoforged/neoforge/registries/DeferredItem",
            "reentrant",
            V::NULL,
        );
        let error = rt.take_exception().unwrap();
        assert!(rt
            .exception_summary(error)
            .contains("IllegalStateException"));
        assert!(store.deferred_registers[&items].entries.is_empty());
        store.finish_registry_event("item");
        let new_items = store.create_deferred_register("item", "later");
        assert!(store.deferred_registration_closed(new_items));
        store.enqueue_deferred_entry(
            &mut rt,
            new_items,
            "net/neoforged/neoforge/registries/DeferredItem",
            "late",
            V::NULL,
        );
        let error = rt.take_exception().unwrap();
        assert!(rt
            .exception_summary(error)
            .contains("IllegalStateException"));
        assert!(store.deferred_registers[&new_items].entries.is_empty());
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

//! `mmga-neoforge`: NeoForge mod host implemented beside the Rust VM.
//!
//! `EventBusEngine`, `RegistryStore`, config/capability/attachment/network/command
//! subsystems, and `WorldState` are native Rust. [`loader::FmlLoader`] implements
//! `HostApi` and drives the FML lifecycle for jars linked into the VM.

pub mod armor;
pub mod bus;
pub mod codecs;
pub mod components;
pub mod loader;
pub mod minecraft;
pub mod registries;
pub mod sounds;
pub mod subsystems;
pub mod world;

pub use bus::{
    dispatch_event_on_bus, find_builtin_event, is_mod_bus_event_class, BuiltinEventSpec, BusId,
    EventBusEngine, EventCallback, EventListener, EventPriority, BUILTIN_EVENTS,
};
pub use loader::{FmlLoader, LoadPhase, LoadedMod, PreparedLoad};
pub use registries::{
    canonical_registry_key, ContentProperties, DeferredRegisterRecord, DeferredSupplier,
    PendingDeferredItem, RegisteredEntry, RegistryStore, REGISTRATION_ORDER,
};
pub use subsystems::{
    ArgumentKind, AttachmentStore, AttachmentTypeDef, BrigadierNode, BrigadierTree,
    BuiltConfigSpec, CapabilityRegistration, CapabilityStore, CommandNodeKind, ConfigBuilderState,
    ConfigEntrySpec, ConfigPrimitive, ConfigSubsystem, NetworkStore, RegisteredPayloadChannel,
};
pub use world::{BlockCoord, SimEntity, WorldState};

/// Aggregate NeoForge host state. The FML loader drives this; it does not own the VM heap.
#[derive(Debug, Clone)]
pub struct NeoForgeState {
    pub bus: EventBusEngine,
    pub registries: RegistryStore,
    pub config: ConfigSubsystem,
    pub capabilities: CapabilityStore,
    pub attachments: AttachmentStore,
    pub network: NetworkStore,
    pub commands: BrigadierTree,
    pub world: WorldState,
    pub milk_fluid_enabled: bool,
}

impl Default for NeoForgeState {
    fn default() -> Self {
        Self::new()
    }
}

impl NeoForgeState {
    pub fn new() -> Self {
        Self {
            bus: EventBusEngine::new(),
            registries: RegistryStore::new(),
            config: ConfigSubsystem::default(),
            capabilities: CapabilityStore::default(),
            attachments: AttachmentStore::default(),
            network: NetworkStore::default(),
            commands: BrigadierTree::default(),
            world: WorldState::new(),
            milk_fluid_enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{ExecCtx, LinkedSpace, NullHost, Rt, V};

    fn hit(rt: &mut Rt, event: V) {
        let n = rt.get_field(event, "hits").i();
        rt.put_field(event, "hits", V::I(n + 1));
    }

    #[test]
    fn wired_state_posts_a_game_bus_event_against_seeded_registries() {
        let mut state = NeoForgeState::new();
        assert!(state
            .registries
            .lookup("block", "minecraft:stone")
            .is_some());
        state
            .bus
            .register_listener(
                BusId::Game,
                "net/neoforged/neoforge/event/tick/ServerTickEvent$Post".into(),
                EventPriority::Normal,
                false,
                V::NULL,
                EventCallback::Native(hit),
            )
            .unwrap();

        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let event = rt.new_instance("net/neoforged/neoforge/event/tick/ServerTickEvent$Post", 0);
        rt.put_field(event, "$canceled", V::I(0));
        rt.put_field(event, "hits", V::I(0));
        let canceled = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            state.bus.post(
                &mut ctx,
                &BusId::Game,
                "net/neoforged/neoforge/event/tick/ServerTickEvent$Post",
                event,
            )
        };
        assert!(!canceled);
        assert_eq!(rt.get_field(event, "hits").i(), 1);
        assert_eq!(state.bus.callbacks_invoked, 1);
        assert!(BUILTIN_EVENTS
            .iter()
            .any(|spec| spec.class.ends_with("FMLCommonSetupEvent") && spec.is_mod_bus));
    }
}

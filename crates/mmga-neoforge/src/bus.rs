//! NeoForge Event Class Table, Priority Ordering, Mod Bus vs Game Bus Isolation,
//! `ICancellableEvent`, and High-Speed Rust `EventBusEngine` (`net.neoforged.bus`).

use mmga_runtime::{ExecCtx, Rt, V};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// NeoForge `EventPriority` ordinals (`HIGHEST = 0` .. `LOWEST = 4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum EventPriority {
    Highest = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Lowest = 4,
}

impl EventPriority {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "HIGHEST" => Self::Highest,
            "HIGH" => Self::High,
            "LOW" => Self::Low,
            "LOWEST" => Self::Lowest,
            _ => Self::Normal,
        }
    }
}

/// Bus kind enforcing NeoForge `IModBusEvent` separation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BusId {
    /// `NeoForge.EVENT_BUS`: rejects `IModBusEvent` subclasses.
    Game,
    /// Per-mod `FMLModContainer#getEventBus()`: accepts ONLY `IModBusEvent` subclasses.
    Mod(String),
}

/// Static facts about NeoForge's built-in event classes.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinEventSpec {
    pub class: &'static str,
    pub superclass: &'static str,
    pub is_mod_bus: bool,
    pub is_cancellable: bool,
    pub is_abstract: bool,
}

/// Canonical NeoForge 21.1 / 26.3 event hierarchy table.
pub const BUILTIN_EVENTS: &[BuiltinEventSpec] = &[
    BuiltinEventSpec {
        class: "net/neoforged/bus/api/Event",
        superclass: "java/lang/Object",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    // Mod-bus lifecycle & registration events (`IModBusEvent`)
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/ModLifecycleEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ModLifecycleEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/FMLClientSetupEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/FMLDedicatedServerSetupEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/InterModEnqueueEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/InterModProcessEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/lifecycle/FMLLoadCompleteEvent",
        superclass: "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/registries/RegisterEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/registries/NewRegistryEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    // Despite its registries package this is a game-bus Event. NeoForge fires
    // it after a server data-map reload or client sync; it is not IModBusEvent.
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/registries/datamaps/DataMapsUpdatedEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    // Official source explicitly implements IModBusEvent despite living under
    // common.world.chunk rather than the registries or lifecycle packages.
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/common/world/chunk/RegisterTicketControllersEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/capabilities/RegisterCapabilitiesEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/network/event/RegisterPayloadHandlersEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/BuildCreativeModeTabContentsEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/config/ModConfigEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: true,
        is_cancellable: false,
        // FancyModLoader 1.21.1 declares a concrete class with a package-private
        // constructor. Base listeners are valid and receive its nested events.
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/config/ModConfigEvent$Loading",
        superclass: "net/neoforged/fml/event/config/ModConfigEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/config/ModConfigEvent$Reloading",
        superclass: "net/neoforged/fml/event/config/ModConfigEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/fml/event/config/ModConfigEvent$Unloading",
        superclass: "net/neoforged/fml/event/config/ModConfigEvent",
        is_mod_bus: true,
        is_cancellable: false,
        is_abstract: false,
    },
    // Game-bus server, tick, command, world, block, entity, and player events
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/server/ServerAboutToStartEvent",
        superclass: "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/server/ServerStartingEvent",
        superclass: "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/server/ServerStartedEvent",
        superclass: "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/server/ServerStoppingEvent",
        superclass: "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/server/ServerStoppedEvent",
        superclass: "net/neoforged/neoforge/event/server/ServerLifecycleEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/RegisterCommandsEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/tick/ServerTickEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/tick/ServerTickEvent$Pre",
        superclass: "net/neoforged/neoforge/event/tick/ServerTickEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/tick/ServerTickEvent$Post",
        superclass: "net/neoforged/neoforge/event/tick/ServerTickEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/tick/LevelTickEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/tick/LevelTickEvent$Pre",
        superclass: "net/neoforged/neoforge/event/tick/LevelTickEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/tick/LevelTickEvent$Post",
        superclass: "net/neoforged/neoforge/event/tick/LevelTickEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/level/BlockEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent",
        superclass: "net/neoforged/neoforge/event/level/BlockEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/level/BlockEvent$EntityPlaceEvent",
        superclass: "net/neoforged/neoforge/event/level/BlockEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/EntityEvent",
        superclass: "net/neoforged/bus/api/Event",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/EntityJoinLevelEvent",
        superclass: "net/neoforged/neoforge/event/entity/EntityEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/living/LivingEvent",
        superclass: "net/neoforged/neoforge/event/entity/EntityEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/living/LivingIncomingDamageEvent",
        superclass: "net/neoforged/neoforge/event/entity/living/LivingEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/living/LivingDeathEvent",
        superclass: "net/neoforged/neoforge/event/entity/living/LivingEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerEvent",
        superclass: "net/neoforged/neoforge/event/entity/living/LivingEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerEvent$PlayerLoggedInEvent",
        superclass: "net/neoforged/neoforge/event/entity/player/PlayerEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerEvent$PlayerLoggedOutEvent",
        superclass: "net/neoforged/neoforge/event/entity/player/PlayerEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent",
        superclass: "net/neoforged/neoforge/event/entity/player/PlayerEvent",
        is_mod_bus: false,
        is_cancellable: false,
        is_abstract: true,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent$RightClickBlock",
        superclass: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent$RightClickItem",
        superclass: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
    BuiltinEventSpec {
        class: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent$LeftClickBlock",
        superclass: "net/neoforged/neoforge/event/entity/player/PlayerInteractEvent",
        is_mod_bus: false,
        is_cancellable: true,
        is_abstract: false,
    },
];

pub fn find_builtin_event(class_name: &str) -> Option<&'static BuiltinEventSpec> {
    BUILTIN_EVENTS.iter().find(|e| e.class == class_name)
}

pub fn is_mod_bus_event_class(class_name: &str) -> bool {
    if let Some(spec) = find_builtin_event(class_name) {
        return spec.is_mod_bus;
    }
    if class_name == "net/neoforged/neoforge/event/RegisterCommandsEvent"
        || class_name == "net/neoforged/neoforge/client/event/RegisterClientCommandsEvent"
    {
        return false;
    }
    class_name.starts_with("net/neoforged/fml/event/")
        || class_name.starts_with("net/neoforged/neoforge/registries/")
        || class_name.starts_with("net/neoforged/neoforge/data/event/")
        || class_name.starts_with("net/neoforged/neoforge/client/event/Register")
        || class_name.starts_with("net/neoforged/neoforge/client/event/EntityRenderersEvent$")
        || class_name.starts_with("net/neoforged/neoforge/client/event/ModelEvent$")
        || class_name.starts_with(
            "net/neoforged/neoforge/client/extensions/common/RegisterClientExtensionsEvent",
        )
        || matches!(
            class_name,
            "net/neoforged/neoforge/capabilities/RegisterCapabilitiesEvent"
                | "net/neoforged/neoforge/network/event/RegisterPayloadHandlersEvent"
                | "net/neoforged/neoforge/event/BuildCreativeModeTabContentsEvent"
                | "net/neoforged/neoforge/event/ModifyDefaultComponentsEvent"
                | "net/neoforged/neoforge/event/AddPackFindersEvent"
                | "net/neoforged/neoforge/event/entity/EntityAttributeCreationEvent"
                | "net/neoforged/neoforge/event/entity/EntityAttributeModificationEvent"
                | "net/neoforged/neoforge/event/entity/RegisterSpawnPlacementsEvent"
        )
}

/// Target callback of a registered event listener.
#[derive(Debug, Clone)]
pub enum EventCallback {
    /// Java `Consumer<Event>` closure or user object on the heap.
    Consumer(V),
    /// `@SubscribeEvent` static method `(Event)V`.
    StaticMethod {
        class_name: String,
        method_name: String,
        method_desc: String,
    },
    /// `@SubscribeEvent` instance method `(Event)V` on `instance`.
    InstanceMethod {
        instance: V,
        class_name: String,
        method_name: String,
        method_desc: String,
    },
    /// Native listener. Mods use the Java variants; the host and tests use this
    /// to observe or mutate the event object without a class file.
    Native(fn(&mut Rt, V)),
}

/// Registered NeoForge event listener entry.
#[derive(Debug, Clone)]
pub struct EventListener {
    pub id: u64,
    pub bus: BusId,
    pub event_class: String,
    pub priority: EventPriority,
    pub receive_canceled: bool,
    /// Owner token used by `IEventBus.unregister(target)`.
    pub owner_token: V,
    pub callback: EventCallback,
}

/// High-speed Rust implementation of `net.neoforged.bus` supporting priority queues,
/// cancellation propagation, Mod-vs-Game bus validation, and deferred work queues.
#[derive(Debug, Default, Clone)]
pub struct EventBusEngine {
    next_id: u64,
    listeners: Vec<EventListener>,
    /// Deferred tasks queued via `ParallelDispatchEvent#enqueueWork(Runnable|Supplier)`.
    pub deferred_work: Vec<V>,
    pub events_posted: u64,
    pub callbacks_invoked: u64,
}

impl EventBusEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }

    pub fn listeners_by_event(&self) -> BTreeMap<String, usize> {
        let mut map = BTreeMap::new();
        for l in &self.listeners {
            *map.entry(l.event_class.clone()).or_insert(0) += 1;
        }
        map
    }

    /// Validates bus/event compatibility per NeoForge `EventBus` rules and registers the listener.
    pub fn register_listener(
        &mut self,
        bus: BusId,
        event_class: String,
        priority: EventPriority,
        receive_canceled: bool,
        owner_token: V,
        callback: EventCallback,
    ) -> Result<u64, String> {
        if let Some(spec) = find_builtin_event(&event_class) {
            if spec.is_abstract {
                return Err(format!(
                    "Cannot register listeners for abstract class {}. Register a listener to one of its subclasses instead!",
                    event_class.replace('/', ".")
                ));
            }
        }
        let mod_event = is_mod_bus_event_class(&event_class);
        match &bus {
            BusId::Game if mod_event => {
                return Err(format!(
                    "IModBusEvent {} is not allowed on the common NeoForge.EVENT_BUS! Use the mod event bus instead.",
                    event_class.replace('/', ".")
                ));
            }
            BusId::Mod(_) if !mod_event => {
                return Err(format!(
                    "Mod event bus only accepts subclasses of IModBusEvent, which class {} is not.",
                    event_class.replace('/', ".")
                ));
            }
            _ => {}
        }

        let id = self.next_id;
        self.next_id += 1;
        self.listeners.push(EventListener {
            id,
            bus,
            event_class,
            priority,
            receive_canceled,
            owner_token,
            callback,
        });
        Ok(id)
    }

    pub fn unregister(&mut self, bus: &BusId, target: V) {
        self.listeners
            .retain(|l| &l.bus != bus || l.owner_token != target);
    }

    /// Collects matching listeners for `(bus, event_class)` sorted by `(priority, registration_order)`.
    pub fn matching_listeners(&self, bus: &BusId, event_class: &str) -> Vec<EventListener> {
        let mut matched: Vec<EventListener> = self
            .listeners
            .iter()
            .filter(|l| &l.bus == bus && event_matches(&l.event_class, event_class))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
        matched
    }

    /// Records one completed `post` (`events_posted` / `callbacks_invoked`).
    pub fn record_dispatch(&mut self, invoked: usize) {
        self.events_posted = self.events_posted.saturating_add(1);
        self.callbacks_invoked = self.callbacks_invoked.saturating_add(invoked as u64);
    }

    /// Posts `event_obj` on `bus`. The engine must not be borrowed by `ctx.host`.
    /// Returns whether a listener set `$canceled`.
    pub fn post(
        &mut self,
        ctx: &mut ExecCtx<'_>,
        bus: &BusId,
        event_class: &str,
        event_obj: V,
    ) -> bool {
        let listeners = self.matching_listeners(bus, event_class);
        let (ran, canceled) = dispatch_event_on_bus(ctx, &listeners, event_obj);
        self.record_dispatch(ran);
        canceled
    }
}

fn event_matches(listener_event: &str, fired_event: &str) -> bool {
    if listener_event == fired_event {
        return true;
    }
    let mut cur = fired_event;
    while let Some(spec) = find_builtin_event(cur) {
        if spec.superclass == listener_event {
            return true;
        }
        if spec.superclass == "java/lang/Object" {
            break;
        }
        cur = spec.superclass;
    }
    false
}

/// Dispatches `event_obj` (`$canceled` field = `0`/`1`) across `listeners`.
/// Returns `(callbacks_invoked, canceled)`.
pub fn dispatch_event_on_bus(
    ctx: &mut ExecCtx<'_>,
    listeners: &[EventListener],
    event_obj: V,
) -> (usize, bool) {
    let mut ran = 0usize;
    for listener in listeners {
        let is_canceled = ctx.rt.get_field(event_obj, "$canceled").i() != 0;
        if is_canceled && !listener.receive_canceled {
            continue;
        }
        ran += 1;
        match &listener.callback {
            EventCallback::Consumer(consumer_v) => {
                let _ = mmga_runtime::invoke_closure(ctx, *consumer_v, &[event_obj]);
            }
            EventCallback::StaticMethod {
                class_name,
                method_name,
                method_desc,
            } => {
                let mref = mmga_classfile::MemberRef::new(class_name, method_name, method_desc);
                let _ = mmga_runtime::invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKESTATIC,
                    &mref,
                    &[event_obj],
                );
            }
            EventCallback::InstanceMethod {
                instance,
                class_name,
                method_name,
                method_desc,
            } => {
                let mref = mmga_classfile::MemberRef::new(class_name, method_name, method_desc);
                let _ = mmga_runtime::invoke_member(
                    ctx,
                    mmga_classfile::op::INVOKEVIRTUAL,
                    &mref,
                    &[*instance, event_obj],
                );
            }
            EventCallback::Native(func) => func(ctx.rt, event_obj),
        }
        if ctx.rt.pending_exception.is_some() {
            break;
        }
    }
    let final_canceled = ctx.rt.get_field(event_obj, "$canceled").i() != 0;
    (ran, final_canceled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmga_runtime::{ExecCtx, LinkedSpace, NullHost, Rt};

    fn hit(rt: &mut Rt, event: V) {
        let n = rt.get_field(event, "hits").i();
        rt.put_field(event, "hits", V::I(n + 1));
    }

    fn cancel(rt: &mut Rt, event: V) {
        rt.put_field(event, "$canceled", V::I(1));
        hit(rt, event);
    }

    fn fresh_event(rt: &mut Rt, class_name: &str) -> V {
        let event = rt.new_instance(class_name, 0);
        rt.put_field(event, "$canceled", V::I(0));
        rt.put_field(event, "hits", V::I(0));
        event
    }

    #[test]
    fn rejects_abstract_listeners_and_cross_bus_events() {
        let mut engine = EventBusEngine::new();
        let abstract_err = engine
            .register_listener(
                BusId::Game,
                "net/neoforged/neoforge/event/level/BlockEvent".into(),
                EventPriority::Normal,
                false,
                V::NULL,
                EventCallback::Native(hit),
            )
            .unwrap_err();
        assert!(abstract_err.contains("abstract"));

        let mod_on_game = engine
            .register_listener(
                BusId::Game,
                "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent".into(),
                EventPriority::Normal,
                false,
                V::NULL,
                EventCallback::Native(hit),
            )
            .unwrap_err();
        assert!(mod_on_game.contains("IModBusEvent"));

        let game_on_mod = engine
            .register_listener(
                BusId::Mod("examplemod".into()),
                "net/neoforged/neoforge/event/tick/ServerTickEvent$Post".into(),
                EventPriority::Normal,
                false,
                V::NULL,
                EventCallback::Native(hit),
            )
            .unwrap_err();
        assert!(game_on_mod.contains("IModBusEvent"));
        assert_eq!(engine.listener_count(), 0);
    }

    #[test]
    fn mekanism_data_maps_listener_belongs_to_the_game_bus() {
        let event_class = "net/neoforged/neoforge/registries/datamaps/DataMapsUpdatedEvent";
        let mut engine = EventBusEngine::new();
        let game = engine.register_listener(
            BusId::Game,
            event_class.into(),
            EventPriority::Normal,
            false,
            V::NULL,
            EventCallback::Native(hit),
        );
        assert!(
            game.is_ok(),
            "DataMapsUpdatedEvent extends Event, not IModBusEvent: {game:?}"
        );
        let wrong_bus = engine.register_listener(
            BusId::Mod("mekanism".into()),
            event_class.into(),
            EventPriority::Normal,
            false,
            V::NULL,
            EventCallback::Native(hit),
        );
        assert!(wrong_bus.unwrap_err().contains("IModBusEvent"));
        assert!(!is_mod_bus_event_class(event_class));
        assert_eq!(engine.listener_count(), 1);

        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let event = rt.new_instance(event_class, 0);
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        assert!(!engine.post(&mut ctx, &BusId::Game, event_class, event));
        assert_eq!(ctx.rt.get_field(event, "hits"), V::I(1));
        assert_eq!(engine.callbacks_invoked, 1);
    }

    #[test]
    fn ticket_controller_registration_is_a_mod_bus_event() {
        let event_class =
            "net/neoforged/neoforge/common/world/chunk/RegisterTicketControllersEvent";
        let mut engine = EventBusEngine::new();
        let accepted = engine.register_listener(
            BusId::Mod("mekanism".into()),
            event_class.into(),
            EventPriority::Normal,
            false,
            V::NULL,
            EventCallback::Native(hit),
        );
        assert!(
            accepted.is_ok(),
            "RegisterTicketControllersEvent implements IModBusEvent: {accepted:?}"
        );
        let rejected = engine.register_listener(
            BusId::Game,
            event_class.into(),
            EventPriority::Normal,
            false,
            V::NULL,
            EventCallback::Native(hit),
        );
        assert!(rejected.unwrap_err().contains("IModBusEvent"));
        assert!(is_mod_bus_event_class(event_class));
    }

    #[test]
    fn concrete_mod_config_base_listener_receives_config_subclasses() {
        let base = "net/neoforged/fml/event/config/ModConfigEvent";
        let mut engine = EventBusEngine::new();
        let bus = BusId::Mod("mekanism".into());
        let registered = engine.register_listener(
            bus.clone(),
            base.into(),
            EventPriority::Normal,
            false,
            V::NULL,
            EventCallback::Native(hit),
        );
        assert!(
            registered.is_ok(),
            "ModConfigEvent is concrete in FancyModLoader1.21.1: {registered:?}"
        );
        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &linked,
            host: &mut host,
            depth: 0,
        };
        for class in [
            "net/neoforged/fml/event/config/ModConfigEvent$Loading",
            "net/neoforged/fml/event/config/ModConfigEvent$Reloading",
            "net/neoforged/fml/event/config/ModConfigEvent$Unloading",
        ] {
            let event = ctx.rt.new_instance(class, 0);
            engine.post(&mut ctx, &bus, class, event);
            assert_eq!(ctx.rt.get_field(event, "hits"), V::I(1));
        }
        assert_eq!(engine.callbacks_invoked, 3);
    }

    #[test]
    fn superclass_listener_matches_only_its_descendants() {
        assert!(event_matches(
            "net/neoforged/neoforge/event/level/BlockEvent",
            "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent",
        ));
        assert!(event_matches(
            "net/neoforged/bus/api/Event",
            "net/neoforged/neoforge/event/server/ServerStartedEvent",
        ));
        assert!(!event_matches(
            "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent",
            "net/neoforged/neoforge/event/level/BlockEvent$EntityPlaceEvent",
        ));
        assert!(!event_matches(
            "net/neoforged/neoforge/event/tick/ServerTickEvent$Pre",
            "net/neoforged/neoforge/event/tick/ServerTickEvent$Post",
        ));
    }

    #[test]
    fn dispatches_by_priority_and_skips_listeners_after_cancel() {
        let mut engine = EventBusEngine::new();
        let owner = V::I(7);
        engine
            .register_listener(
                BusId::Game,
                "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent".into(),
                EventPriority::Low,
                true,
                owner,
                EventCallback::Native(hit),
            )
            .unwrap();
        engine
            .register_listener(
                BusId::Game,
                "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent".into(),
                EventPriority::Highest,
                false,
                owner,
                EventCallback::Native(cancel),
            )
            .unwrap();
        engine
            .register_listener(
                BusId::Game,
                "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent".into(),
                EventPriority::Normal,
                false,
                owner,
                EventCallback::Native(hit),
            )
            .unwrap();
        engine
            .register_listener(
                BusId::Mod("examplemod".into()),
                "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent".into(),
                EventPriority::Normal,
                false,
                V::I(1),
                EventCallback::Native(hit),
            )
            .unwrap();

        let mut rt = Rt::new();
        let linked = LinkedSpace::new();
        let mut host = NullHost;
        let break_event = fresh_event(
            &mut rt,
            "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent",
        );
        let canceled = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            engine.post(
                &mut ctx,
                &BusId::Game,
                "net/neoforged/neoforge/event/level/BlockEvent$BreakEvent",
                break_event,
            )
        };
        assert!(canceled);
        // Highest cancels, Normal is skipped, Low still runs (`receiveCanceled`).
        assert_eq!(rt.get_field(break_event, "hits").i(), 2);
        assert_eq!(engine.events_posted, 1);
        assert_eq!(engine.callbacks_invoked, 2);

        let setup = fresh_event(
            &mut rt,
            "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent",
        );
        let setup_canceled = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked: &linked,
                host: &mut host,
                depth: 0,
            };
            engine.post(
                &mut ctx,
                &BusId::Game,
                "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent",
                setup,
            )
        };
        assert!(!setup_canceled);
        assert_eq!(rt.get_field(setup, "hits").i(), 0);

        engine.unregister(&BusId::Game, owner);
        assert_eq!(engine.listener_count(), 1);
    }
}

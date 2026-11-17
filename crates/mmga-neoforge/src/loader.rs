//! FML-style mod load: discover jars, order dependencies, construct `@Mod` classes,
//! bind `DeferredRegister`s, and post the NeoForge lifecycle on the mod and game buses.
//!
//! Bytecode calls into NeoForge and Minecraft land in [`FmlLoader`]'s `HostApi`.
//! The host covers the registration and lifecycle surface a typical Java mod touches.
//! Chunk streaming, `BlockState` palettes, and the real packet codec are still absent;
//! `ServerLevel.setBlock(int, int, int, String)` is the stand-in for `Level#setBlock`.

use crate::{
    dispatch_event_on_bus, is_mod_bus_event_class, BusId, ContentProperties, DeferredSupplier,
    EventCallback, EventPriority, NeoForgeState, REGISTRATION_ORDER,
};
use mmga_classfile::{
    op, parse_method_descriptor, resolve_mod_load_order, version_in_range, ClassFile,
    DescriptorType, DiscoveredModEntrypoint, DiscoveredSubscriber, MemberRef, ModEntry, ModJar,
    ModLoaderKind, ModsToml, SubscriberBusTarget,
};
use mmga_runtime::{
    alloc_new_object, box_primitive, invoke_closure, invoke_member, Data, ExecCtx, HostApi,
    LinkedSpace, Rt, V,
};
use std::collections::{BTreeMap, BTreeSet};

const REGISTER_EVENT: &str = "net/neoforged/neoforge/registries/RegisterEvent";

/// Minecraft and NeoForge versions this host claims when checking `versionRange`.
///
/// Chosen from published compatibility, not a guess: YSM 2.6.5 accepts NeoForge
/// `[21.0,21.2)` and Minecraft `[1.21,1.21.2)`; Create 6.0.10 requires NeoForge
/// `[21.1.219,)` and Minecraft `[1.21.1]`; Mekanism 10.7.19 requires NeoForge
/// `[21.1.194,)` and Minecraft `[1.21.1]`. `21.1.266` is the newest stable
/// NeoForge 21.1 release on maven.neoforged.net that sits in all three ranges.
pub const TARGET_MINECRAFT: &str = "1.21.1";
pub const TARGET_NEOFORGE: &str = "21.1.266";

/// Where a loaded mod set sits in the FML state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPhase {
    Discovered,
    Constructed,
    Registered,
    CommonSetup,
    SidedSetup,
    LoadComplete,
    ServerStarted,
    Running,
    Stopped,
}

impl LoadPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Constructed => "constructed",
            Self::Registered => "registered",
            Self::CommonSetup => "common-setup",
            Self::SidedSetup => "sided-setup",
            Self::LoadComplete => "load-complete",
            Self::ServerStarted => "server-started",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}

/// One mod accepted onto the load list.
#[derive(Debug, Clone)]
pub struct LoadedMod {
    pub mod_id: String,
    pub version: String,
    pub display_name: String,
    pub source_name: String,
    pub entry_class: Option<String>,
    pub instance: Option<V>,
    /// Side-matching `@Mod` classes, each with its constructor descriptors.
    entrypoints: Vec<(String, Vec<String>)>,
    subscribers: Vec<DiscoveredSubscriber>,
}

/// Jars linked into a class space plus the host that will execute them.
pub struct PreparedLoad {
    pub linked: LinkedSpace,
    pub loader: FmlLoader,
}

impl std::fmt::Debug for PreparedLoad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedLoad")
            .field("phase", &self.loader.phase)
            .field("mods", &self.loader.mods)
            .finish_non_exhaustive()
    }
}

/// Recorded `ModLoadingIssue` reported through `ModLoader.addLoadingIssue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedLoadingIssue {
    pub severity: String,
    pub mod_id: Option<String>,
    pub translation_key: String,
    pub message: String,
}

/// Queued `InterModComms.IMCMessage` entry.
#[derive(Debug, Clone)]
pub struct ImcEntry {
    pub sender_mod_id: String,
    pub mod_id: String,
    pub method: String,
    pub supplier: V,
}

/// NeoForge `FML` / `ModLoader` host. Owns [`NeoForgeState`] and the bus handles mods see.
#[derive(Debug)]
pub struct FmlLoader {
    pub state: NeoForgeState,
    pub phase: LoadPhase,
    pub mods: Vec<LoadedMod>,
    pub dedicated_server: bool,
    /// Phase labels in the order `bootstrap` entered them.
    pub trace: Vec<&'static str>,
    game_bus: Option<V>,
    mod_buses: BTreeMap<String, V>,
    mod_containers: BTreeMap<String, V>,
    /// Mod id currently constructing or receiving a mod-bus event (`ModLoadingContext`).
    constructing: Option<String>,
    config_dir: std::path::PathBuf,
    /// `(mod id, config type)` recorded by `ModContainer.registerConfig`.
    pub registered_configs: Vec<(String, String)>,
    /// Issues submitted via `ModLoader.addLoadingIssue`.
    pub loading_issues: Vec<RecordedLoadingIssue>,
    /// Messages queued via `InterModComms.sendTo`.
    pub imc_messages: Vec<ImcEntry>,
    /// Deferred main-thread tasks queued via `ParallelDispatchEvent.enqueueWork`: `(mod_id, closure, future)`.
    queued_work: Vec<(Option<String>, V, V)>,
    /// Non-class resource bytes merged from loaded jars (`Class#getResource`).
    pub jar_resources: BTreeMap<String, Vec<u8>>,
    /// Language keys loaded from `assets/*/lang/en_us.json` in loaded jars.
    pub translations: BTreeMap<String, String>,
}

impl PreparedLoad {
    /// Parses mods, rejects missing / incompatible / cyclic dependencies, and links classes.
    pub fn from_jars(jars: Vec<ModJar>, dedicated_server: bool) -> Result<Self, String> {
        let mut located: Vec<(usize, ModsToml)> = Vec::new();
        for (idx, jar) in jars.iter().enumerate() {
            for manifest in manifests_in(jar)? {
                located.push((idx, manifest));
            }
        }
        let refs: Vec<&ModsToml> = located.iter().map(|(_, manifest)| manifest).collect();
        let order = resolve_mod_load_order(&refs, dedicated_server)?;
        check_version_ranges(&refs, dedicated_server)?;

        let mut jar_of: BTreeMap<String, usize> = BTreeMap::new();
        let mut manifest_of: BTreeMap<String, usize> = BTreeMap::new();
        for (loc_idx, (jar_idx, manifest)) in located.iter().enumerate() {
            for entry in &manifest.mods {
                if jar_of.insert(entry.mod_id.clone(), *jar_idx).is_some() {
                    return Err(format!("duplicate mod id `{}`", entry.mod_id));
                }
                manifest_of.insert(entry.mod_id.clone(), loc_idx);
            }
        }

        let mut classes: BTreeMap<String, ClassFile> = BTreeMap::new();
        let mut jar_resources: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut translations: BTreeMap<String, String> = BTreeMap::new();
        let mut services: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for jar in &jars {
            crate::minecraft::extract_lang_entries(&jar.resources, &mut translations);
            for (spi, impls) in &jar.services {
                let entry = services.entry(spi.clone()).or_default();
                for imp in impls {
                    if !entry.contains(imp) {
                        entry.push(imp.clone());
                    }
                }
            }
            for (rpath, rbytes) in &jar.resources {
                jar_resources
                    .entry(rpath.clone())
                    .or_insert_with(|| rbytes.clone());
            }
            for (name, cf) in &jar.classes {
                if classes.contains_key(name) {
                    return Err(format!(
                        "duplicate class `{name}` (also in `{}`)",
                        jar.source_name
                    ));
                }
                classes.insert(name.clone(), cf.clone());
            }
        }

        let mut mods = Vec::with_capacity(order.len());
        for mod_id in &order {
            let jar_idx = jar_of[mod_id];
            let jar = &jars[jar_idx];
            let manifest = &located[manifest_of[mod_id]].1;
            let entry = manifest
                .mods
                .iter()
                .find(|m| &m.mod_id == mod_id)
                .ok_or_else(|| format!("mod `{mod_id}` vanished during ordering"))?;
            let discovered = jar.discover_entrypoints();
            let for_mod: Vec<_> = discovered
                .iter()
                .filter(|ep| &ep.mod_id == mod_id)
                .cloned()
                .collect();
            let matching: Vec<_> = for_mod
                .iter()
                .filter(|ep| ep.dist_restriction.matches_server(dedicated_server))
                .cloned()
                .collect();
            if !for_mod.is_empty() && matching.is_empty() {
                continue;
            }
            let sole = manifest.mods.len() == 1;
            mods.push(loaded_mod(entry, jar, matching, sole));
        }

        let mut linked = LinkedSpace::new();
        linked.services = services;
        linked.add_bytecode_classes(classes);
        let mut loader = FmlLoader {
            state: NeoForgeState::new(),
            phase: LoadPhase::Discovered,
            mods,
            dedicated_server,
            trace: Vec::new(),
            game_bus: None,
            mod_buses: BTreeMap::new(),
            mod_containers: BTreeMap::new(),
            constructing: None,
            config_dir: std::env::temp_dir().join("mmga-config"),
            registered_configs: Vec::new(),
            loading_issues: Vec::new(),
            imc_messages: Vec::new(),
            queued_work: Vec::new(),
            jar_resources,
            translations,
        };
        loader.enter(LoadPhase::Discovered);
        Ok(Self { linked, loader })
    }

    pub fn bootstrap(&mut self, rt: &mut Rt) -> Result<(), String> {
        self.loader.bootstrap(rt, &self.linked)
    }
}

impl FmlLoader {
    fn enter(&mut self, phase: LoadPhase) {
        self.phase = phase;
        self.trace.push(phase.label());
    }

    /// Constructs mods and advances through server start.
    pub fn bootstrap(&mut self, rt: &mut Rt, linked: &LinkedSpace) -> Result<(), String> {
        let count = self.mods.len();
        for index in 0..count {
            self.construct_one(rt, linked, index)?;
            self.register_subscribers(index)?;
        }
        self.enter(LoadPhase::Constructed);

        self.run_registration(rt, linked)?;
        self.enter(LoadPhase::Registered);

        self.post_each_mod(
            rt,
            linked,
            "net/neoforged/neoforge/network/event/RegisterPayloadHandlersEvent",
        )?;
        self.post_each_mod(
            rt,
            linked,
            "net/neoforged/neoforge/capabilities/RegisterCapabilitiesEvent",
        )?;

        self.post_each_mod(
            rt,
            linked,
            "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent",
        )?;
        self.enter(LoadPhase::CommonSetup);

        let sided = if self.dedicated_server {
            "net/neoforged/fml/event/lifecycle/FMLDedicatedServerSetupEvent"
        } else {
            "net/neoforged/fml/event/lifecycle/FMLClientSetupEvent"
        };
        self.post_each_mod(rt, linked, sided)?;
        self.post_each_mod(
            rt,
            linked,
            "net/neoforged/fml/event/lifecycle/InterModEnqueueEvent",
        )?;
        self.post_each_mod(
            rt,
            linked,
            "net/neoforged/fml/event/lifecycle/InterModProcessEvent",
        )?;
        self.enter(LoadPhase::SidedSetup);

        self.post_each_mod(
            rt,
            linked,
            "net/neoforged/fml/event/lifecycle/FMLLoadCompleteEvent",
        )?;
        self.enter(LoadPhase::LoadComplete);

        self.post_game(
            rt,
            linked,
            "net/neoforged/neoforge/event/server/ServerAboutToStartEvent",
        )?;
        self.post_game(
            rt,
            linked,
            "net/neoforged/neoforge/event/server/ServerStartingEvent",
        )?;
        self.post_game(
            rt,
            linked,
            "net/neoforged/neoforge/event/RegisterCommandsEvent",
        )?;
        self.post_game(
            rt,
            linked,
            "net/neoforged/neoforge/event/server/ServerStartedEvent",
        )?;
        self.enter(LoadPhase::ServerStarted);
        self.enter(LoadPhase::Running);
        Ok(())
    }

    /// Posts pre/post server tick events and advances [`crate::WorldState`].
    pub fn tick_server(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        ticks: u32,
    ) -> Result<(), String> {
        for _ in 0..ticks {
            let watermark = rt.enter_young_epoch();
            self.post_game(
                rt,
                linked,
                "net/neoforged/neoforge/event/tick/ServerTickEvent$Pre",
            )?;
            self.state.world.tick();
            self.post_game(
                rt,
                linked,
                "net/neoforged/neoforge/event/tick/ServerTickEvent$Post",
            )?;
            let _ = rt.collect_young(watermark, &[]);
        }
        Ok(())
    }

    pub fn stop_server(&mut self, rt: &mut Rt, linked: &LinkedSpace) -> Result<(), String> {
        self.post_game(
            rt,
            linked,
            "net/neoforged/neoforge/event/server/ServerStoppingEvent",
        )?;
        self.post_game(
            rt,
            linked,
            "net/neoforged/neoforge/event/server/ServerStoppedEvent",
        )?;
        self.enter(LoadPhase::Stopped);
        Ok(())
    }

    /// Dispatches a slash command against registered Brigadier roots, returning `(exit_code, messages)`.
    pub fn execute_command(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        command_line: &str,
        is_player: bool,
    ) -> Result<(i32, Vec<String>), String> {
        let Some((cmd_closure, parsed_args)) = self.state.commands.match_command(command_line)
        else {
            return Err(format!("unknown or incomplete command `{command_line}`"));
        };
        let source = rt.new_instance("net/minecraft/commands/CommandSourceStack", 0);
        let msg_list = rt.alloc("java/util/ArrayList", 0, Data::List(Vec::new()));
        rt.put_field(source, "$is_player", V::bool(is_player));
        rt.put_field(source, "$messages", msg_list);

        let cmd_ctx = rt.new_instance("com/mojang/brigadier/context/CommandContext", 0);
        rt.put_field(cmd_ctx, "$source", source);
        for (k, val) in parsed_args {
            let v = match val {
                crate::ConfigPrimitive::Bool(b) => box_primitive(rt, "java/lang/Boolean", V::bool(b)),
                crate::ConfigPrimitive::Int(i) => box_primitive(rt, "java/lang/Integer", V::I(i)),
                crate::ConfigPrimitive::Long(l) => box_primitive(rt, "java/lang/Long", V::J(l)),
                crate::ConfigPrimitive::Double(d) => box_primitive(rt, "java/lang/Double", V::D(d)),
                crate::ConfigPrimitive::Text(s) => rt.new_str(s),
            };
            rt.put_field(cmd_ctx, format!("$arg:{k}"), v);
        }

        let code = {
            let mut ctx = ExecCtx {
                rt,
                linked,
                host: self,
                depth: 0,
            };
            invoke_closure(&mut ctx, cmd_closure, &[cmd_ctx]).i()
        };
        raise_if_pending(rt, &format!("executing command `{command_line}`"))?;
        let messages = match rt.get_obj(msg_list).map(|o| &o.data) {
            Some(Data::List(items)) => items
                .iter()
                .filter_map(|&v| rt.str_of(v).map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        Ok((code, messages))
    }

    fn construct_one(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        index: usize,
    ) -> Result<(), String> {
        let entrypoints = self.mods[index].entrypoints.clone();
        if entrypoints.is_empty() {
            return Ok(());
        }
        let mod_id = self.mods[index].mod_id.clone();
        let bus = self.mod_bus_handle(rt, &mod_id);
        let container = self.mod_container_handle(rt, &mod_id);
        let dist = self.dist_handle(rt);
        let mut first = None;
        for (class_name, descriptors) in entrypoints {
            let (desc, params) = pick_constructor(&descriptors)?;
            self.constructing = Some(mod_id.clone());
            let instance = {
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                };
                let obj = alloc_new_object(&mut ctx, &class_name);
                if ctx.rt.pending_exception.is_none() {
                    let mut args = vec![obj];
                    for param in params {
                        args.push(match param {
                            CtorParam::Bus => bus,
                            CtorParam::Container => container,
                            CtorParam::Dist => dist,
                        });
                    }
                    let member = MemberRef::new(&class_name, "<init>", &desc);
                    let _ = invoke_member(&mut ctx, op::INVOKESPECIAL, &member, &args);
                }
                obj
            };
            self.constructing = None;
            raise_if_pending(rt, &format!("constructing mod `{mod_id}` ({class_name})"))?;
            rt.pin(instance);
            if first.is_none() {
                first = Some(instance);
            }
        }
        self.mods[index].instance = first;
        Ok(())
    }

    fn register_subscribers(&mut self, index: usize) -> Result<(), String> {
        let mod_id = self.mods[index].mod_id.clone();
        let subscribers = self.mods[index].subscribers.clone();
        for sub in subscribers {
            if !sub.side.matches_server(self.dedicated_server) {
                continue;
            }
            for method in &sub.methods {
                if !method.is_static {
                    continue;
                }
                let bus = match sub.bus {
                    SubscriberBusTarget::Game => BusId::Game,
                    SubscriberBusTarget::Mod => BusId::Mod(mod_id.clone()),
                    SubscriberBusTarget::Auto => {
                        if is_mod_bus_event_class(&method.event_class) {
                            BusId::Mod(mod_id.clone())
                        } else {
                            BusId::Game
                        }
                    }
                };
                self.state
                    .bus
                    .register_listener(
                        bus,
                        method.event_class.clone(),
                        EventPriority::parse(&method.priority),
                        method.receive_canceled,
                        V::NULL,
                        EventCallback::StaticMethod {
                            class_name: sub.class_name.clone(),
                            method_name: method.method_name.clone(),
                            method_desc: method.method_desc.clone(),
                        },
                    )
                    .map_err(|err| format!("mod `{mod_id}` subscriber: {err}"))?;
            }
        }
        Ok(())
    }

    fn run_registration(&mut self, rt: &mut Rt, linked: &LinkedSpace) -> Result<(), String> {
        let mut keys: Vec<String> = REGISTRATION_ORDER.iter().map(|k| (*k).to_owned()).collect();
        let mut seen: BTreeSet<String> = keys.iter().cloned().collect();
        for custom in &self.state.registries.custom_registries {
            if seen.insert(custom.clone()) {
                keys.push(custom.clone());
            }
        }
        for supplier in self.state.registries.pending_suppliers() {
            if seen.insert(supplier.registry_key.clone()) {
                keys.push(supplier.registry_key);
            }
        }
        let mod_ids: Vec<String> = self.mods.iter().map(|m| m.mod_id.clone()).collect();
        for key in &keys {
            self.flush_registry(rt, linked, key)?;
            for mod_id in &mod_ids {
                let registry_key = key.clone();
                let prev = self.constructing.replace(mod_id.clone());
                let res = self.post(
                    rt,
                    linked,
                    &BusId::Mod(mod_id.clone()),
                    REGISTER_EVENT,
                    |rt, event| {
                        let label = rt.intern_str(&registry_key);
                        rt.put_field(event, "registryKey", label);
                    },
                );
                self.constructing = prev;
                res?;
            }
            self.flush_registry(rt, linked, key)?;
            self.state.registries.finish_registry_event(key);
        }
        self.state.registries.freeze();
        Ok(())
    }

    fn flush_registry(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        key: &str,
    ) -> Result<(), String> {
        self.state.registries.begin_deferred_registration(key);
        let pending: Vec<DeferredSupplier> = self
            .state
            .registries
            .pending_suppliers()
            .into_iter()
            .filter(|supplier| supplier.registry_key == key)
            .filter(|supplier| {
                self.state
                    .registries
                    .lookup(&supplier.registry_key, &supplier.full_id)
                    .is_none_or(|entry| entry.mod_id == "minecraft")
            })
            .collect();
        for item in pending {
            let value = {
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                };
                invoke_closure(&mut ctx, item.supplier, &[])
            };
            raise_if_pending(rt, &format!("registering `{}`", item.full_id))?;
            let props = content_properties(rt, &item.registry_key, value);
            self.state.registries.register_direct(
                rt,
                &item.registry_key,
                &item.full_id,
                &item.namespace,
                value,
                props,
            );
        }
        Ok(())
    }

    fn post_each_mod(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        event_class: &str,
    ) -> Result<(), String> {
        let ids: Vec<String> = self.mods.iter().map(|m| m.mod_id.clone()).collect();
        for id in ids {
            let prev = self.constructing.replace(id.clone());
            let res = self.post(rt, linked, &BusId::Mod(id), event_class, |_, _| {});
            self.constructing = prev;
            res?;
        }
        self.drain_queued_work(rt, linked, event_class)?;
        self.fail_on_error_issues()?;
        Ok(())
    }

    fn drain_queued_work(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        phase_event: &str,
    ) -> Result<(), String> {
        while !self.queued_work.is_empty() {
            let batch = std::mem::take(&mut self.queued_work);
            for (mod_id, task, future) in batch {
                let prev = self.constructing.clone();
                self.constructing = mod_id;
                let out = {
                    let mut ctx = ExecCtx {
                        rt,
                        linked,
                        host: self,
                        depth: 0,
                    };
                    invoke_closure(&mut ctx, task, &[])
                };
                self.constructing = prev;
                raise_if_pending(rt, &format!("running enqueueWork for `{phase_event}`"))?;
                rt.put_field(future, "$done", V::I(1));
                rt.put_field(
                    future,
                    "$value",
                    if matches!(out, V::Void) { V::NULL } else { out },
                );
            }
        }
        Ok(())
    }

    fn fail_on_error_issues(&self) -> Result<(), String> {
        if let Some(issue) = self.loading_issues.iter().find(|i| i.severity == "ERROR") {
            return Err(format!(
                "ModLoadingIssue(ERROR, mod={:?}): {}",
                issue.mod_id, issue.message
            ));
        }
        Ok(())
    }

    fn drain_imc_stream(&mut self, rt: &mut Rt, target_mod_id: &str) -> V {
        let mut delivered = Vec::new();
        let mut kept = Vec::new();
        for msg in std::mem::take(&mut self.imc_messages) {
            if msg.mod_id == target_mod_id {
                let obj = rt.new_instance("net/neoforged/fml/InterModComms$IMCMessage", 0);
                let s_v = rt.new_str(msg.sender_mod_id);
                let m_v = rt.new_str(msg.mod_id);
                let meth_v = rt.new_str(msg.method);
                rt.put_field(obj, "senderModId", s_v);
                rt.put_field(obj, "modId", m_v);
                rt.put_field(obj, "method", meth_v);
                rt.put_field(obj, "messageSupplier", msg.supplier);
                delivered.push(obj);
            } else {
                kept.push(msg);
            }
        }
        self.imc_messages = kept;
        rt.alloc("java/util/stream/Stream", 0, Data::Stream(delivered))
    }

    fn post_game(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        event_class: &str,
    ) -> Result<(), String> {
        self.post(rt, linked, &BusId::Game, event_class, |_, _| {})
    }

    fn post(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        bus: &BusId,
        event_class: &str,
        decorate: impl FnOnce(&mut Rt, V),
    ) -> Result<(), String> {
        let event = rt.new_instance(event_class, 0);
        rt.put_field(event, "$canceled", V::I(0));
        decorate(rt, event);
        let listeners = self.state.bus.matching_listeners(bus, event_class);
        let (ran, _) = {
            let mut ctx = ExecCtx {
                rt,
                linked,
                host: self,
                depth: 0,
            };
            dispatch_event_on_bus(&mut ctx, &listeners, event)
        };
        self.state.bus.record_dispatch(ran);
        raise_if_pending(rt, &format!("dispatching `{event_class}`"))
    }

    fn mod_bus_handle(&mut self, rt: &mut Rt, mod_id: &str) -> V {
        if let Some(handle) = self.mod_buses.get(mod_id) {
            return *handle;
        }
        let handle = rt.new_pinned_handle(
            "net/neoforged/bus/api/IEventBus",
            "mod_event_bus",
            0,
            mod_id,
        );
        self.mod_buses.insert(mod_id.to_owned(), handle);
        handle
    }

    fn game_bus_handle(&mut self, rt: &mut Rt) -> V {
        if let Some(handle) = self.game_bus {
            return handle;
        }
        let handle = rt.new_pinned_handle(
            "net/neoforged/bus/api/IEventBus",
            "game_event_bus",
            0,
            "neoforge",
        );
        self.game_bus = Some(handle);
        handle
    }

    fn dist_handle(&mut self, rt: &mut Rt) -> V {
        let name = if self.dedicated_server {
            "DEDICATED_SERVER"
        } else {
            "CLIENT"
        };
        rt.new_pinned_handle("net/neoforged/api/distmarker/Dist", "dist", 0, name)
    }

    fn mod_container_handle(&mut self, rt: &mut Rt, mod_id: &str) -> V {
        if let Some(handle) = self.mod_containers.get(mod_id) {
            return *handle;
        }
        let handle =
            rt.new_pinned_handle("net/neoforged/fml/ModContainer", "mod_container", 0, mod_id);
        self.mod_containers.insert(mod_id.to_owned(), handle);
        handle
    }

    fn version_of(&self, mod_id: &str) -> Option<String> {
        self.mods
            .iter()
            .find(|loaded| loaded.mod_id == mod_id)
            .map(|loaded| loaded.version.clone())
    }

    /// Real mod-identity and config-path calls. Unknown methods return `None`
    /// so the VM reports them as unsupported instead of inventing a result.
    fn fml_identity(
        &mut self,
        rt: &mut Rt,
        member: &MemberRef,
        is_static: bool,
        args: &[V],
    ) -> Option<V> {
        let owner = member.owner.as_str();
        let name = member.name.as_str();
        if is_static && owner == "net/neoforged/fml/ModList" && name == "get" {
            return Some(rt.new_pinned_handle(owner, "mod_list", 0, ""));
        }
        if owner == "net/neoforged/fml/ModList" && name == "isLoaded" {
            let id_value = if is_static {
                args.first().copied()
            } else {
                args.get(1).copied()
            }?;
            let id = rt.str_of(id_value).unwrap_or("").to_owned();
            let loaded = self.version_of(&id).is_some()
                || matches!(id.as_str(), "minecraft" | "neoforge" | "forge" | "fml");
            return Some(V::bool(loaded));
        }
        if is_static && owner == "net/neoforged/fml/ModLoadingContext" && name == "get" {
            return Some(rt.new_pinned_handle(owner, "mod_loading_context", 0, ""));
        }
        if is_static && owner == "net/neoforged/fml/loading/FMLLoader" {
            return match name {
                "isProduction" => Some(V::TRUE),
                "getDist" => Some(self.dist_handle(rt)),
                _ => None,
            };
        }
        if is_static
            && owner == "net/neoforged/neoforge/data/loading/DatagenModLoader"
            && name == "isRunningDataGen"
        {
            return Some(V::FALSE);
        }
        if is_static
            && owner == "net/neoforged/neoforge/common/NeoForgeMod"
            && name == "enableMilkFluid"
        {
            self.state.milk_fluid_enabled = true;
            return Some(V::Void);
        }
        if is_static && owner == "net/neoforged/fml/ModLoadingIssue" && matches!(name, "warning" | "error") {
            let key = args
                .first()
                .and_then(|&v| rt.str_of(v))
                .unwrap_or("")
                .to_owned();
            let fmt_args: Vec<String> = match args.get(1).and_then(|&v| rt.get_obj(v)).map(|o| &o.data) {
                Some(Data::Array(xs) | Data::List(xs)) => {
                    xs.iter().map(|&v| rt.value_to_string(v)).collect()
                }
                _ => Vec::new(),
            };
            let template = self
                .translations
                .get(&key)
                .map(String::as_str)
                .unwrap_or(&key);
            let msg = crate::minecraft::format_translation_template(template, &fmt_args);
            let issue = rt.new_instance("net/neoforged/fml/ModLoadingIssue", 0);
            let sev_v = rt.intern_str(if name == "error" { "ERROR" } else { "WARNING" });
            let key_v = rt.new_str(key);
            let msg_v = rt.new_str(msg);
            rt.put_field(issue, "$severity", sev_v);
            rt.put_field(issue, "$key", key_v);
            rt.put_field(issue, "$message", msg_v);
            return Some(issue);
        }
        if is_static && owner == "net/neoforged/fml/ModLoader" && name == "addLoadingIssue" {
            let issue = *args.first()?;
            let severity = rt
                .str_of(rt.get_field(issue, "$severity"))
                .unwrap_or("WARNING")
                .to_owned();
            let translation_key = rt
                .str_of(rt.get_field(issue, "$key"))
                .unwrap_or("")
                .to_owned();
            let message = rt
                .str_of(rt.get_field(issue, "$message"))
                .unwrap_or("")
                .to_owned();
            let mod_id = rt
                .str_of(rt.get_field(issue, "$mod_id"))
                .map(str::to_owned)
                .or_else(|| self.constructing.clone());
            self.loading_issues.push(RecordedLoadingIssue {
                severity,
                mod_id,
                translation_key,
                message,
            });
            return Some(V::Void);
        }
        if is_static && owner == "net/neoforged/fml/InterModComms" && name == "sendTo" {
            let (sender, target_id, method_name, supplier) = if args.len() == 4 {
                (
                    rt.str_of(args[0])?.to_owned(),
                    rt.str_of(args[1])?.to_owned(),
                    rt.str_of(args[2])?.to_owned(),
                    args[3],
                )
            } else {
                (
                    self.constructing.clone().unwrap_or_default(),
                    rt.str_of(*args.first()?)?.to_owned(),
                    rt.str_of(*args.get(1)?)?.to_owned(),
                    *args.get(2)?,
                )
            };
            let is_loaded = self.version_of(&target_id).is_some()
                || matches!(target_id.as_str(), "minecraft" | "neoforge" | "forge" | "fml");
            if !is_loaded {
                return Some(V::FALSE);
            }
            rt.pin(supplier);
            self.imc_messages.push(ImcEntry {
                sender_mod_id: sender,
                mod_id: target_id,
                method: method_name,
                supplier,
            });
            return Some(V::TRUE);
        }
        if is_static && owner == "net/neoforged/fml/InterModComms" && name == "getMessages" {
            let target_id = rt.str_of(*args.first()?)?.to_owned();
            return Some(self.drain_imc_stream(rt, &target_id));
        }
        if is_static && owner == "net/neoforged/fml/loading/FMLPaths" && name == "getOrCreateGameRelativePath" {
            let rel = path_text(rt, *args.first()?)?;
            let p = std::path::Path::new(&rel);
            let full = if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.config_dir
                    .parent()
                    .unwrap_or(&self.config_dir)
                    .join(p)
            };
            if std::fs::create_dir_all(&full).is_err() {
                rt.throw_exception("java/lang/RuntimeException", &format!("Failed to create {rel}"));
                return Some(V::NULL);
            }
            return Some(rt.new_pinned_handle(
                "java/nio/file/Path",
                "nio_path",
                0,
                full.display().to_string(),
            ));
        }
        if is_static && owner == "java/nio/file/Files" {
            return self.nio_files(rt, name, args);
        }
        if !is_static {
            match (owner, name) {
                ("net/neoforged/fml/ModList", "getModFileById") => {
                    let id = rt.str_of(*args.get(1)?)?.to_owned();
                    return Some(if self.version_of(&id).is_some() {
                        rt.new_pinned_handle(
                            "net/neoforged/neoforgespi/language/IModFileInfo",
                            "mod_file_info",
                            0,
                            id,
                        )
                    } else {
                        V::NULL
                    });
                }
                ("net/neoforged/fml/ModList", "getModContainerById") => {
                    let id = rt.str_of(*args.get(1)?)?.to_owned();
                    let found = if self.version_of(&id).is_some() {
                        Some(self.mod_container_handle(rt, &id))
                    } else {
                        None
                    };
                    return Some(rt.alloc("java/util/Optional", 0, Data::Optional(found)));
                }
                ("net/neoforged/api/distmarker/Dist", "isClient") => {
                    let tag = handle_tag(rt, args[0])?;
                    return Some(V::bool(tag == "CLIENT"));
                }
                ("net/neoforged/api/distmarker/Dist", "isDedicatedServer") => {
                    let tag = handle_tag(rt, args[0])?;
                    return Some(V::bool(tag == "DEDICATED_SERVER"));
                }
                ("net/neoforged/neoforgespi/language/IModFileInfo", "getFile")
                | ("net/neoforged/fml/ModFileInfo", "getFile") => {
                    let id = handle_tag(rt, args[0])?;
                    return Some(rt.new_pinned_handle(
                        "net/neoforged/neoforgespi/locating/IModFile",
                        "mod_file",
                        0,
                        id,
                    ));
                }
                ("net/neoforged/neoforgespi/locating/IModFile", "getModInfos") => {
                    let id = handle_tag(rt, args[0])?;
                    let info = rt.new_pinned_handle(
                        "net/neoforged/neoforgespi/language/IModInfo",
                        "mod_info",
                        0,
                        id,
                    );
                    return Some(rt.alloc("java/util/ArrayList", 0, Data::List(vec![info])));
                }
                ("net/neoforged/neoforgespi/language/IModInfo", "getVersion")
                | ("net/neoforged/fml/ModInfo", "getVersion") => {
                    let id = handle_tag(rt, args[0])?;
                    let version = self.version_of(&id)?;
                    return Some(rt.new_pinned_handle(
                        "org/apache/maven/artifact/versioning/DefaultArtifactVersion",
                        "artifact_version",
                        0,
                        version,
                    ));
                }
                ("net/neoforged/neoforgespi/language/IModInfo", "getModId") => {
                    let id = handle_tag(rt, args[0])?;
                    return Some(rt.intern_str(&id));
                }
                ("org/apache/maven/artifact/versioning/ArtifactVersion", _)
                | ("org/apache/maven/artifact/versioning/DefaultArtifactVersion", _) => {
                    let version = handle_tag(rt, args[0])?;
                    return dispatch_maven_artifact_version(rt, &version, name);
                }
                ("net/neoforged/fml/ModContainer", "getModInfo") => {
                    let id = handle_tag(rt, args[0])?;
                    return Some(rt.new_pinned_handle(
                        "net/neoforged/neoforgespi/language/IModInfo",
                        "mod_info",
                        0,
                        id,
                    ));
                }
                ("net/neoforged/fml/ModContainer", "getModId") => {
                    let id = handle_tag(rt, args[0])?;
                    return Some(rt.intern_str(&id));
                }
                ("net/neoforged/fml/ModContainer", "getEventBus") => {
                    let id = handle_tag(rt, args[0])?;
                    return Some(self.mod_bus_handle(rt, &id));
                }
                ("net/neoforged/fml/ModContainer", "registerConfig") => {
                    let id = handle_tag(rt, args[0])?;
                    let kind = args
                        .get(1)
                        .and_then(|value| handle_tag(rt, *value))
                        .unwrap_or_else(|| "COMMON".into());
                    let file_name = args.get(3).and_then(|v| rt.str_of(*v)).map(str::to_owned);
                    if let Some(("config_spec", spec_id, _, _)) =
                        args.get(2).and_then(|v| rt.handle_of(*v))
                    {
                        self.state.config.assign_mod(spec_id, &id, &kind, file_name);
                    }
                    self.registered_configs.push((id, kind));
                    return Some(V::Void);
                }
                ("net/neoforged/fml/ModLoadingContext", "getActiveContainer") => {
                    let id = self.constructing.clone()?;
                    return Some(self.mod_container_handle(rt, &id));
                }
                ("net/neoforged/fml/ModLoadingIssue", "withAffectedMod") => {
                    if let Some(info) = args.get(1).copied() {
                        if let Some(id) = handle_tag(rt, info) {
                            let id_v = rt.intern_str(&id);
                            rt.put_field(args[0], "$mod_id", id_v);
                        }
                    }
                    return Some(args[0]);
                }
                ("net/neoforged/fml/ModLoadingIssue", "withCause") => {
                    if let Some(&cause) = args.get(1) {
                        rt.put_field(args[0], "$cause", cause);
                    }
                    return Some(args[0]);
                }
                ("net/neoforged/fml/event/lifecycle/InterModProcessEvent", "getIMCStream")
                    if args.len() == 1 =>
                {
                    let target = self.constructing.clone().unwrap_or_default();
                    return Some(self.drain_imc_stream(rt, &target));
                }
                ("net/neoforged/fml/InterModComms$IMCMessage", "senderModId") => {
                    return Some(rt.get_field(args[0], "senderModId"));
                }
                ("net/neoforged/fml/InterModComms$IMCMessage", "modId") => {
                    return Some(rt.get_field(args[0], "modId"));
                }
                ("net/neoforged/fml/InterModComms$IMCMessage", "method") => {
                    return Some(rt.get_field(args[0], "method"));
                }
                ("net/neoforged/fml/InterModComms$IMCMessage", "messageSupplier") => {
                    return Some(rt.get_field(args[0], "messageSupplier"));
                }
                (
                    "net/neoforged/fml/event/lifecycle/ParallelDispatchEvent"
                    | "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent"
                    | "net/neoforged/fml/event/lifecycle/FMLClientSetupEvent"
                    | "net/neoforged/fml/event/lifecycle/FMLDedicatedServerSetupEvent"
                    | "net/neoforged/fml/event/lifecycle/InterModEnqueueEvent"
                    | "net/neoforged/fml/event/lifecycle/InterModProcessEvent"
                    | "net/neoforged/fml/event/lifecycle/FMLLoadCompleteEvent",
                    "enqueueWork",
                ) => {
                    let task = *args.get(1)?;
                    rt.pin(task);
                    let fut = rt.new_instance("java/util/concurrent/CompletableFuture", 0);
                    rt.put_field(fut, "$done", V::I(0));
                    rt.pin(fut);
                    self.state.bus.deferred_work.push(task);
                    self.queued_work
                        .push((self.constructing.clone(), task, fut));
                    return Some(fut);
                }
                ("net/neoforged/fml/loading/FMLPaths", "get") => {
                    let which = handle_tag(rt, args[0])?;
                    let base_root = self
                        .config_dir
                        .parent()
                        .unwrap_or(&self.config_dir)
                        .join("mmga-game");
                    let dir = match which.as_str() {
                        "config" | "configdir" => self.config_dir.clone(),
                        "gamedir" => base_root,
                        "modsdir" => base_root.join("mods"),
                        _ => return None,
                    };
                    let _ = std::fs::create_dir_all(&dir);
                    return Some(rt.new_pinned_handle(
                        "java/nio/file/Path",
                        "nio_path",
                        0,
                        dir.display().to_string(),
                    ));
                }
                ("java/nio/file/Path", "resolve") => {
                    let base = path_text(rt, args[0])?;
                    let child = path_text(rt, *args.get(1)?)?;
                    let joined = std::path::Path::new(&base).join(child);
                    return Some(rt.new_pinned_handle(
                        "java/nio/file/Path",
                        "nio_path",
                        0,
                        joined.display().to_string(),
                    ));
                }
                ("java/nio/file/Path", "toFile") => {
                    let path = path_text(rt, args[0])?;
                    return Some(rt.new_pinned_handle("java/io/File", "io_file", 0, path));
                }
                ("java/io/File", "<init>") => {
                    let path = rt.str_of(*args.get(1)?)?.to_owned();
                    let stored = rt.intern_str(&path);
                    rt.put_field(args[0], "path", stored);
                    return Some(V::Void);
                }
                ("java/io/File", "isFile" | "exists") => {
                    let path = path_text(rt, args[0])?;
                    let exists = std::path::Path::new(&path).exists();
                    let ok = if name == "isFile" {
                        std::path::Path::new(&path).is_file()
                    } else {
                        exists
                    };
                    return Some(V::bool(ok));
                }
                ("java/io/File", "delete") => {
                    let path = path_text(rt, args[0])?;
                    return Some(V::bool(std::fs::remove_file(path).is_ok()));
                }
                ("java/io/File", "renameTo") => {
                    let from = path_text(rt, args[0])?;
                    let to = path_text(rt, *args.get(1)?)?;
                    return Some(V::bool(std::fs::rename(from, to).is_ok()));
                }
                _ => {}
            }
        }
        None
    }

    fn nio_files(&mut self, rt: &mut Rt, name: &str, args: &[V]) -> Option<V> {
        match name {
            "isDirectory" => {
                let path = path_text(rt, *args.first()?)?;
                Some(V::bool(std::path::Path::new(&path).is_dir()))
            }
            "createDirectories" => {
                let path = path_text(rt, *args.first()?)?;
                if std::fs::create_dir_all(&path).is_err() {
                    rt.throw_exception("java/io/IOException", &format!("createDirectories {path}"));
                    return Some(V::NULL);
                }
                Some(rt.new_pinned_handle("java/nio/file/Path", "nio_path", 0, path))
            }
            _ => None,
        }
    }

    fn bus_id_of(&self, rt: &Rt, bus: V) -> Option<BusId> {
        let (kind, _, tag, _) = rt.handle_of(bus)?;
        match kind {
            "mod_event_bus" => Some(BusId::Mod(tag.to_owned())),
            "game_event_bus" => Some(BusId::Game),
            _ => None,
        }
    }

    fn add_listener(&mut self, rt: &mut Rt, args: &[V], desc: &str) -> Option<V> {
        let bus = self.bus_id_of(rt, *args.first()?)?;
        let mut priority = EventPriority::Normal;
        let mut receive_canceled = false;
        let mut explicit_cls: Option<String> = None;
        let mut consumer: Option<V> = None;

        for &arg in &args[1..] {
            if let Some((kind, id, tag, _)) = rt.handle_of(arg) {
                if kind == "event_priority" {
                    priority = match id {
                        0 => EventPriority::Highest,
                        1 => EventPriority::High,
                        3 => EventPriority::Low,
                        4 => EventPriority::Lowest,
                        _ => EventPriority::Normal,
                    };
                    continue;
                }
                if kind == "class" {
                    explicit_cls = Some(tag.to_owned());
                    continue;
                }
            }
            if matches!(arg, V::I(_)) && desc.contains('Z') {
                receive_canceled = arg.i() != 0;
                continue;
            }
            consumer = Some(arg);
        }

        let consumer = consumer?;
        let event_class = explicit_cls
            .or_else(|| event_class_of_listener(rt, consumer))
            .or_else(|| {
                rt.throw_exception(
                    "java/lang/IllegalArgumentException",
                    "addListener could not determine the event class from the callback",
                );
                None
            })?;
        rt.pin(consumer);
        if let Err(err) = self.state.bus.register_listener(
            bus,
            event_class,
            priority,
            receive_canceled,
            consumer,
            EventCallback::Consumer(consumer),
        ) {
            rt.throw_exception("java/lang/IllegalArgumentException", &err);
        }
        Some(V::Void)
    }

    fn register_bus_target(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        args: &[V],
    ) -> Option<V> {
        let bus = self.bus_id_of(rt, *args.first()?)?;
        let target = *args.get(1)?;
        if target.is_null() {
            return Some(rt.throw_exception(
                "java/lang/NullPointerException",
                "IEventBus.register(null)",
            ));
        }
        let (only_static, class_name) = match rt.handle_of(target) {
            Some(("class", _, tag, _)) => (true, tag.to_owned()),
            _ => (false, rt.class_of(target)?.to_owned()),
        };
        if !only_static {
            rt.pin(target);
        }
        let mut cur = Some(class_name.as_str());
        while let Some(cls) = cur {
            if let Some(cf) = linked.bytecode_classes.get(cls) {
                for m in mmga_classfile::scan_subscribe_methods(cf, only_static) {
                    if only_static != m.is_static {
                        continue;
                    }
                    let cb = if m.is_static {
                        EventCallback::StaticMethod {
                            class_name: cls.to_owned(),
                            method_name: m.method_name,
                            method_desc: m.method_desc,
                        }
                    } else {
                        EventCallback::InstanceMethod {
                            instance: target,
                            class_name: cls.to_owned(),
                            method_name: m.method_name,
                            method_desc: m.method_desc,
                        }
                    };
                    if let Err(err) = self.state.bus.register_listener(
                        bus.clone(),
                        m.event_class,
                        EventPriority::parse(&m.priority),
                        m.receive_canceled,
                        target,
                        cb,
                    ) {
                        rt.throw_exception("java/lang/IllegalArgumentException", &err);
                        return Some(V::Void);
                    }
                }
            }
            cur = linked.superclass_of(cls);
        }
        Some(V::Void)
    }

    fn deferred_create(&mut self, rt: &mut Rt, name: &str, args: &[V]) -> Option<V> {
        let (registry_key, class_name, namespace) = match name {
            "createBlocks" => (
                "minecraft:block".to_owned(),
                "net/neoforged/neoforge/registries/DeferredRegister$Blocks",
                rt.str_of(*args.first()?)?.to_owned(),
            ),
            "createItems" => (
                "minecraft:item".to_owned(),
                "net/neoforged/neoforge/registries/DeferredRegister$Items",
                rt.str_of(*args.first()?)?.to_owned(),
            ),
            "create" | "createDataComponents" | "createEntities" => {
                let namespace = args
                    .iter()
                    .rev()
                    .find_map(|v| rt.str_of(*v))
                    .unwrap_or("minecraft")
                    .to_owned();
                let registry_key = match name {
                    "createDataComponents" => "minecraft:data_component_type".to_owned(),
                    "createEntities" => "minecraft:entity_type".to_owned(),
                    _ => args
                        .first()
                        .and_then(|v| {
                            crate::minecraft::resource_key_parts(rt, *v)
                                .map(|(_, loc)| loc)
                                .or_else(|| resource_id(rt, *v))
                                .or_else(|| rt.str_of(*v).map(str::to_owned))
                                .or_else(|| rt.handle_of(*v).map(|(_, _, tag, _)| tag.to_owned()))
                        })
                        .unwrap_or_else(|| "minecraft:custom".into()),
                };
                (
                    registry_key,
                    "net/neoforged/neoforge/registries/DeferredRegister",
                    namespace,
                )
            }
            _ => return None,
        };
        let dr_id = self
            .state
            .registries
            .create_deferred_register(&registry_key, &namespace);
        Some(rt.new_pinned_handle(class_name, "deferred_register", dr_id, registry_key))
    }

    fn deferred_register(&mut self, rt: &mut Rt, args: &[V], desc: &str) -> Option<V> {
        let receiver = *args.first()?;
        let (dr_id, registry_key) = {
            let (kind, id, tag, _) = rt.handle_of(receiver)?;
            if kind != "deferred_register" {
                return None;
            }
            (id, tag.to_owned())
        };
        if desc.starts_with("(Ljava/lang/String;Ljava/util/function/Supplier;)") {
            let name = rt.str_of(*args.get(1)?)?.to_owned();
            let supplier = *args.get(2)?;
            let holder_class = holder_class_for(&registry_key);
            return self.state.registries.enqueue_deferred_entry(
                rt,
                dr_id,
                holder_class,
                &name,
                supplier,
            );
        }
        if desc.starts_with("(Lnet/neoforged/bus/api/IEventBus;)") {
            if let Some(bus) = args.get(1).copied() {
                if let Some(("mod_event_bus", _, tag, _)) = rt.handle_of(bus) {
                    let mod_id = tag.to_owned();
                    self.state.registries.bind_mod_bus(dr_id, &mod_id);
                }
            }
            return Some(V::Void);
        }
        None
    }

    /// Pure Minecraft value types whose behavior is fully defined and does not
    /// pretend a world, registry, or renderer exists.
    fn minecraft_value(
        &mut self,
        rt: &mut Rt,
        member: &MemberRef,
        is_static: bool,
        args: &[V],
    ) -> Option<V> {
        let owner = member.owner.as_str();
        let name = member.name.as_str();
        if owner == "net/minecraft/util/StringUtil" && is_static && name == "isNullOrEmpty" {
            let text = args.first().and_then(|value| rt.str_of(*value));
            return Some(V::bool(text.is_none_or(|text| text.is_empty())));
        }
        if owner == "net/minecraft/Util" && is_static {
            match name {
                "makeDescriptionId" => {
                    let kind = rt.str_of(*args.first()?)?.to_owned();
                    let out = match args.get(1).copied().and_then(|v| resource_id(rt, v)) {
                        Some(id) => {
                            let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
                            format!("{kind}.{ns}.{}", path.replace('/', "."))
                        }
                        None => format!("{kind}.unregistered_sadface"),
                    };
                    return Some(rt.new_str(out));
                }
                "getMillis" => return Some(V::J(1_000)),
                "getNanos" => return Some(V::J(1_000_000_000)),
                _ => {}
            }
        }
        if owner == "net/minecraft/resources/ResourceLocation" {
            if is_static && name == "withDefaultNamespace" {
                let path = rt.str_of(*args.first()?)?.to_owned();
                return Some(resource_location(rt, "minecraft", &path));
            }
            if is_static && name == "fromNamespaceAndPath" {
                let namespace = rt.str_of(*args.first()?)?.to_owned();
                let path = rt.str_of(*args.get(1)?)?.to_owned();
                return Some(resource_location(rt, &namespace, &path));
            }
            if is_static && (name == "parse" || name == "tryParse" || name == "bySeparator") {
                let raw = rt.str_of(*args.first()?)?.to_owned();
                let (namespace, path) = match raw.split_once(':') {
                    Some((namespace, path)) => (namespace.to_owned(), path.to_owned()),
                    None => ("minecraft".to_owned(), raw),
                };
                return Some(resource_location(rt, &namespace, &path));
            }
            if !is_static && name == "<init>" {
                let (namespace, path) = if args.len() >= 3 {
                    (
                        rt.str_of(*args.get(1)?)?.to_owned(),
                        rt.str_of(*args.get(2)?)?.to_owned(),
                    )
                } else {
                    let raw = rt.str_of(*args.get(1)?)?;
                    let (namespace, path) = raw.split_once(':').unwrap_or(("minecraft", raw));
                    (namespace.to_owned(), path.to_owned())
                };
                let tag = format!("{namespace}:{path}");
                let stored = rt.intern_str(&tag);
                rt.put_field(args[0], "id", stored);
                return Some(V::Void);
            }
            if !is_static && name == "toString" {
                return Some(rt.new_str(resource_id(rt, args[0])?));
            }
            if !is_static && name == "getNamespace" {
                let id = resource_id(rt, args[0])?;
                let namespace = id.split_once(':').map(|(ns, _)| ns).unwrap_or("minecraft");
                return Some(rt.intern_str(namespace));
            }
            if !is_static && name == "getPath" {
                let id = resource_id(rt, args[0])?;
                let path = id
                    .split_once(':')
                    .map(|(_, path)| path)
                    .unwrap_or(id.as_str());
                return Some(rt.intern_str(path));
            }
            if !is_static && name == "withPrefix" {
                let id = resource_id(rt, args[0])?;
                let prefix = rt.str_of(*args.get(1)?)?.to_owned();
                let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
                return Some(resource_location(rt, ns, &format!("{prefix}{path}")));
            }
            if !is_static && name == "withSuffix" {
                let id = resource_id(rt, args[0])?;
                let suffix = rt.str_of(*args.get(1)?)?.to_owned();
                let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
                return Some(resource_location(rt, ns, &format!("{path}{suffix}")));
            }
            if !is_static && name == "withPath" && member.desc.starts_with("(Ljava/lang/String;)") {
                let id = resource_id(rt, args[0])?;
                let new_path = rt.str_of(*args.get(1)?)?.to_owned();
                let (ns, _) = id.split_once(':').unwrap_or(("minecraft", &id));
                return Some(resource_location(rt, ns, &new_path));
            }
            if !is_static && name == "toLanguageKey" {
                let id = resource_id(rt, args[0])?;
                let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
                let body = format!("{ns}.{}", path.replace('/', "."));
                let out = match args.get(1).and_then(|&v| rt.str_of(v)) {
                    Some(prefix) => format!("{prefix}.{body}"),
                    None => body,
                };
                return Some(rt.new_str(out));
            }
            if !is_static && name == "hashCode" {
                let id = resource_id(rt, args[0])?;
                let h = id.encode_utf16().fold(0i32, |acc, u| {
                    acc.wrapping_mul(31).wrapping_add(i32::from(u))
                });
                return Some(V::I(h));
            }
            if !is_static && name == "compareTo" {
                let left = resource_id(rt, args[0])?;
                let right = resource_id(rt, *args.get(1)?)?;
                let (l_ns, l_p) = left.split_once(':').unwrap_or(("minecraft", &left));
                let (r_ns, r_p) = right.split_once(':').unwrap_or(("minecraft", &right));
                let ord = l_p.cmp(r_p).then_with(|| l_ns.cmp(r_ns)) as i32;
                return Some(V::I(ord));
            }
            if !is_static && name == "equals" {
                let left = resource_id(rt, args[0])?;
                let right = args
                    .get(1)
                    .and_then(|value| resource_id(rt, *value))
                    .unwrap_or_default();
                return Some(V::bool(left == right));
            }
        }
        None
    }

    fn content_init(&mut self, rt: &mut Rt, member: &MemberRef, args: &[V]) -> Option<V> {
        let owner = member.owner.as_str();
        let name = member.name.as_str();
        match (owner, name) {
            ("net/minecraft/world/level/block/Block$Properties", "<init>") => Some(V::Void),
            ("net/minecraft/world/level/block/Block$Properties", "of") => {
                Some(rt.new_instance("net/minecraft/world/level/block/Block$Properties", 0))
            }
            ("net/minecraft/world/level/block/Block$Properties", "strength") => {
                let hardness = args.get(1).copied().unwrap_or(V::F(0.0));
                rt.put_field(args[0], "hardness", hardness);
                if args.len() > 2 {
                    rt.put_field(args[0], "resistance", args[2]);
                }
                Some(args[0])
            }
            ("net/minecraft/world/level/block/Block", "<init>") => {
                if let Some(props) = args.get(1).copied() {
                    if let Some(hardness) = instance_field(rt, props, "hardness") {
                        rt.put_field(args[0], "hardness", hardness);
                    }
                    if let Some(resistance) = instance_field(rt, props, "resistance") {
                        rt.put_field(args[0], "resistance", resistance);
                    }
                }
                Some(V::Void)
            }
            ("net/minecraft/world/item/Item$Properties", "<init>") => Some(V::Void),
            ("net/minecraft/world/item/Item$Properties", "stacksTo") => {
                let count = args.get(1).copied().unwrap_or(V::I(64));
                rt.put_field(args[0], "maxStackSize", count);
                Some(args[0])
            }
            ("net/minecraft/world/item/Item", "<init>") => {
                if let Some(props) = args.get(1).copied() {
                    if let Some(count) = instance_field(rt, props, "maxStackSize") {
                        rt.put_field(args[0], "maxStackSize", count);
                    }
                }
                Some(V::Void)
            }
            ("net/minecraft/server/level/ServerLevel", "setBlock") => {
                let x = args.first().copied().unwrap_or(V::I(0)).i();
                let y = args.get(1).copied().unwrap_or(V::I(0)).i();
                let z = args.get(2).copied().unwrap_or(V::I(0)).i();
                let block_id = args
                    .get(3)
                    .and_then(|v| rt.str_of(*v))
                    .unwrap_or("minecraft:air")
                    .to_owned();
                self.state
                    .world
                    .set_block(crate::BlockCoord::overworld(x, y, z), block_id);
                Some(V::Void)
            }
            _ => None,
        }
    }

    fn holder_get(&self, rt: &mut Rt, args: &[V]) -> Option<V> {
        let (kind, id, _, _) = rt.handle_of(*args.first()?)?;
        if kind != "deferred_holder" {
            return None;
        }
        Some(self.state.registries.require_holder_value(rt, id))
    }
}

impl HostApi for FmlLoader {
    fn get_static_field(&mut self, rt: &mut Rt, owner: &str, name: &str, desc: &str) -> Option<V> {
        if owner == "net/neoforged/neoforge/common/NeoForge" && name == "EVENT_BUS" {
            return Some(self.game_bus_handle(rt));
        }
        if owner == "net/minecraft/core/registries/Registries" && name == "ROOT_REGISTRY_NAME" {
            return Some(resource_location(rt, "minecraft", "root"));
        }
        if owner == "net/minecraft/Util" && name == "NIL_UUID" {
            return Some(rt.alloc_pinned(
                "java/util/UUID",
                0,
                Data::NativeHandle {
                    kind: "uuid",
                    id: 0,
                    tag: "00000000-0000-0000-0000-000000000000".into(),
                    payload: [0, 0],
                },
            ));
        }
        if let Some(reg_key) = crate::minecraft::lookup_builtin_registry_key(owner, name) {
            if desc.starts_with("Lnet/minecraft/resources/ResourceKey;") {
                return Some(crate::minecraft::new_resource_key(rt, "minecraft:root", reg_key));
            }
            let cls = if desc.starts_with("Lnet/minecraft/core/DefaultedRegistry;") {
                "net/minecraft/core/DefaultedRegistry"
            } else {
                "net/minecraft/core/Registry"
            };
            return Some(rt.new_pinned_handle(cls, "registry", 0, reg_key));
        }
        if owner.ends_with("EventPriority") {
            let priority = EventPriority::parse(name);
            return Some(rt.new_pinned_handle(owner, "event_priority", priority as u64, name));
        }
        if owner == "net/neoforged/fml/loading/FMLEnvironment" {
            return match name {
                "dist" => Some(self.dist_handle(rt)),
                "production" => Some(V::TRUE),
                "naming" => Some(rt.intern_str("moj")),
                _ => None,
            };
        }
        if owner == "net/neoforged/api/distmarker/Dist"
            && matches!(name, "CLIENT" | "DEDICATED_SERVER")
        {
            return Some(rt.new_pinned_handle(owner, "dist", 0, name));
        }
        if owner == "net/neoforged/fml/loading/FMLPaths"
            && matches!(name, "GAMEDIR" | "CONFIGDIR" | "MODSDIR" | "FMLCONFIG")
        {
            return Some(rt.new_pinned_handle(owner, "fml_path", 0, name.to_ascii_lowercase()));
        }
        if owner == "net/neoforged/fml/config/ModConfig$Type"
            && matches!(name, "COMMON" | "CLIENT" | "SERVER" | "STARTUP")
        {
            return Some(rt.new_pinned_handle(owner, "config_type", 0, name));
        }
        if owner == "org/apache/commons/lang3/SystemUtils" {
            let arch = std::env::consts::ARCH;
            let os = std::env::consts::OS;
            return match name {
                "OS_ARCH" => Some(rt.intern_str(arch)),
                "OS_NAME" => Some(rt.intern_str(os)),
                "OS_VERSION" => Some(rt.intern_str("")),
                "IS_OS_LINUX" => Some(V::bool(os == "linux")),
                "IS_OS_WINDOWS" => Some(V::bool(os == "windows")),
                "IS_OS_MAC" | "IS_OS_MAC_OSX" => Some(V::bool(os == "macos")),
                _ => None,
            };
        }
        if let Some(value) = crate::armor::get_static_field(rt, &mut self.state.registries, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::attributes::get_static_field(rt, &mut self.state.registries, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::direction::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::blocks::get_static_field(rt, &mut self.state.registries, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::block_entities::get_static_field(rt, &mut self.state.registries, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::dfu::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::item_stack::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::custom_data::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::attribute_modifier::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::dye_color::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::string_representable::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::tags::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::sided_threads::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::menu_types::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::push_reaction::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::note_instruments::get_static_field(rt, &mut self.state.registries, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::capabilities::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::codecs::get_static_field(rt, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::sounds::get_static_field(rt, &mut self.state.registries, owner, name, desc) {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::get_static_field(rt, owner, name) {
            return Some(value);
        }
        None
    }

    fn call_method(
        &mut self,
        rt: &mut Rt,
        linked: &LinkedSpace,
        member: &MemberRef,
        is_static: bool,
        args: &[V],
    ) -> Option<V> {
        if let Some(value) = self.fml_identity(rt, member, is_static, args) {
            return Some(value);
        }
        if member.owner == "net/neoforged/neoforge/common/util/Lazy" {
            if is_static && (member.name == "of" || member.name == "concurrentOf") {
                let sup = *args.first()?;
                rt.pin(sup);
                let lazy = rt.new_instance("net/neoforged/neoforge/common/util/Lazy", 0);
                rt.put_field(lazy, "$supplier", sup);
                rt.put_field(lazy, "$computed", V::I(0));
                return Some(lazy);
            }
            if !is_static {
                let recv = *args.first()?;
                return match member.name.as_str() {
                    "get" => {
                        if rt.get_field(recv, "$computed").i() != 0 {
                            return Some(rt.get_field(recv, "$cached"));
                        }
                        let sup = rt.get_field(recv, "$supplier");
                        let val = {
                            let mut ctx = ExecCtx {
                                rt,
                                linked,
                                host: self,
                                depth: 0,
                            };
                            invoke_closure(&mut ctx, sup, &[])
                        };
                        if rt.pending_exception.is_none() {
                            rt.put_field(recv, "$cached", val);
                            rt.put_field(recv, "$computed", V::I(1));
                        }
                        Some(val)
                    }
                    "invalidate" => {
                        rt.put_field(recv, "$computed", V::I(0));
                        rt.put_field(recv, "$cached", V::NULL);
                        Some(V::Void)
                    }
                    _ => None,
                };
            }
        }
        if !is_static
            && member.owner == "java/util/function/Supplier"
            && member.name == "get"
            && rt.class_of(*args.first()?) == Some("net/neoforged/neoforge/common/util/Lazy")
        {
            let recv = args[0];
            if rt.get_field(recv, "$computed").i() != 0 {
                return Some(rt.get_field(recv, "$cached"));
            }
            let sup = rt.get_field(recv, "$supplier");
            let val = {
                let mut ctx = ExecCtx {
                    rt,
                    linked,
                    host: self,
                    depth: 0,
                };
                invoke_closure(&mut ctx, sup, &[])
            };
            if rt.pending_exception.is_none() {
                rt.put_field(recv, "$cached", val);
                rt.put_field(recv, "$computed", V::I(1));
            }
            return Some(val);
        }
        if let Some(value) =
            crate::minecraft::call_resources_and_io(rt, &self.jar_resources, member, is_static, args)
        {
            return Some(value);
        }
        if let Some(value) = crate::common_components::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) =
            crate::minecraft::call_chat(rt, &self.translations, member, is_static, args)
        {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::call_brigadier(
            rt,
            &mut self.state.commands,
            &self.translations,
            member,
            is_static,
            args,
        ) {
            return Some(value);
        }
        if let Some(value) = crate::deferred_holders::call(rt, &mut self.state.registries, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::logging_markers::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::resource_reload::call(rt, member, is_static, args) {
            return Some(value);
        }
        {
            let mut ctx = ExecCtx { rt, linked, host: self, depth: 0 };
            if let Some(value) = crate::util_values::call(&mut ctx, member, is_static, args) {
                return Some(value);
            }
        }
        if let Some(value) = crate::blocks::call(rt, &mut self.state.registries, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::block_entities::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::string_representable::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::attribute_modifier::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::dye_color::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::guava_builders::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::item_stack::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::custom_data::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::dfu::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::call_keys_authlib_and_libs(
            rt,
            &mut self.state.registries,
            member,
            is_static,
            args,
        ) {
            return Some(value);
        }
        if let Some(value) = crate::sounds::call_sound_event(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::call_fast_color(member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::direction::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::attributes::call(rt, member, is_static, args) {
            return Some(value);
        }
        {
            let mut ctx = ExecCtx { rt, linked, host: self, depth: 0 };
            if let Some(value) = crate::armor::call(&mut ctx, member, is_static, args) {
                return Some(value);
            }
        }
        if let Some(value) = crate::deferred_items::register(rt, &mut self.state.registries, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::deferred_items::call_factory(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::sided_threads::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::menu_types::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::push_reaction::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::note_instruments::call(rt, &mut self.state.registries, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::capabilities::call(rt, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::attachments::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::components::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::nf_stream_codecs::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::codecs::call(rt, linked, self, member, is_static, args) {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::call_network_and_codecs(
            rt,
            linked,
            self,
            member,
            is_static,
            args,
            |host| {
                // SAFETY: `self` is `FmlLoader` passed as `&mut dyn HostApi` above.
                let ptr = host as *mut dyn HostApi as *mut FmlLoader;
                unsafe { (&mut (*ptr).state.network, (*ptr).constructing.clone()) }
            },
        ) {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::call_dfu_either_and_pair(
            rt,
            linked,
            self,
            member,
            is_static,
            args,
        ) {
            return Some(value);
        }
        if let Some(value) = crate::minecraft::call_mod_config_spec(
            rt,
            linked,
            self,
            member,
            is_static,
            args,
            |host| {
                // SAFETY: `self` is `FmlLoader` passed as `&mut dyn HostApi` above.
                let ptr = host as *mut dyn HostApi as *mut FmlLoader;
                unsafe { &mut (*ptr).state.config }
            },
        ) {
            return Some(value);
        }
        let owner = member.owner.as_str();
        if owner.starts_with("net/neoforged/neoforge/registries/DeferredRegister")
            && is_static
            && matches!(
                member.name.as_str(),
                "create" | "createBlocks" | "createItems" | "createDataComponents" | "createEntities"
            )
        {
            return self.deferred_create(rt, &member.name, args);
        }
        if owner == "net/neoforged/neoforge/registries/RegisterEvent" && !is_static {
            let recv = *args.first()?;
            let reg_key = rt
                .str_of(rt.get_field(recv, "registryKey"))
                .unwrap_or("minecraft:custom")
                .to_owned();
            return match member.name.as_str() {
                "getRegistryKey" => Some(crate::minecraft::new_resource_key(
                    rt,
                    "minecraft:root",
                    &reg_key,
                )),
                "getRegistry" => Some(rt.new_pinned_handle(
                    "net/minecraft/core/Registry",
                    "registry",
                    0,
                    reg_key,
                )),
                "register" if args.len() == 4 => {
                    let id_str = resource_id(rt, args[2])?;
                    let ns = id_str
                        .split_once(':')
                        .map_or("minecraft", |(n, _)| n)
                        .to_owned();
                    let val = if rt.get_obj(args[3]).is_some_and(|o| matches!(o.data, Data::Closure(_))) {
                        let mut ctx = ExecCtx {
                            rt,
                            linked,
                            host: self,
                            depth: 0,
                        };
                        invoke_closure(&mut ctx, args[3], &[])
                    } else {
                        args[3]
                    };
                    if rt.pending_exception.is_some() {
                        return Some(V::Void);
                    }
                    let props = content_properties(rt, &reg_key, val);
                    self.state
                        .registries
                        .register_direct(rt, &reg_key, &id_str, &ns, val, props);
                    Some(V::Void)
                }
                _ => None,
            };
        }
        if owner.starts_with("net/neoforged/neoforge/registries/DeferredRegister")
            && !is_static
            && member.name == "register"
        {
            return self.deferred_register(rt, args, &member.desc);
        }
        if (owner == "net/neoforged/bus/api/IEventBus" || owner.ends_with("/EventBus"))
            && !is_static
        {
            return match member.name.as_str() {
                "addListener" => self.add_listener(rt, args, &member.desc),
                "register" => self.register_bus_target(rt, linked, args),
                "unregister" => {
                    let bus = self.bus_id_of(rt, *args.first()?)?;
                    self.state.bus.unregister(&bus, *args.get(1)?);
                    Some(V::Void)
                }
                _ => None,
            };
        }
        if owner.ends_with("DeferredHolder") && is_static && member.name == "create" {
            // `ResourceKey<Registry<T>>` lives in the root registry; its location is the target registry id.
            let (_, registry) = crate::minecraft::resource_key_parts(rt, *args.first()?)?;
            let location = rt
                .handle_of(*args.get(1)?)
                .filter(|(kind, _, _, _)| *kind == "resource_location")
                .map(|(_, _, tag, _)| tag.to_owned())
                .or_else(|| rt.str_of(*args.get(1)?).map(str::to_owned))?;
            return Some(self.state.registries.bind_holder(
                rt,
                &registry,
                &location,
                "net/neoforged/neoforge/registries/DeferredHolder",
            ));
        }
        if owner.ends_with("DeferredHolder") && !is_static && member.name == "get" {
            return self.holder_get(rt, args);
        }
        if owner.starts_with("net/minecraft/") {
            if let Some(value) = self.minecraft_value(rt, member, is_static, args) {
                return Some(value);
            }
            return self.content_init(rt, member, args);
        }
        None
    }

    fn is_api_subtype(&self, sub_class: &str, super_class: &str) -> bool {
        if sub_class == super_class {
            return true;
        }
        // These host-owned values have no bytecode superclass/interface table.
        // Keep the exact 1.21.1 hierarchy visible to CHECKCAST, INSTANCEOF, and Class.
        if sub_class == "net/minecraft/network/codec/StreamCodec"
            && matches!(super_class,
                "net/minecraft/network/codec/StreamEncoder"
                    | "net/minecraft/network/codec/StreamDecoder")
        {
            return true;
        }
        if sub_class == "net/minecraft/world/entity/ai/attributes/RangedAttribute"
            && super_class == "net/minecraft/world/entity/ai/attributes/Attribute"
        {
            return true;
        }
        if matches!(sub_class,
            "net/minecraft/core/Direction"
                | "net/minecraft/world/item/ArmorItem$Type"
                | "net/minecraft/world/entity/EquipmentSlot"
                | "net/minecraft/world/entity/EquipmentSlot$Type")
            && matches!(super_class, "java/lang/Enum" | "java/lang/Comparable" | "java/io/Serializable")
        {
            return true;
        }
        if matches!(sub_class,
            "net/minecraft/core/Direction"
                | "net/minecraft/world/item/ArmorItem$Type"
                | "net/minecraft/world/entity/EquipmentSlot")
            && super_class == "net/minecraft/util/StringRepresentable"
        {
            return true;
        }
        if crate::blocks::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::string_representable::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::dfu::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::logging_markers::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::attribute_modifier::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::dye_color::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::resource_reload::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::sided_threads::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::menu_types::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::item_stack::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::custom_data::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::push_reaction::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::note_instruments::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::capabilities::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::attachments::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::components::is_subtype(sub_class, super_class) {
            return true;
        }
        if crate::minecraft::is_subtype(sub_class, super_class) {
            return true;
        }
        if super_class == "net/neoforged/neoforge/registries/DeferredRegister"
            && sub_class.starts_with("net/neoforged/neoforge/registries/DeferredRegister")
        {
            return true;
        }
        if super_class == "net/neoforged/neoforge/registries/DeferredHolder"
            && (sub_class.ends_with("DeferredHolder")
                || sub_class.ends_with("DeferredBlock")
                || sub_class.ends_with("DeferredItem"))
        {
            return true;
        }
        let mut cur = sub_class;
        while let Some(spec) = crate::find_builtin_event(cur) {
            if spec.class == super_class || spec.superclass == super_class {
                return true;
            }
            if spec.superclass == "java/lang/Object" {
                break;
            }
            cur = spec.superclass;
        }
        false
    }
}

fn loaded_mod(
    entry: &ModEntry,
    jar: &ModJar,
    entrypoints: Vec<DiscoveredModEntrypoint>,
    sole_mod_in_jar: bool,
) -> LoadedMod {
    let subscribers = jar
        .discover_event_bus_subscribers()
        .into_iter()
        .filter(|sub| match &sub.mod_id {
            Some(id) => id == &entry.mod_id,
            None => sole_mod_in_jar,
        })
        .collect();
    let inits = entrypoints
        .iter()
        .map(|ep| (ep.class_name.clone(), ep.constructor_descriptors.clone()))
        .collect::<Vec<_>>();
    LoadedMod {
        mod_id: entry.mod_id.clone(),
        version: entry.version.clone(),
        display_name: entry.display_name.clone(),
        source_name: jar.source_name.clone(),
        entry_class: inits.first().map(|(class, _)| class.clone()),
        instance: None,
        entrypoints: inits,
        subscribers,
    }
}

fn manifests_in(jar: &ModJar) -> Result<Vec<ModsToml>, String> {
    let mut out = Vec::new();
    if let Some(root) = &jar.mods_toml {
        out.push(root.clone());
    }
    out.extend(jar.nested_manifests.iter().cloned());
    if out.is_empty() {
        out.push(manifest_for(jar)?);
    }
    Ok(out)
}

fn check_version_ranges(manifests: &[&ModsToml], dedicated_server: bool) -> Result<(), String> {
    let mut versions = BTreeMap::from([
        ("minecraft".to_owned(), TARGET_MINECRAFT.to_owned()),
        ("neoforge".to_owned(), TARGET_NEOFORGE.to_owned()),
        ("forge".to_owned(), TARGET_NEOFORGE.to_owned()),
        ("fml".to_owned(), TARGET_NEOFORGE.to_owned()),
    ]);
    for manifest in manifests {
        for entry in &manifest.mods {
            versions.insert(entry.mod_id.clone(), entry.version.clone());
        }
    }
    for manifest in manifests {
        for (owner, deps) in &manifest.dependencies {
            for dep in deps {
                if !dep.side.matches_server(dedicated_server) {
                    continue;
                }
                let Some(actual) = versions.get(&dep.mod_id) else {
                    continue;
                };
                let in_range =
                    dep.version_range.is_empty() || version_in_range(actual, &dep.version_range);
                match dep.dep_type {
                    mmga_classfile::DependencyType::Required
                    | mmga_classfile::DependencyType::Optional
                        if !in_range =>
                    {
                        return Err(format!(
                            "mod `{owner}` requires `{}` {}, but {} is loaded",
                            dep.mod_id, dep.version_range, actual
                        ));
                    }
                    mmga_classfile::DependencyType::Incompatible
                        if dep.version_range.is_empty() || in_range =>
                    {
                        return Err(format!(
                            "mod `{owner}` is incompatible with `{}` {} (loaded {actual})",
                            dep.mod_id, dep.version_range
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn manifest_for(jar: &ModJar) -> Result<ModsToml, String> {
    if let Some(toml) = &jar.mods_toml {
        return Ok(toml.clone());
    }
    let entrypoints = jar.discover_entrypoints();
    if entrypoints.is_empty() {
        return Err(format!(
            "{}: no neoforge.mods.toml and no @Mod class",
            jar.source_name
        ));
    }
    Ok(ModsToml {
        mod_loader: ModLoaderKind::JavaFml,
        loader_version: "[0,)".into(),
        license: "All Rights Reserved".into(),
        mods: entrypoints
            .iter()
            .map(|ep| ModEntry {
                mod_id: ep.mod_id.clone(),
                namespace: ep.mod_id.clone(),
                version: "0.0.0".into(),
                display_name: ep.mod_id.clone(),
                description: String::new(),
                authors: String::new(),
                enum_extensions: None,
            })
            .collect(),
        mixins: Vec::new(),
        access_transformers: Vec::new(),
        dependencies: BTreeMap::new(),
    })
}

#[derive(Clone, Copy)]
enum CtorParam {
    Bus,
    Container,
    Dist,
}

fn pick_constructor(descriptors: &[String]) -> Result<(String, Vec<CtorParam>), String> {
    let mut best: Option<(String, Vec<CtorParam>)> = None;
    for desc in descriptors {
        let Ok(params) = injectable_params(desc) else {
            continue;
        };
        let replace = best
            .as_ref()
            .is_none_or(|(_, have)| params.len() > have.len());
        if replace {
            best = Some((desc.clone(), params));
        }
    }
    best.ok_or_else(|| {
        format!(
            "no supported mod constructor among [{}]",
            descriptors.join(", ")
        )
    })
}

fn injectable_params(desc: &str) -> Result<Vec<CtorParam>, ()> {
    let (params, _) = parse_method_descriptor(desc).map_err(|_| ())?;
    params
        .into_iter()
        .map(|param| match param {
            DescriptorType::Object(name) if name == "net/neoforged/bus/api/IEventBus" => {
                Ok(CtorParam::Bus)
            }
            DescriptorType::Object(name) if name == "net/neoforged/fml/ModContainer" => {
                Ok(CtorParam::Container)
            }
            DescriptorType::Object(name) if name == "net/neoforged/api/distmarker/Dist" => {
                Ok(CtorParam::Dist)
            }
            _ => Err(()),
        })
        .collect()
}

fn holder_class_for(registry_key: &str) -> &'static str {
    match registry_key {
        "minecraft:block" => "net/neoforged/neoforge/registries/DeferredBlock",
        "minecraft:item" => "net/neoforged/neoforge/registries/DeferredItem",
        _ => "net/neoforged/neoforge/registries/DeferredHolder",
    }
}

fn event_class_of_listener(rt: &Rt, callback: V) -> Option<String> {
    let obj = rt.get_obj(callback)?;
    let Data::Closure(closure) = &obj.data else {
        return None;
    };
    object_param(&closure.instantiated_desc).or_else(|| object_param(&closure.sam_desc))
}

fn object_param(desc: &str) -> Option<String> {
    let params = desc.strip_prefix('(')?.split_once(')')?.0;
    let name = params.strip_prefix('L')?.strip_suffix(';')?;
    if name.is_empty() || params.len() != name.len() + 2 {
        return None;
    }
    Some(name.to_owned())
}

fn instance_field(rt: &Rt, obj: V, name: &str) -> Option<V> {
    match rt.get_obj(obj).map(|object| &object.data) {
        Some(Data::Fields(map)) => map.get(name).copied(),
        _ => None,
    }
}

fn content_properties(rt: &Rt, registry: &str, value: V) -> ContentProperties {
    let kind = if registry.ends_with("block") {
        "block"
    } else if registry.contains("item") {
        "item"
    } else {
        "object"
    };
    ContentProperties {
        kind: kind.into(),
        template: rt.class_of(value).unwrap_or(kind).to_owned(),
        hardness: instance_field(rt, value, "hardness").map(|v| v.f()),
        resistance: instance_field(rt, value, "resistance").map(|v| v.f()),
        max_stack_size: instance_field(rt, value, "maxStackSize").map(|v| v.i()),
        ..ContentProperties::default()
    }
}

fn resource_location(rt: &mut Rt, namespace: &str, path: &str) -> V {
    rt.new_pinned_handle(
        "net/minecraft/resources/ResourceLocation",
        "resource_location",
        0,
        format!("{namespace}:{path}"),
    )
}

fn resource_id(rt: &Rt, value: V) -> Option<String> {
    if let Some(("resource_location", _, tag, _)) = rt.handle_of(value) {
        return Some(tag.to_owned());
    }
    rt.str_of(rt.get_field(value, "id")).map(str::to_owned)
}

fn handle_tag(rt: &Rt, value: V) -> Option<String> {
    rt.handle_of(value).map(|(_, _, tag, _)| tag.to_owned())
}

fn dispatch_maven_artifact_version(rt: &mut Rt, raw: &str, method: &str) -> Option<V> {
    let base = raw.split('+').next().unwrap_or(raw);
    let (core, qualifier) = match base.split_once('-') {
        Some((c, q)) => (c, Some(q)),
        None => (base, None),
    };
    let mut nums = core
        .split('.')
        .map(|s| s.parse::<i32>().unwrap_or(0))
        .collect::<Vec<i32>>();
    while nums.len() < 4 {
        nums.push(0);
    }
    match method {
        "toString" => Some(rt.new_str(raw)),
        "getMajorVersion" => Some(V::I(nums[0])),
        "getMinorVersion" => Some(V::I(nums[1])),
        "getIncrementalVersion" => Some(V::I(nums[2])),
        "getBuildNumber" => Some(V::I(nums[3])),
        "getQualifier" => match qualifier {
            Some(q) => Some(rt.new_str(q)),
            None => Some(V::NULL),
        },
        _ => None,
    }
}

fn path_text(rt: &Rt, value: V) -> Option<String> {
    if let Some((kind, _, tag, _)) = rt.handle_of(value) {
        if kind == "nio_path" || kind == "io_file" || kind == "fml_path" {
            return Some(tag.to_owned());
        }
    }
    rt.str_of(rt.get_field(value, "path"))
        .map(str::to_owned)
        .or_else(|| rt.str_of(value).map(str::to_owned))
}

fn raise_if_pending(rt: &mut Rt, context: &str) -> Result<(), String> {
    match rt.take_exception() {
        Some(exc) => Err(format!("{context}: {}", rt.exception_summary(exc))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockCoord;
    use std::io::{Cursor, Write};
    use std::process::Command;

    fn empty_jar(name: &str, toml: &str) -> ModJar {
        ModJar {
            source_name: name.into(),
            manifest_headers: BTreeMap::new(),
            mods_toml: Some(ModsToml::parse(toml, None).unwrap()),
            classes: BTreeMap::new(),
            mixin_configs: BTreeMap::new(),
            services: BTreeMap::new(),
            resources: BTreeMap::new(),
            nested_jars: Vec::new(),
            nested_manifests: Vec::new(),
        }
    }

    #[test]
    fn rejects_missing_and_incompatible_dependencies() {
        let missing = empty_jar(
            "orphan.jar",
            r#"
            modLoader = "javafml"
            loaderVersion = "[4,)"
            license = "MIT"
            [[mods]]
            modId = "orphan"
            version = "1.0.0"
            [[dependencies.orphan]]
            modId = "missing_lib"
            type = "required"
            versionRange = "[1,)"
            ordering = "AFTER"
            side = "BOTH"
            "#,
        );
        let err = PreparedLoad::from_jars(vec![missing], true).unwrap_err();
        assert!(err.contains("missing_lib"), "{err}");

        let left = empty_jar(
            "left.jar",
            r#"
            modLoader = "javafml"
            loaderVersion = "[4,)"
            license = "MIT"
            [[mods]]
            modId = "left"
            version = "1"
            [[dependencies.left]]
            modId = "right"
            type = "incompatible"
            versionRange = "*"
            ordering = "NONE"
            side = "BOTH"
            "#,
        );
        let right = empty_jar(
            "right.jar",
            r#"
            modLoader = "javafml"
            loaderVersion = "[4,)"
            license = "MIT"
            [[mods]]
            modId = "right"
            version = "1"
            "#,
        );
        let err = PreparedLoad::from_jars(vec![left, right], true).unwrap_err();
        assert!(err.contains("incompatibil"), "{err}");
    }

    #[test]
    fn resource_location_round_trips_and_empty_string_is_detected() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let namespace = rt.intern_str("mekanism");
        let path = rt.intern_str("energy");
        let empty = rt.intern_str("");
        let linked = &prepared.linked;
        let id = {
            let mut ctx = ExecCtx {
                rt: &mut rt,
                linked,
                host: &mut prepared.loader,
                depth: 0,
            };
            let loc = invoke_member(
                &mut ctx,
                op::INVOKESTATIC,
                &MemberRef::new(
                    "net/minecraft/resources/ResourceLocation",
                    "fromNamespaceAndPath",
                    "(Ljava/lang/String;Ljava/lang/String;)Lnet/minecraft/resources/ResourceLocation;",
                ),
                &[namespace, path],
            );
            let text = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new(
                    "net/minecraft/resources/ResourceLocation",
                    "toString",
                    "()Ljava/lang/String;",
                ),
                &[loc],
            );
            assert!(
                invoke_member(
                    &mut ctx,
                    op::INVOKESTATIC,
                    &MemberRef::new(
                        "net/minecraft/util/StringUtil",
                        "isNullOrEmpty",
                        "(Ljava/lang/String;)Z",
                    ),
                    &[empty],
                )
                .i() != 0
            );
            ctx.rt.str_of(text).unwrap().to_owned()
        };
        assert_eq!(id, "mekanism:energy");
        assert!(rt.take_exception().is_none());
    }

    #[test]
    fn constructor_parameter_order_follows_the_descriptor() {
        let (desc, params) = pick_constructor(&[
            ("(Lnet/neoforged/fml/ModContainer;Lnet/neoforged/bus/api/IEventBus;)V".into()),
        ])
        .unwrap();
        assert!(desc.contains("ModContainer"));
        assert!(matches!(params[0], CtorParam::Container));
        assert!(matches!(params[1], CtorParam::Bus));
    }

    #[test]
    fn embedded_jar_in_jar_mod_satisfies_a_required_dependency() {
        let mut nested = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut nested);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/neoforge.mods.toml", opts).unwrap();
            zip.write_all(
                br#"
modLoader = "javafml"
loaderVersion = "[0,)"
license = "MIT"
[[mods]]
modId = "ponder"
version = "1.0.82+mc1.21.1"
[[dependencies.ponder]]
modId = "neoforge"
type = "required"
versionRange = "[21.1.206,)"
side = "BOTH"
[[dependencies.ponder]]
modId = "minecraft"
type = "required"
versionRange = "[1.21.1]"
side = "BOTH"
"#,
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let mut outer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut outer);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/jarjar/ponder.jar", opts).unwrap();
            zip.write_all(nested.get_ref()).unwrap();
            zip.start_file("META-INF/neoforge.mods.toml", opts).unwrap();
            zip.write_all(
                br#"
modLoader = "javafml"
loaderVersion = "[0,)"
license = "MIT"
[[mods]]
modId = "create"
version = "6.0.10"
[[dependencies.create]]
modId = "neoforge"
type = "required"
versionRange = "[21.1.219,)"
side = "BOTH"
[[dependencies.create]]
modId = "minecraft"
type = "required"
versionRange = "[1.21.1]"
side = "BOTH"
[[dependencies.create]]
modId = "ponder"
type = "required"
versionRange = "[1.0.82,)"
ordering = "AFTER"
side = "BOTH"
[[dependencies.create]]
modId = "flywheel"
type = "required"
versionRange = "[1.0.0,2.0)"
ordering = "AFTER"
side = "CLIENT"
"#,
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let jar = ModJar::from_bytes("create-fixture.jar", outer.get_ref()).unwrap();
        assert!(jar
            .nested_manifests
            .iter()
            .any(|manifest| manifest.mods.iter().any(|entry| entry.mod_id == "ponder")));
        let prepared = PreparedLoad::from_jars(vec![jar], true).unwrap();
        let ids: Vec<_> = prepared
            .loader
            .mods
            .iter()
            .map(|loaded| loaded.mod_id.as_str())
            .collect();
        assert_eq!(ids, vec!["ponder", "create"]);
    }

    #[test]
    fn loads_compiled_pulse_mod_through_fml() {
        let jar_path = match compile_pulse_jar() {
            Some(path) => path,
            None => return,
        };
        let jar = ModJar::from_path(&jar_path).unwrap();
        let _ = std::fs::remove_dir_all(jar_path.parent().unwrap());

        let mut prepared = PreparedLoad::from_jars(vec![jar], true).unwrap();
        assert_eq!(prepared.loader.mods.len(), 1);
        assert_eq!(prepared.loader.mods[0].mod_id, "pulse");
        assert_eq!(prepared.loader.mods[0].version, "1.0.0");

        let mut rt = Rt::new();
        prepared.bootstrap(&mut rt).unwrap();
        assert_eq!(prepared.loader.phase, LoadPhase::Running);
        assert_eq!(
            prepared.loader.trace,
            [
                "discovered",
                "constructed",
                "registered",
                "common-setup",
                "sided-setup",
                "load-complete",
                "server-started",
                "running",
            ]
        );
        assert!(prepared.loader.mods[0].instance.is_some());

        let block = prepared
            .loader
            .state
            .registries
            .lookup("block", "pulse:pulse_block")
            .unwrap();
        assert_eq!(block.mod_id, "pulse");
        assert_eq!(block.properties.hardness, Some(2.5));
        let item = prepared
            .loader
            .state
            .registries
            .lookup("item", "pulse:pulse_item")
            .unwrap();
        assert_eq!(item.properties.max_stack_size, Some(16));
        assert!(prepared.loader.state.registries.frozen);

        let lines = rt
            .get_static("com/example/pulse/PulseLog", "LINES")
            .expect("PulseLog.LINES");
        assert_eq!(rt.value_to_string(lines), "[common-setup, server-started]");
        assert_eq!(
            prepared
                .loader
                .state
                .world
                .get_block(&BlockCoord::overworld(0, 64, 0)),
            "pulse:pulse_block"
        );

        prepared
            .loader
            .tick_server(&mut rt, &prepared.linked, 3)
            .unwrap();
        assert_eq!(prepared.loader.state.world.game_time, 3);
        prepared
            .loader
            .stop_server(&mut rt, &prepared.linked)
            .unwrap();
        assert_eq!(prepared.loader.phase, LoadPhase::Stopped);

        assert!(
            rt.telemetry.unmapped_calls.is_empty(),
            "unmapped host calls: {:?}",
            rt.telemetry.unmapped_calls
        );
    }

    fn compile_pulse_jar() -> Option<std::path::PathBuf> {
        if Command::new("javac").arg("-version").output().is_err() {
            return None;
        }
        let root = std::env::temp_dir().join(format!("mmga-pulse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let src = root.join("src");
        let classes = root.join("classes");
        let resources = root.join("resources");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&classes).unwrap();
        std::fs::create_dir_all(resources.join("META-INF")).unwrap();

        let files = [
            (
                "net/neoforged/fml/common/Mod.java",
                r#"package net.neoforged.fml.common;
                import java.lang.annotation.*;
                @Retention(RetentionPolicy.CLASS)
                public @interface Mod { String value(); }"#,
            ),
            (
                "net/neoforged/bus/api/EventPriority.java",
                r#"package net.neoforged.bus.api;
                public enum EventPriority { HIGHEST, HIGH, NORMAL, LOW, LOWEST }"#,
            ),
            (
                "net/neoforged/bus/api/SubscribeEvent.java",
                r#"package net.neoforged.bus.api;
                import java.lang.annotation.*;
                @Retention(RetentionPolicy.CLASS)
                public @interface SubscribeEvent {
                    EventPriority priority() default EventPriority.NORMAL;
                    boolean receiveCanceled() default false;
                }"#,
            ),
            (
                "net/neoforged/bus/api/IEventBus.java",
                r#"package net.neoforged.bus.api;
                import java.util.function.Consumer;
                public interface IEventBus {
                    <T> void addListener(Consumer<T> listener);
                }"#,
            ),
            (
                "net/neoforged/fml/common/EventBusSubscriber.java",
                r#"package net.neoforged.fml.common;
                import java.lang.annotation.*;
                @Retention(RetentionPolicy.CLASS)
                public @interface EventBusSubscriber {
                    String modid() default "";
                    Bus bus() default Bus.GAME;
                    enum Bus { GAME, MOD }
                }"#,
            ),
            (
                "net/neoforged/fml/event/lifecycle/FMLCommonSetupEvent.java",
                "package net.neoforged.fml.event.lifecycle; public class FMLCommonSetupEvent {}",
            ),
            (
                "net/neoforged/neoforge/event/server/ServerStartedEvent.java",
                "package net.neoforged.neoforge.event.server; public class ServerStartedEvent {}",
            ),
            (
                "net/neoforged/neoforge/registries/DeferredRegister.java",
                r#"package net.neoforged.neoforge.registries;
                import java.util.function.Supplier;
                import net.minecraft.world.item.Item;
                import net.minecraft.world.level.block.Block;
                import net.neoforged.bus.api.IEventBus;
                public class DeferredRegister<T> {
                    public static DeferredRegister<Block> createBlocks(String modid) { return null; }
                    public static DeferredRegister<Item> createItems(String modid) { return null; }
                    public DeferredHolder<T> register(String name, Supplier<? extends T> supplier) { return null; }
                    public void register(IEventBus bus) {}
                }"#,
            ),
            (
                "net/neoforged/neoforge/registries/DeferredHolder.java",
                r#"package net.neoforged.neoforge.registries;
                public class DeferredHolder<T> { public T get() { return null; } }"#,
            ),
            (
                "net/minecraft/world/level/block/Block.java",
                r#"package net.minecraft.world.level.block;
                public class Block {
                    public Block(Properties properties) {}
                    public static class Properties {
                        public static Properties of() { return new Properties(); }
                        public Properties strength(float hardness) { return this; }
                    }
                }"#,
            ),
            (
                "net/minecraft/world/item/Item.java",
                r#"package net.minecraft.world.item;
                public class Item {
                    public Item(Properties properties) {}
                    public static class Properties {
                        public Properties stacksTo(int count) { return this; }
                    }
                }"#,
            ),
            (
                "net/minecraft/server/level/ServerLevel.java",
                r#"package net.minecraft.server.level;
                public class ServerLevel {
                    public static void setBlock(int x, int y, int z, String blockId) {}
                }"#,
            ),
            (
                "com/example/pulse/PulseMod.java",
                r#"package com.example.pulse;
                import java.util.ArrayList;
                import net.minecraft.server.level.ServerLevel;
                import net.minecraft.world.item.Item;
                import net.minecraft.world.level.block.Block;
                import net.neoforged.bus.api.IEventBus;
                import net.neoforged.bus.api.SubscribeEvent;
                import net.neoforged.fml.common.EventBusSubscriber;
                import net.neoforged.fml.common.Mod;
                import net.neoforged.fml.event.lifecycle.FMLCommonSetupEvent;
                import net.neoforged.neoforge.event.server.ServerStartedEvent;
                import net.neoforged.neoforge.registries.DeferredHolder;
                import net.neoforged.neoforge.registries.DeferredRegister;

                @Mod("pulse")
                public class PulseMod {
                    public static final DeferredRegister<Block> BLOCKS = DeferredRegister.createBlocks("pulse");
                    public static final DeferredRegister<Item> ITEMS = DeferredRegister.createItems("pulse");
                    public static final DeferredHolder<Block> PULSE_BLOCK =
                        BLOCKS.register("pulse_block", () -> new Block(Block.Properties.of().strength(2.5f)));
                    public static final DeferredHolder<Item> PULSE_ITEM =
                        ITEMS.register("pulse_item", () -> new Item(new Item.Properties().stacksTo(16)));

                    public PulseMod(IEventBus modBus) {
                        BLOCKS.register(modBus);
                        ITEMS.register(modBus);
                        modBus.addListener(PulseMod::onCommonSetup);
                    }

                    private static void onCommonSetup(FMLCommonSetupEvent event) {
                        PulseLog.line("common-setup");
                    }
                }

                @EventBusSubscriber(modid = "pulse", bus = EventBusSubscriber.Bus.GAME)
                class PulseGameEvents {
                    @SubscribeEvent
                    public static void onStarted(ServerStartedEvent event) {
                        PulseLog.line("server-started");
                        ServerLevel.setBlock(0, 64, 0, "pulse:pulse_block");
                    }
                }

                class PulseLog {
                    public static final ArrayList<String> LINES = new ArrayList<String>();
                    public static void line(String text) { LINES.add(text); }
                }"#,
            ),
        ];

        let mut java_paths = Vec::new();
        for (rel, body) in files {
            let path = src.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, body).unwrap();
            java_paths.push(path);
        }
        let javac = Command::new("javac")
            .arg("-d")
            .arg(&classes)
            .args(&java_paths)
            .output()
            .unwrap();
        assert!(
            javac.status.success(),
            "javac failed: {}",
            String::from_utf8_lossy(&javac.stderr)
        );

        std::fs::write(
            resources.join("META-INF/neoforge.mods.toml"),
            r#"
modLoader = "javafml"
loaderVersion = "[4,)"
license = "MIT"

[[mods]]
modId = "pulse"
version = "1.0.0"
displayName = "Pulse"
description = "Sample NeoForge mod loaded by mmga"

[[dependencies.pulse]]
modId = "neoforge"
type = "required"
versionRange = "[21,)"
ordering = "NONE"
side = "BOTH"
"#,
        )
        .unwrap();

        let jar_path = root.join("pulse.jar");
        let packed = Command::new("jar")
            .arg("cf")
            .arg(&jar_path)
            .arg("-C")
            .arg(&classes)
            .arg("com")
            .arg("-C")
            .arg(&resources)
            .arg("META-INF")
            .output()
            .unwrap();
        assert!(
            packed.status.success(),
            "jar failed: {}",
            String::from_utf8_lossy(&packed.stderr)
        );
        Some(jar_path)
    }

    #[test]
    fn modeled_api_values_preserve_reflective_assignability() {
        let mut prepared = PreparedLoad::from_jars(Vec::new(), true).unwrap();
        let mut rt = Rt::new();
        let mut ctx = ExecCtx {
            rt: &mut rt,
            linked: &prepared.linked,
            host: &mut prepared.loader,
            depth: 0,
        };
        for (owner, field, desc, parents, unrelated) in [
            (
                "net/minecraft/network/codec/ByteBufCodecs",
                "INT",
                "Lnet/minecraft/network/codec/StreamCodec;",
                &[
                    "net/minecraft/network/codec/StreamEncoder",
                    "net/minecraft/network/codec/StreamDecoder",
                ][..],
                "net/minecraft/network/codec/StreamMemberEncoder",
            ),
            (
                "net/minecraft/core/Direction",
                "NORTH",
                "Lnet/minecraft/core/Direction;",
                &[
                    "java/lang/Enum",
                    "java/lang/Comparable",
                    "java/io/Serializable",
                    "net/minecraft/util/StringRepresentable",
                ][..],
                "net/minecraft/world/item/ArmorItem$Type",
            ),
            (
                "net/minecraft/world/item/ArmorItem$Type",
                "HELMET",
                "Lnet/minecraft/world/item/ArmorItem$Type;",
                &["java/lang/Enum", "net/minecraft/util/StringRepresentable"][..],
                "net/minecraft/core/Direction",
            ),
            (
                "net/minecraft/world/entity/EquipmentSlot",
                "HEAD",
                "Lnet/minecraft/world/entity/EquipmentSlot;",
                &["java/lang/Enum", "net/minecraft/util/StringRepresentable"][..],
                "net/minecraft/world/entity/EquipmentSlot$Type",
            ),
            (
                "net/minecraft/world/entity/EquipmentSlot$Type",
                "HUMANOID_ARMOR",
                "Lnet/minecraft/world/entity/EquipmentSlot$Type;",
                &["java/lang/Enum", "java/io/Serializable"][..],
                "net/minecraft/util/StringRepresentable",
            ),
        ] {
            let instance = mmga_runtime::read_static_field(&mut ctx, owner, field, desc);
            assert!(ctx.rt.pending_exception.is_none(), "missing {owner}.{field}");
            let actual_class = invoke_member(
                &mut ctx,
                op::INVOKEVIRTUAL,
                &MemberRef::new("java/lang/Object", "getClass", "()Ljava/lang/Class;"),
                &[instance],
            );
            for (parent, expected) in parents.iter().copied().map(|p| (p, V::TRUE))
                .chain(std::iter::once((unrelated, V::FALSE)))
            {
                let target = ctx.rt.new_pinned_handle("java/lang/Class", "class", 0, parent);
                let assignable = invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Class", "isAssignableFrom", "(Ljava/lang/Class;)Z"),
                    &[target, actual_class],
                );
                assert_eq!(assignable, expected, "{owner}.{field} assignable to {parent}");
                let is_instance = invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Class", "isInstance", "(Ljava/lang/Object;)Z"),
                    &[target, instance],
                );
                assert_eq!(is_instance, expected, "{owner}.{field} instanceof {parent}");
                assert_eq!(invoke_member(
                    &mut ctx,
                    op::INVOKEVIRTUAL,
                    &MemberRef::new("java/lang/Class", "isAssignableFrom", "(Ljava/lang/Class;)Z"),
                    &[actual_class, target],
                ), V::FALSE, "{parent} is not a subtype of the concrete value");
            }
        }
        let ranged = ctx.rt.new_pinned_handle(
            "java/lang/Class", "class", 0,
            "net/minecraft/world/entity/ai/attributes/RangedAttribute",
        );
        let attribute = ctx.rt.new_pinned_handle(
            "java/lang/Class", "class", 0,
            "net/minecraft/world/entity/ai/attributes/Attribute",
        );
        assert_eq!(invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Class", "isAssignableFrom", "(Ljava/lang/Class;)Z"),
            &[attribute, ranged],
        ), V::TRUE);
        assert_eq!(invoke_member(
            &mut ctx,
            op::INVOKEVIRTUAL,
            &MemberRef::new("java/lang/Class", "isAssignableFrom", "(Ljava/lang/Class;)Z"),
            &[ranged, attribute],
        ), V::FALSE);
        assert!(!ctx.linked.is_subtype("example/UnknownEnum", "java/lang/Enum", ctx.host));
        assert!(ctx.rt.pending_exception.is_none());
    }
}

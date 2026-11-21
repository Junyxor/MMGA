//! SDK host-contract tests; these do not start Horizon or validate gameplay.

use crate::{
    config::Config,
    runtime::{Action, State},
};
use mod_api::{
    self, Backend, BackendMut, BackendMutRef, BackendRef, BlockKey, BlockState, BlockStateRecord,
    BlockUpdateFlags, Hooks, Host, LevelRef, LevelWriteError, ModEntry, Registrar,
    RegistrationContext, ServerHost,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex, OnceLock},
    thread::{self, ThreadId},
};

const POSITION: [i32; 3] = [0, 300, 0];
const OAK: &str = "minecraft:oak_planks";
const SPRUCE: &str = "minecraft:spruce_planks";

struct Keys(Arc<Mutex<Vec<BlockKey>>>);
#[mod_api::__subscriptions]
impl Hooks for Keys {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        *self.0.lock().unwrap() = [OAK, SPRUCE].map(|id| registrar.block_key(id)).to_vec();
    }
}

struct ContractHost {
    thread: ThreadId,
    level: LevelRef,
    states: [BlockState; 2],
    blocks: BTreeMap<[i32; 3], BlockState>,
    writes: Vec<([i32; 3], BlockState, i32)>,
    refuse_write: bool,
    panic_write: bool,
}
impl ContractHost {
    fn new(keys: &[BlockKey]) -> Self {
        Self {
            thread: thread::current().id(),
            level: mod_api::testing::FakeLevel::default().level(),
            states: [keys[0], keys[1]].map(|key| BlockState::from(BlockStateRecord::new(key, 0))),
            blocks: BTreeMap::new(),
            writes: Vec::new(),
            refuse_write: false,
            panic_write: false,
        }
    }
    fn assert_owner(&self) {
        assert_eq!(thread::current().id(), self.thread);
    }
}
mod_api::impl_host_areas!(ContractHost, except(block_states, entities));
impl mod_api::prelude::BlockStatesHost for ContractHost {
    fn block_state_id(&self, name: &str) -> Option<BlockState> {
        self.assert_owner();
        [OAK, SPRUCE]
            .iter()
            .position(|id| *id == name)
            .map(|index| self.states[index])
    }
    fn block_state_string(&self, state: BlockState) -> Option<String> {
        self.assert_owner();
        self.states
            .iter()
            .position(|value| *value == state)
            .map(|index| [OAK, SPRUCE][index].into())
    }
}
impl mod_api::prelude::EntitiesHost for ContractHost {
    fn level(&self, dimension: &str) -> Option<LevelRef> {
        self.assert_owner();
        (dimension == "minecraft:overworld").then_some(self.level)
    }
}
impl Host for ContractHost {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(mod_api::__private::parts_installation::backend_ref(self))
    }
}
mod_api::impl_server_host_areas!(ContractHost);
impl ServerHost for ContractHost {
    fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
        Some(mod_api::__private::parts_installation::backend_mut_ref(
            self,
        ))
    }
}
mod_api::impl_backend_areas!(ContractHost, except(levels));
impl mod_api::prelude::LevelsBackend for ContractHost {
    fn level_block_state(&self, level: LevelRef, position: [i32; 3]) -> Option<BlockState> {
        self.assert_owner();
        (level == self.level)
            .then(|| self.blocks.get(&position).copied())
            .flatten()
    }
    fn level_game_time(&self, level: LevelRef) -> Option<i64> {
        self.assert_owner();
        (level == self.level).then_some(100)
    }
    fn level_day_time(&self, level: LevelRef) -> Option<i64> {
        self.assert_owner();
        (level == self.level).then_some(30)
    }
}
impl Backend for ContractHost {}
mod_api::impl_backend_mut_areas!(ContractHost, except(levels));
impl mod_api::prelude::LevelsBackendMut for ContractHost {
    fn level_set_block(
        &mut self,
        level: LevelRef,
        position: [i32; 3],
        state: BlockState,
        flags: BlockUpdateFlags,
    ) -> Result<bool, LevelWriteError> {
        self.assert_owner();
        if self.panic_write {
            panic!("contract host panic during world write");
        }
        if self.refuse_write {
            return Err(LevelWriteError::Unavailable);
        }
        if level != self.level {
            return Err(LevelWriteError::Stale);
        }
        assert!(self.states.contains(&state));
        self.writes.push((position, state, flags.bits()));
        if self.blocks.get(&position) == Some(&state) {
            return Ok(false);
        }
        self.blocks.insert(position, state);
        Ok(true)
    }
}
impl BackendMut for ContractHost {}

fn with_host(run: impl FnOnce(&mut ContractHost)) {
    let keys = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods =
        vec![ModEntry::new("sdk_contract", Box::new(Keys(keys.clone())))].into();
    mod_api::testing::with_mods(mods.clone(), || {
        mod_api::__private::part_tests::install_parts_fixture(&mods).unwrap();
        let mut host = ContractHost::new(&keys.lock().unwrap());
        run(&mut host);
    });
}
fn loaded_fixture() -> State {
    State::load(Config {
        jars: vec![fixture_jar()],
        dedicated: true,
    })
    .unwrap()
}
fn starting(state: &mut State, host: &mut ContractHost) {
    state.callback(host, Action::AboutToStart).unwrap();
    state.callback(host, Action::Starting).unwrap();
}

#[test]
fn java_lifecycle_reaches_the_sdk_world_and_keeps_its_worker_thread() {
    with_host(|host| {
        let mut state = loaded_fixture();
        assert!(state.status().contains("LoadComplete"));
        assert!(host.blocks.is_empty());
        starting(&mut state, host);
        assert!(host.blocks.is_empty(), "starting is not server-started");
        state.callback(host, Action::Started).unwrap();
        assert_eq!(host.blocks.get(&POSITION), Some(&host.states[0]));
        assert_eq!(
            host.writes,
            vec![(POSITION, host.states[0], 3), (POSITION, host.states[0], 3)]
        );
        state.callback(host, Action::TickPre).unwrap();
        state.callback(host, Action::TickPost).unwrap();
        assert_eq!(host.blocks.get(&POSITION), Some(&host.states[1]));
        assert_eq!(host.writes.last(), Some(&(POSITION, host.states[1], 3)));
        state.callback(host, Action::Stopping).unwrap();
        state.callback(host, Action::Stopped).unwrap();
        assert!(state.status().contains("Stopped"));
    });
}

#[test]
fn native_refusal_or_host_panic_cannot_be_reported_as_a_successful_java_write() {
    with_host(|host| {
        for panic in [false, true] {
            let mut state = loaded_fixture();
            starting(&mut state, host);
            host.refuse_write = !panic;
            host.panic_write = panic;
            let error = state.callback(host, Action::Started).unwrap_err();
            assert!(
                error.contains(if panic {
                    "Horizon host callback panicked"
                } else {
                    "Unavailable"
                }),
                "{error}"
            );
            assert!(host.blocks.is_empty());
            assert!(host.writes.is_empty());
            host.refuse_write = false;
            host.panic_write = false;
        }
    });
}

fn fixture_jar() -> PathBuf {
    static JAR: OnceLock<PathBuf> = OnceLock::new();
    JAR.get_or_init(|| {
        let directory = std::env::temp_dir().join(format!("mmga-horizon-contract-{}", std::process::id()));
        let source = directory.join("source");
        let classes = directory.join("classes");
        let resources = directory.join("resources/META-INF");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&classes).unwrap();
        std::fs::create_dir_all(&resources).unwrap();
        let files = [
            ("net/neoforged/fml/common/Mod.java", r#"package net.neoforged.fml.common;
                import java.lang.annotation.*; @Retention(RetentionPolicy.CLASS)
                public @interface Mod { String value(); }"#),
            ("net/neoforged/fml/common/EventBusSubscriber.java", r#"package net.neoforged.fml.common;
                import java.lang.annotation.*; @Retention(RetentionPolicy.CLASS)
                public @interface EventBusSubscriber { String modid(); Bus bus() default Bus.GAME;
                    enum Bus { GAME, MOD } }"#),
            ("net/neoforged/bus/api/SubscribeEvent.java", r#"package net.neoforged.bus.api;
                import java.lang.annotation.*; @Retention(RetentionPolicy.CLASS)
                public @interface SubscribeEvent {}"#),
            ("net/neoforged/neoforge/event/server/ServerStartedEvent.java", r#"package net.neoforged.neoforge.event.server;
                public class ServerStartedEvent { public net.minecraft.server.MinecraftServer getServer() { return null; } }"#),
            ("net/neoforged/neoforge/event/tick/ServerTickEvent.java", r#"package net.neoforged.neoforge.event.tick;
                public class ServerTickEvent { public static class Pre extends ServerTickEvent {}
                    public static class Post extends ServerTickEvent {} }"#),
            ("net/neoforged/neoforge/server/ServerLifecycleHooks.java", r#"package net.neoforged.neoforge.server;
                public class ServerLifecycleHooks { public static net.minecraft.server.MinecraftServer getCurrentServer() { return null; } }"#),
            ("net/minecraft/server/MinecraftServer.java", r#"package net.minecraft.server;
                public class MinecraftServer { public net.minecraft.server.level.ServerLevel overworld() { return null; } }"#),
            ("net/minecraft/server/level/ServerLevel.java", r#"package net.minecraft.server.level;
                public class ServerLevel {
                    public boolean setBlockAndUpdate(net.minecraft.core.BlockPos p, net.minecraft.world.level.block.state.BlockState s) { return false; }
                    public net.minecraft.world.level.block.state.BlockState getBlockState(net.minecraft.core.BlockPos p) { return null; }
                    public long getGameTime() { return 0; } public long getDayTime() { return 0; }
                }"#),
            ("net/minecraft/core/BlockPos.java", r#"package net.minecraft.core; public class BlockPos { public BlockPos(int x,int y,int z) {} }"#),
            ("net/minecraft/world/level/block/state/BlockState.java", r#"package net.minecraft.world.level.block.state; public class BlockState {}"#),
            ("net/minecraft/world/level/block/Block.java", r#"package net.minecraft.world.level.block;
                public class Block { public net.minecraft.world.level.block.state.BlockState defaultBlockState() { return null; } }"#),
            ("net/minecraft/world/level/block/Blocks.java", r#"package net.minecraft.world.level.block;
                public class Blocks { public static Block OAK_PLANKS; public static Block SPRUCE_PLANKS; }"#),
            ("contract/Guest.java", r#"package contract;
                import net.minecraft.core.BlockPos;
                import net.minecraft.server.level.ServerLevel;
                import net.minecraft.world.level.block.Blocks;
                import net.neoforged.fml.common.Mod;
                import net.neoforged.fml.common.EventBusSubscriber;
                import net.neoforged.bus.api.SubscribeEvent;
                import net.neoforged.neoforge.event.server.ServerStartedEvent;
                import net.neoforged.neoforge.event.tick.ServerTickEvent;
                import net.neoforged.neoforge.server.ServerLifecycleHooks;
                @Mod("contract")
                @EventBusSubscriber(modid="contract", bus=EventBusSubscriber.Bus.GAME)
                public class Guest {
                    private static final Thread OWNER = Thread.currentThread();
                    private static final ThreadLocal<Object> LOCAL = new ThreadLocal<Object>();
                    private static final Object MARKER = new Object();
                    public Guest() { LOCAL.set(MARKER); }
                    private static void sameThread() {
                        if (Thread.currentThread() != OWNER || LOCAL.get() != MARKER)
                            throw new IllegalStateException("Java runtime changed worker thread");
                    }
                    @SubscribeEvent public static void started(ServerStartedEvent event) {
                        sameThread();
                        ServerLevel level = event.getServer().overworld();
                        BlockPos pos = new BlockPos(0,300,0);
                        if (!level.setBlockAndUpdate(pos, Blocks.OAK_PLANKS.defaultBlockState()))
                            throw new IllegalStateException("first write did not change world");
                        if (level.setBlockAndUpdate(pos, Blocks.OAK_PLANKS.defaultBlockState()))
                            throw new IllegalStateException("unchanged write became true");
                    }
                    @SubscribeEvent public static void pre(ServerTickEvent.Pre event) {
                        sameThread();
                        ServerLevel level = ServerLifecycleHooks.getCurrentServer().overworld();
                        BlockPos pos = new BlockPos(0,300,0);
                        if (level.getBlockState(pos) != Blocks.OAK_PLANKS.defaultBlockState())
                            throw new IllegalStateException("read missed native world");
                        if (level.getGameTime() != 100 || level.getDayTime() != 30)
                            throw new IllegalStateException("read used simulated time instead of the SDK host");
                        if (!level.setBlockAndUpdate(pos, Blocks.SPRUCE_PLANKS.defaultBlockState()))
                            throw new IllegalStateException("tick write did not change world");
                    }
                    @SubscribeEvent public static void post(ServerTickEvent.Post event) { sameThread(); }
                }"#),
        ];
        let paths = files.iter().map(|(relative, text)| {
            let path = source.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            path
        }).collect::<Vec<_>>();
        let compiled = Command::new("javac").arg("-d").arg(&classes).args(&paths).output()
            .expect("SDK contract test requires javac; do not silently skip Java execution");
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
        std::fs::write(resources.join("neoforge.mods.toml"), r#"modLoader="javafml"
loaderVersion="[4,)"
license="MIT"
[[mods]]
modId="contract"
version="1.0.0"
"#).unwrap();
        let jar = directory.join("contract.jar");
        let packaged = Command::new("jar").arg("--create").arg("--file").arg(&jar)
            .arg("-C").arg(&classes).arg("contract")
            .arg("-C").arg(directory.join("resources")).arg("META-INF").output().unwrap();
        assert!(packaged.status.success(), "{}", String::from_utf8_lossy(&packaged.stderr));
        let parsed = mmga_classfile::ModJar::from_path(&jar).unwrap();
        assert_eq!(parsed.classes.len(), 1, "compile-only APIs must never be shipped or executed");
        assert!(parsed.classes.contains_key("contract/Guest"));
        jar
    }).clone()
}

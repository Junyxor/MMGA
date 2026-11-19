use crate::{config::Config, world};
use mmga_classfile::ModJar;
use mmga_neoforge::{
    host_world::{world_channel, HostWorld},
    PreparedLoad,
};
use mmga_runtime::Rt;
use mod_api::ServerHost;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;

#[derive(Default)]
pub struct State {
    worker: Option<Worker>,
    status: String,
    error: Option<String>,
}

struct Worker {
    jobs: Option<SyncSender<Job>>,
    thread: Option<JoinHandle<()>>,
}
struct Job {
    action: Action,
    world: Box<dyn HostWorld>,
    done: SyncSender<Result<String, String>>,
}
struct Session {
    prepared: PreparedLoad,
    rt: Rt,
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    AboutToStart,
    Starting,
    Started,
    TickPre,
    TickPost,
    Stopping,
    Stopped,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl State {
    pub fn load(config: Config) -> Result<Self, String> {
        let (jobs, requests) = mpsc::sync_channel(0);
        let (ready, result) = mpsc::sync_channel(0);
        let thread = std::thread::Builder::new()
            .name("MMGA Java server".into())
            .spawn(move || run(config, requests, ready))
            .map_err(|e| e.to_string())?;
        let worker = Worker {
            jobs: Some(jobs),
            thread: Some(thread),
        };
        match result
            .recv()
            .map_err(|_| "Java runtime initialization ended without a result".to_owned())?
        {
            Ok(status) => Ok(Self {
                worker: Some(worker),
                status,
                error: None,
            }),
            Err(error) => Err(error),
        }
    }
    pub fn fail(&mut self, error: String) {
        eprintln!("[MMGA] runtime stopped: {error}");
        self.error = Some(error);
        self.worker = None;
    }
    pub fn status(&self) -> String {
        if let Some(error) = &self.error {
            return format!("failed: {error}");
        }
        if self.worker.is_none() {
            "not initialized".into()
        } else {
            self.status.clone()
        }
    }
    pub fn callback(&mut self, host: &mut dyn ServerHost, action: Action) -> Result<(), String> {
        if self.error.is_some() {
            return Ok(());
        }
        let Some(worker) = self.worker.as_ref() else {
            return Ok(());
        };
        let (endpoint, requests) = world_channel();
        let (done, completion) = mpsc::sync_channel(0);
        worker
            .jobs
            .as_ref()
            .ok_or("Java worker is stopped")?
            .send(Job {
                action,
                world: endpoint,
                done,
            })
            .map_err(|_| "Java worker ended".to_owned())?;
        // Only owned requests cross threads; the real host stays on its callback thread.
        let host_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            while let Ok(request) = requests.recv() {
                world::serve(host, request);
            }
        }));
        // A host panic disconnects in-flight world replies before waiting for the VM.
        drop(requests);
        let result = completion
            .recv()
            .map_err(|_| "Java callback ended without a result".to_owned())?;
        if host_result.is_err() {
            return Err("Horizon host callback panicked".into());
        }
        self.status = result?;
        Ok(())
    }
}

fn run(config: Config, requests: Receiver<Job>, ready: SyncSender<Result<String, String>>) {
    let loaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Session::load(config)))
        .unwrap_or_else(|_| Err("Java runtime initialization panicked".into()));
    let mut session = match loaded {
        Ok(session) => session,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(session.status())).is_err() {
        return;
    }
    for job in requests {
        session.prepared.loader.host_world = Some(job.world);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.action(job.action)))
                .unwrap_or_else(|_| Err("Java runtime callback panicked".into()));
        // Disconnect the per-call world channel before returning completion or exiting.
        session.prepared.loader.host_world = None;
        let failed = result.is_err();
        let _ = job.done.send(result.map(|()| session.status()));
        if failed {
            break;
        }
    }
}

impl Session {
    fn load(config: Config) -> Result<Self, String> {
        let jars = config
            .jars
            .iter()
            .map(ModJar::from_path)
            .collect::<Result<Vec<_>, _>>()?;
        for jar in &jars {
            validate_jar(jar)?;
        }
        let mut prepared = PreparedLoad::from_jars(jars, config.dedicated)?;
        prepared.loader.native_world_required = true;
        let mut rt = Rt::new();
        prepared.loader.load_mods(&mut rt, &prepared.linked)?;
        let count = prepared.loader.state.registries.total_mod_entries();
        if count != 0 {
            return Err(format!(
                "MMGA cannot publish {count} guest registry entries to Horizon yet"
            ));
        }
        if !prepared.loader.state.network.channels.is_empty() {
            return Err("MMGA has no Horizon transport for guest payload channels".into());
        }
        for event in prepared.loader.state.bus.game_listener_events() {
            if !matches!(
                event.as_str(),
                "net/neoforged/neoforge/event/server/ServerAboutToStartEvent"
                    | "net/neoforged/neoforge/event/server/ServerStartingEvent"
                    | "net/neoforged/neoforge/event/server/ServerStartedEvent"
                    | "net/neoforged/neoforge/event/server/ServerStoppingEvent"
                    | "net/neoforged/neoforge/event/server/ServerStoppedEvent"
                    | "net/neoforged/neoforge/event/tick/ServerTickEvent$Pre"
                    | "net/neoforged/neoforge/event/tick/ServerTickEvent$Post"
                    | "net/neoforged/neoforge/event/RegisterCommandsEvent"
            ) {
                return Err(format!("MMGA has no Horizon event bridge for {event}"));
            }
        }
        Ok(Self { prepared, rt })
    }
    fn status(&self) -> String {
        format!("{:?}; Java mods={}; callbacks={}; virtual Minecraft 1.21.1 / Horizon 26.3; experimental bridge",
            self.prepared.loader.phase, self.prepared.loader.mods.len(), self.prepared.loader.state.bus.callbacks_invoked)
    }
    fn action(&mut self, action: Action) -> Result<(), String> {
        if matches!(action, Action::AboutToStart) {
            // Objects retained by Java cannot become handles for a different world's server.
            let old = self
                .rt
                .statics
                .iter()
                .filter(|(key, _)| key.starts_with("mmga/internal/NativeWorldHandles."))
                .map(|(key, value)| (key.clone(), *value))
                .collect::<Vec<_>>();
            for (key, value) in old {
                self.rt.statics.remove(&key);
                self.rt.pinned.remove(&value.r());
            }
        }
        let loader = &mut self.prepared.loader;
        let linked = &self.prepared.linked;
        match action {
            Action::AboutToStart => loader.server_about_to_start(&mut self.rt, linked),
            Action::Starting => {
                loader.server_starting(&mut self.rt, linked)?;
                if !loader.state.commands.roots.is_empty() {
                    return Err("MMGA cannot publish the guest command tree to Horizon yet".into());
                }
                Ok(())
            }
            Action::Started => loader.server_started(&mut self.rt, linked),
            Action::TickPre => loader.server_tick_pre(&mut self.rt, linked),
            Action::TickPost => loader.server_tick_post(&mut self.rt, linked),
            Action::Stopping => loader.server_stopping(&mut self.rt, linked),
            Action::Stopped => loader.server_stopped(&mut self.rt, linked),
        }
    }
}

fn validate_jar(jar: &ModJar) -> Result<(), String> {
    if !jar.nested_jars.is_empty() {
        return Err(format!(
            "{} contains nested jars whose game transformations have not been audited for Horizon",
            jar.source_name
        ));
    }
    let manifests = jar.mods_toml.iter().chain(&jar.nested_manifests);
    for manifest in manifests {
        if !manifest.mixins.is_empty()
            || !manifest.access_transformers.is_empty()
            || manifest
                .mods
                .iter()
                .any(|entry| entry.enum_extensions.is_some())
        {
            return Err(format!("{} needs Java mixins, access transformers or enum extensions unsupported by Horizon", jar.source_name));
        }
    }
    if !jar.mixin_configs.is_empty()
        || jar.manifest_headers.keys().any(|key| {
            matches!(
                key.to_ascii_lowercase().as_str(),
                "mixinconfigs"
                    | "fmlat"
                    | "fmlcoreplugin"
                    | "fmlcoreplugincontainsfmlmod"
                    | "premain-class"
                    | "agent-class"
            )
        })
        || jar.resources.keys().any(|path| {
            let path = path.to_ascii_lowercase();
            path.ends_with(".mixins.json")
                || path.starts_with("mixins.") && path.ends_with(".json")
                || path == "meta-inf/accesstransformer.cfg"
                || path == "meta-inf/coremods.json"
                || path.starts_with("meta-inf/services/cpw.mods.modlauncher.api.")
                || path.starts_with("meta-inf/services/net.neoforged.neoforgespi.earlywindow.")
        })
    {
        return Err(format!(
            "{} requires a Java game transformation that MMGA cannot apply to Horizon",
            jar.source_name
        ));
    }
    Ok(())
}

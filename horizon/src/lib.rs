//! Native Horizon entry point for MMGA's explicitly supported Java API subset.

mod config;
mod runtime;
mod world;

#[cfg(test)]
mod tests;

use mod_api::{
    self, Command, Flow, Hooks, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerAboutToStartEvent, ServerHost, ServerStartedEvent,
    ServerStartingEvent, ServerStoppedEvent, ServerStoppingEvent, ServerTickEvent, ServerTickPhase,
    Text,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Default)]
struct Mod {
    state: Mutex<runtime::State>,
    interrupted: AtomicBool,
}

impl Mod {
    fn callback(&self, host: &mut dyn ServerHost, action: runtime::Action) {
        if !host.on("mmga") {
            return;
        }
        let Ok(mut state) = self.state.try_lock() else {
            self.interrupted.store(true, Ordering::Release);
            eprintln!("[MMGA] refused a reentrant runtime callback");
            return;
        };
        if self.interrupted.swap(false, Ordering::AcqRel) {
            state.fail("A concurrent or reentrant Horizon callback was refused".into());
            return;
        }
        if let Err(error) = state.callback(host, action) {
            state.fail(error);
        }
        if self.interrupted.swap(false, Ordering::AcqRel) {
            state.fail("A concurrent or reentrant Horizon callback was refused".into());
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, _registrar: &mut dyn Registrar, context: &RegistrationContext) {
        let result = config::Config::read(context).and_then(runtime::State::load);
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match result {
            Ok(loaded) => *state = loaded,
            Err(error) => {
                context.refuse(&error);
                state.fail(error);
            }
        }
    }

    fn server_about_to_start(
        &self,
        host: &mut dyn ServerHost,
        _event: &mut ServerAboutToStartEvent,
    ) {
        self.callback(host, runtime::Action::AboutToStart);
    }
    fn server_starting(&self, host: &mut dyn ServerHost, _event: &mut ServerStartingEvent) {
        self.callback(host, runtime::Action::Starting);
    }
    fn server_started(&self, host: &mut dyn ServerHost, _event: &mut ServerStartedEvent) {
        self.callback(host, runtime::Action::Started);
    }
    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        let action = match event.phase {
            ServerTickPhase::ServerTickStart { .. } => runtime::Action::TickPre,
            ServerTickPhase::ServerTickEnd { .. } => runtime::Action::TickPost,
            _ => return,
        };
        self.callback(host, action);
    }
    fn server_stopping(&self, host: &mut dyn ServerHost, _event: &mut ServerStoppingEvent) {
        self.callback(host, runtime::Action::Stopping);
    }
    fn server_stopped(&self, host: &mut dyn ServerHost, _event: &mut ServerStoppedEvent) {
        self.callback(host, runtime::Action::Stopped);
    }
    fn register_commands(&self, host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        if !host.on("mmga") {
            return;
        }
        event.commands.push(Command::op("mmga status", 2));
    }
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if !host.on("mmga") {
            return Flow::Continue;
        }
        if event
            .command
            .literals
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["mmga", "status"]
        {
            return Flow::Continue;
        }
        let status = self
            .state
            .try_lock()
            .map(|state| state.status())
            .unwrap_or_else(|_| "runtime callback is active".to_owned());
        let _ = host
            .commands()
            .feedback(Text::literal(format!("MMGA: {status}")));
        Flow::Cancel
    }
}

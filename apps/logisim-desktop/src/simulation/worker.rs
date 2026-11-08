//! The GUI sends ordered intents and takes the latest immutable presentation.
//! Only the worker owns the simulator; no simulation lock is held by the GUI.
mod engine;
mod propagation;
#[cfg(test)]
mod tests;
use super::Host;
use logisim_core::{Id, MemoryContents, Project};
use logisim_plugin::Registry;
use logisim_ui::{Language, SimulationAction, SimulationMode, SimulationView, Workspace};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const DISPLAY_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

#[derive(Clone)]
struct Context {
    revision: u64,
    project: Arc<Project>,
    root: Id,
    circuit: Id,
    path: Vec<Id>,
    language: Language,
    mode: SimulationMode,
    plugins: Registry,
}
enum Command {
    Configure(Context),
    Action(u64, SimulationAction),
    Memory(u64, Vec<Id>, Id, MemoryContents),
    Suspend(bool),
    NewDocument,
}
struct Snapshot {
    revision: u64,
    view: SimulationView,
    total_ticks: u64,
}
#[derive(Default)]
struct Inbox {
    commands: VecDeque<Command>,
    stop: bool,
}
#[derive(Default)]
struct Shared {
    inbox: Mutex<Inbox>,
    wake: Condvar,
    latest: Mutex<Option<Snapshot>>,
}
impl Shared {
    fn send(&self, command: Command) {
        let discarded = {
            let mut inbox = self.inbox.lock().unwrap();
            // Superseded edits may be skipped, but never cross an action barrier.
            let discarded = if matches!(command, Command::Configure(_))
                && matches!(inbox.commands.back(), Some(Command::Configure(_)))
            {
                inbox.commands.pop_back()
            } else {
                None
            };
            inbox.commands.push_back(command);
            discarded
        };
        self.wake.notify_one();
        drop(discarded); // Potentially large documents are released outside the lock.
    }
}

pub(crate) struct Worker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    context: Option<Context>,
    revision: u64,
    plugins: Registry,
    suspended: bool,
    ready_memories: BTreeSet<Id>,
    reported_ticks: u64,
}
impl Worker {
    pub fn new(repaint: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(Shared::default());
        let service = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("logisim-simulation".into())
            .spawn(move || engine::run(service, repaint))
            .expect("start simulation worker");
        Self {
            shared,
            thread: Some(thread),
            context: None,
            revision: 0,
            plugins: Registry::default(),
            suspended: false,
            ready_memories: BTreeSet::new(),
            reported_ticks: 0,
        }
    }
    /// Clone a document only on edits, not at the render or simulation frequency.
    pub fn synchronize(&mut self, workspace: &mut Workspace, suspended: bool) {
        if self.stopped() {
            return;
        }
        if self.suspended != suspended {
            self.suspended = suspended;
            self.shared.send(Command::Suspend(suspended));
        }
        let previous = self.context.as_ref();
        let same_project = previous.is_some_and(|c| *c.project == *workspace.project());
        let same_view = same_project
            && previous.is_some_and(|c| {
                c.root == workspace.simulation_root()
                    && c.circuit == workspace.active_circuit()
                    && c.path == workspace.simulation_path()
                    && c.language == workspace.language()
                    && c.plugins.same(&self.plugins)
            });
        if same_view && previous.is_some_and(|c| c.mode == workspace.simulation_mode()) {
            return;
        }
        self.revision += 1;
        let context = Context {
            revision: self.revision,
            project: if same_project {
                Arc::clone(&previous.unwrap().project)
            } else {
                Arc::new(workspace.project().clone())
            },
            root: workspace.simulation_root(),
            circuit: workspace.active_circuit(),
            path: workspace.simulation_path().to_vec(),
            language: workspace.language(),
            mode: workspace.simulation_mode(),
            plugins: self.plugins.clone(),
        };
        if !same_view {
            self.ready_memories.clear();
            // IDs can be reused by a new document/root: never paint previous values
            // on the newly selected circuit while its first snapshot is in flight.
            let previous_view = std::mem::take(workspace.simulation_mut());
            workspace.set_simulation(SimulationView {
                enabled: previous_view.enabled,
                automatic_ticks: previous_view.automatic_ticks,
                tick_hz: if previous_view.tick_hz == 0 {
                    logisim_ui::DEFAULT_TICK_HZ
                } else {
                    previous_view.tick_hz
                },
                ..Default::default()
            });
        }
        self.shared.send(Command::Configure(context.clone()));
        self.context = Some(context);
    }
    pub fn action(&self, action: SimulationAction) {
        if self.context.is_some() && !self.stopped() {
            self.shared.send(Command::Action(self.revision, action));
        }
    }
    fn stopped(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    /// Consumes at most one snapshot; a slow frame never drains a render backlog.
    pub fn receive(&mut self, workspace: &mut Workspace) -> u64 {
        if self.stopped() {
            let message = logisim_ui::i18n::with_language(workspace.language(), || {
                logisim_ui::i18n::tr("Simulation worker stopped. Restart the application.")
                    .to_owned()
            });
            let view = workspace.simulation_mut();
            view.enabled = false;
            view.automatic_ticks = false;
            view.pending = 0;
            view.issues.clear();
            view.issues.push(logisim_ui::SimulationIssue {
                message,
                targets: Default::default(),
            });
            return 0;
        }
        let snapshot = self.shared.latest.lock().unwrap().take();
        let Some(snapshot) = snapshot else { return 0 };
        let ticks = snapshot.total_ticks.saturating_sub(self.reported_ticks);
        self.reported_ticks = snapshot.total_ticks;
        if snapshot.revision == self.revision && self.context.is_some() {
            self.ready_memories = snapshot.view.memories.keys().copied().collect();
            workspace.set_simulation(snapshot.view);
        }
        ticks
    }
}
impl Host for Worker {
    fn new_document(&mut self) {
        self.revision += 1;
        self.context = None;
        self.ready_memories.clear();
        self.plugins = Registry::default();
        if !self.stopped() {
            self.shared.send(Command::NewDocument);
        }
    }
    fn set_plugins(&mut self, plugins: Registry) {
        self.plugins = plugins;
    }
    fn replace_memory(&mut self, path: &[Id], id: Id, contents: MemoryContents) -> bool {
        if self.stopped()
            || !self.ready_memories.contains(&id)
            || !self.context.as_ref().is_some_and(|c| c.path == path)
        {
            return false;
        }
        self.shared
            .send(Command::Memory(self.revision, path.to_vec(), id, contents));
        true
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.inbox.lock().unwrap().stop = true;
        self.shared.wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

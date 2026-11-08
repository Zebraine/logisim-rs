use super::*;
use crate::{
    clocks::Clocks,
    simulation::{apply_project, present_project},
};
use logisim_sim::ProjectSimulator;
use std::time::Instant;

#[derive(Default)]
struct Engine {
    sim: ProjectSimulator,
    clocks: Clocks,
    context: Option<Context>,
    suspended: bool,
    dirty: bool,
    total_ticks: u64,
    unsettled_since: Option<Instant>,
}
impl Engine {
    fn command(&mut self, command: Command) {
        let before = self.sim.ticks();
        match command {
            Command::Configure(context) => {
                self.unsettled_since = None;
                self.sim.set_plugins(context.plugins.clone());
                if self.sim.synchronize(&context.project, context.root) {
                    self.clocks.suspend();
                }
                self.context = Some(context);
            }
            Command::Suspend(suspended) => {
                self.suspended = suspended;
                self.clocks.suspend();
            }
            Command::NewDocument => {
                self.sim = ProjectSimulator::default();
                self.context = None;
                self.unsettled_since = None;
                self.clocks.suspend();
            }
            Command::Action(revision, action) => {
                let Some(context) = &self.context else { return };
                if revision != context.revision {
                    return;
                }
                self.clocks.action(&action);
                apply_project(&mut self.sim, &context.path, action);
                self.total_ticks += self.sim.ticks().saturating_sub(before);
            }
            Command::Memory(revision, path, id, contents) => {
                if !self
                    .context
                    .as_ref()
                    .is_some_and(|c| c.revision == revision && c.path == path)
                {
                    return;
                }
                self.sim.replace_memory(&path, id, contents);
            }
        }
        self.dirty = true;
    }
    fn advance(&mut self) -> Option<Duration> {
        if self.suspended || self.context.is_none() {
            return None;
        }
        let before = (self.sim.ticks(), self.sim.steps());
        let delay = self.clocks.poll(Instant::now(), &mut self.sim);
        self.total_ticks += self.sim.ticks().saturating_sub(before.0);
        let started = Instant::now();
        // Finish pending manual/automatic propagation, yielding between batches so
        // pause and editor commands do not wait for an entire expensive clock edge.
        for _ in 0..4096 {
            if !self.sim.enabled() || self.sim.pending_count() == 0 || !self.sim.step() {
                break;
            }
            if started.elapsed() >= Duration::from_millis(2) {
                break;
            }
        }
        self.dirty |= before != (self.sim.ticks(), self.sim.steps());
        if self.sim.enabled()
            && !self.sim.limited()
            && self.sim.error().is_none()
            && self.sim.pending_count() > 0
        {
            Some(Duration::ZERO)
        } else {
            delay.map(|delay| delay.saturating_sub(started.elapsed()))
        }
    }
    fn snapshot_ready(&mut self, now: Instant) -> bool {
        if !self.sim.enabled()
            || self.suspended
            || self.sim.limited()
            || self.sim.error().is_some()
            || self.sim.pending_count() == 0
        {
            self.unsettled_since = None;
            return true;
        }
        // Short propagation transients should not toggle controls and status at
        // the display rate. Long-running propagation still needs visible progress.
        let since = *self.unsettled_since.get_or_insert(now);
        now.saturating_duration_since(since) >= Duration::from_millis(100)
    }
    fn snapshot(&mut self) -> Option<Snapshot> {
        let context = self.context.as_ref()?;
        let circuit = &context.project.circuit(context.circuit)?.circuit;
        let mut view = logisim_ui::i18n::with_language(context.language, || {
            present_project(circuit, &self.sim, &context.path)
        });
        self.clocks.present(&mut view);
        self.dirty = false;
        Some(Snapshot {
            revision: context.revision,
            view,
            total_ticks: self.total_ticks,
        })
    }
}

pub(super) fn run(shared: Arc<Shared>, repaint: impl Fn()) {
    let mut engine = Engine::default();
    let mut next_display = Instant::now();
    loop {
        let commands = {
            let mut inbox = shared.inbox.lock().unwrap();
            if inbox.stop {
                return;
            }
            std::mem::take(&mut inbox.commands)
        };
        for command in commands {
            engine.command(command);
        }
        // Observe a settled boundary before the scheduler can start another edge.
        // Paused/stepped and faulted states remain eligible even with pending work.
        let now = Instant::now();
        let ready = engine.snapshot_ready(now);
        if engine.dirty && now >= next_display && ready {
            if let Some(snapshot) = engine.snapshot() {
                let old = shared.latest.lock().unwrap().replace(snapshot);
                drop(old);
                repaint();
            }
            next_display = now + DISPLAY_INTERVAL;
        }
        let mut delay = engine.advance();
        if engine.dirty && engine.context.is_some() {
            // A final propagation batch may have settled without another clock
            // deadline. Wake for its snapshot instead of leaving the old view up.
            let display_delay = next_display.saturating_duration_since(Instant::now());
            delay = Some(delay.map_or(display_delay, |d| d.min(display_delay)));
        }
        let inbox = shared.inbox.lock().unwrap();
        if inbox.stop {
            return;
        }
        if !inbox.commands.is_empty() {
            continue;
        }
        match delay {
            Some(delay) if delay.is_zero() => {
                drop(inbox);
            }
            Some(delay) => {
                drop(shared.wake.wait_timeout(inbox, delay).unwrap());
            }
            None => {
                drop(shared.wake.wait(inbox).unwrap());
            }
        }
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;

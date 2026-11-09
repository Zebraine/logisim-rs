//! Wall-clock pacing belongs to the host. The simulator accepts deterministic integer ticks.
use logisim_sim::ProjectSimulator;
use logisim_ui::{DEFAULT_TICK_HZ, MAX_TICK_HZ, SimulationAction, SimulationView};
use std::time::{Duration, Instant};

pub struct Clocks {
    automatic: bool,
    hz: u32,
    deadline: Option<Instant>,
}
impl Default for Clocks {
    fn default() -> Self {
        Self {
            automatic: false,
            hz: DEFAULT_TICK_HZ,
            deadline: None,
        }
    }
}
impl Clocks {
    pub fn action(&mut self, action: &SimulationAction) {
        match action {
            SimulationAction::ToggleTicks => {
                self.automatic = !self.automatic;
                self.deadline = None;
            }
            SimulationAction::SetTickRate(hz) if (1..=MAX_TICK_HZ).contains(hz) => {
                self.hz = *hz;
                self.deadline = None;
            }
            SimulationAction::Reset | SimulationAction::ToggleEnabled => self.deadline = None,
            _ => {}
        }
    }
    pub fn present(&self, view: &mut SimulationView) {
        view.automatic_ticks = self.automatic;
        view.tick_hz = self.hz;
    }
    pub fn suspend(&mut self) {
        self.deadline = None;
    }
    pub fn poll(
        &mut self,
        now: Instant,
        sim: &mut ProjectSimulator,
        propagate: impl FnMut(&mut ProjectSimulator) -> bool,
    ) -> Option<Duration> {
        let started = Instant::now();
        self.poll_with_elapsed(now, sim, propagate, || started.elapsed())
    }
    fn poll_with_elapsed(
        &mut self,
        now: Instant,
        sim: &mut ProjectSimulator,
        mut propagate: impl FnMut(&mut ProjectSimulator) -> bool,
        mut elapsed: impl FnMut() -> Duration,
    ) -> Option<Duration> {
        if !self.automatic
            || !sim.enabled()
            || sim.limited()
            || sim.error().is_some()
            || !sim.has_clocks()
        {
            self.deadline = None;
            return None;
        }
        let period = Duration::from_secs_f64(1.0 / f64::from(self.hz));
        let mut due = *self.deadline.get_or_insert(now + period);
        // Absorb normal scheduling jitter, but never replay a stalled worker's backlog.
        if now.saturating_duration_since(due) > Duration::from_millis(50) {
            due = now;
        }
        let mut ticks = 0;
        let mut steps = 0;
        while due <= now {
            if !sim.tick() {
                due = now + period;
                break;
            }
            ticks += 1;
            due += period;
            // Every edge propagates fully before the next one. Bound both work and
            // elapsed time so high rates cannot delay queued commands on a costly circuit.
            while sim.pending_count() > 0
                && sim.enabled()
                && !sim.limited()
                && steps < 4096
                && elapsed() < Duration::from_millis(4)
            {
                steps += 1;
                if !propagate(sim) {
                    break;
                }
            }
            if sim.limited() || !sim.enabled() || sim.error().is_some() {
                self.suspend();
                return None;
            }
            if sim.pending_count() > 0
                || ticks >= 256
                || steps >= 4096
                || elapsed() >= Duration::from_millis(4)
            {
                due = now + elapsed() + period;
                break;
            }
        }
        self.deadline = Some(due);
        Some(due.saturating_duration_since(now + elapsed()))
    }
}

#[cfg(test)]
mod tests;

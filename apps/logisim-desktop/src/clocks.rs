//! Wall-clock pacing belongs to the host. The simulator accepts deterministic integer ticks.
use logisim_sim::ProjectSimulator;
use logisim_ui::{SimulationAction, SimulationView};
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
            hz: 1,
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
            SimulationAction::SetTickRate(hz) if [1, 2, 4, 8, 16, 32, 64, 128].contains(hz) => {
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
    pub fn poll(&mut self, now: Instant, sim: &mut ProjectSimulator) -> Option<Duration> {
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
        let due = *self.deadline.get_or_insert(now + period);
        if now >= due {
            // Never merge edges or replay a backlog after a stalled window or an expensive circuit.
            if sim.pending_count() == 0 {
                sim.tick();
            }
            self.deadline = Some(now + period);
        }
        Some(self.deadline.unwrap().saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::{Component, Kind, Point, Project};
    #[test]
    fn pacing_pauses_without_backlog_and_waits_for_propagation() {
        let mut p = Project::default();
        p.circuit_mut(p.main)
            .unwrap()
            .circuit
            .add_component(Component::new(Kind::Clock, Point::ZERO));
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, p.main);
        sim.advance(16);
        let mut clocks = Clocks::default();
        let start = Instant::now();
        assert_eq!(clocks.poll(start, &mut sim), None);
        clocks.action(&SimulationAction::ToggleTicks);
        assert_eq!(clocks.poll(start, &mut sim), Some(Duration::from_secs(1)));
        clocks.poll(start + Duration::from_millis(999), &mut sim);
        assert_eq!(sim.ticks(), 0);
        clocks.poll(start + Duration::from_secs(1), &mut sim);
        assert_eq!(sim.ticks(), 1);
        clocks.poll(start + Duration::from_secs(20), &mut sim);
        assert_eq!(sim.ticks(), 1); // pending edge
        sim.advance(16);
        clocks.poll(start + Duration::from_secs(21), &mut sim);
        assert_eq!(sim.ticks(), 2);
        sim.advance(16);
        sim.set_enabled(false);
        assert_eq!(clocks.poll(start + Duration::from_secs(22), &mut sim), None);
        sim.set_enabled(true);
        clocks.poll(start + Duration::from_secs(60), &mut sim);
        assert_eq!(sim.ticks(), 2);
        clocks.poll(start + Duration::from_secs(61), &mut sim);
        assert_eq!(sim.ticks(), 3);
        clocks.action(&SimulationAction::SetTickRate(4));
        sim.advance(16);
        assert_eq!(
            clocks.poll(start + Duration::from_secs(62), &mut sim),
            Some(Duration::from_millis(250))
        );
    }
}

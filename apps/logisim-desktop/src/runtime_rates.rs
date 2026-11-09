//! Wall-clock telemetry, independent of document state and simulation scheduling.
use logisim_ui::SimulationView;
use std::time::{Duration, Instant};

pub const REFRESH_INTERVAL: Duration = Duration::from_millis(250);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct RuntimeRates {
    baseline: Option<(Instant, u64)>,
    ticks: u64,
    fps: Option<f64>,
    tick_hz: f64,
}

impl RuntimeRates {
    /// Record only ticks actually accepted by the simulator. Restored circuit
    /// counters and resets must never be interpreted as work performed.
    pub fn record_ticks(&mut self, ticks: u64) {
        self.ticks = self.ticks.saturating_add(ticks);
    }

    pub fn present(&mut self, now: Instant, completed_frames: u64, view: &mut SimulationView) {
        let (started, frames) = *self.baseline.get_or_insert((now, completed_frames));
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= SAMPLE_INTERVAL {
            let seconds = elapsed.as_secs_f64();
            self.fps = Some(completed_frames.saturating_sub(frames) as f64 / seconds);
            self.tick_hz = self.ticks as f64 / seconds;
            self.ticks = 0;
            self.baseline = Some((now, completed_frames));
        }
        view.frames_per_second = self.fps;
        view.actual_tick_hz = self.tick_hz;
    }
}

#[cfg(test)]
mod tests;

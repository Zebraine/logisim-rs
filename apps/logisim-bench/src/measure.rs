//! Timers cover the simulator only. Four-state observation and checks run after each timed interval.
use crate::{
    Result,
    workload::{Fixture, Stimulus},
};
use logisim_core::{Bit, Value};
use logisim_sim::{ProjectSimulator, stable::Report};
use serde::Serialize;
use std::time::{Duration, Instant};

pub fn ms(t: Duration) -> f64 {
    t.as_secs_f64() * 1000.0
}

#[derive(Serialize)]
pub struct Distribution {
    pub p50: f64,
    pub p95: f64,
}

pub fn distribution(samples: &mut [Duration]) -> Distribution {
    samples.sort_unstable();
    let rank = |percent: usize| ms(samples[(samples.len() * percent).div_ceil(100) - 1]);
    Distribution {
        p50: rank(50usize),
        p95: rank(95),
    }
}

#[derive(Default, Debug, PartialEq, Eq, Serialize)]
pub struct Work {
    pub combinational_passes: usize,
    pub evaluated_instructions: usize,
    pub plugin_outputs: usize,
    pub plugin_samples: usize,
}
impl Work {
    fn include(&mut self, r: &Report) {
        self.combinational_passes += r.combinational_passes;
        self.evaluated_instructions += r.evaluated_instructions;
        self.plugin_outputs += r.evaluated_plugins;
        self.plugin_samples += r.sampled_plugins;
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct Trace {
    pub digest: u64,
    pub ticks: u64,
    pub steps: u64,
    pub work: Work,
}

// FNV-1a over width followed by one byte per four-state bit, in fixed probe and LSB-first order.
pub fn hash_value(hash: &mut u64, value: Value) {
    let mut absorb = |byte: u8| {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    };
    absorb(value.width());
    for i in 0..value.width() {
        absorb(match value.bit(i) {
            Bit::Zero => 0,
            Bit::One => 1,
            Bit::Unknown => 2,
            Bit::Error => 3,
        });
    }
}

pub struct Trial {
    pub trace: Trace,
    pub settles: Vec<Duration>,
    pub iterations: Vec<Duration>,
}

pub fn trial(f: &Fixture, sim: &mut ProjectSimulator) -> Result<Trial> {
    reset(f, sim)?;
    trace(f, sim)
}

pub fn reset(f: &Fixture, sim: &mut ProjectSimulator) -> Result<()> {
    sim.reset();
    sim.settle_compiled()?;
    f.stimulus.reset_inputs(sim)
}

pub fn trace(f: &Fixture, sim: &mut ProjectSimulator) -> Result<Trial> {
    let steps_at_start = sim.steps();
    let count = f.stimulus.iterations();
    let mut iterations = Vec::with_capacity(count);
    let mut settles = Vec::with_capacity(2 * count);
    let mut digest = 0xcbf29ce484222325;
    let mut work = Work::default();
    for i in 0..count {
        let start = Instant::now();
        f.stimulus.drive(sim, i);
        // Resolve the register-file data bus before the clock edge.
        let data_settle = if matches!(f.stimulus, Stimulus::Plugin { .. }) {
            let when = Instant::now();
            let report = sim.settle_compiled()?;
            Some((when.elapsed(), report))
        } else {
            None
        };
        if f.stimulus.clocked() && !sim.tick() {
            return Err(format!("Clock did not advance at iteration {i}").into());
        }
        let when = Instant::now();
        let report = sim.settle_compiled()?;
        let end = Instant::now();
        iterations.push(end - start);
        if let Some((time, report)) = data_settle {
            settles.push(time);
            work.include(&report);
        }
        settles.push(end - when);
        work.include(&report);
        let view = sim.view(&[]).ok_or("No root simulation")?;
        for &id in &f.probes {
            hash_value(
                &mut digest,
                view.port_value(id, 0).ok_or("Missing probe value")?,
            );
        }
    }
    let ticks = sim.ticks();
    let diagnostics = sim.diagnostics(&[]);
    let expected_ticks = if f.stimulus.clocked() {
        count as u64
    } else {
        0
    };
    let faults = sim.plugin_failures();
    if ticks != expected_ticks
        || sim.limited()
        || sim.pending_count() != 0
        || sim.error().is_some()
        || !faults.is_empty()
        || !diagnostics.is_empty()
    {
        return Err(format!(
            "Invalid final state: ticks={ticks}/{expected_ticks}, limited={}, pending={}, error={:?}, diagnostics={diagnostics:?}, plugin faults={faults:?}",
            sim.limited(), sim.pending_count(), sim.error()
        ).into());
    }
    Ok(Trial {
        trace: Trace {
            digest,
            ticks,
            steps: sim.steps() - steps_at_start,
            work,
        },
        settles,
        iterations,
    })
}

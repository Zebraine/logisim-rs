//! Run from the workspace root. See CONTRIBUTING.md for sampling and counter units.
#[cfg(feature = "engine-study")]
mod engine_study;
mod measure;
#[cfg(feature = "plugin-metrics")]
mod phases;
#[cfg(test)]
mod tests;
mod workload;

use logisim_sim::ProjectSimulator;
use measure::{distribution, ms, trial};
use serde_json::{Value, json};
use std::{
    process::ExitCode,
    time::{Duration, Instant},
};
use workload::Fixture;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WARMUP: usize = 3;

fn options(mut args: impl Iterator<Item = String>) -> Result<(String, usize)> {
    let name = args
        .next()
        .ok_or("Usage: logisim-bench <wide|deep|single-cycle|plugin> [--runs N]")?;
    let runs = match args.next().as_deref() {
        None => 20,
        Some("--runs") => args.next().ok_or("Missing --runs value")?.parse()?,
        Some(_) => return Err("Expected --runs N".into()),
    };
    if !(1..=1000).contains(&runs) || args.next().is_some() {
        return Err("Expected 1 to 1000 runs and a single workload".into());
    }
    Ok((name, runs))
}

struct Open {
    fixture: Fixture,
    sim: ProjectSimulator,
    sync_ms: f64,
    compile_ms: f64,
    compiled_nodes: usize,
    first_settle_ms: f64,
}

fn open(fixture: Result<Fixture>) -> Result<Open> {
    let fixture = fixture?;
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(fixture.plugins.clone());
    let syncing = Instant::now();
    sim.synchronize(&fixture.project, fixture.project.main);
    let sync_ms = ms(syncing.elapsed());
    let compiling = Instant::now();
    let plan = sim.prepare_compiled()?;
    let compile_ms = ms(compiling.elapsed());
    let initial = Instant::now();
    sim.settle_compiled()?;
    let first_settle_ms = ms(initial.elapsed());
    Ok(Open {
        fixture,
        sim,
        sync_ms,
        compile_ms,
        compiled_nodes: plan.compiled_nodes,
        first_settle_ms,
    })
}

fn benchmark(name: &str, runs: usize) -> Result<Value> {
    let start = Instant::now();
    let initialize = || open(Fixture::load(name, std::path::Path::new(".")));
    #[cfg(feature = "plugin-metrics")]
    let mut cold = logisim_plugin::metrics::Recording::default();
    #[cfg(feature = "plugin-metrics")]
    let initial = phases::capture(&mut cold, initialize);
    #[cfg(not(feature = "plugin-metrics"))]
    let initial = initialize();
    let Open {
        fixture,
        mut sim,
        sync_ms,
        compile_ms,
        compiled_nodes,
        first_settle_ms,
        ..
    } = initial?;
    let cold_ms = ms(start.elapsed());
    // Isolate a single stable VM instantiation on the already loaded module.
    // Synchronization and plan preparation above include their real runtime instance costs.
    let instance_ms = fixture
        .project
        .circuits
        .iter()
        .flat_map(|d| &d.circuit.components)
        .find_map(|c| c.plugin.as_ref())
        .map(|definition| -> Result<f64> {
            let start = Instant::now();
            let vm = fixture
                .plugins
                .instantiate_stable(definition)?
                .ok_or("Missing stable plugin capability")?;
            let time = ms(start.elapsed());
            drop(vm);
            Ok(time)
        })
        .transpose()?;
    let reference = trial(&fixture, &mut sim)?.trace;
    if reference.digest != fixture.stimulus.expected_digest() {
        return Err(format!(
            "Unexpected {name} four-state trace: {:016x}",
            reference.digest
        )
        .into());
    }
    let mut settles = Vec::new();
    let mut iterations = Vec::new();
    let mut per_run_rates = Vec::with_capacity(runs);
    #[cfg(feature = "plugin-metrics")]
    let mut periods = phases::Intervals::default();
    for i in 1..WARMUP + runs {
        #[cfg(feature = "plugin-metrics")]
        let sample = if i >= WARMUP {
            periods.setup(|| measure::reset(&fixture, &mut sim))?;
            periods.hot(|| measure::trace(&fixture, &mut sim))?
        } else {
            trial(&fixture, &mut sim)?
        };
        #[cfg(not(feature = "plugin-metrics"))]
        let sample = trial(&fixture, &mut sim)?;
        if sample.trace != reference {
            return Err(format!(
                "Trace divergence at run {i}: {:?} != {reference:?}",
                sample.trace
            )
            .into());
        }
        if i >= WARMUP {
            let duration: Duration = sample.iterations.iter().sum();
            per_run_rates.push(sample.iterations.len() as f64 / duration.as_secs_f64());
            settles.extend(sample.settles);
            iterations.extend(sample.iterations);
        }
    }
    // A 50/50 mixture of short and long edges can move its median between modes.
    // Split chronological samples before computing the pooled quantiles in place.
    let phases: &[&str] = match fixture.stimulus {
        workload::Stimulus::Plugin { .. } => &[
            "data_before_odd_tick",
            "odd_tick",
            "data_before_even_tick",
            "even_tick",
        ],
        workload::Stimulus::Cpu { .. } => &["odd_tick", "even_tick"],
        _ => &[],
    };
    let mut phase_times = serde_json::Map::new();
    for (offset, &phase) in phases.iter().enumerate() {
        let mut samples = settles
            .iter()
            .skip(offset)
            .step_by(phases.len())
            .copied()
            .collect::<Vec<_>>();
        phase_times.insert(phase.into(), json!(distribution(&mut samples)));
    }
    let elapsed: Duration = iterations.iter().sum();
    let rate = iterations.len() as f64 / elapsed.as_secs_f64();
    #[cfg(feature = "plugin-metrics")]
    let plugin_phases = json!({
        "schema": 1, "enabled": true, "cold": phases::report(&mut cold),
        "reset": phases::report(&mut periods.setup),
        "hot": phases::hot(&mut periods.hot, settles.iter().sum::<Duration>().as_nanos() as u64,
                           reference.work.plugin_outputs * runs, reference.work.plugin_samples * runs)?
    });
    #[cfg(not(feature = "plugin-metrics"))]
    let plugin_phases = json!({"schema": 1, "enabled": false});
    Ok(json!({
        "plugin_phases": plugin_phases,
        "schema": 1,
        "workload": name,
        "engine": "stable",
        "threads": 1,
        "runs": runs,
        "warmup": WARMUP,
        "cold_ms": cold_ms,
        "synchronize_ms": sync_ms,
        "compile_ms": compile_ms,
        "module_load_ms": fixture.module_load_ms,
        "plugin_instance_ms": instance_ms,
        "first_settle_ms": first_settle_ms,
        "compiled_nodes": compiled_nodes,
        "settle_ms": distribution(&mut settles),
        "phase_settle_ms": phase_times,
        "iteration_ms": distribution(&mut iterations),
        "settles_per_run": settles.len() / runs,
        "iterations_per_run": iterations.len() / runs,
        "iterations_per_s": rate,
        "per_run_iterations_per_s": per_run_rates,
        "ticks_per_s": if fixture.stimulus.clocked() { rate } else { 0.0 },
        "ticks": reference.ticks,
        "steps": reference.steps,
        "work_per_run": reference.work,
        "digest": format!("{:016x}", reference.digest),
        "diagnostics": 0
    }))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    #[cfg(feature = "engine-study")]
    if args.peek().is_some_and(|arg| arg == "engine-study") {
        args.next();
        return print_result(engine_study::run(args));
    }
    print_result(options(args.by_ref()).and_then(|(name, runs)| benchmark(&name, runs)))
}

fn print_result(result: Result<Value>) -> ExitCode {
    match result {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("logisim-bench: {err}");
            ExitCode::FAILURE
        }
    }
}

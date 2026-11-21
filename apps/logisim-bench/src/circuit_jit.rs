//! Pure-kernel timings and whole-project runs use the same expression plans and inputs.
use super::*;
use logisim_core::Value as Bus;
use logisim_sim::{
    combinational::CompiledCircuit,
    jit_experiment::{self, Mode, Stats},
};

#[derive(Default, serde::Serialize)]
pub(super) struct Profile {
    evaluator_ns: u64,
    native_region_ns: u64,
    evaluated_instructions: usize,
    native_instructions: usize,
    native_calls: usize,
}
impl Profile {
    pub(super) fn is_profiled(&self) -> bool {
        self.evaluator_ns > 0
    }
    pub(super) fn accumulate(&mut self, a: &Stats, b: &Stats) {
        self.evaluator_ns += b.evaluator_ns - a.evaluator_ns;
        self.native_region_ns += b.native_region_ns - a.native_region_ns;
        self.evaluated_instructions += b.evaluated_instructions - a.evaluated_instructions;
        self.native_instructions += b.native_instructions - a.native_instructions;
        self.native_calls += b.native_calls - a.native_calls;
    }
}

pub(super) fn run(mut args: impl Iterator<Item = String>, engine: WasmEngine) -> Result<Value> {
    let option = args.next();
    let mode = match option.as_deref() {
        Some("interpreter") => None,
        Some("native") => Some(Mode::Native),
        Some("verify") => Some(Mode::Verify),
        Some("profile-native") => Some(Mode::ProfileNative),
        Some("profile-interpreter") => Some(Mode::ProfileInterpreter),
        _ => {
            return Err(
                "Expected interpreter, native, verify, profile-native or profile-interpreter"
                    .into(),
            );
        }
    };
    let mut args = args.peekable();
    let pure = args.next_if(|arg| arg == "--evaluator-only").is_some();
    let (name, runs) = options(args)?;
    let execute = || {
        if pure {
            evaluator(&name, runs)
        } else {
            benchmark(&name, runs, engine)
        }
    };
    let mut report = if let Some(mode) = mode {
        let (report, stats) = jit_experiment::run(mode, execute);
        let mut report = report?;
        report["jit_total"] = json!(stats);
        report
    } else {
        execute()?
    };
    report["circuit_jit_mode"] = json!(option);
    Ok(report)
}

fn evaluator(name: &str, runs: usize) -> Result<Value> {
    if !matches!(name, "wide" | "deep") {
        return Err("Use wide or deep for the evaluator-only trace".into());
    }
    let f = Fixture::load_with(
        name,
        std::path::Path::new("."),
        logisim_plugin::Plugin::load,
    )?;
    let start = Instant::now();
    let circuit = &f.project.main_circuit().circuit;
    let compiled = CompiledCircuit::new(circuit);
    let mut ev = compiled.experiment_evaluator();
    let cold = ms(start.elapsed());
    let ids = match &f.stimulus {
        workload::Stimulus::Wide(ids) => ids.clone(),
        workload::Stimulus::Deep(id) => vec![*id],
        _ => unreachable!(),
    };
    let width = if name == "wide" { 32 } else { 1 };
    let mut inputs: std::collections::BTreeMap<_, _> = ids
        .iter()
        .map(|&id| ((id, 0), Bus::known(width, 0)))
        .collect();
    let mut times = Vec::new();
    let mut digest = None;
    for run in 0..WARMUP + runs {
        inputs.values_mut().for_each(|v| *v = Bus::known(width, 0));
        ev.evaluate(|port| inputs.get(&port).copied())?;
        let mut hash = 0xcbf29ce484222325;
        for i in 0..f.stimulus.iterations() {
            let value = Bus::known(width, i as u32 + 1);
            inputs.insert((ids[i % ids.len()], 0), value);
            let start = Instant::now();
            ev.evaluate_cached(|port| inputs.get(&port).copied())?;
            let elapsed = start.elapsed();
            if run >= WARMUP {
                times.push(elapsed);
            }
            // Observe every drive outside the timed interval.
            for (_, value) in ev.values() {
                measure::hash_value(&mut hash, value);
            }
        }
        if digest.replace(hash).is_some_and(|old| old != hash) {
            return Err("Evaluator trace diverged".into());
        }
    }
    let rate = times.len() as f64 / times.iter().sum::<Duration>().as_secs_f64();
    Ok(
        json!({"workload":name,"scope":"cached-evaluator","compile_ms":cold,"iteration_ms":distribution(&mut times),"iterations_per_s":rate,"runs":runs,"warmup":WARMUP,"drive_digest":format!("{:016x}",digest.unwrap())}),
    )
}

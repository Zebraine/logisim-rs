//! Each cache observation reopens the actual project, then checks its entire four-state trace.
use super::*;
use logisim_plugin::study::{Loader, Reuse, with_multiplier};
use std::path::Path;

pub fn run(mut args: impl Iterator<Item = String>, engine: WasmEngine) -> Result<Value> {
    let action = args.next().ok_or("Expected trace or opens")?;
    let name = args.next().ok_or("Expected plugin or single-cycle")?;
    if !matches!(name.as_str(), "plugin" | "single-cycle") {
        return Err("Expected plugin or single-cycle".into());
    }
    let mode = args
        .next()
        .ok_or("Expected fuel multiplier or reuse policy")?;
    let (_, runs) = options(std::iter::once(name.clone()).chain(args))?;
    match action.as_str() {
        "trace" => {
            let multiple = mode.parse()?;
            let mut report = with_multiplier(multiple, || benchmark(&name, runs, engine))??;
            report["fuel_per_request"] = json!(multiple * 10_000_000);
            Ok(report)
        }
        "opens" => {
            let reuse = match mode.as_str() {
                "none" => Reuse::None,
                "engine" => Reuse::Engine,
                "module" => Reuse::Module,
                #[cfg(feature = "compare-wasmtime")]
                "private-disk" => {
                    // Only the comparison runner provisions this disposable, trusted directory.
                    let path = std::env::var_os("LOGISIM_BENCH_PRIVATE_CACHE").ok_or(
                        "Set LOGISIM_BENCH_PRIVATE_CACHE to a fresh private experiment directory",
                    )?;
                    Reuse::Disk(path.into())
                }
                _ => return Err("Unknown module reuse policy".into()),
            };
            let mut loader = Loader::with_engine(reuse, engine);
            let mut rows = Vec::new();
            for _ in 0..=runs {
                let mut recording = logisim_plugin::metrics::Recording::default();
                let start = Instant::now();
                let mut opened = phases::capture(&mut recording, || {
                    open(Fixture::load_with(&name, Path::new("."), |bytes| {
                        loader.load(bytes)
                    }))
                })?;
                let elapsed = ms(start.elapsed());
                let trace = trial(&opened.fixture, &mut opened.sim)?.trace;
                if trace.digest != opened.fixture.stimulus.expected_digest() {
                    return Err(format!("Trace mismatch reopening {name}").into());
                }
                rows.push(
                    json!({"open_ms":elapsed,"first_settle_ms":opened.first_settle_ms,
                    "hits_so_far":loader.hits,"misses_so_far":loader.misses,
                    "spans":recording.spans,"digest":format!("{:016x}",trace.digest),
                    "ticks":trace.ticks,"steps":trace.steps}),
                );
            }
            #[cfg(feature = "compare-wasmtime")]
            let disk_error = loader.cache_error();
            #[cfg(not(feature = "compare-wasmtime"))]
            let disk_error: Option<&str> = None;
            Ok(
                json!({"schema":1,"workload":name,"reuse":mode,"opens":rows,"cache_error":disk_error}),
            )
        }
        _ => Err("Expected trace or opens".into()),
    }
}

//! Field names are part of the benchmark JSON; all accumulated spans use ns.
use crate::{Result, measure::Distribution};
use logisim_plugin::metrics::{self, Calls, Recording};
use serde_json::{Value, json};

/// Keep setup and hot samples in different accumulators. A scope restores the prior
/// recorder even if a workload fails. Each benchmark runs on one simulation thread.
#[derive(Default)]
pub struct Intervals {
    pub setup: Recording,
    pub hot: Recording,
}
impl Intervals {
    pub fn setup<T>(&mut self, run: impl FnOnce() -> T) -> T {
        capture(&mut self.setup, run)
    }
    pub fn hot<T>(&mut self, run: impl FnOnce() -> T) -> T {
        capture(&mut self.hot, run)
    }
}
pub fn capture<T>(recording: &mut Recording, run: impl FnOnce() -> T) -> T {
    struct Scope<'a>(&'a mut Recording, Option<Recording>);
    impl Drop for Scope<'_> {
        fn drop(&mut self) {
            *self.0 = metrics::replace(self.1.take()).expect("measurement scope");
        }
    }
    let previous = metrics::replace(Some(std::mem::take(recording)));
    let _scope = Scope(recording, previous);
    run()
}

fn calls(c: &mut Calls) -> Value {
    let n = c.latency_ns.len();
    let first = c.latency_ns.first().copied();
    let sum: u64 = c.latency_ns.iter().sum();
    c.latency_ns.sort_unstable();
    let timing = (n > 0).then(|| Distribution {
        p50: c.latency_ns[n.div_ceil(2) - 1] as f64 / 1e6,
        p95: c.latency_ns[(95 * n).div_ceil(100) - 1] as f64 / 1e6,
    });
    json!({
        "calls": n, "total_ns": sum, "first_ns": first, "latency_ms": timing,
        "input_signals": c.input_signals, "output_signals": c.output_signals,
        "linear_memory_used": c.frame_calls != 0,
        "frame_state_copy_ns": {"within_encode": c.state_write_ns, "within_decode": c.state_read_ns},
        "frame_calls": c.frame_calls, "json_calls": c.json_calls,
        "request_bytes": c.request_bytes, "response_bytes": c.response_bytes,
        "input_state_bytes": c.input_state_bytes, "returned_state_bytes": c.returned_state_bytes,
        "spans": c.spans
    })
}
pub fn report(r: &mut Recording) -> Value {
    json!({"spans": r.spans, "outputs": calls(&mut r.outputs), "transition": calls(&mut r.transition)})
}
pub fn hot(r: &mut Recording, settle_ns: u64, outputs: usize, samples: usize) -> Result<Value> {
    let scheduled: u64 = ["output_schedule", "transition_schedule", "commit_schedule"]
        .iter()
        .filter_map(|key| r.spans.get(key))
        .map(|s| s.ns)
        .sum();
    let mut guest = 0;
    let mut call_ns = 0;
    for c in [&r.outputs, &r.transition] {
        let total: u64 = c.latency_ns.iter().sum();
        let parts: u64 = c.spans.values().map(|s| s.ns).sum();
        if c.state_write_ns > c.spans.get("encode").map_or(0, |s| s.ns)
            || c.state_read_ns > c.spans.get("decode").map_or(0, |s| s.ns)
            || parts > total
        {
            return Err("Overlapping plugin call intervals".into());
        }
        guest += ["guest_dispatch", "guest_buffer"]
            .iter()
            .filter_map(|k| c.spans.get(k))
            .map(|s| s.ns)
            .sum::<u64>();
        call_ns += total;
    }
    if call_ns > scheduled
        || scheduled > settle_ns
        || r.outputs.latency_ns.len() != outputs
        || r.transition.latency_ns.len() != samples
    {
        return Err("Plugin timing or phase count does not match stable settle".into());
    }
    let mut v = report(r);
    v["settle_ns"] = json!(settle_ns);
    v["plugin_schedule_ns"] = json!(scheduled);
    v["guest_ns"] = json!(guest);
    Ok(v)
}

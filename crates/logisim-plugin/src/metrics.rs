//! Benchmark-only, thread-local wall-clock accounting. A collection owns its interval;
//! normal application builds compile out the module and all instrumentation sites.
use serde::Serialize;
use std::{cell::RefCell, collections::BTreeMap, time::Instant};

#[derive(Default, Serialize)]
pub struct Tally {
    pub count: u64,
    pub ns: u64,
}

#[derive(Default)]
pub struct Calls {
    pub latency_ns: Vec<u64>,
    pub spans: BTreeMap<&'static str, Tally>,
    pub frame_calls: usize,
    pub json_calls: usize,
    pub input_signals: usize,
    pub output_signals: usize,
    pub request_bytes: usize,
    pub response_bytes: usize,
    pub input_state_bytes: usize,
    pub returned_state_bytes: usize,
}

#[derive(Default)]
pub struct Recording {
    pub spans: BTreeMap<&'static str, Tally>,
    pub outputs: Calls,
    pub transition: Calls,
    current: Option<Operation>,
}
#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Outputs,
    Transition,
}
impl Recording {
    fn call(&mut self) -> Option<&mut Calls> {
        match self.current? {
            Operation::Outputs => Some(&mut self.outputs),
            Operation::Transition => Some(&mut self.transition),
        }
    }
}

thread_local! {
    static RECORDING: RefCell<Option<Recording>> = const { RefCell::new(None) };
}

/// Switch intervals outside timed calls. Returning the previous recording allows
/// a benchmark to accumulate hot traces while measuring resets in a separate bucket.
pub fn replace(next: Option<Recording>) -> Option<Recording> {
    RECORDING.with(|r| std::mem::replace(&mut *r.borrow_mut(), next))
}

fn record(f: impl FnOnce(&mut Recording)) {
    RECORDING.with_borrow_mut(|r| {
        if let Some(r) = r {
            f(r);
        }
    });
}

pub(crate) fn on_call(f: impl FnOnce(&mut Calls)) {
    record(|r| {
        if let Some(call) = r.call() {
            f(call);
        }
    });
}

pub(crate) fn operation<T>(
    op: Operation,
    signals: usize,
    state: usize,
    run: impl FnOnce() -> T,
) -> T {
    record(|r| r.current = Some(op));
    let start = Instant::now();
    let result = run();
    let ns = start.elapsed().as_nanos() as u64;
    on_call(|c| {
        c.latency_ns.push(ns);
        c.input_signals += signals;
        c.input_state_bytes += state;
    });
    record(|r| r.current = None);
    result
}

pub struct Timer {
    key: &'static str,
    start: Instant,
}
pub fn timer(key: &'static str) -> Timer {
    Timer {
        key,
        start: Instant::now(),
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        let ns = self.start.elapsed().as_nanos() as u64;
        record(|r| {
            let spans = match r.call() {
                Some(call) => &mut call.spans,
                None => &mut r.spans,
            };
            let t = spans.entry(self.key).or_default();
            t.count += 1;
            t.ns += ns;
        });
    }
}
pub fn count(key: &'static str) {
    record(|r| r.spans.entry(key).or_default().count += 1);
}

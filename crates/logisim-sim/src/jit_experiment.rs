//! Explicit, thread-scoped benchmark context. The cache dies on leaving the experiment.
use logisim_jit_spike::{Code, Region};
use std::{cell::RefCell, collections::BTreeMap, sync::Arc, time::Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Native,
    Verify,
    ProfileNative,
    ProfileInterpreter,
}
impl Mode {
    pub(crate) fn profile(self) -> bool {
        matches!(self, Self::ProfileNative | Self::ProfileInterpreter)
    }
}
#[derive(Clone, Default, serde::Serialize)]
pub struct Stats {
    pub planned_components: usize,
    pub plans: usize,
    pub pure_nodes: usize,
    pub folded_nodes: usize,
    pub plan_instructions: usize,
    pub native_nodes: usize,
    pub native_regions: usize,
    pub compiled_kernels: usize,
    pub cache_hits: usize,
    pub compile_failures: usize,
    pub cache_limit_skips: usize,
    pub compile_ns: u64,
    pub first_error: Option<String>,
    /// Profile modes only: includes boundary reads and changed-drive bookkeeping.
    pub evaluator_ns: u64,
    pub evaluated_instructions: usize,
    /// ProfileNative only: includes packing, machine code and changed-drive scatter.
    pub native_region_ns: u64,
    pub native_instructions: usize,
    pub native_calls: usize,
}

struct Session {
    mode: Mode,
    // One fixed backend/ISA (Linux x86-64 SysV), instruction semantics and
    // convention per binary. No persistent entries or cross-target reuse.
    kernels: BTreeMap<Region, Option<Arc<Code>>>,
    stats: Stats,
    limit: usize,
}
thread_local! { static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) }; }

/// A bounded experiment owns its compilation cache. Restores the outer context on unwind.
pub fn run<T>(mode: Mode, f: impl FnOnce() -> T) -> (T, Stats) {
    scoped(mode, 128, f)
}
fn scoped<T>(mode: Mode, limit: usize, f: impl FnOnce() -> T) -> (T, Stats) {
    struct Restore(Option<Session>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SESSION.set(self.0.take());
        }
    }
    let _restore = Restore(SESSION.replace(Some(Session {
        mode,
        limit,
        kernels: BTreeMap::new(),
        stats: Stats::default(),
    })));
    let result = f();
    let finished = SESSION.take().expect("active experiment");
    (result, finished.stats)
}
pub fn stats() -> Option<Stats> {
    SESSION.with_borrow(|s| s.as_ref().map(|s| s.stats.clone()))
}
pub(crate) fn mode() -> Option<Mode> {
    SESSION.with_borrow(|s| s.as_ref().map(|s| s.mode))
}
pub(crate) fn record(f: impl FnOnce(&mut Stats)) {
    SESSION.with_borrow_mut(|s| {
        if let Some(s) = s {
            f(&mut s.stats)
        }
    })
}
pub(crate) fn kernel(region: Region) -> Option<Arc<Code>> {
    SESSION.with_borrow_mut(|s| {
        let s = s.as_mut()?;
        if let Some(existing) = s.kernels.get(&region) {
            s.stats.cache_hits += usize::from(existing.is_some());
            return existing.clone();
        }
        if s.kernels.len() >= s.limit {
            s.stats.cache_limit_skips += 1;
            return None;
        }
        let start = Instant::now();
        let code = match Code::compile(&region) {
            Ok(code) => {
                s.stats.compiled_kernels += 1;
                Some(Arc::new(code))
            }
            Err(e) => {
                s.stats.compile_failures += 1;
                s.stats.first_error.get_or_insert(e);
                None
            }
        };
        s.stats.compile_ns += start.elapsed().as_nanos() as u64;
        s.kernels.insert(region, code.clone());
        code
    })
}

#[cfg(all(
    test,
    target_arch = "x86_64",
    target_pointer_width = "64",
    target_os = "linux"
))]
mod tests {
    use super::*;
    #[test]
    fn compile_failure_is_cached_without_disabling_other_regions() {
        let (_, stats) = run(Mode::Native, || {
            let invalid = Region::Unary {
                width: 0,
                invert: vec![true],
            };
            assert!(kernel(invalid.clone()).is_none());
            assert!(kernel(invalid).is_none());
            assert!(
                kernel(Region::Unary {
                    width: 1,
                    invert: vec![true]
                })
                .is_some()
            );
        });
        assert_eq!((stats.compiled_kernels, stats.compile_failures), (1, 1));
        assert!(stats.first_error.is_some());
    }

    #[test]
    fn full_cache_keeps_the_interpreted_drive_and_changed_set() {
        use crate::combinational::CompiledCircuit;
        use logisim_core::{Circuit, Component, Gate, Kind, Point, Value};
        let gate = Component::new(Kind::Gate(Gate::Not), Point::ZERO);
        let mut circuit = Circuit::default();
        let pin = circuit.add_component(Component::new(Kind::Pin, gate.ports()[1]));
        let out = circuit.add_component(gate);
        let (_, stats) = scoped(Mode::Verify, 0, || {
            let mut evaluator = CompiledCircuit::new(&circuit).experiment_evaluator();
            for value in 0..4 {
                evaluator
                    .evaluate_cached(|p| (p == (pin, 0)).then(|| Value::known(1, value)))
                    .unwrap();
                assert_eq!(evaluator.value((out, 0)), Some(Value::known(1, !value)));
            }
        });
        assert_eq!(
            (
                stats.compiled_kernels,
                stats.native_nodes,
                stats.cache_limit_skips
            ),
            (0, 0, 1)
        );
    }

    #[test]
    fn repeated_regions_share_code_within_the_budget_and_release_on_exit() {
        let (weak, s) = scoped(Mode::Native, 1, || {
            let key = Region::Unary {
                width: 1,
                invert: vec![true],
            };
            let code = kernel(key.clone()).unwrap();
            assert!(Arc::ptr_eq(&code, &kernel(key).unwrap()));
            assert!(
                kernel(Region::Unary {
                    width: 32,
                    invert: vec![true]
                })
                .is_none()
            );
            Arc::downgrade(&code)
        });
        assert!(weak.upgrade().is_none());
        assert_eq!(
            (s.compiled_kernels, s.cache_hits, s.cache_limit_skips),
            (1, 1, 1)
        );
    }
}

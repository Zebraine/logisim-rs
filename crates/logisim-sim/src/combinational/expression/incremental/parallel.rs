//! Only a single evaluator borrows its pool during a frontier. Workers read slots;
//! one caller commits staged writes in topological layer, instruction and port order.
use super::*;
use rayon_core::{ThreadPool, ThreadPoolBuilder};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
};

pub struct DataflowPool {
    pool: Option<ThreadPool>,
    workers: Vec<JoinHandle<()>>,
    cancelled: AtomicBool,
}

impl DataflowPool {
    /// The calling simulation thread is one of the requested lanes.
    pub fn for_threads(threads: usize) -> Result<Option<Arc<Self>>, String> {
        if threads == 1 {
            return Ok(None);
        }
        if !matches!(threads, 2 | 4) {
            return Err("Expected 1, 2 or 4 dataflow threads".into());
        }
        let mut workers = Vec::new();
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads - 1)
            .spawn_handler(|worker| {
                let thread = thread::Builder::new()
                    .name(format!("dataflow-{}", worker.index()))
                    .spawn(move || worker.run())?;
                workers.push(thread);
                Ok(())
            })
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Some(Arc::new(Self {
            pool: Some(pool),
            workers,
            cancelled: AtomicBool::new(false),
        })))
    }

    /// Stop parallel dispatch. An in-flight frontier drains; unpublished work can run serially.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn run<T: Send>(&self, lanes: &mut [T], work: impl Fn(usize, &mut T) + Sync) -> bool {
        let (first, rest) = lanes.split_first_mut().unwrap();
        let work = &work;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.pool.as_ref().unwrap().in_place_scope(|scope| {
                for (i, lane) in rest.iter_mut().enumerate() {
                    scope.spawn(move |_| work(i + 1, lane));
                }
                work(0, first);
            });
        }));
        if outcome.is_err() {
            self.cancel();
        }
        !self.cancelled.load(Ordering::Acquire)
    }
}

impl Drop for DataflowPool {
    fn drop(&mut self) {
        drop(self.pool.take());
        for worker in self.workers.drain(..) {
            worker.join().expect("all jobs were scoped");
        }
    }
}

struct Lane {
    scratch: Scratch,
    writes: Vec<(usize, Value)>,
    #[cfg(feature = "dataflow-study")]
    elapsed: std::time::Duration,
}

pub(super) struct Schedule {
    pool: Arc<DataflowPool>,
    layers: Vec<Vec<usize>>,
    lanes: Vec<Lane>,
    ready: Vec<usize>,
}

impl Schedule {
    fn new(pool: Arc<DataflowPool>, program: &Program) -> Self {
        let mut depth = vec![0; program.instructions.len()];
        let mut layers = Vec::<Vec<usize>>::new();
        for (i, op) in program.instructions.iter().enumerate() {
            let d = depth[i];
            if d == layers.len() {
                layers.push(Vec::new());
            }
            layers[d].push(i);
            for c in op
                .outputs
                .iter()
                .flatten()
                .flat_map(|&slot| &program.consumers[slot])
            {
                depth[c.instruction] = depth[c.instruction].max(d + 1);
            }
        }
        if layers.iter().all(|layer| layer.len() <= 1) {
            layers.clear();
        }
        let lanes = (0..=pool.workers.len())
            .map(|_| Lane {
                scratch: Scratch::new(program.max_ports),
                writes: Vec::new(),
                #[cfg(feature = "dataflow-study")]
                elapsed: Default::default(),
            })
            .collect();
        Self {
            pool,
            layers,
            lanes,
            ready: Vec::new(),
        }
    }

    fn evaluate(&mut self, eval: &mut Evaluator, full: bool) {
        let mut count = 0;
        for layer in &self.layers {
            self.ready.clear();
            self.ready.extend(
                layer
                    .iter()
                    .copied()
                    .filter(|&i| full || eval.queue.marked[i]),
            );
            if self.ready.is_empty() {
                continue;
            }
            count += self.ready.len();
            #[cfg(feature = "dataflow-study")]
            if let Some(study) = &mut eval.study {
                study.active.extend_from_slice(&self.ready);
            }
            if run_frontier(&self.pool, &mut self.lanes, &self.ready, eval, full) {
                continue;
            }
            for &i in &self.ready {
                eval.program.instructions[i].execute(
                    &mut eval.values,
                    &mut eval.scratch,
                    |slot, old, v| {
                        eval.changed.push(slot);
                        if !full {
                            eval.queue.changed(&eval.program.consumers[slot], old, v);
                        }
                    },
                );
            }
        }
        eval.queue.clear();
        eval.stats.instructions = count;
    }
}

/// A scoped job has no mutable view of the evaluator. Only a completed barrier may
/// publish its private writes. The caller replays the frontier on cancellation or panic.
fn run_frontier(
    pool: &DataflowPool,
    lanes: &mut [Lane],
    ready: &[usize],
    eval: &mut Evaluator,
    full: bool,
) -> bool {
    let n = ready.len().min(lanes.len());
    if n < 2
        || pool.cancelled.load(Ordering::Acquire)
        || ready
            .iter()
            .any(|&i| !eval.program.instructions[i].operation.dataflow_eligible())
    {
        return false;
    }
    let lanes = &mut lanes[..n];
    #[cfg(feature = "dataflow-study")]
    let timing = eval.study.as_ref().map(|_| std::time::Instant::now());
    let complete = pool.run(lanes, |i, lane| {
        #[cfg(feature = "dataflow-study")]
        let start = timing.map(|_| std::time::Instant::now());
        lane.writes.clear();
        for &j in &ready[i * ready.len() / n..(i + 1) * ready.len() / n] {
            let op = &eval.program.instructions[j];
            op.gather(&eval.values, &mut lane.scratch);
            op.compute(&mut lane.scratch, |slot, value| {
                if eval.values[slot] != value {
                    lane.writes.push((slot, value));
                }
            });
        }
        #[cfg(feature = "dataflow-study")]
        if let Some(start) = start {
            lane.elapsed = start.elapsed();
        }
    });
    #[cfg(feature = "dataflow-study")]
    if let (Some(start), Some(study)) = (timing, &mut eval.study) {
        study.trace.barrier += start.elapsed();
        study.trace.dispatches += 1;
        study.trace.dispatched_nodes += ready.len();
        study.trace.longest_lane += lanes.iter().map(|l| l.elapsed).max().unwrap();
    }
    if complete {
        for lane in lanes {
            for &(slot, value) in &lane.writes {
                let old = std::mem::replace(&mut eval.values[slot], value);
                eval.changed.push(slot);
                if !full {
                    eval.queue
                        .changed(&eval.program.consumers[slot], old, value);
                }
            }
        }
    }
    complete
}

impl Evaluator {
    pub fn set_dataflow_pool(&mut self, pool: Option<&Arc<DataflowPool>>) {
        if let Some(pool) = pool {
            if self
                .parallel
                .as_ref()
                .is_none_or(|s| !Arc::ptr_eq(&s.pool, pool))
            {
                self.parallel = Some(Box::new(Schedule::new(pool.clone(), &self.program)));
            }
        } else {
            self.parallel = None;
        }
    }

    pub(super) fn execute_parallel(&mut self, full: bool) -> bool {
        if !full && self.queue.pending.len() < 2 {
            return false;
        }
        let Some(mut schedule) = self
            .parallel
            .take_if(|s| !s.layers.is_empty() && !s.pool.cancelled.load(Ordering::Acquire))
        else {
            return false;
        };
        schedule.evaluate(self, full);
        self.parallel = Some(schedule);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, sync::mpsc};

    #[test]
    fn cancellation_and_worker_panic_drain_the_frontier_and_shutdown_joins() {
        struct OnExit(mpsc::Sender<()>);
        impl Drop for OnExit {
            fn drop(&mut self) {
                self.0.send(()).unwrap();
            }
        }
        thread_local! { static EXIT: RefCell<Option<OnExit>> = const { RefCell::new(None) }; }
        for panic in [false, true] {
            let pool = DataflowPool::for_threads(2).unwrap().unwrap();
            let (tx, rx) = mpsc::channel();
            assert!(!pool.run(&mut [(), ()], |i, _| {
                if i == 1 {
                    EXIT.with_borrow_mut(|slot| *slot = Some(OnExit(tx.clone())));
                    if panic {
                        panic!("injected background lane failure");
                    }
                    pool.cancel();
                }
            }));
            assert!(pool.cancelled.load(Ordering::Acquire));
            drop(pool);
            assert_eq!(rx.try_recv(), Ok(()));
        }
        assert!(DataflowPool::for_threads(8).is_err());
    }
}

use super::*;
use std::{cmp::Reverse, collections::BinaryHeap};

pub(super) struct Consumer {
    instruction: usize,
    bits: u32,
}

pub(super) fn consumers(instructions: &[Instruction], initial: &[Value]) -> Vec<Box<[Consumer]>> {
    let mut consumers: Vec<Vec<Consumer>> = (0..initial.len()).map(|_| vec![]).collect();
    for (instruction, node) in instructions.iter().enumerate() {
        let mut reads = BTreeMap::<usize, u32>::new();
        for (_, read) in &node.inputs {
            match read {
                Read::Constant(_) => {}
                Read::Direct(slot) => {
                    reads.insert(*slot, u32::MAX);
                }
                Read::Gather(bits) => {
                    for bit in bits {
                        bit.dependencies(&mut reads);
                    }
                }
            }
        }
        for (slot, bits) in reads {
            let full = u32::MAX >> (32 - initial[slot].width());
            consumers[slot].push(Consumer {
                instruction,
                bits: if bits & full == full { u32::MAX } else { bits },
            });
        }
    }
    consumers.into_iter().map(Vec::into_boxed_slice).collect()
}

/// Work performed by the last successful call, excluding compile-time folding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvaluationStats {
    pub instructions: usize,
    /// First evaluation (or recovery after an error) reports every output.
    pub changed_outputs: usize,
}

struct Queue {
    pending: BinaryHeap<Reverse<usize>>,
    marked: Vec<bool>,
}
impl Queue {
    fn new(len: usize) -> Self {
        Self {
            pending: BinaryHeap::with_capacity(len),
            marked: vec![false; len],
        }
    }
    fn clear(&mut self) {
        while let Some(Reverse(index)) = self.pending.pop() {
            self.marked[index] = false;
        }
    }
    fn changed(&mut self, consumers: &[Consumer], old: Value, new: Value) {
        for consumer in consumers {
            if !self.marked[consumer.instruction]
                && (consumer.bits == u32::MAX
                    || (0..new.width())
                        .any(|bit| consumer.bits & (1 << bit) != 0 && old.bit(bit) != new.bit(bit)))
            {
                self.marked[consumer.instruction] = true;
                self.pending.push(Reverse(consumer.instruction));
            }
        }
    }
}

/// Per-instance working values. Evaluators share code but never mutable signals.
pub struct Evaluator {
    program: Arc<Program>,
    values: Vec<Value>,
    scratch: Scratch,
    queue: Queue,
    changed: Vec<usize>,
    all_changed: bool,
    valid: bool,
    stats: EvaluationStats,
    #[cfg(feature = "circuit-jit-experiment")]
    native: Option<super::native::Native>,
    #[cfg(feature = "circuit-jit-experiment")]
    shadow: Option<Box<Evaluator>>,
    #[cfg(feature = "circuit-jit-experiment")]
    profile: bool,
}
impl Evaluator {
    pub(super) fn new(program: Arc<Program>) -> Self {
        Self {
            values: program.initial.clone(),
            scratch: Scratch::new(program.max_ports),
            queue: Queue::new(program.instructions.len()),
            changed: Vec::with_capacity(program.outputs.len()),
            all_changed: false,
            valid: false,
            stats: EvaluationStats::default(),
            #[cfg(feature = "circuit-jit-experiment")]
            native: None,
            #[cfg(feature = "circuit-jit-experiment")]
            shadow: None,
            #[cfg(feature = "circuit-jit-experiment")]
            profile: false,
            program,
        }
    }

    #[cfg(feature = "circuit-jit-experiment")]
    pub(super) fn configure_experiment(&mut self, mode: crate::jit_experiment::Mode) {
        use crate::jit_experiment::{self, Mode};
        let stats = self.program.stats;
        jit_experiment::record(|s| {
            s.plans += 1;
            s.pure_nodes += stats.nodes;
            s.folded_nodes += stats.folded_nodes;
            s.plan_instructions += stats.instructions;
        });
        self.profile = mode.profile();
        if mode != Mode::ProfileInterpreter {
            self.native = super::native::Native::new(&self.program, mode);
        }
        if mode == Mode::Verify {
            self.shadow = Some(Box::new(Self::new(self.program.clone())));
        }
    }

    /// Read one coherent boundary snapshot and evaluate every instruction in
    /// topological order, including X/E bits. Physically undriven inputs are X.
    /// Missing or incorrectly sized drives invalidate all result queries.
    pub fn evaluate(
        &mut self,
        mut read: impl FnMut(Port) -> Option<Value>,
    ) -> Result<(), EvaluationError> {
        self.run(|_, port, _| read(port), true).map(|_| ())
    }

    /// Validate every boundary drive, then evaluate only affected instructions.
    /// Unchanged outputs stop propagation, including unused bits of gathered buses.
    /// Returns true on a changed boundary value or a full refresh; outputs may still
    /// be unchanged. First calls and recovery after errors evaluate all instructions.
    pub fn evaluate_cached(
        &mut self,
        mut read: impl FnMut(Port) -> Option<Value>,
    ) -> Result<bool, EvaluationError> {
        self.run(|_, port, _| read(port), false)
    }

    /// Read in CompiledCircuit::input_ports order. Width validation and queue
    /// scheduling are shared with the public port-based evaluator.
    pub(crate) fn evaluate_bound(
        &mut self,
        mut read: impl FnMut(usize, u8) -> Value,
    ) -> Result<bool, EvaluationError> {
        self.run(|index, _, width| Some(read(index, width)), false)
    }

    fn run(
        &mut self,
        mut read: impl FnMut(usize, Port, u8) -> Option<Value>,
        force: bool,
    ) -> Result<bool, EvaluationError> {
        #[cfg(feature = "circuit-jit-experiment")]
        let start = self.profile.then(std::time::Instant::now);
        let full = force || !self.valid;
        let mut refreshed = full;
        self.all_changed = !self.valid;
        self.valid = false;
        self.stats = EvaluationStats::default();
        self.queue.clear();
        self.changed.clear();
        for (index, &(port, slot)) in self.program.inputs.iter().enumerate() {
            let expected = self.program.initial[slot].width();
            let value =
                read(index, port, expected).ok_or(EvaluationError::MissingInput { port })?;
            if value.width() != expected {
                return Err(EvaluationError::WidthMismatch {
                    port,
                    expected,
                    actual: value.width(),
                });
            }
            let old = self.values[slot];
            if old != value {
                refreshed = true;
                if !full {
                    self.queue
                        .changed(&self.program.consumers[slot], old, value);
                }
                self.values[slot] = value;
            }
        }
        self.execute_instructions(full);
        self.stats.changed_outputs = if self.all_changed {
            self.program.outputs.len()
        } else {
            self.changed.len()
        };
        self.valid = true;
        #[cfg(feature = "circuit-jit-experiment")]
        {
            if let Some(start) = start {
                let ns = start.elapsed().as_nanos() as u64;
                crate::jit_experiment::record(|s| {
                    s.evaluator_ns += ns;
                    s.evaluated_instructions += self.stats.instructions;
                });
            }
            self.verify_drives(force, refreshed)?;
        }
        Ok(refreshed)
    }

    #[cfg(feature = "circuit-jit-experiment")]
    fn verify_drives(&mut self, force: bool, refreshed: bool) -> Result<(), EvaluationError> {
        if let Some(mut shadow) = self.shadow.take() {
            if self.all_changed {
                shadow.valid = false;
            }
            let other = shadow.run(|i, _, _| Some(self.values[self.program.inputs[i].1]), force)?;
            assert_eq!(refreshed, other, "stable boundary refresh");
            assert_eq!(self.values, shadow.values, "all expression drive slots");
            assert_eq!(
                self.changed_values().collect::<BTreeMap<_, _>>(),
                shadow.changed_values().collect::<BTreeMap<_, _>>(),
                "materialized drive deltas"
            );
            self.shadow = Some(shadow);
        }
        Ok(())
    }

    fn execute_instructions(&mut self, full: bool) {
        #[cfg(feature = "circuit-jit-experiment")]
        if self.native.is_some() {
            if full {
                let mut index = 0;
                while index < self.program.instructions.len() {
                    index += self.experimental_step(index, true);
                }
            } else {
                while let Some(Reverse(index)) = self.queue.pending.pop() {
                    self.queue.marked[index] = false;
                    self.experimental_step(index, false);
                }
            }
            return;
        }
        if full {
            for instruction in &self.program.instructions {
                instruction.execute(&mut self.values, &mut self.scratch, |slot, _, _| {
                    self.changed.push(slot)
                });
            }
            self.stats.instructions = self.program.instructions.len();
        } else {
            // Instruction indices are topological. Every predecessor executes
            // before its consumers, even when unequal paths reconverge. Membership
            // deduplicates fan-out, so each instruction executes at most once.
            while let Some(Reverse(index)) = self.queue.pending.pop() {
                self.queue.marked[index] = false;
                self.program.instructions[index].execute(
                    &mut self.values,
                    &mut self.scratch,
                    |slot, old, new| {
                        self.changed.push(slot);
                        self.queue.changed(&self.program.consumers[slot], old, new);
                    },
                );
                self.stats.instructions += 1;
            }
        }
    }

    #[cfg(feature = "circuit-jit-experiment")]
    fn experimental_step(&mut self, index: usize, full: bool) -> usize {
        let mut update = |end: usize, slot: usize, old: Value, new: Value| {
            self.changed.push(slot);
            if !full {
                let consumers = &self.program.consumers[slot];
                // Earlier consumers within this block have already been evaluated.
                let external = consumers.partition_point(|c| c.instruction < end);
                self.queue.changed(&consumers[external..], old, new);
            }
        };
        let count = self
            .native
            .as_mut()
            .unwrap()
            .execute(index, &mut self.values, &mut update)
            .unwrap_or_else(|| {
                self.program.instructions[index].execute(
                    &mut self.values,
                    &mut self.scratch,
                    |slot, old, new| update(index + 1, slot, old, new),
                );
                1
            });
        self.stats.instructions += count;
        count
    }

    pub fn stats(&self) -> Option<EvaluationStats> {
        self.valid.then_some(self.stats)
    }

    /// Compiled drives, including internal nodes. Boundary ports are not outputs.
    pub fn value(&self, port: Port) -> Option<Value> {
        self.valid
            .then(|| self.program.outputs.get(&port).map(|&s| self.values[s]))
            .flatten()
    }
    pub fn values(&self) -> impl Iterator<Item = (Port, Value)> + '_ {
        self.program
            .outputs
            .iter()
            .filter_map(|(&port, &slot)| self.valid.then_some((port, self.values[slot])))
    }

    /// Drives changed since the preceding successful call, in unspecified order.
    /// First evaluation and recovery after errors include folded and unknown drives.
    /// No results are exposed after a failed call.
    pub fn changed_values(&self) -> impl Iterator<Item = (Port, Value)> + '_ {
        self.values()
            .take(if self.all_changed { usize::MAX } else { 0 })
            .chain(
                self.changed
                    .iter()
                    .take(if self.valid && !self.all_changed {
                        usize::MAX
                    } else {
                        0
                    })
                    .map(|&slot| (self.program.output_ports[slot].unwrap(), self.values[slot])),
            )
    }
}

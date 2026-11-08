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
                        if let BitRead::Slot(slot, bit) = bit {
                            *reads.entry(*slot).or_default() |= 1 << bit;
                        }
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
            program,
        }
    }

    /// Read one coherent boundary snapshot and evaluate every instruction in
    /// topological order, including X/E bits. Physically undriven inputs are X.
    /// Missing or incorrectly sized drives invalidate all result queries.
    pub fn evaluate(
        &mut self,
        read: impl FnMut(Port) -> Option<Value>,
    ) -> Result<(), EvaluationError> {
        self.run(read, true).map(|_| ())
    }

    /// Validate every boundary drive, then evaluate only affected instructions.
    /// Unchanged outputs stop propagation, including unused bits of gathered buses.
    /// Returns true on a changed boundary value or a full refresh; outputs may still
    /// be unchanged. First calls and recovery after errors evaluate all instructions.
    pub fn evaluate_cached(
        &mut self,
        read: impl FnMut(Port) -> Option<Value>,
    ) -> Result<bool, EvaluationError> {
        self.run(read, false)
    }

    fn run(
        &mut self,
        mut read: impl FnMut(Port) -> Option<Value>,
        force: bool,
    ) -> Result<bool, EvaluationError> {
        let full = force || !self.valid;
        let mut refreshed = full;
        self.all_changed = !self.valid;
        self.valid = false;
        self.stats = EvaluationStats::default();
        self.queue.clear();
        self.changed.clear();
        for &(port, slot) in &self.program.inputs {
            let value = read(port).ok_or(EvaluationError::MissingInput { port })?;
            let expected = self.program.initial[slot].width();
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
        self.stats.changed_outputs = if self.all_changed {
            self.program.outputs.len()
        } else {
            self.changed.len()
        };
        self.valid = true;
        Ok(refreshed)
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

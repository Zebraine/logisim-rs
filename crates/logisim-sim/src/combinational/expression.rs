//! Dense, immutable expression plans. No event queue, clock or mutable circuit state.
mod operation;
#[cfg(test)]
mod tests;

use super::{Analysis, Input, Port};
use crate::{Bit, Simulator, Value};
use logisim_core::Circuit;
use operation::Operation;
use std::{collections::BTreeMap, fmt, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompileStats {
    pub regions: usize,
    pub nodes: usize,
    pub instructions: usize,
    pub folded_nodes: usize,
    pub boundary_ports: usize,
    pub output_ports: usize,
    pub value_slots: usize,
}

/// A snapshot of a circuit's pure acyclic regions, shared cheaply between instances.
/// Recompile after electrical edits (including attributes and ROM contents). This
/// does not replace event propagation or establish timing-safe state boundaries.
#[derive(Clone)]
pub struct CompiledCircuit(Arc<Program>);

struct Program {
    analysis: Analysis,
    inputs: Vec<(Port, usize)>,
    outputs: BTreeMap<Port, usize>,
    initial: Vec<Value>,
    instructions: Vec<Instruction>,
    stats: CompileStats,
    max_ports: usize,
}

struct Instruction {
    operation: Operation,
    widths: Box<[u8]>,
    inputs: Vec<(usize, Read)>,
    outputs: Box<[Option<usize>]>,
}

enum Read {
    Constant(Value),
    Direct(usize),
    Gather(Box<[BitRead]>),
}

#[derive(Clone, Copy)]
enum BitRead {
    Constant(Bit),
    Slot(usize, u8),
}

impl BitRead {
    fn get(self, values: &[Value]) -> Bit {
        match self {
            Self::Constant(bit) => bit,
            Self::Slot(slot, bit) => values[slot].bit(bit),
        }
    }
}

impl Read {
    fn compile(
        input: &Input,
        slots: &BTreeMap<Port, usize>,
        known: &[bool],
        values: &[Value],
    ) -> Self {
        let bits: Box<[_]> = input
            .bits
            .iter()
            .map(|source| match source {
                None => BitRead::Constant(Bit::Unknown),
                Some(source) => {
                    let slot = slots[&source.port];
                    if known[slot] {
                        BitRead::Constant(values[slot].bit(source.bit))
                    } else {
                        BitRead::Slot(slot, source.bit)
                    }
                }
            })
            .collect();
        if bits.iter().all(|b| matches!(b, BitRead::Constant(_))) {
            Self::Constant(Value::from_bits(bits.iter().map(|b| b.get(values))))
        } else if let Some(&BitRead::Slot(slot, 0)) = bits.first()
            && usize::from(values[slot].width()) == bits.len()
            && bits.iter().enumerate().all(
                |(i, b)| matches!(b, BitRead::Slot(s, bit) if *s == slot && usize::from(*bit) == i),
            )
        {
            Self::Direct(slot)
        } else {
            Self::Gather(bits)
        }
    }

    fn get(&self, values: &[Value]) -> Value {
        match self {
            Self::Constant(value) => *value,
            Self::Direct(slot) => values[*slot],
            Self::Gather(bits) => Value::from_bits(bits.iter().map(|b| b.get(values))),
        }
    }
}

struct Scratch {
    ports: Vec<Value>,
    optional: Vec<Option<Value>>,
}

impl Scratch {
    fn new(capacity: usize) -> Self {
        Self {
            ports: Vec::with_capacity(capacity),
            optional: Vec::with_capacity(capacity),
        }
    }
}

impl Instruction {
    fn execute(&self, values: &mut [Value], scratch: &mut Scratch) {
        scratch.ports.clear();
        scratch
            .ports
            .extend(self.widths.iter().map(|&w| Value::filled(w, Bit::Unknown)));
        for (index, read) in &self.inputs {
            scratch.ports[*index] = read.get(values);
        }
        self.operation
            .evaluate(&scratch.ports, &mut scratch.optional, |index, value| {
                let slot = self.outputs[index].expect("compiled output slot");
                values[slot] = value;
            });
    }
}

impl Simulator {
    /// Compile this instance's document, reusing its topology. Does not advance it.
    pub fn compile_combinational(&self) -> CompiledCircuit {
        CompiledCircuit::compile(&self.circuit, self.combinational_analysis())
    }
}

impl CompiledCircuit {
    pub fn new(circuit: &Circuit) -> Self {
        Self::compile(circuit, Analysis::new(circuit))
    }

    fn compile(circuit: &Circuit, analysis: Analysis) -> Self {
        let components: BTreeMap<_, _> = circuit.components.iter().map(|c| (c.id, c)).collect();
        let mut slots = BTreeMap::new();
        let mut initial = vec![];
        let mut slot_for = |port: Port| {
            *slots.entry(port).or_insert_with(|| {
                let slot = initial.len();
                let width = components[&port.0].port_width(port.1).expect("drive width");
                initial.push(Value::filled(width, Bit::Unknown));
                slot
            })
        };
        let inputs: BTreeMap<_, _> = analysis
            .regions
            .iter()
            .flat_map(|r| &r.inputs)
            .map(|s| (s.port, slot_for(s.port)))
            .collect();
        let outputs = analysis
            .regions
            .iter()
            .flat_map(|r| &r.nodes)
            .flat_map(|n| &n.outputs)
            .map(|&p| (p, slot_for(p)))
            .collect();
        let mut known = vec![false; initial.len()];
        let mut instructions = vec![];
        let mut scratch = Scratch::new(0);
        let mut stats = CompileStats {
            regions: analysis.regions.len(),
            boundary_ports: inputs.len(),
            value_slots: initial.len(),
            ..CompileStats::default()
        };
        let mut max_ports = 0;
        for node in analysis.regions.iter().flat_map(|r| &r.nodes) {
            let component = components[&node.component];
            let instruction = Instruction {
                operation: Operation::new(component, node),
                widths: (0..component.ports().len())
                    .map(|i| component.port_width(i).expect("pure operation port width"))
                    .collect(),
                inputs: node
                    .inputs
                    .iter()
                    .map(|input| (input.port.1, Read::compile(input, &slots, &known, &initial)))
                    .collect(),
                outputs: (0..component.ports().len())
                    .map(|i| {
                        node.outputs
                            .contains(&(node.component, i))
                            .then(|| slots[&(node.component, i)])
                    })
                    .collect(),
            };
            stats.nodes += 1;
            stats.output_ports += node.outputs.len();
            if instruction
                .inputs
                .iter()
                .all(|(_, r)| matches!(r, Read::Constant(_)))
            {
                // Use exactly the same four-state kernel; no Boolean identities.
                instruction.execute(&mut initial, &mut scratch);
                for slot in instruction.outputs.iter().flatten() {
                    known[*slot] = true;
                }
                stats.folded_nodes += 1;
            } else {
                max_ports = max_ports.max(instruction.widths.len());
                instructions.push(instruction);
            }
        }
        stats.instructions = instructions.len();
        Self(Arc::new(Program {
            analysis,
            inputs: inputs.into_iter().collect(),
            outputs,
            initial,
            instructions,
            stats,
            max_ports,
        }))
    }

    pub fn analysis(&self) -> &Analysis {
        &self.0.analysis
    }

    pub fn stats(&self) -> CompileStats {
        self.0.stats
    }

    pub fn evaluator(&self) -> Evaluator {
        Evaluator {
            program: Arc::clone(&self.0),
            values: self.0.initial.clone(),
            scratch: Scratch::new(self.0.max_ports),
            valid: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    MissingInput {
        port: Port,
    },
    WidthMismatch {
        port: Port,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInput { port } => write!(f, "missing expression input {port:?}"),
            Self::WidthMismatch {
                port,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "expression input {port:?}: expected {expected} bits, got {actual}"
                )
            }
        }
    }
}
impl std::error::Error for EvaluationError {}

/// Per-instance working values. Evaluators share code but never mutable signals.
pub struct Evaluator {
    program: Arc<Program>,
    values: Vec<Value>,
    scratch: Scratch,
    valid: bool,
}

impl Evaluator {
    /// Read each boundary drive port once, then evaluate all regions in topological
    /// order. The caller supplies one coherent snapshot, including X/E bits. Missing
    /// or incorrectly sized drives fail the whole evaluation; queries then return
    /// no results. Physically undriven inputs are compiled as X without callbacks.
    pub fn evaluate(
        &mut self,
        read: impl FnMut(Port) -> Option<Value>,
    ) -> Result<(), EvaluationError> {
        self.run(read, true).map(|_| ())
    }

    /// Reuse the previous result when every four-state boundary value is unchanged.
    /// Returns true when refreshing results instead of reusing a valid result.
    /// Every input is still validated; an error invalidates the cache and queries.
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
        let mut changed = force || !self.valid;
        self.valid = false;
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
            changed |= self.values[slot] != value;
            self.values[slot] = value;
        }
        if changed {
            for instruction in &self.program.instructions {
                instruction.execute(&mut self.values, &mut self.scratch);
            }
        }
        self.valid = true;
        Ok(changed)
    }

    /// Compiled output drives, including internal nodes, after a successful evaluate.
    /// Boundary ports and excluded components have no computed value here.
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
}

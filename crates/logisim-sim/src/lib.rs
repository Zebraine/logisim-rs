//! Deterministic, bounded combinational propagation, independent of the GUI.
mod network;
#[cfg(test)]
mod tests;
mod value;

use logisim_core::{Circuit, Extension, Id, Kind};
use network::Network;
use std::collections::BTreeMap;
pub use value::{Bit, Value};

const PROPAGATION_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    WidthMismatch,
    ErrorSignal,
    PropagationLimit,
}
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub problem: Problem,
    pub components: Vec<Id>,
    pub wires: Vec<Id>,
}

pub struct Simulator {
    circuit: Circuit,
    network: Network,
    inputs: BTreeMap<Id, Value>,
    drives: BTreeMap<Id, Value>,
    pending: BTreeMap<Id, Value>,
    values: Vec<Value>,
    enabled: bool,
    limited: bool,
    steps: u64,
    unsettled_steps: usize,
}
impl Default for Simulator {
    fn default() -> Self {
        Self {
            circuit: Circuit::default(),
            network: Network::default(),
            inputs: BTreeMap::new(),
            drives: BTreeMap::new(),
            pending: BTreeMap::new(),
            values: vec![],
            enabled: true,
            limited: false,
            steps: 0,
            unsettled_steps: 0,
        }
    }
}
impl Simulator {
    /// Recompile edited topology. Selection, input pokes and view changes are not document edits.
    pub fn synchronize(&mut self, circuit: &Circuit) -> bool {
        if self.circuit == *circuit {
            return false;
        }
        if electrically_equal(&self.circuit, circuit) {
            self.circuit = circuit.clone();
            return false;
        }
        self.inputs = circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Pin && !c.output)
            .map(|c| {
                let value = self
                    .inputs
                    .get(&c.id)
                    .copied()
                    .filter(|v| v.width() == c.bits.clamp(1, 32))
                    .unwrap_or_else(|| Value::known(c.bits, 0));
                (c.id, value)
            })
            .collect();
        self.circuit = circuit.clone();
        self.network = Network::compile(circuit);
        self.initialize();
        true
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled && self.limited {
            self.limited = false;
            self.unsettled_steps = 0;
        }
    }
    pub fn steps(&self) -> u64 {
        self.steps
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn limited(&self) -> bool {
        self.limited
    }
    pub fn input(&self, id: Id) -> Option<Value> {
        self.inputs.get(&id).copied()
    }
    pub fn set_input(&mut self, id: Id, number: u32) {
        let Some(input) = self.inputs.get_mut(&id) else {
            return;
        };
        let value = Value::known(input.width(), number);
        if *input == value {
            return;
        }
        *input = value;
        self.limited = false;
        self.unsettled_steps = 0;
        self.schedule(id, value);
    }
    pub fn toggle_input(&mut self, id: Id, bit: u8) {
        if let Some(value) = self.input(id)
            && bit < value.width()
        {
            self.set_input(id, value.number().unwrap_or(0) ^ (1 << bit));
        }
    }
    pub fn reset(&mut self) {
        for input in self.inputs.values_mut() {
            *input = Value::known(input.width(), 0);
        }
        self.initialize();
    }
    fn initialize(&mut self) {
        self.drives.clear();
        self.pending = self.inputs.clone();
        self.values = self
            .network
            .nets
            .iter()
            .map(|net| Value::filled(net.width, Bit::Unknown))
            .collect();
        self.steps = 0;
        self.unsettled_steps = 0;
        self.limited = false;
        self.schedule_components();
    }
    fn schedule(&mut self, id: Id, value: Value) {
        if self.drives.get(&id) == Some(&value) {
            self.pending.remove(&id);
        } else {
            self.pending.insert(id, value);
        }
    }
    fn schedule_components(&mut self) {
        let outputs: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter_map(|c| {
                let output = match c.kind {
                    Kind::Gate(gate) => {
                        let inputs: Vec<_> = (1..=usize::from(c.input_count()))
                            .filter(|i| gate.unary() || self.network.connected((c.id, *i)))
                            .map(|i| self.port_value(c.id, i).expect("compiled gate port"))
                            .collect();
                        value::evaluate(gate, c.bits, &inputs)
                    }
                    Kind::Constant => Value::known(c.bits, c.constant),
                    Kind::Power => Value::filled(c.bits, Bit::One),
                    Kind::Ground => Value::known(c.bits, 0),
                    Kind::BitExtender => {
                        let input = self.port_value(c.id, 1).expect("compiled extender input");
                        let extra = match c.extension {
                            Extension::Zero => Bit::Zero,
                            Extension::One => Bit::One,
                            Extension::Sign => input.bit(input.width() - 1),
                            Extension::Input => self
                                .port_value(c.id, 2)
                                .expect("compiled extension input")
                                .bit(0),
                        };
                        Value::from_bits((0..c.bits.clamp(1, 32)).map(|i| {
                            if i < input.width() {
                                input.bit(i)
                            } else {
                                extra
                            }
                        }))
                    }
                    _ => return None,
                };
                Some((c.id, output))
            })
            .collect();
        for (id, value) in outputs {
            self.schedule(id, value);
        }
    }
    /// Commit one batch of driver changes, then schedule resulting component changes.
    pub fn step(&mut self) -> bool {
        if self.pending.is_empty() || self.limited {
            return false;
        }
        self.drives.extend(std::mem::take(&mut self.pending));
        self.values = self.network.resolve(&self.drives);
        self.steps = self.steps.saturating_add(1);
        self.schedule_components();
        if self.pending.is_empty() {
            self.unsettled_steps = 0;
        } else {
            self.unsettled_steps += 1;
            if self.unsettled_steps >= PROPAGATION_LIMIT {
                self.limited = true;
                self.enabled = false;
            }
        }
        true
    }
    /// A caller-chosen frame budget; no wall clock or UI dependency.
    pub fn advance(&mut self, budget: usize) {
        if self.enabled {
            for _ in 0..budget {
                if !self.step() {
                    break;
                }
            }
        }
    }
    pub fn wire_value(&self, id: Id) -> Option<Value> {
        self.network.wires.get(&id).map(|i| self.values[*i])
    }
    pub fn wire_mismatch(&self, id: Id) -> bool {
        self.network
            .wires
            .get(&id)
            .is_some_and(|i| self.network.nets[*i].mismatch)
    }
    pub fn port_mismatch(&self, id: Id, index: usize) -> bool {
        self.network
            .ports
            .get(&(id, index))
            .is_some_and(|i| self.network.nets[*i].mismatch)
    }
    pub fn port_value(&self, id: Id, index: usize) -> Option<Value> {
        let net = *self.network.ports.get(&(id, index))?;
        let width = self
            .circuit
            .component(id)?
            .port_width(index)
            .unwrap_or(self.network.nets[net].width);
        Some(if self.network.nets[net].mismatch {
            Value::filled(width, Bit::Unknown)
        } else {
            self.values[net]
        })
    }
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut result: Vec<_> = self
            .network
            .nets
            .iter()
            .zip(&self.values)
            .filter_map(|(net, value)| {
                let problem = if net.mismatch {
                    Problem::WidthMismatch
                } else if value.contains(Bit::Error) {
                    Problem::ErrorSignal
                } else {
                    return None;
                };
                let mut components: Vec<_> = net.ports.iter().map(|(id, _)| *id).collect();
                components.sort_unstable();
                components.dedup();
                Some(Diagnostic {
                    problem,
                    components,
                    wires: net.wires.clone(),
                })
            })
            .collect();
        if self.limited {
            result.push(Diagnostic {
                problem: Problem::PropagationLimit,
                components: self.pending.keys().copied().collect(),
                wires: vec![],
            });
        }
        result
    }
}

fn electrically_equal(a: &Circuit, b: &Circuit) -> bool {
    let components = |c: &logisim_core::Component| {
        (
            c.id,
            c.kind,
            c.ports(),
            c.bits.clamp(1, 32),
            c.output && c.kind == Kind::Pin,
            (c.kind == Kind::Constant).then_some(c.constant),
            (c.kind == Kind::Splitter).then_some(c.splitter.clone()),
            (c.kind == Kind::BitExtender).then_some((c.input_bits, c.extension)),
            if c.kind == Kind::Tunnel {
                c.label.clone()
            } else {
                String::new()
            },
        )
    };
    a.wires == b.wires
        && a.components
            .iter()
            .filter(|c| c.kind != Kind::Text)
            .map(components)
            .eq(b
                .components
                .iter()
                .filter(|c| c.kind != Kind::Text)
                .map(components))
}

//! Deterministic, bounded signal propagation and sequential state, independent of the GUI.
mod hierarchy;
pub use hierarchy::ProjectSimulator;
mod arithmetic;
#[cfg(test)]
mod arithmetic_tests;
mod bus;
mod displays;
mod io;
mod memory;
#[cfg(test)]
mod memory_tests;
mod network;
#[cfg(test)]
mod plexer_tests;
mod plexers;
mod sequential;
#[cfg(test)]
mod sequential_extra_tests;
#[cfg(test)]
mod sequential_tests;
#[cfg(test)]
mod tests;
mod transistors;
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
    matrices: BTreeMap<Id, displays::MatrixState>,
    display_ticks: u64,
    storage: BTreeMap<Id, sequential::State>,
    memories: BTreeMap<Id, memory::State>,
    drives: BTreeMap<network::Port, Value>,
    pending: BTreeMap<network::Port, Value>,
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
            matrices: BTreeMap::new(),
            display_ticks: 0,
            storage: BTreeMap::new(),
            memories: BTreeMap::new(),
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
        self.synchronize_displays(circuit);
        if electrically_equal(&self.circuit, circuit) {
            self.circuit = circuit.clone();
            self.update_displays();
            return false;
        }
        self.inputs = circuit
            .components
            .iter()
            .filter(|c| io::control_bits(c).is_some())
            .map(|c| {
                let bits = io::control_bits(c).expect("interactive input");
                let previous = self.inputs.get(&c.id).copied().filter(|_| {
                    self.circuit
                        .component(c.id)
                        .is_some_and(|old| old.kind == c.kind)
                });
                let value = if c.kind == Kind::DipSwitch {
                    Value::known(bits, previous.and_then(Value::number).unwrap_or(0))
                } else {
                    previous
                        .filter(|v| v.width() == bits)
                        .unwrap_or_else(|| Value::known(bits, 0))
                };
                (c.id, value)
            })
            .collect();
        self.storage = circuit
            .components
            .iter()
            .filter(|c| c.kind.is_sequential())
            .map(|c| {
                let state = self
                    .storage
                    .remove(&c.id)
                    .and_then(|s| s.adapt(c))
                    .unwrap_or_else(|| sequential::State::new(c));
                (c.id, state)
            })
            .collect();
        self.memories = circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Ram)
            .map(|c| {
                let state = self
                    .memories
                    .remove(&c.id)
                    .map(|s| s.adapt(c))
                    .unwrap_or_else(|| memory::State::new(c));
                (c.id, state)
            })
            .collect();
        let held = self
            .drives
            .iter()
            .filter(|((id, index), _)| {
                *index < 7
                    && circuit.component(*id).is_some_and(|c| {
                        c.kind == Kind::Plexer(logisim_core::Plexer::DisplayDecoder)
                            && self
                                .circuit
                                .component(*id)
                                .is_some_and(|old| old.kind == c.kind)
                    })
            })
            .map(|(p, v)| (*p, *v))
            .collect();
        self.circuit = circuit.clone();
        self.network = Network::compile(circuit);
        self.initialize(held);
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
        if self
            .circuit
            .component(id)
            .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch))
        {
            self.schedule_controls();
        } else {
            self.schedule(id, value);
        }
    }
    pub fn toggle_input(&mut self, id: Id, bit: u8) {
        if let Some(value) = self.input(id)
            && bit < value.width()
        {
            self.set_input(id, value.number().unwrap_or(0) ^ (1 << bit));
        }
    }
    pub fn reset(&mut self) {
        self.reset_displays();
        for input in self.inputs.values_mut() {
            *input = Value::known(input.width(), 0);
        }
        self.storage = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind.is_sequential())
            .map(|c| (c.id, sequential::State::new(c)))
            .collect();
        self.memories = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Ram)
            .map(|c| (c.id, memory::State::new(c)))
            .collect();
        self.initialize(BTreeMap::new());
    }
    fn initialize(&mut self, held: BTreeMap<network::Port, Value>) {
        self.drives = held;
        self.pending = self
            .inputs
            .iter()
            .filter(|(id, _)| {
                self.circuit
                    .component(**id)
                    .is_some_and(|c| !matches!(c.kind, Kind::Switch | Kind::DipSwitch))
            })
            .map(|(id, value)| ((*id, 0), *value))
            .collect();
        self.values = self.network.resolve(&self.drives);
        self.steps = 0;
        self.unsettled_steps = 0;
        self.limited = false;
        self.schedule_components(true);
    }
    fn schedule(&mut self, id: Id, value: Value) {
        self.drive_port((id, 0), value);
    }
    fn drive_port(&mut self, port: network::Port, value: Value) {
        if self.drives.get(&port) == Some(&value) {
            self.pending.remove(&port);
        } else {
            self.pending.insert(port, value);
        }
    }
    fn external_input(&mut self, id: Id, value: Value) {
        if let Some(input) = self.inputs.get_mut(&id) {
            *input = value;
            self.schedule(id, value);
        }
    }
    fn schedule_components(&mut self, initializing: bool) {
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
                    Kind::ControlledBuffer | Kind::ControlledInverter => bus::evaluate(
                        c,
                        self.port_value(c.id, 1).expect("compiled data port"),
                        self.port_value(c.id, 2)
                            .expect("compiled enable port")
                            .bit(0),
                    ),
                    Kind::Transistor | Kind::TransmissionGate => transistors::evaluate(
                        c,
                        self.port_value(c.id, 1).expect("compiled transistor data"),
                        self.port_value(c.id, 2)
                            .expect("compiled transistor control")
                            .bit(0),
                        self.port_value(c.id, 3).map_or(Bit::Unknown, |v| v.bit(0)),
                    ),
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
        let multiple_outputs: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| matches!(c.kind, Kind::Plexer(_) | Kind::Arithmetic(_)))
            // Restore held outputs before sampling freshly compiled inputs.
            .filter(|c| {
                !(initializing
                    && c.kind == Kind::Plexer(logisim_core::Plexer::DisplayDecoder)
                    && self.drives.contains_key(&(c.id, 0)))
            })
            .flat_map(|c| {
                let ports: Vec<_> = (0..c.ports().len())
                    .map(|i| self.port_value(c.id, i).expect("compiled component port"))
                    .collect();
                let outputs = match c.kind {
                    Kind::Arithmetic(_) => arithmetic::evaluate(c, &ports),
                    _ => plexers::evaluate(c, &ports),
                };
                outputs
                    .into_iter()
                    .map(|(i, v)| ((c.id, i), v))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (port, value) in multiple_outputs {
            self.drive_port(port, value);
        }
        self.schedule_controls();
        self.schedule_storage();
        self.update_displays();
        self.schedule_memories();
    }
    fn schedule_memories(&mut self) {
        let samples: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind.is_memory())
            .map(|c| {
                let ports = (0..c.ports().len())
                    .map(|i| self.port_value(c.id, i).expect("compiled memory port"))
                    .collect::<Vec<_>>();
                (c.clone(), ports)
            })
            .collect();
        for (c, ports) in samples {
            let output = if c.kind == Kind::Rom {
                memory::rom(&c, &ports)
            } else {
                self.memories
                    .get_mut(&c.id)
                    .expect("RAM state")
                    .evaluate(&c, &ports)
            };
            self.drive_port((c.id, 0), output);
        }
    }
    pub fn memory_contents(&self, id: Id) -> Option<&logisim_core::MemoryContents> {
        self.memories.get(&id).map(|s| &s.contents)
    }
    pub fn set_memory_word(&mut self, id: Id, address: u32, value: u32) {
        if let Some(end) = address.checked_add(1) {
            self.fill_memory(id, address, end, value);
        }
    }
    pub fn fill_memory(&mut self, id: Id, start: u32, end: u32, value: u32) {
        let Some(c) = self.circuit.component(id) else {
            return;
        };
        if start >= end || end > c.memory_capacity() {
            return;
        }
        let mask = u32::MAX >> (32 - c.bits.clamp(1, 32));
        let Some(state) = self.memories.get_mut(&id) else {
            return;
        };
        state.contents.fill(start, end, value & mask);
        self.limited = false;
        self.unsettled_steps = 0;
        self.schedule_components(false);
    }
    pub fn clear_memory(&mut self, id: Id) {
        let Some(state) = self.memories.get_mut(&id) else {
            return;
        };
        state.contents.clear();
        self.limited = false;
        self.unsettled_steps = 0;
        self.schedule_components(false);
    }
    fn schedule_storage(&mut self) {
        let samples: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind.is_sequential())
            .map(|c| {
                let ports: Vec<_> = (0..c.ports().len())
                    .map(|i| self.port_value(c.id, i).expect("compiled sequential port"))
                    .collect();
                (c.clone(), ports)
            })
            .collect();
        for (c, ports) in samples {
            let output = self
                .storage
                .get_mut(&c.id)
                .expect("sequential state")
                .evaluate(&c, &ports);
            self.drive_port((c.id, 0), output);
            if c.kind == Kind::Counter {
                let carry = self.storage[&c.id].carry(&c, &ports);
                self.drive_port((c.id, 5), carry);
            }
            if matches!(c.kind, Kind::FlipFlop(_)) {
                self.drive_port((c.id, 1), Value::known(1, output.number().unwrap_or(0) ^ 1));
            }
        }
    }
    pub fn stored_value(&self, id: Id) -> Option<Value> {
        self.storage
            .get(&id)
            .filter(|_| {
                self.circuit
                    .component(id)
                    .is_some_and(|c| c.kind.is_storage())
            })
            .map(|s| s.value)
    }
    /// Tick controls are useful for clock sources and for display persistence.
    pub fn has_clocks(&self) -> bool {
        self.circuit.components.iter().any(|c| {
            c.kind == Kind::Clock || (c.kind == Kind::DotMatrix && c.io.matrix_persistence > 0)
        })
    }
    /// Queue a tick only on a settled network, so separate edges cannot collapse into one batch.
    pub fn tick(&mut self) -> bool {
        if !self.pending.is_empty() || self.limited {
            return false;
        }
        self.tick_clocks();
        self.has_clocks()
    }
    fn tick_clocks(&mut self) {
        self.display_ticks = self.display_ticks.saturating_add(1);
        self.update_displays();
        let clocks: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Clock)
            .cloned()
            .collect();
        for c in clocks {
            let state = self.storage.get_mut(&c.id).unwrap();
            state.tick(&c);
            let value = state.value;
            self.drive_port((c.id, 0), value);
        }
    }
    pub fn poke_clock(&mut self, id: Id) {
        if self
            .circuit
            .component(id)
            .is_some_and(|c| c.kind == Kind::Clock)
        {
            let state = self.storage.get_mut(&id).unwrap();
            state.toggle();
            let value = state.value;
            self.drive_port((id, 0), value);
            self.limited = false;
            self.unsettled_steps = 0;
        }
    }
    pub fn toggle_stored(&mut self, id: Id) {
        if self
            .circuit
            .component(id)
            .is_some_and(|c| matches!(c.kind, Kind::FlipFlop(_)))
        {
            self.storage.get_mut(&id).unwrap().toggle();
            // Apply asserted asynchronous controls and output scheduling through the same path.
            self.schedule_storage();
            self.limited = false;
            self.unsettled_steps = 0;
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
        self.schedule_components(false);
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
                components: self.pending.keys().map(|(id, _)| *id).collect(),
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
            c.interface
                .ports
                .iter()
                .map(|p| (p.pin, p.bits, p.output))
                .collect::<Vec<_>>(),
            c.output && c.kind == Kind::Pin,
            (c.kind == Kind::Constant).then_some(c.constant),
            (c.kind == Kind::Splitter).then_some(c.splitter.clone()),
            (c.kind == Kind::BitExtender).then_some((c.input_bits, c.extension)),
            c.kind.is_sequential().then_some(c.sequential),
            (
                (c.kind.is_controlled() || c.kind == Kind::PullResistor).then_some(c.bus),
                c.kind.is_transistor().then_some(c.transistor),
                matches!(c.kind, Kind::Plexer(_)).then_some(c.plexing),
                matches!(c.kind, Kind::Arithmetic(_)).then_some(c.arithmetic),
                c.kind.is_memory().then_some(c.memory.clone()),
                (c.kind == Kind::Slider).then_some(c.io.slider_value),
                (c.kind == Kind::RgbLed).then_some(c.io.rgb_multibit),
                (c.kind == Kind::DotMatrix).then_some((
                    c.io.matrix_columns,
                    c.io.matrix_rows,
                    c.io.matrix_input,
                )),
            ),
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

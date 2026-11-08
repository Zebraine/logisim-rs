//! Deterministic driver batches and component scheduling. Preserve gather-before-mutate order.
use crate::{
    Bit, Simulator, Value, arithmetic, bus, gates, memory, network, parity, pla, plexers,
    transistors, ttl,
};
use logisim_core::{Extension, Id, Kind};

impl Simulator {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled && self.limited {
            self.limited = false;
            self.progress.restart();
        }
    }
    pub fn steps(&self) -> u64 {
        self.progress.steps()
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn limited(&self) -> bool {
        self.limited
    }
    pub(crate) fn schedule(&mut self, id: Id, value: Value) {
        self.drive_port((id, 0), value);
    }
    pub(crate) fn drive_port(&mut self, port: network::Port, value: Value) {
        if self.drives.get(&port) == Some(&value) {
            self.pending.remove(&port);
        } else {
            self.pending.insert(port, value);
        }
    }
    pub(crate) fn schedule_components(&mut self, initializing: bool) {
        let outputs: Vec<_> = self
            .plan
            .single
            .iter()
            .map(|entry| {
                let c = &self.circuit.components[entry.component];
                let read = |index| entry.port(index, &self.values);
                let output = match c.kind {
                    Kind::Parity(_) => {
                        let inputs: Vec<_> = (1..=usize::from(c.input_count()))
                            .map(|i| entry.connected(i).then(|| read(i)))
                            .collect();
                        parity::evaluate(c, &inputs)
                    }
                    Kind::Gate(gate) => {
                        let inputs: Vec<_> = (1..=usize::from(c.input_count()))
                            .map(|i| (gate.unary() || entry.connected(i)).then(|| read(i)))
                            .collect();
                        gates::evaluate(c, &inputs)
                    }
                    Kind::ControlledBuffer | Kind::ControlledInverter => {
                        bus::evaluate(c, read(1), read(2).bit(0))
                    }
                    Kind::Transistor | Kind::TransmissionGate => transistors::evaluate(
                        c,
                        read(1),
                        read(2).bit(0),
                        entry
                            .optional_port(3, &self.values)
                            .map_or(Bit::Unknown, |v| v.bit(0)),
                    ),
                    Kind::PlaRom => pla::evaluate(&c.pla, read(1), read(2).bit(0)),
                    Kind::Constant => Value::known(c.bits, c.constant),
                    Kind::Power => Value::filled(c.bits, Bit::One),
                    Kind::Ground => Value::known(c.bits, 0),
                    Kind::BitExtender => {
                        let input = read(1);
                        let extra = match c.extension {
                            Extension::Zero => Bit::Zero,
                            Extension::One => Bit::One,
                            Extension::Sign => input.bit(input.width() - 1),
                            Extension::Input => read(2).bit(0),
                        };
                        Value::from_bits((0..c.bits.clamp(1, 32)).map(|i| {
                            if i < input.width() {
                                input.bit(i)
                            } else {
                                extra
                            }
                        }))
                    }
                    _ => unreachable!("compiled single-output component"),
                };
                (c.id, output)
            })
            .collect();
        for (id, value) in outputs {
            self.schedule(id, value);
        }
        let multiple_outputs: Vec<_> = self
            .plan
            .multiple
            .iter()
            // Restore held outputs before sampling freshly compiled inputs.
            .filter(|entry| {
                let c = &self.circuit.components[entry.component];
                !(initializing
                    && c.kind == Kind::Plexer(logisim_core::Plexer::DisplayDecoder)
                    && self.drives.contains_key(&(c.id, 0)))
            })
            .flat_map(|entry| {
                let c = &self.circuit.components[entry.component];
                let ports = entry.sample(&self.values);
                let outputs = match c.kind {
                    Kind::Arithmetic(_) => arithmetic::evaluate(c, &ports),
                    Kind::Ttl(_) => ttl::evaluate(c, &ports),
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
        self.schedule_plugins(None);
        self.update_displays();
        self.schedule_memories();
        self.schedule_terminals();
    }
    pub(crate) fn schedule_memories(&mut self) {
        let samples: Vec<_> = self
            .plan
            .memories
            .iter()
            .map(|entry| (entry.component, entry.sample(&self.values)))
            .collect();
        for (index, ports) in samples {
            let c = &self.circuit.components[index];
            let output = if c.kind == Kind::Rom {
                memory::rom(c, &ports)
            } else {
                self.memories
                    .get_mut(&c.id)
                    .expect("RAM state")
                    .evaluate(c, &ports)
            };
            self.drive_port((c.id, 0), output);
        }
    }
    pub(crate) fn schedule_storage(&mut self) {
        let samples: Vec<_> = self
            .plan
            .storage
            .iter()
            .map(|entry| (entry.component, entry.sample(&self.values)))
            .collect();
        let mut outputs = Vec::with_capacity(samples.len());
        for (index, ports) in samples {
            let c = &self.circuit.components[index];
            let output = self
                .storage
                .get_mut(&c.id)
                .expect("sequential state")
                .evaluate(c, &ports);
            outputs.push(((c.id, 0), output));
            if c.kind == Kind::Counter {
                let carry = self.storage[&c.id].carry(c, &ports);
                outputs.push(((c.id, 5), carry));
            }
            if matches!(c.kind, Kind::FlipFlop(_) | Kind::Latch(_)) {
                outputs.push(((c.id, 1), Value::known(1, output.number().unwrap_or(0) ^ 1)));
            }
        }
        for (port, value) in outputs {
            self.drive_port(port, value);
        }
    }
    /// Commit one batch of driver changes, then schedule resulting component changes.
    pub fn step(&mut self) -> bool {
        if self.pending.is_empty() || self.limited {
            return false;
        }
        self.drives.extend(std::mem::take(&mut self.pending));
        self.values = self.network.resolve(&self.drives);
        self.schedule_components(false);
        if self.progress.record_batch(!self.pending.is_empty()) {
            self.limited = true;
            self.enabled = false;
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
}

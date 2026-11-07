//! Topology synchronization, retained instance state and electrical invalidation.
use crate::network::Network;
use crate::{Simulator, Value, io, memory, network, pins, register_file, sequential, terminal};
use logisim_core::{Circuit, Kind, Point};
use std::collections::BTreeMap;

impl Simulator {
    /// Recompile edited topology. Selection, input pokes and view changes are not document edits.
    pub fn synchronize(&mut self, circuit: &Circuit) -> bool {
        if self.circuit == *circuit {
            return false;
        }
        self.synchronize_displays(circuit);
        self.synchronize_terminals(circuit);
        self.synchronize_plugins(circuit);
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
                        .unwrap_or_else(|| pins::initial(c, bits))
                };
                (c.id, value)
            })
            .collect();
        self.joysticks = circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Joystick)
            .map(|c| (c.id, self.joysticks.get(&c.id).copied().unwrap_or_default()))
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
        self.register_files = circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::RegisterFile)
            .map(|c| (c.id, self.register_files.remove(&c.id).unwrap_or_default()))
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
    pub fn reset(&mut self) {
        self.plugin_states.clear();
        self.synchronize_plugins(&self.circuit.clone());
        self.reset_displays();
        self.terminals = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind.is_terminal())
            .map(|c| (c.id, terminal::State::new(c)))
            .collect();
        for offset in self.joysticks.values_mut() {
            *offset = Point::ZERO;
        }
        for (id, input) in &mut self.inputs {
            *input = pins::initial(
                self.circuit.component(*id).expect("interactive input"),
                input.width(),
            );
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
        self.register_files = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::RegisterFile)
            .map(|c| (c.id, register_file::State::default()))
            .collect();
        self.initialize(BTreeMap::new());
    }
    pub(crate) fn initialize(&mut self, held: BTreeMap<network::Port, Value>) {
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
        self.progress.reset();
        self.limited = false;
        self.schedule_components(true);
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
            (
                (c.kind == Kind::Pin).then_some((c.output, c.pin)),
                matches!(c.kind, Kind::Gate(_)).then_some(c.gate_options),
                c.plugin.clone(),
            ),
            (c.kind == Kind::Constant).then_some(c.constant),
            (c.kind == Kind::Splitter).then_some(c.splitter.clone()),
            (c.kind == Kind::BitExtender).then_some((c.input_bits, c.extension)),
            (c.kind.is_sequential() || c.kind == Kind::RegisterFile).then_some(c.sequential),
            (
                (c.kind.is_controlled() || c.kind == Kind::PullResistor).then_some(c.bus),
                c.kind.is_transistor().then_some(c.transistor),
                matches!(c.kind, Kind::Parity(_)).then_some((c.inputs, c.parity)),
                matches!(c.kind, Kind::Latch(_)).then_some(c.latch_enable),
                matches!(c.kind, Kind::Plexer(_)).then_some(c.plexing),
                matches!(c.kind, Kind::Arithmetic(_)).then_some(c.arithmetic),
                c.kind.is_memory().then_some(c.memory.clone()),
                (c.kind == Kind::PlaRom).then_some(c.pla.clone()),
                c.kind.is_terminal().then_some(c.terminal),
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

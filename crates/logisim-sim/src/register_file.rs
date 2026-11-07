use crate::{Bit, Simulator, Value};
use logisim_core::{Id, Kind, Trigger};

pub(crate) struct State {
    pub words: [Value; 32],
    clock: Bit,
}
impl Default for State {
    fn default() -> Self {
        Self {
            words: [Value::known(32, 0); 32],
            clock: Bit::Unknown,
        }
    }
}
impl State {
    fn read(&self, address: Value) -> Value {
        address
            .number()
            .and_then(|i| self.words.get(i as usize).copied())
            .unwrap_or_else(|| Value::filled(32, Bit::Unknown))
    }
    fn evaluate(&mut self, trigger: Trigger, ports: &[Value]) -> [Value; 2] {
        let clock = ports[3].bit(0);
        let write = match trigger {
            Trigger::Rising => self.clock == Bit::Zero && clock == Bit::One,
            Trigger::Falling => self.clock == Bit::One && clock == Bit::Zero,
            Trigger::High => clock == Bit::One,
            Trigger::Low => clock == Bit::Zero,
        };
        self.clock = clock;
        if write && ports[4].bit(0) != Bit::Zero {
            match ports[5].number() {
                Some(1..=31) => self.words[ports[5].number().unwrap() as usize] = ports[2],
                Some(_) => {}
                None => self.words[1..].fill(Value::filled(32, Bit::Unknown)),
            }
        }
        [self.read(ports[6]), self.read(ports[7])]
    }
}
impl Simulator {
    pub fn register_file(&self, id: Id) -> Option<&[Value; 32]> {
        self.register_files.get(&id).map(|s| &s.words)
    }
    /// Runtime-only Poke; zero is immutable and each hierarchy instance owns its state.
    pub fn set_register_file_word(&mut self, id: Id, address: u8, value: u32) {
        if !(1..32).contains(&address) {
            return;
        }
        let Some(state) = self.register_files.get_mut(&id) else {
            return;
        };
        state.words[usize::from(address)] = Value::known(32, value);
        self.limited = false;
        self.unsettled_steps = 0;
        self.schedule_register_files();
    }
    pub(crate) fn schedule_register_files(&mut self) {
        let samples: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::RegisterFile)
            .map(|c| {
                (
                    c.id,
                    c.sequential.trigger,
                    (0..8)
                        .map(|i| {
                            self.port_value(c.id, i)
                                .expect("compiled register-file port")
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        for (id, trigger, ports) in samples {
            let outputs = self
                .register_files
                .get_mut(&id)
                .expect("register-file state")
                .evaluate(trigger, &ports);
            for (i, value) in outputs.into_iter().enumerate() {
                self.drive_port((id, i), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ports() -> [Value; 8] {
        [
            Value::known(32, 0),
            Value::known(32, 0),
            Value::known(32, 0xa5a55a5a),
            Value::known(1, 0),
            Value::known(1, 1),
            Value::known(5, 1),
            Value::known(5, 1),
            Value::known(5, 0),
        ]
    }
    #[test]
    fn all_triggers_initial_clock_transparency_and_dual_read() {
        for trigger in Trigger::ALL {
            let mut s = State::default();
            let mut p = ports();
            assert_eq!(
                s.evaluate(trigger, &p)[0].number(),
                Some(if trigger == Trigger::Low {
                    0xa5a55a5a
                } else {
                    0
                })
            );
            p[3] = Value::known(1, 1);
            s.evaluate(trigger, &p);
            assert_eq!(
                s.words[1].number(),
                Some(if trigger == Trigger::Falling {
                    0
                } else {
                    0xa5a55a5a
                })
            );
            p[2] = Value::known(32, 42);
            s.evaluate(trigger, &p);
            assert_eq!(
                s.words[1].number(),
                Some(if trigger == Trigger::High {
                    42
                } else if trigger == Trigger::Falling {
                    0
                } else {
                    0xa5a55a5a
                })
            );
            p[3] = Value::known(1, 0);
            let out = s.evaluate(trigger, &p);
            assert_eq!(out[1].number(), Some(0));
            assert_eq!(
                out[0].number(),
                Some(if trigger == Trigger::Rising {
                    0xa5a55a5a
                } else {
                    42
                })
            );
        }
        for (trigger, clock) in [(Trigger::Rising, 1), (Trigger::Falling, 0)] {
            let mut s = State::default();
            let mut p = ports();
            p[3] = Value::known(1, clock);
            assert_eq!(s.evaluate(trigger, &p)[0].number(), Some(0));
        }
    }
    #[test]
    fn four_state_controls_addresses_and_data() {
        for we in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
            let mut s = State::default();
            let mut p = ports();
            p[3] = Value::known(1, 1);
            p[4] = Value::filled(1, we);
            assert_eq!(
                s.evaluate(Trigger::High, &p)[0].number(),
                Some(if we == Bit::Zero { 0 } else { 0xa5a55a5a })
            );
        }
        for bad in [Bit::Unknown, Bit::Error] {
            let mut s = State::default();
            let mut p = ports();
            p[3] = Value::filled(1, bad);
            assert_eq!(s.evaluate(Trigger::High, &p)[0].number(), Some(0));
            p[3] = Value::known(1, 1);
            p[2] = Value::filled(32, bad);
            assert_eq!(s.evaluate(Trigger::High, &p)[0], p[2]);
            p[2] = Value::known(32, 7);
            p[5] = Value::filled(5, bad);
            s.evaluate(Trigger::High, &p);
            assert_eq!(s.words[0].number(), Some(0));
            assert!(
                s.words[1..]
                    .iter()
                    .all(|v| *v == Value::filled(32, Bit::Unknown))
            );
            p[4] = Value::known(1, 0);
            p[6] = Value::filled(5, bad);
            assert_eq!(
                s.evaluate(Trigger::High, &p)[0],
                Value::filled(32, Bit::Unknown)
            );
        }
        let mut s = State::default();
        let mut p = ports();
        p[3] = Value::known(1, 1);
        p[5] = Value::known(5, 0);
        p[6] = p[5];
        assert_eq!(s.evaluate(Trigger::High, &p)[0].number(), Some(0));
        p[5] = Value::known(5, 31);
        p[6] = p[5];
        p[7] = p[5];
        p[2] = Value::from_bits((0..32).map(|i| if i == 0 { Bit::Unknown } else { Bit::Zero }));
        assert_eq!(s.evaluate(Trigger::High, &p), [p[2]; 2]);
    }

    fn add_file(circuit: &mut logisim_core::Circuit, x: i32) -> Id {
        use logisim_core::{Component, Point};
        let c = Component::new(Kind::RegisterFile, Point::new(x, 200));
        for (port, width, value) in [(4, 1, 1), (5, 5, 1), (6, 5, 1), (7, 5, 0)] {
            let mut source = Component::new(Kind::Constant, c.ports()[port]);
            source.bits = width;
            source.constant = value;
            circuit.add_component(source);
        }
        circuit.add_component(c)
    }
    fn settle(s: &mut Simulator) {
        s.advance(128);
        assert_eq!(s.pending_count(), 0);
        assert!(!s.limited());
    }
    #[test]
    fn network_same_edge_sampling_pause_poke_reset_and_passive_probe() {
        use logisim_core::{Circuit, Component, Point};
        let mut circuit = Circuit::default();
        let a = add_file(&mut circuit, 300);
        let b = add_file(&mut circuit, 800);
        let mut data = Component::new(Kind::Pin, Point::new(140, 190));
        data.bits = 32;
        let data = circuit.add_component(data);
        let clock = circuit.add_component(Component::new(Kind::Pin, Point::new(140, 280)));
        circuit.add_wire(Point::new(140, 280), Point::new(640, 280));
        circuit.add_wire(Point::new(300, 150), Point::new(640, 150));
        circuit.add_wire(Point::new(640, 150), Point::new(640, 190));
        let probe = circuit.add_component(Component::new(Kind::RiscVProbe, Point::new(500, 150)));
        let mut s = Simulator::default();
        s.synchronize(&circuit);
        settle(&mut s);
        s.set_input(data, 42);
        settle(&mut s);
        s.set_enabled(false);
        s.set_input(clock, 1);
        s.advance(128);
        assert_eq!(s.register_file(a).unwrap()[1].number(), Some(0));
        s.set_enabled(true);
        settle(&mut s);
        assert_eq!(s.port_value(a, 0).unwrap().number(), Some(42));
        assert_eq!(s.port_value(b, 0).unwrap().number(), Some(0));
        assert_eq!(s.port_value(probe, 0).unwrap().number(), Some(42));
        s.set_input(clock, 0);
        settle(&mut s);
        s.set_input(clock, 1);
        settle(&mut s);
        assert_eq!(s.port_value(b, 0).unwrap().number(), Some(42));
        s.set_register_file_word(a, 0, u32::MAX);
        s.set_register_file_word(a, 1, 0x12345678);
        settle(&mut s);
        assert_eq!(s.port_value(a, 0).unwrap().number(), Some(0x12345678));
        assert_eq!(s.port_value(a, 1).unwrap().number(), Some(0));
        let mut changed = circuit.component(a).unwrap().clone();
        changed.external_library = "jar#relocated.jar#edu.cornell.cs3410.Components".into();
        circuit.replace_component(changed);
        assert!(!s.synchronize(&circuit));
        assert_eq!(s.register_file(a).unwrap()[1].number(), Some(0x12345678));
        s.reset();
        settle(&mut s);
        assert!(
            s.register_file(a)
                .unwrap()
                .iter()
                .all(|v| v.number() == Some(0))
        );
        assert!(s.diagnostics().is_empty());
    }
    #[test]
    fn hierarchy_instances_isolate_words_clocks_and_runtime_pokes() {
        use logisim_core::{Circuit, Component, Point, Project};
        let mut project = Project::single("main", Circuit::default());
        let child = project.add("register bank".into()).unwrap();
        let circuit = &mut project.circuit_mut(child).unwrap().circuit;
        let id = add_file(circuit, 300);
        let mut data = Component::new(Kind::Constant, Point::new(140, 190));
        data.bits = 32;
        data.constant = 0x13579;
        circuit.add_component(data);
        let clock = circuit.add_component(Component::new(Kind::Clock, Point::new(140, 280)));
        project.refresh_interfaces(false);
        let a = project.instance(child, Point::new(100, 100)).unwrap();
        let b = project.instance(child, Point::new(300, 100)).unwrap();
        let root = project.main;
        let circuit = &mut project.circuit_mut(root).unwrap().circuit;
        let a = circuit.add_component(a);
        let b = circuit.add_component(b);
        let mut s = crate::ProjectSimulator::default();
        s.synchronize(&project, root);
        s.advance(256);
        s.poke_clock(&[a], clock);
        s.advance(256);
        assert_eq!(
            s.view(&[a]).unwrap().register_file(id).unwrap()[1].number(),
            Some(0x13579)
        );
        assert_eq!(
            s.view(&[b]).unwrap().register_file(id).unwrap()[1].number(),
            Some(0)
        );
        s.set_register_file_word(&[b], id, 31, u32::MAX);
        s.advance(256);
        assert_eq!(
            s.view(&[b]).unwrap().register_file(id).unwrap()[31].number(),
            Some(u32::MAX)
        );
        assert_eq!(
            s.view(&[a]).unwrap().register_file(id).unwrap()[31].number(),
            Some(0)
        );
        s.reset();
        s.advance(256);
        for path in [&[a][..], &[b][..]] {
            assert!(
                s.view(path)
                    .unwrap()
                    .register_file(id)
                    .unwrap()
                    .iter()
                    .all(|v| v.number() == Some(0))
            );
        }
    }
}

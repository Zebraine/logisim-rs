//! Adapter between the scheduler and the independent native component pack.
use crate::{Simulator, Value};
use logisim_core::{Id, Kind};
pub(crate) use logisim_course::RegisterFile as State;

impl Simulator {
    pub fn register_file(&self, id: Id) -> Option<&[Value; 32]> {
        self.register_files.get(&id).map(State::words)
    }
    /// Runtime-only Poke; zero is immutable and each hierarchy instance owns its state.
    pub fn set_register_file_word(&mut self, id: Id, address: u8, value: u32) {
        let Some(state) = self.register_files.get_mut(&id) else {
            return;
        };
        if !state.poke(address, value) {
            return;
        }
        self.limited = false;
        self.progress.restart();
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
                    std::array::from_fn(|i| {
                        self.port_value(c.id, i)
                            .expect("compiled register-file port")
                    }),
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

use super::*;
use crate::{ProjectSimulator, Simulator};
use logisim_core::{Circuit, Component, Id, Kind, Point, Project};
fn network(pla: Pla, input: Value, select: Option<Bit>) -> (Circuit, Id, Id, Simulator) {
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::PlaRom, Point::new(200, 200));
    c.pla = pla;
    let ports = c.ports();
    let mut pin = Component::new(Kind::Pin, ports[1]);
    pin.bits = c.pla.dimensions().0;
    let data = circuit.add_component(pin);
    let enable = select.map(|_| circuit.add_component(Component::new(Kind::Pin, ports[2])));
    let id = circuit.add_component(c);
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    sim.external_input(data, input);
    if let Some(en) = enable {
        sim.external_input(en, Value::filled(1, select.unwrap()));
    }
    sim.advance(64);
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
    (circuit, id, data, sim)
}
#[test]
fn observed_literal_and_select_four_state_matrices() {
    use Bit::*;
    let states = [Zero, One, Unknown, Error];
    for (literal, expected) in [
        (PlaInput::Disconnected, [Error; 4]),
        (PlaInput::Inverted, [One, Zero, Error, Error]),
        (PlaInput::Direct, [Zero, One, Error, Error]),
    ] {
        for connected in [false, true] {
            for (data, result) in states.into_iter().zip(expected) {
                for high in [false, true] {
                    for sel in [Some(Zero), Some(One), Some(Unknown), Some(Error), None] {
                        let mut pla = Pla::new(1, 1, 1);
                        pla.select_high = high;
                        pla.set_input(0, 0, literal);
                        pla.set_output(0, 0, connected);
                        let (_, id, _, sim) = network(pla, Value::filled(1, data), sel);
                        let expected = if sel == Some(if high { Zero } else { One }) {
                            Unknown
                        } else if !connected {
                            Error
                        } else {
                            result
                        };
                        assert_eq!(
                            sim.port_value(id, 0),
                            Some(Value::filled(1, expected)),
                            "{literal:?} {connected} {data:?} {sel:?} {high}"
                        );
                    }
                }
            }
        }
    }
}
fn observed_program() -> Pla {
    let mut p = Pla::new(2, 3, 4);
    p.set_input(0, 0, PlaInput::Direct);
    p.set_input(1, 1, PlaInput::Direct);
    for (row, col) in [(0, 0), (0, 1), (0, 3), (1, 0), (2, 2), (2, 3)] {
        p.set_output(row, col, true);
    }
    p
}
#[test]
fn observed_partial_data_and_empty_terms_resolve_per_output() {
    use Bit::*;
    for (input, expected, and, negated) in [
        (Value::known(2, 0), "00EE", "0", "0"),
        (Value::known(2, 1), "10EE", "0", "1"),
        (Value::known(2, 2), "11E1", "0", "0"),
        (Value::known(2, 3), "11E1", "1", "0"),
        (
            Value::from_bits([Zero, Unknown].into_iter()),
            "EEEE",
            "0",
            "0",
        ),
        (
            Value::from_bits([One, Unknown].into_iter()),
            "1EEE",
            "E",
            "E",
        ),
        (
            Value::from_bits([Error, Zero].into_iter()),
            "E0EE",
            "0",
            "E",
        ),
        (Value::from_bits([Error, One].into_iter()), "11E1", "E", "0"),
    ] {
        let (_, id, _, sim) = network(observed_program(), input, None);
        assert_eq!(sim.port_value(id, 0).unwrap().binary(), expected);
        let mut p = Pla::new(2, 1, 1);
        p.set_input(0, 0, PlaInput::Direct);
        p.set_input(0, 1, PlaInput::Direct);
        p.set_output(0, 0, true);
        let (_, id, _, sim) = network(p.clone(), input, None);
        assert_eq!(sim.port_value(id, 0).unwrap().binary(), and);
        p.set_input(0, 0, PlaInput::Inverted);
        let (_, id, _, sim) = network(p, input, None);
        assert_eq!(sim.port_value(id, 0).unwrap().binary(), negated);
    }
}
#[test]
fn maximum_planes_and_program_edits_resynchronize_and_reset_keeps_program() {
    let mut p = Pla::new(32, 32, 32);
    p.set_input(0, 0, PlaInput::Direct);
    p.set_input(31, 31, PlaInput::Inverted);
    for col in 0..16 {
        p.set_output(0, col, true);
        p.set_output(31, col + 16, true);
    }
    for (input, expected) in [
        (0, 0xffff),
        (1, 0),
        (0x8000_0000, u32::MAX),
        (0x8000_0001, 0xffff_0000),
    ] {
        let (_, id, _, sim) = network(p.clone(), Value::known(32, input), None);
        assert_eq!(sim.port_value(id, 0), Some(Value::known(32, expected)));
    }
    let (mut circuit, id, data, mut sim) = network(p, Value::known(32, 1), None);
    let mut c = circuit.component(id).unwrap().clone();
    c.pla.set_input(0, 0, PlaInput::Inverted);
    circuit.replace_component(c);
    assert!(sim.synchronize(&circuit));
    sim.advance(64);
    assert_eq!(sim.port_value(id, 0), Some(Value::known(32, 0xffff_0000)));
    sim.reset();
    sim.external_input(data, Value::known(32, 1));
    sim.advance(64);
    assert_eq!(sim.port_value(id, 0), Some(Value::known(32, 0xffff_0000)));
    let mut c = circuit.component(id).unwrap().clone();
    c.label = "用户标签".into();
    circuit.replace_component(c);
    assert!(!sim.synchronize(&circuit));
}
#[test]
fn hierarchical_instances_use_their_own_inputs_and_share_program_edits() {
    let mut inner = Circuit::default();
    let mut pin = Component::new(Kind::Pin, Point::new(100, 100));
    pin.bits = 2;
    inner.add_component(pin);
    let mut pla = Component::new(Kind::PlaRom, Point::new(100, 100));
    pla.pla = observed_program();
    let pla_id = inner.add_component(pla);
    let mut out = Component::new(Kind::Pin, Point::new(160, 100));
    out.output = true;
    out.bits = 4;
    inner.add_component(out);
    let mut project = Project::single("main", Circuit::default());
    let child = project.add("logic".into()).unwrap();
    project.circuit_mut(child).unwrap().circuit = inner;
    let root = project.main;
    let a = project
        .circuit_mut(root)
        .unwrap()
        .circuit
        .add_component(Component::new(
            Kind::Subcircuit(child),
            Point::new(300, 100),
        ));
    let b = project
        .circuit_mut(root)
        .unwrap()
        .circuit
        .add_component(Component::new(
            Kind::Subcircuit(child),
            Point::new(300, 300),
        ));
    project.refresh_interfaces(false);
    for (id, value) in [(a, 1), (b, 2)] {
        let c = project
            .circuit(root)
            .unwrap()
            .circuit
            .component(id)
            .unwrap();
        let index = c.interface.ports.iter().position(|p| !p.output).unwrap();
        let mut source = Component::new(Kind::Constant, c.ports()[index]);
        source.bits = 2;
        source.constant = value;
        project
            .circuit_mut(root)
            .unwrap()
            .circuit
            .add_component(source);
    }
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, root);
    sim.advance(128);
    assert_eq!(
        sim.view(&[a])
            .unwrap()
            .port_value(pla_id, 0)
            .unwrap()
            .binary(),
        "10EE"
    );
    assert_eq!(
        sim.view(&[b])
            .unwrap()
            .port_value(pla_id, 0)
            .unwrap()
            .binary(),
        "11E1"
    );
    let circuit = &mut project.circuit_mut(child).unwrap().circuit;
    let mut c = circuit.component(pla_id).unwrap().clone();
    c.pla.clear();
    circuit.replace_component(c);
    sim.synchronize(&project, root);
    sim.advance(128);
    for id in [a, b] {
        assert_eq!(
            sim.view(&[id])
                .unwrap()
                .port_value(pla_id, 0)
                .unwrap()
                .binary(),
            "EEEE"
        );
    }
}

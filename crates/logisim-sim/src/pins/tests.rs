use super::*;
use crate::ProjectSimulator;
use logisim_core::{Circuit, Facing, PinOptions, Point, Project};

#[test]
fn tristate_pokes_preserve_other_bits_pause_and_reset() {
    for bits in [1, 4, 32] {
        let mut circuit = Circuit::default();
        let mut pin = Component::new(Kind::Pin, Point::ZERO);
        pin.bits = bits;
        pin.pin = PinOptions {
            three_state: true,
            pull: PinPull::Up,
        };
        let id = circuit.add_component(pin);
        let binary = circuit.add_component(Component::new(Kind::Pin, Point::new(100, 0)));
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        sim.advance(16);
        assert_eq!(
            sim.port_value(id, 0),
            Some(Value::filled(bits, Bit::Unknown))
        );
        sim.set_enabled(false);
        for next in [Bit::Zero, Bit::One, Bit::Unknown] {
            sim.toggle_input(id, bits - 1);
            assert_eq!(sim.input(id).unwrap().bit(bits - 1), next);
            if bits > 1 {
                assert_eq!(sim.input(id).unwrap().bit(0), Bit::Unknown);
            }
            sim.step();
            assert_eq!(sim.port_value(id, 0).unwrap().bit(bits - 1), next);
        }
        sim.set_input(id, u32::MAX);
        sim.set_input_unknown(id);
        assert_eq!(sim.input(id), Some(Value::filled(bits, Bit::Unknown)));
        sim.set_input_unknown(binary);
        assert_eq!(sim.input(binary), Some(Value::known(1, 0)));
        sim.set_input(id, 3);
        sim.toggle_input(binary, 0);
        sim.reset();
        assert_eq!(sim.input(id), Some(Value::filled(bits, Bit::Unknown)));
        assert_eq!(sim.input(binary), Some(Value::known(1, 0)));
    }
}

#[test]
fn hierarchical_pin_pulls_match_initial_and_changed_four_state_matrix() {
    for tri in [false, true] {
        for pull in PinPull::ALL {
            let mut circuit = Circuit::default();
            let mut pin = Component::new(Kind::Pin, Point::ZERO);
            pin.bits = 4;
            pin.pin = PinOptions {
                three_state: tri,
                pull,
            };
            let id = circuit.add_component(pin);
            let mut sim = Simulator::default();
            sim.synchronize(&circuit);
            let unknown = Value::filled(4, Bit::Unknown);
            sim.external_input(id, unknown);
            sim.advance(16);
            let expected = if tri || pull == PinPull::None {
                unknown
            } else {
                Value::known(4, if pull == PinPull::Up { 15 } else { 0 })
            };
            assert_eq!(sim.port_value(id, 0), Some(expected));
            for value in [
                Value::known(4, 0),
                Value::known(4, 10),
                Value::filled(4, Bit::Error),
                Value::from_bits([Bit::Unknown, Bit::Zero, Bit::Zero, Bit::Zero].into_iter()),
                unknown,
            ] {
                sim.external_input(id, value);
                sim.advance(16);
                let expected = match pull {
                    PinPull::None => value,
                    PinPull::Up => match value.binary().as_str() {
                        "EEEE" | "XXXX" => Value::known(4, 15),
                        "000X" => Value::known(4, 1),
                        _ => value,
                    },
                    PinPull::Down => match value.binary().as_str() {
                        "EEEE" | "XXXX" | "000X" => Value::known(4, 0),
                        _ => value,
                    },
                };
                assert_eq!(
                    sim.port_value(id, 0),
                    Some(expected),
                    "{tri} {pull:?} {value:?}"
                );
            }
        }
    }
}

#[test]
fn pulled_instances_are_isolated_and_nested_pokes_cannot_override_parent() {
    let mut project = Project::default();
    let child = project.add("pulled".into()).unwrap();
    let mut pin = Component::new(Kind::Pin, Point::new(100, 100));
    pin.pin = PinOptions {
        three_state: true,
        pull: PinPull::Up,
    };
    let circuit = &mut project.circuit_mut(child).unwrap().circuit;
    let child_input = circuit.add_component(pin);
    let mut out = Component::new(Kind::Pin, Point::new(200, 100));
    out.output = true;
    out.facing = Facing::West;
    let child_output = circuit.add_component(out);
    circuit.add_wire(Point::new(100, 100), Point::new(200, 100));
    let mut pairs = vec![];
    for y in [100, 200] {
        let instance = project.instance(child, Point::new(300, y)).unwrap();
        let circuit = &mut project.main_circuit_mut().circuit;
        let instance = circuit.add_component(instance);
        let mut pin = Component::new(Kind::Pin, Point::new(200, y));
        pin.pin.three_state = true;
        let input = circuit.add_component(pin);
        circuit.add_wire(Point::new(200, y), Point::new(270, y));
        pairs.push((instance, input));
    }
    project.refresh_interfaces(false);
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, project.main);
    sim.advance(64);
    let (instance, input) = pairs[0];
    sim.set_input(input, 0);
    sim.advance(64);
    sim.set_input_unknown_at(&[], input);
    sim.advance(64);
    assert_eq!(
        sim.view(&[instance]).unwrap().port_value(child_output, 0),
        Some(Value::known(1, 1))
    );
    assert_eq!(
        sim.view(&[pairs[1].0]).unwrap().port_value(child_output, 0),
        Some(Value::filled(1, Bit::Unknown))
    );
    sim.set_input_unknown_at(&[instance], child_input);
    sim.advance(16);
    assert_eq!(
        sim.view(&[instance]).unwrap().input(child_input),
        Some(Value::known(1, 1))
    );
    sim.reset();
    sim.advance(64);
    assert_eq!(
        sim.view(&[instance]).unwrap().port_value(child_output, 0),
        Some(Value::filled(1, Bit::Unknown))
    );
    assert!(!sim.limited());
}

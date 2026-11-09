use crate::*;
use logisim_core::{Gate, Point};

use super::support::*;

#[test]
fn observed_truth_tables_through_real_networks() {
    for (gate, expected) in [
        (Gate::And, [0, 0, 0, 1]),
        (Gate::Or, [0, 1, 1, 1]),
        (Gate::Nand, [1, 1, 1, 0]),
        (Gate::Nor, [1, 0, 0, 0]),
        (Gate::Xor, [0, 1, 1, 0]),
        (Gate::Xnor, [1, 0, 0, 1]),
    ] {
        let (circuit, output, inputs) = fixture(gate, 2, 1);
        let mut sim = run(&circuit);
        for (vector, expected) in expected.into_iter().enumerate() {
            sim.set_input(inputs[0], (vector >> 1) as u32);
            sim.set_input(inputs[1], vector as u32);
            sim.advance(16);
            assert_eq!(
                sim.port_value(output, 0).unwrap().number(),
                Some(expected),
                "{gate:?} {vector}"
            );
        }
    }
    for (gate, expected) in [(Gate::Not, [1, 0]), (Gate::Buffer, [0, 1])] {
        let (circuit, output, inputs) = fixture(gate, 1, 1);
        let mut sim = run(&circuit);
        for (number, expected) in expected.into_iter().enumerate() {
            sim.set_input(inputs[0], number as u32);
            sim.advance(16);
            assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(expected));
        }
    }
}

#[test]
fn observed_three_input_bus_vectors_use_exactly_one_xor() {
    for (gate, expected) in [
        (Gate::And, [0, 8]),
        (Gate::Or, [14, 15]),
        (Gate::Xor, [6, 1]),
        (Gate::Xnor, [9, 14]),
    ] {
        let (circuit, output, inputs) = fixture(gate, 3, 4);
        let mut sim = run(&circuit);
        sim.set_input(inputs[0], 10);
        sim.set_input(inputs[1], 12);
        for (c, expected) in [0, 15].into_iter().zip(expected) {
            sim.set_input(inputs[2], c);
            sim.advance(16);
            assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(expected));
        }
        sim.reset();
        sim.advance(16);
        assert_eq!(sim.input(inputs[0]).unwrap().number(), Some(0));
    }
}

#[test]
fn open_inputs_are_ignored_but_floating_wires_are_not() {
    for (gate, open, floating) in [
        (Gate::And, 1, Bit::Error),
        (Gate::Or, 1, Bit::One),
        (Gate::Nand, 0, Bit::Error),
        (Gate::Nor, 0, Bit::Zero),
        (Gate::Xor, 1, Bit::Error),
        (Gate::Xnor, 0, Bit::Error),
    ] {
        let (mut circuit, output, inputs) = fixture(gate, 2, 1);
        let point = circuit.component(inputs[1]).unwrap().position;
        circuit.remove(&[inputs[1]].into());
        let mut sim = run(&circuit);
        sim.set_input(inputs[0], 1);
        sim.advance(16);
        assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(open));
        circuit.add_wire(point, point + Point::new(-40, 0));
        sim.synchronize(&circuit);
        sim.advance(16);
        assert_eq!(sim.port_value(output, 0).unwrap().bit(0), floating);
        circuit.remove(&[inputs[0]].into());
        circuit.wires.clear();
        sim.synchronize(&circuit);
        sim.advance(16);
        assert_eq!(sim.port_value(output, 0).unwrap().bit(0), Bit::Error);
    }
    for (gate, expected) in [(Gate::Not, Bit::Error), (Gate::Buffer, Bit::Unknown)] {
        let (mut circuit, output, inputs) = fixture(gate, 1, 1);
        circuit.remove(&inputs.into_iter().collect());
        assert_eq!(
            run(&circuit).port_value(output, 0).unwrap().bit(0),
            expected
        );
    }
}

#[test]
fn controlling_inputs_override_error_nets() {
    for (gate, control, expected) in [(Gate::And, 0, 0), (Gate::Or, 1, 1)] {
        let (mut circuit, output, inputs) = fixture(gate, 2, 1);
        let position = circuit.component(inputs[0]).unwrap().position;
        let conflict = pin(&mut circuit, position + Point::new(-40, 0), 1, false);
        circuit.add_wire(position, position + Point::new(-40, 0));
        let mut sim = run(&circuit);
        sim.set_input(conflict, 1);
        sim.set_input(inputs[1], control);
        sim.advance(16);
        assert_eq!(sim.port_value(inputs[0], 0).unwrap().bit(0), Bit::Error);
        assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(expected));
    }
}

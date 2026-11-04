use super::*;
use logisim_core::{Component, Gate, Point};

fn pin(circuit: &mut Circuit, point: Point, bits: u8, output: bool) -> Id {
    let mut pin = Component::new(Kind::Pin, point);
    pin.bits = bits;
    pin.output = output;
    circuit.add_component(pin)
}
fn fixture(gate: Gate, count: u8, bits: u8) -> (Circuit, Id, Vec<Id>) {
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::Gate(gate), Point::new(200, 100));
    c.bits = bits;
    c.inputs = count;
    let ports = c.ports();
    let gate = circuit.add_component(c);
    let inputs = ports
        .into_iter()
        .skip(1)
        .map(|p| pin(&mut circuit, p, bits, false))
        .collect();
    let output = Point::new(340, 100);
    pin(&mut circuit, output, bits, true);
    circuit.add_wire(Point::new(200, 100), output);
    (circuit, gate, inputs)
}
fn run(circuit: &Circuit) -> Simulator {
    let mut sim = Simulator::default();
    sim.synchronize(circuit);
    sim.advance(256);
    sim
}

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
fn pause_step_reset_and_cancelled_input_changes_preserve_batch_boundaries() {
    let (circuit, output, inputs) = fixture(Gate::And, 2, 1);
    let mut sim = run(&circuit);
    sim.set_enabled(false);
    sim.set_input(inputs[0], 1);
    sim.set_input(inputs[1], 1);
    sim.advance(100);
    assert_eq!(sim.port_value(inputs[0], 0).unwrap().number(), Some(0));
    assert_eq!(sim.input(inputs[0]).unwrap().number(), Some(1));
    sim.step();
    assert_eq!(sim.port_value(inputs[0], 0).unwrap().number(), Some(1));
    assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(0));
    sim.step();
    assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(1));
    assert!(!sim.step());
    sim.set_input(inputs[0], 0);
    sim.set_input(inputs[0], 1);
    assert_eq!(sim.pending_count(), 0);
    sim.reset();
    assert!(!sim.enabled());
    assert_eq!(sim.input(inputs[0]).unwrap().number(), Some(0));
    assert!(sim.port_value(output, 0).unwrap().contains(Bit::Unknown));
    sim.set_enabled(true);
    sim.advance(16);
    assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(0));
    assert_eq!(sim.circuit, circuit);
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
fn conflicting_drivers_resolve_per_bit_and_recover() {
    let mut circuit = Circuit::default();
    let a = pin(&mut circuit, Point::ZERO, 4, false);
    let b = pin(&mut circuit, Point::new(100, 0), 4, false);
    let wire = circuit.add_wire(Point::ZERO, Point::new(100, 0)).unwrap();
    let mut sim = run(&circuit);
    sim.set_input(a, 10);
    sim.set_input(b, 12);
    sim.advance(4);
    assert_eq!(sim.wire_value(wire).unwrap().binary(), "1EE0");
    assert!(
        sim.diagnostics()
            .iter()
            .any(|d| d.problem == Problem::ErrorSignal)
    );
    sim.set_input(b, 10);
    sim.advance(4);
    assert_eq!(sim.wire_value(wire).unwrap().number(), Some(10));
    assert!(sim.diagnostics().is_empty());
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

#[test]
fn crossings_stay_separate_until_an_endpoint_or_port_joins_them() {
    let mut circuit = Circuit::default();
    let a = pin(&mut circuit, Point::new(-100, 0), 1, false);
    let b = pin(&mut circuit, Point::new(0, -100), 1, false);
    let horizontal = circuit
        .add_wire(Point::new(-100, 0), Point::new(100, 0))
        .unwrap();
    let vertical = circuit
        .add_wire(Point::new(0, -100), Point::new(0, 100))
        .unwrap();
    let mut sim = run(&circuit);
    sim.set_input(b, 1);
    sim.advance(16);
    assert_eq!(sim.wire_value(horizontal).unwrap().number(), Some(0));
    assert_eq!(sim.wire_value(vertical).unwrap().number(), Some(1));
    let port = pin(&mut circuit, Point::ZERO, 1, true);
    sim.synchronize(&circuit);
    sim.advance(16);
    assert_eq!(sim.port_value(port, 0).unwrap().bit(0), Bit::Error);
    circuit.remove(&[port].into());
    circuit.add_wire(Point::ZERO, Point::new(50, 0));
    sim.synchronize(&circuit);
    sim.advance(16);
    assert_eq!(sim.port_value(a, 0).unwrap().bit(0), Bit::Error);
}

#[test]
fn tunnel_labels_are_case_sensitive_and_width_mismatches_are_not_separate_nets() {
    let mut circuit = Circuit::default();
    let source = pin(&mut circuit, Point::ZERO, 1, false);
    let mut ids = vec![];
    for (x, label, bits) in [(0, "DATA", 1), (100, "DATA", 1), (200, "data", 1)] {
        let mut c = Component::new(Kind::Tunnel, Point::new(x, 0));
        c.label = label.into();
        c.bits = bits;
        ids.push(circuit.add_component(c));
    }
    let mut sim = run(&circuit);
    sim.set_input(source, 1);
    sim.advance(4);
    assert_eq!(sim.port_value(ids[1], 0).unwrap().number(), Some(1));
    assert_eq!(sim.port_value(ids[2], 0).unwrap().bit(0), Bit::Unknown);
    let mut changed = circuit.component(ids[1]).unwrap().clone();
    changed.bits = 4;
    circuit.replace_component(changed);
    sim.synchronize(&circuit);
    sim.advance(4);
    assert!(
        sim.diagnostics()
            .iter()
            .any(|d| d.problem == Problem::WidthMismatch && d.components.contains(&source))
    );
    assert_eq!(sim.port_value(ids[0], 0).unwrap().binary(), "X");
    assert_eq!(sim.port_value(ids[1], 0).unwrap().binary(), "XXXX");
}

#[test]
fn edits_preserve_valid_inputs_prune_deleted_state_and_initialize_new_widths() {
    let (mut circuit, output, inputs) = fixture(Gate::Not, 1, 1);
    let mut sim = run(&circuit);
    sim.set_input(inputs[0], 1);
    sim.advance(16);
    let mut component = circuit.component(output).unwrap().clone();
    component.label = "renamed".into();
    circuit.replace_component(component);
    let steps = sim.steps();
    assert!(!sim.synchronize(&circuit));
    circuit.add_component(Component::new(Kind::Text, Point::new(400, 200)));
    assert!(!sim.synchronize(&circuit));
    assert_eq!(sim.steps(), steps);
    sim.advance(16);
    assert_eq!(sim.input(inputs[0]).unwrap().number(), Some(1));
    let mut component = circuit.component(inputs[0]).unwrap().clone();
    component.bits = 4;
    circuit.replace_component(component);
    sim.synchronize(&circuit);
    assert_eq!(sim.input(inputs[0]).unwrap().number(), Some(0));
    circuit.remove(&[inputs[0]].into());
    sim.synchronize(&circuit);
    assert_eq!(sim.input(inputs[0]), None);
    sim.set_input(inputs[0], 1);
    assert_eq!(sim.input(inputs[0]), None);
}

#[test]
fn full_u32_width_and_high_bit_toggles_do_not_overflow() {
    let (circuit, output, inputs) = fixture(Gate::Not, 1, 32);
    let mut sim = run(&circuit);
    assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(u32::MAX));
    sim.toggle_input(inputs[0], 31);
    sim.advance(16);
    assert_eq!(
        sim.port_value(output, 0).unwrap().number(),
        Some(0x7fff_ffff)
    );
    sim.toggle_input(inputs[0], 32);
    sim.advance(16);
    assert_eq!(sim.input(inputs[0]).unwrap().number(), Some(0x8000_0000));
}

#[test]
fn feedback_is_bounded_and_can_recover_after_changing_input() {
    let (mut circuit, output, inputs) = fixture(Gate::Nor, 2, 1);
    let feedback = circuit.component(inputs[0]).unwrap().position;
    circuit.remove(&[inputs[0]].into());
    circuit.add_route(Point::new(200, 100), feedback, false);
    let mut sim = run(&circuit);
    sim.set_input(inputs[1], 1);
    sim.advance(16);
    assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(0));
    sim.set_input(inputs[1], 0);
    let steps = sim.steps();
    sim.advance(3);
    assert_eq!(sim.steps() - steps, 3);
    assert!(sim.pending_count() > 0);
    sim.advance(PROPAGATION_LIMIT * 2);
    assert!(sim.limited());
    assert!(!sim.enabled());
    assert!(
        sim.diagnostics()
            .iter()
            .any(|d| d.problem == Problem::PropagationLimit)
    );
    assert!(!sim.step());
    sim.set_input(inputs[1], 1);
    sim.set_enabled(true);
    sim.advance(16);
    assert!(!sim.limited());
    assert_eq!(sim.pending_count(), 0);
    assert_eq!(sim.port_value(output, 0).unwrap().number(), Some(0));
}

fn source(circuit: &mut Circuit, at: Point, bits: u8, number: u32) -> Id {
    let mut c = Component::new(Kind::Constant, at);
    c.bits = bits;
    c.constant = number;
    circuit.add_component(c)
}
#[test]
fn fixed_sources_and_probe_width_follow_runtime_and_survive_reset() {
    let mut circuit = Circuit::default();
    for (x, kind, expected) in [
        (0, Kind::Power, u32::MAX),
        (100, Kind::Ground, 0),
        (200, Kind::Constant, 0xa6),
    ] {
        let mut c = Component::new(kind, Point::new(x, 0));
        c.bits = 32;
        c.constant = expected;
        circuit.add_component(c);
        let probe = circuit.add_component(Component::new(Kind::Probe, Point::new(x, 0)));
        let mut sim = run(&circuit);
        assert_eq!(sim.port_value(probe, 0), Some(Value::known(32, expected)));
        sim.reset();
        sim.advance(8);
        assert_eq!(sim.port_value(probe, 0), Some(Value::known(32, expected)));
    }
    let probe = circuit.add_component(Component::new(Kind::Probe, Point::new(400, 0)));
    assert_eq!(run(&circuit).port_value(probe, 0).unwrap().width(), 0);
}
#[test]
fn splitter_splits_merges_and_remaps_noncontiguous_bits_without_driving_unknowns() {
    let mut circuit = Circuit::default();
    let mut splitter = Component::new(Kind::Splitter, Point::new(200, 100));
    splitter.bits = 8;
    splitter.splitter.fanout = 3;
    splitter.splitter.distribute(8, false);
    let ports = splitter.ports();
    let id = circuit.add_component(splitter);
    let input = source(&mut circuit, ports[0], 8, 0xa6);
    let sim = run(&circuit);
    for (index, width, number) in [(1, 3, 6), (2, 3, 4), (3, 2, 2)] {
        assert_eq!(sim.port_value(id, index), Some(Value::known(width, number)));
    }
    let mut descending = circuit.component(id).unwrap().clone();
    descending.splitter.distribute(8, true);
    circuit.replace_component(descending.clone());
    let descending_sim = run(&circuit);
    for (index, width, number) in [(1, 3, 5), (2, 3, 1), (3, 2, 2)] {
        assert_eq!(
            descending_sim.port_value(id, index),
            Some(Value::known(width, number))
        );
    }
    descending.splitter.distribute(8, false);
    circuit.replace_component(descending);
    circuit.remove(&std::collections::BTreeSet::from([input]));
    for (index, width, number) in [(1, 3, 6), (2, 3, 4), (3, 2, 2)] {
        source(&mut circuit, ports[index], width, number);
    }
    assert_eq!(run(&circuit).port_value(id, 0), Some(Value::known(8, 0xa6)));
    let mut changed = circuit.component(id).unwrap().clone();
    changed.splitter.mapping[..8].copy_from_slice(&[
        Some(2),
        Some(0),
        Some(1),
        Some(0),
        Some(1),
        Some(0),
        Some(1),
        Some(2),
    ]);
    circuit.replace_component(changed);
    assert_eq!(run(&circuit).port_value(id, 0), Some(Value::known(8, 0xe8)));
}
#[test]
fn splitter_unknown_conflict_empty_and_mismatched_branches_stay_local() {
    let mut circuit = Circuit::default();
    let splitter = Component::new(Kind::Splitter, Point::ZERO);
    let ports = splitter.ports();
    let id = circuit.add_component(splitter);
    source(&mut circuit, ports[1], 1, 1);
    assert_eq!(run(&circuit).port_value(id, 0).unwrap().binary(), "XXX1");
    source(&mut circuit, ports[0], 4, 0xa);
    assert_eq!(run(&circuit).port_value(id, 0).unwrap().binary(), "101E");
    let mut changed = circuit.component(id).unwrap().clone();
    changed.splitter.mapping[0] = None;
    circuit.replace_component(changed);
    assert_eq!(run(&circuit).port_value(id, 0).unwrap().binary(), "1010");
    source(&mut circuit, ports[2], 2, 3);
    let sim = run(&circuit);
    assert!(sim.port_mismatch(id, 2));
    assert_eq!(sim.port_value(id, 0).unwrap().binary(), "1010");
    assert_eq!(sim.port_value(id, 3), Some(Value::known(1, 0)));
}
#[test]
fn chained_splitters_share_bits_bidirectionally_and_passive_cycles_settle() {
    let mut circuit = Circuit::default();
    let mut first = Component::new(Kind::Splitter, Point::ZERO);
    first.bits = 8;
    first.splitter.fanout = 2;
    first.splitter.distribute(8, false);
    let ports = first.ports();
    let a = circuit.add_component(first);
    let second = Component::new(Kind::Splitter, ports[2]);
    let ends = second.ports();
    let b = circuit.add_component(second);
    source(&mut circuit, ends[4], 1, 1);
    source(&mut circuit, ports[1], 4, 6);
    let sim = run(&circuit);
    assert_eq!(sim.port_value(a, 0).unwrap().binary(), "1XXX0110");
    assert_eq!(sim.port_value(b, 0).unwrap().binary(), "1XXX");
    // Connect two individual branches. No component actively feeds back its resolved value.
    let outer = Point::new(ends[1].x + 20, ends[1].y);
    let bottom = Point::new(outer.x, ends[4].y);
    circuit.add_wire(ends[1], outer);
    circuit.add_wire(outer, bottom);
    circuit.add_wire(bottom, ends[4]);
    let sim = run(&circuit);
    assert!(!sim.limited());
    assert_eq!(sim.pending_count(), 0);
    assert_eq!(sim.port_value(a, 0).unwrap().binary(), "1XX10110");
}
#[test]
fn extender_modes_truncation_and_external_unknown_match_observed_vectors() {
    for (mode, input_bits, bits, input, extension, expected) in [
        (Extension::Zero, 4, 8, 0xa, None, "00001010"),
        (Extension::One, 4, 8, 0xa, None, "11111010"),
        (Extension::Sign, 4, 8, 0xa, None, "11111010"),
        (Extension::Sign, 4, 8, 0x2, None, "00000010"),
        (Extension::Input, 4, 8, 0xa, None, "XXXX1010"),
        (Extension::Input, 4, 8, 0xa, Some(1), "11111010"),
        (Extension::Input, 4, 8, 0xa, Some(0), "00001010"),
        (Extension::Zero, 8, 4, 0xa6, None, "0110"),
    ] {
        let mut circuit = Circuit::default();
        let mut c = Component::new(Kind::BitExtender, Point::ZERO);
        c.input_bits = input_bits;
        c.bits = bits;
        c.extension = mode;
        let ports = c.ports();
        let id = circuit.add_component(c);
        source(&mut circuit, ports[1], input_bits, input);
        if let Some(value) = extension {
            source(&mut circuit, ports[2], 1, value);
        }
        let sim = run(&circuit);
        assert_eq!(sim.port_value(id, 0).unwrap().binary(), expected);
        assert!(!sim.port_mismatch(id, 0));
    }
}
#[test]
fn changing_source_value_or_bit_mapping_recompiles_but_probe_radix_does_not() {
    let mut circuit = Circuit::default();
    let id = source(&mut circuit, Point::ZERO, 8, 0xa6);
    let probe = circuit.add_component(Component::new(Kind::Probe, Point::ZERO));
    let mut sim = run(&circuit);
    circuit
        .components
        .iter_mut()
        .find(|c| c.id == probe)
        .unwrap()
        .radix = logisim_core::Radix::Hex;
    assert!(!sim.synchronize(&circuit));
    circuit
        .components
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .constant = 0x5a;
    assert!(sim.synchronize(&circuit));
    sim.advance(16);
    assert_eq!(sim.port_value(probe, 0), Some(Value::known(8, 0x5a)));
}

use crate::*;
use logisim_core::{Component, Extension, Kind, Point};

use super::support::*;

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

#[test]
fn west_centered_splitter_combines_31_bit_zero_and_comparison_without_mismatch() {
    use logisim_core::{Facing, SplitAppearance};
    let mut c = Circuit::default();
    let mut split = Component::new(Kind::Splitter, Point::new(450, 910));
    split.bits = 32;
    split.facing = Facing::West;
    split.splitter.fanout = 2;
    split.splitter.appearance = SplitAppearance::Center;
    split.splitter.mapping.fill(Some(0));
    split.splitter.mapping[0] = Some(1);
    let id = c.add_component(split);
    let mut high = Component::new(Kind::Constant, Point::new(420, 900));
    high.bits = 31;
    high.constant = 0;
    c.add_component(high);
    c.add_wire(Point::new(420, 900), Point::new(430, 900));
    let mut low = Component::new(Kind::Constant, Point::new(320, 910));
    low.constant = 1;
    c.add_component(low);
    c.add_wire(Point::new(320, 910), Point::new(430, 910));
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.advance(64);
    assert!(sim.diagnostics().is_empty());
    assert_eq!(sim.port_value(id, 0), Some(Value::known(32, 1)));
}

use crate::*;
use logisim_core::{Component, Kind, Point};

use super::support::*;

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
fn port_reads_follow_width_repairs_and_deleted_connections() {
    let mut circuit = Circuit::default();
    let source = pin(&mut circuit, Point::ZERO, 8, false);
    let output = pin(&mut circuit, Point::ZERO, 4, true);
    let probe = circuit.add_component(Component::new(Kind::Probe, Point::ZERO));
    let mut sim = run(&circuit);
    for (id, width) in [(source, 8), (output, 4), (probe, 8)] {
        assert!(sim.port_mismatch(id, 0));
        assert_eq!(
            sim.port_value(id, 0),
            Some(Value::filled(width, Bit::Unknown))
        );
    }
    let mut edited = circuit.component(output).unwrap().clone();
    edited.bits = 8;
    circuit.replace_component(edited);
    assert!(sim.synchronize(&circuit));
    sim.set_input(source, 0xa5);
    sim.advance(16);
    for id in [source, output, probe] {
        assert!(!sim.port_mismatch(id, 0));
        assert_eq!(sim.port_value(id, 0), Some(Value::known(8, 0xa5)));
        assert_eq!(sim.port_value(id, 1), None);
    }
    circuit.remove(&[source, output].into());
    assert!(sim.synchronize(&circuit));
    sim.advance(16);
    assert_eq!(sim.port_value(source, 0), None);
    assert_eq!(sim.port_value(output, 0), None);
    assert_eq!(
        sim.port_value(probe, 0),
        Some(Value::filled(0, Bit::Unknown))
    );
}

#[test]
fn normalization_preserves_connected_junction_signals_and_insulated_crossings() {
    let mut c = Circuit::default();
    let input = pin(&mut c, Point::new(0, 50), 1, false);
    let connected = pin(&mut c, Point::new(50, 100), 1, true);
    c.add_wire(Point::new(0, 50), Point::new(50, 50));
    c.add_wire(Point::new(50, 50), Point::new(100, 50));
    c.add_wire(Point::new(20, 50), Point::new(80, 50));
    c.add_wire(Point::new(50, 0), Point::new(50, 100));
    let other = pin(&mut c, Point::new(200, 50), 1, false);
    let isolated = pin(&mut c, Point::new(250, 100), 1, true);
    c.add_wire(Point::new(200, 50), Point::new(300, 50));
    c.add_wire(Point::new(250, 0), Point::new(250, 100));
    let mut s = run(&c);
    s.set_input(input, 1);
    s.set_input(other, 1);
    s.advance(64);
    let values = (s.port_value(connected, 0), s.port_value(isolated, 0));
    assert_eq!(values.0.unwrap().number(), Some(1));
    assert_eq!(values.1.unwrap().bit(0), Bit::Unknown);
    c.normalize_wires();
    s.synchronize(&c);
    s.advance(64);
    assert_eq!(
        (s.port_value(connected, 0), s.port_value(isolated, 0)),
        values
    );
    assert!(s.diagnostics().is_empty());
}

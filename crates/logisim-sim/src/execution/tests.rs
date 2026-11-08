use crate::{Bit, Simulator, Value};
use logisim_core::{Circuit, Component, Gate, Kind, Point};

fn inverter() -> (Circuit, u64, u64) {
    let mut circuit = Circuit::default();
    let mut gate = Component::new(Kind::Gate(Gate::Not), Point::new(200, 100));
    gate.bits = 4;
    let mut input = Component::new(Kind::Pin, gate.ports()[1]);
    input.bits = 4;
    let gate = circuit.add_component(gate);
    let input = circuit.add_component(input);
    (circuit, gate, input)
}
fn settle(sim: &mut Simulator) {
    sim.advance(128);
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
}

#[test]
fn text_insertions_and_removal_refresh_indices_without_resetting_state() {
    let (mut circuit, gate, input) = inverter();
    let clock = circuit.add_component(Component::new(Kind::Clock, Point::new(400, 100)));
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    settle(&mut sim);
    assert!(sim.tick());
    settle(&mut sim);
    let clock_before = sim.port_value(clock, 0);
    let steps_before = sim.steps();
    let text = circuit.add_component(Component::new(Kind::Text, Point::ZERO));
    let added = circuit.components.pop().unwrap();
    circuit.components.insert(0, added);
    assert!(!sim.synchronize(&circuit));
    assert_eq!(sim.steps(), steps_before);
    sim.set_input(input, 3);
    settle(&mut sim);
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 12)));
    assert_eq!(sim.port_value(clock, 0), clock_before);
    circuit.remove(&[text].into());
    assert!(!sim.synchronize(&circuit));
    sim.set_input(input, 9);
    settle(&mut sim);
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 6)));
    sim.reset();
    settle(&mut sim);
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 15)));
}

#[test]
fn port_readers_follow_width_repairs_reordering_and_component_replacement() {
    let (mut circuit, gate, input) = inverter();
    circuit.components[1].bits = 8;
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    settle(&mut sim);
    assert_eq!(
        sim.plan.single[0].port(1, &sim.values),
        Value::filled(4, Bit::Unknown)
    );
    assert!(sim.port_mismatch(gate, 1));
    circuit.components[1].bits = 4;
    circuit.components.reverse();
    assert!(sim.synchronize(&circuit));
    sim.set_input(input, 6);
    settle(&mut sim);
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 9)));
    assert_eq!(sim.plan.single[0].port(1, &sim.values), Value::known(4, 6));
    let mut changed = circuit.component(gate).unwrap().clone();
    changed.kind = Kind::Rom;
    changed.memory.contents.set(0, 10);
    circuit.replace_component(changed);
    assert!(sim.synchronize(&circuit));
    assert!(sim.plan.single.is_empty());
    assert_eq!(sim.plan.memories.len(), 1);
    circuit.remove(&[gate].into());
    assert!(sim.synchronize(&circuit));
    settle(&mut sim);
    assert_eq!(sim.port_value(gate, 0), None);
    assert!(sim.plan.memories.is_empty());
}

#[test]
fn display_persistence_edits_invalidate_clock_presence_without_recompiling_nets() {
    let mut circuit = Circuit::default();
    let mut matrix = Component::new(Kind::DotMatrix, Point::ZERO);
    matrix.io.matrix_persistence = 0;
    let id = circuit.add_component(matrix.clone());
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    settle(&mut sim);
    assert!(!sim.has_clocks());
    matrix.id = id;
    matrix.io.matrix_persistence = 3;
    circuit.replace_component(matrix.clone());
    assert!(!sim.synchronize(&circuit));
    assert!(sim.has_clocks());
    assert!(sim.tick());
    matrix.io.matrix_persistence = 0;
    circuit.replace_component(matrix);
    assert!(!sim.synchronize(&circuit));
    assert!(!sim.has_clocks());
    let clock = circuit.add_component(Component::new(Kind::Clock, Point::new(500, 100)));
    assert!(sim.synchronize(&circuit));
    assert!(sim.has_clocks());
    circuit.remove(&[clock].into());
    assert!(sim.synchronize(&circuit));
    assert!(!sim.has_clocks());
}

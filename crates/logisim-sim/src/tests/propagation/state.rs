use crate::*;
use logisim_core::{Component, Gate, Kind, Point};

use super::support::*;

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

use crate::ProjectSimulator;
use crate::stable::*;
use logisim_core::{FlipFlop, Gate, Point, Project};

use super::support::*;

#[test]
fn hierarchical_closure_finishes_before_sampling_and_keeps_internal_views() {
    let mut p = Project::default();
    let root = p.main;
    let (child, child_in, child_out) = buffered_child(&mut p, 16);
    let (first, pins) = instance(&mut p, child, root, Point::new(600, 600));
    let ip = pins.iter().find(|p| p.0 == child_in).unwrap().1;
    let op = pins.iter().find(|p| p.0 == child_out).unwrap().1;
    let (second, _) = instance(&mut p, child, root, Point::new(900, 900));
    let c = &mut p.main_circuit_mut().circuit;
    let data = driven(c, (first, ip));
    let other = driven(c, (second, ip));
    let ff = add(c, Kind::FlipFlop(FlipFlop::D), 1);
    let clock = driven(c, (ff, 2));
    link(c, (first, op), (ff, 3));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    sim.settle_compiled().unwrap();
    sim.set_input(data, 1);
    sim.set_input(other, 0);
    sim.set_input(clock, 1);
    let report = sim.settle_compiled().unwrap();
    assert_eq!(report.instances, 3);
    assert_eq!(report.compiled_nodes, 32);
    assert_eq!(stored(sim.view(&[]).unwrap(), ff), 1);
    assert_eq!(
        sim.view(&[first]).unwrap().port_value(child_out, 0),
        Some(Value::known(1, 1))
    );
    assert_eq!(
        sim.view(&[second]).unwrap().port_value(child_out, 0),
        Some(Value::known(1, 0))
    );
    assert!(sim.diagnostics(&[]).is_empty());
    // Pure nested wires are materialized, not just external output pins.
    for c in &p.circuit(child).unwrap().circuit.components {
        if matches!(c.kind, Kind::Gate(_)) {
            assert_eq!(
                sim.view(&[first]).unwrap().port_value(c.id, 0),
                Some(Value::known(1, 1))
            );
        }
    }
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!(stored(sim.view(&[]).unwrap(), ff), 0);
}

#[test]
fn registers_in_sibling_instances_sample_before_any_new_outputs_cross_bridges() {
    let mut p = Project::default();
    let root = p.main;
    let child = p.add("state".into()).unwrap();
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let ff = add(c, Kind::FlipFlop(FlipFlop::D), 1);
    let clock = driven(c, (ff, 2));
    let data = driven(c, (ff, 3));
    let output = output_pin(c);
    link(c, (ff, 0), (output, 0));
    let (a, ports) = instance(&mut p, child, root, Point::new(300, 1000));
    let (b, _) = instance(&mut p, child, root, Point::new(900, 1000));
    let index = |pin| ports.iter().find(|p| p.0 == pin).unwrap().1;
    let c = &mut p.main_circuit_mut().circuit;
    let clk = driven(c, (a, index(clock)));
    link(c, (clk, 0), (b, index(clock)));
    let d = driven(c, (a, index(data)));
    link(c, (a, index(output)), (b, index(data)));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    sim.settle_compiled().unwrap();
    for edge in 0..3 {
        sim.set_input(clk, 0);
        sim.settle_compiled().unwrap();
        sim.set_input(d, 1);
        sim.set_input(clk, 1);
        sim.settle_compiled().unwrap();
        assert_eq!(stored(sim.view(&[a]).unwrap(), ff), 1);
        assert_eq!(stored(sim.view(&[b]).unwrap(), ff), u32::from(edge > 0));
    }
}

#[test]
fn feedback_across_subcircuit_ports_is_rejected_before_any_state_advances() {
    let mut p = Project::default();
    let root = p.main;
    let (child, input, output) = buffered_child(&mut p, 2);
    let (id, ports) = instance(&mut p, child, root, Point::new(300, 1000));
    let ip = ports.iter().find(|p| p.0 == input).unwrap().1;
    let op = ports.iter().find(|p| p.0 == output).unwrap().1;
    link(&mut p.main_circuit_mut().circuit, (id, op), (id, ip));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    let before = (sim.pending_count(), sim.steps());
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        }
    ));
    assert_eq!((sim.pending_count(), sim.steps()), before);
    // Removing the connection also invalidates the cached hierarchy rejection.
    p.main_circuit_mut()
        .circuit
        .components
        .retain(|c| c.kind != Kind::Tunnel);
    sim.synchronize(&p, root);
    sim.settle_compiled().unwrap();
}

#[test]
fn early_clock_and_reset_changes_survive_unrelated_late_hierarchy_batches() {
    let mut p = Project::default();
    let root = p.main;
    let (child, input, output) = buffered_child(&mut p, 4);
    let (id, ports) = instance(&mut p, child, root, Point::new(500, 1000));
    let ip = ports.iter().find(|p| p.0 == input).unwrap().1;
    let op = ports.iter().find(|p| p.0 == output).unwrap().1;
    let c = &mut p.main_circuit_mut().circuit;
    let unrelated = driven(c, (id, ip));
    let buffer = add(c, Kind::Gate(Gate::Buffer), 1);
    link(c, (id, op), (buffer, 1));
    let ff = add(c, Kind::FlipFlop(FlipFlop::D), 1);
    let clock = driven(c, (ff, 2));
    let clear = driven(c, (ff, 5));
    let high = add(c, Kind::Power, 1);
    link(c, (high, 0), (ff, 3));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    sim.settle_compiled().unwrap();
    for i in 0..8 {
        sim.set_input(clock, i & 1);
        sim.set_input(clear, u32::from(i % 4 == 3));
        sim.set_input(unrelated, i & 1);
        let report = sim.settle_compiled().unwrap();
        if i > 0 {
            assert!(report.combinational_passes >= 3);
        }
        assert_eq!(
            stored(sim.view(&[]).unwrap(), ff),
            u32::from(matches!(i % 4, 1 | 2))
        );
        assert_eq!(sim.settle_compiled().unwrap().sampled_components, 0);
    }
}

#[test]
fn cooperative_hierarchy_handoff_preserves_an_already_committed_clock() {
    let mut p = Project::default();
    let root = p.main;
    let (child, child_in, child_out) = buffered_child(&mut p, 8);
    let (inner, pins) = instance(&mut p, child, root, Point::new(600, 600));
    let ip = pins.iter().find(|p| p.0 == child_in).unwrap().1;
    let op = pins.iter().find(|p| p.0 == child_out).unwrap().1;
    let c = &mut p.main_circuit_mut().circuit;
    let input = driven(c, (inner, ip));
    let ff = add(c, Kind::FlipFlop(FlipFlop::D), 1);
    let clock = driven(c, (ff, 2));
    let data = driven(c, (ff, 3));
    let observe = output_pin(c);
    link(c, (inner, op), (observe, 0));
    for mode in 0..4 {
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, root);
        sim.settle_compiled().unwrap();
        sim.set_input(data, 1);
        sim.settle_compiled().unwrap();
        sim.set_input(clock, 1);
        sim.set_input(input, 1);
        sim.step_compiled().unwrap();
        assert!(sim.pending_count() > 0); // Interface still pending, clock already committed.
        assert_eq!(stored(sim.view(&[]).unwrap(), ff), 0);
        sim.set_enabled(false);
        if mode == 2 {
            // Cache an unfinished root, visit another definition, then restore it.
            sim.synchronize(&p, child);
            sim.settle_compiled().unwrap();
            sim.synchronize(&p, root);
        } else if mode == 3 {
            // A label edit must not restart propagation or erase the in-flight clock.
            let mut edited = p.clone();
            edited
                .main_circuit_mut()
                .circuit
                .components
                .iter_mut()
                .find(|c| c.id == ff)
                .unwrap()
                .label = "renamed".into();
            assert!(!sim.synchronize(&edited, root));
        }
        if mode == 1 {
            let steps = sim.steps();
            assert!(sim.step());
            assert_eq!(sim.steps(), steps + 1);
            assert_eq!(stored(sim.view(&[]).unwrap(), ff), 1, "handoff mode {mode}");
            while sim.step() {}
        } else {
            for _ in 0..64 {
                if sim.pending_count() == 0 {
                    break;
                }
                sim.step_compiled().unwrap();
            }
        }
        assert_eq!(sim.pending_count(), 0);
        assert_eq!(stored(sim.view(&[]).unwrap(), ff), 1, "handoff mode {mode}");
        assert_eq!(
            sim.view(&[]).unwrap().port_value(observe, 0),
            Some(Value::known(1, 1))
        );
        assert!(!sim.enabled());
        sim.settle_compiled().unwrap(); // Event -> compiled restores cached values safely.
        assert_eq!(stored(sim.view(&[]).unwrap(), ff), 1, "handoff mode {mode}");
    }
}

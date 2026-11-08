use super::tests::{add, buffered_child, driven, instance, link, stored};
use super::*;
use crate::ProjectSimulator;
use logisim_core::{Circuit, Component, FlipFlop, Gate, Point, Project, Pull};

#[test]
fn shared_controlled_bus_settles_before_same_edge_state_sample() {
    let mut c = Circuit::default();
    let a = add(&mut c, Kind::ControlledBuffer, 1);
    let b = add(&mut c, Kind::ControlledBuffer, 1);
    let data_a = driven(&mut c, (a, 1));
    let data_b = driven(&mut c, (b, 1));
    let enable_a = driven(&mut c, (a, 2));
    let enable_b = driven(&mut c, (b, 2));
    let ff = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
    let next = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
    let clock = driven(&mut c, (ff, 2));
    link(&mut c, (clock, 0), (next, 2));
    link(&mut c, (ff, 0), (next, 3));
    // All branches use the same source label, so both raw drivers meet at D.
    link(&mut c, (a, 0), (b, 0));
    link(&mut c, (a, 0), (ff, 3));
    let mut pull = Component::new(Kind::PullResistor, c.component(ff).unwrap().ports()[3]);
    pull.bus.pull = Pull::Zero;
    c.add_component(pull);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.set_input(data_a, 0);
    sim.set_input(data_b, 1);
    sim.settle_compiled().unwrap();
    let mut previous = 0;
    for (ea, eb, expected) in [(0, 1, 1), (1, 1, 1), (1, 0, 0), (0, 0, 0), (0, 1, 1)] {
        sim.set_input(clock, 0);
        sim.settle_compiled().unwrap();
        // Change the bus and clock together. A propagation intermediate must never
        // leak into either state sample, and the second FF sees the old first FF.
        sim.set_input(enable_a, ea);
        sim.set_input(enable_b, eb);
        sim.set_input(clock, 1);
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, ff), stored(&sim, next)), (expected, previous));
        previous = expected;
        let conflict = ea == 1 && eb == 1;
        assert_eq!(!sim.diagnostics().is_empty(), conflict);
        if conflict {
            assert_eq!(sim.port_value(ff, 3), Some(Value::filled(1, Bit::Error)));
        }
    }
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!((stored(&sim, ff), stored(&sim, next)), (0, 0));
}

#[test]
fn disabled_control_does_not_hide_feedback_through_shared_bus() {
    let mut c = Circuit::default();
    let control = add(&mut c, Kind::ControlledBuffer, 1);
    let pin = driven(&mut c, (control, 0));
    let enable = driven(&mut c, (control, 2));
    link(&mut c, (pin, 0), (control, 1));
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.set_input(enable, 0);
    let before = (sim.pending.clone(), sim.drives.clone(), sim.steps());
    assert_eq!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            component: control,
            reason: BoundaryReason::Feedback
        }
    );
    assert_eq!(
        (sim.pending.clone(), sim.drives.clone(), sim.steps()),
        before
    );
}

#[test]
fn hierarchy_merges_shared_outputs_and_rejects_any_feedback_source() {
    let mut p = Project::default();
    let root = p.main;
    let (child, input, output) = buffered_child(&mut p, 2);
    let (id, ports) = instance(&mut p, child, root, Point::new(400, 1000));
    let ip = ports.iter().find(|p| p.0 == input).unwrap().1;
    let op = ports.iter().find(|p| p.0 == output).unwrap().1;
    let c = &mut p.main_circuit_mut().circuit;
    let data = driven(c, (id, ip));
    let competing = driven(c, (id, op));
    let buffer = add(c, Kind::Gate(Gate::Buffer), 1);
    link(c, (competing, 0), (buffer, 1));
    let mut fast = ProjectSimulator::default();
    let mut slow = ProjectSimulator::default();
    fast.synchronize(&p, root);
    slow.synchronize(&p, root);
    for a in [0, 1, 0] {
        for b in [0, 1, 0] {
            for s in [&mut fast, &mut slow] {
                s.set_input(data, a);
                s.set_input(competing, b);
            }
            fast.settle_compiled().unwrap();
            slow.advance(128);
            assert_eq!(
                fast.view(&[]).unwrap().values,
                slow.view(&[]).unwrap().values
            );
            assert_eq!(
                fast.view(&[id]).unwrap().values,
                slow.view(&[id]).unwrap().values
            );
            assert_eq!(
                fast.view(&[]).unwrap().drive_value(id, op),
                Some(Value::known(1, a))
            );
        }
    }
    // The known root pin is not the only dependency: the child output also feeds
    // its own input across the boundary and must be retained in the cycle graph.
    link(&mut p.main_circuit_mut().circuit, (competing, 0), (id, ip));
    fast.synchronize(&p, root);
    let before = (fast.steps(), fast.pending_count());
    assert!(matches!(
        fast.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        }
    ));
    assert_eq!((fast.steps(), fast.pending_count()), before);
}

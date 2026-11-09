use crate::ProjectSimulator;
use crate::stable::*;
use logisim_core::{Circuit, FlipFlop, Gate, Latch, Point, Project};

use super::support::*;

#[test]
fn unsupported_preflight_is_atomic_and_the_event_engine_remains_available() {
    for kind in [
        Kind::Plugin,
        Kind::Ram,
        Kind::Latch(Latch::D),
        Kind::Keyboard,
    ] {
        let mut c = Circuit::default();
        let input = add(&mut c, Kind::Pin, 1);
        let gate = chain(&mut c, input, 3);
        let unsupported = add(&mut c, kind, 1);
        if kind == Kind::Ram {
            c.components
                .iter_mut()
                .find(|c| c.id == unsupported)
                .unwrap()
                .memory
                .interface = MemoryInterface::SharedAsync;
        }
        let mut sim = Simulator::default();
        sim.synchronize(&c);
        sim.set_input(input, 1);
        let before = (
            sim.drives.clone(),
            sim.pending.clone(),
            sim.values.clone(),
            sim.steps(),
        );
        assert_eq!(
            sim.settle_compiled().unwrap_err().reason,
            Failure::UnsupportedComponent(unsupported)
        );
        assert_eq!(
            (
                sim.drives.clone(),
                sim.pending.clone(),
                sim.values.clone(),
                sim.steps()
            ),
            before
        );
        sim.advance(128);
        assert_eq!(sim.port_value(gate, 0), Some(Value::known(1, 1)));
    }
    let mut p = Project::default();
    let root = p.main;
    let child = p.add("unsupported".into()).unwrap();
    let unsupported = add(&mut p.circuit_mut(child).unwrap().circuit, Kind::Ram, 8);
    p.circuit_mut(child)
        .unwrap()
        .circuit
        .components
        .iter_mut()
        .find(|c| c.id == unsupported)
        .unwrap()
        .memory
        .interface = MemoryInterface::SharedAsync;
    let (instance, _) = instance(&mut p, child, root, Point::ZERO);
    let pin = add(&mut p.main_circuit_mut().circuit, Kind::Pin, 1);
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    sim.set_input(pin, 1);
    let before = (
        sim.steps(),
        sim.pending_count(),
        sim.view(&[]).unwrap().port_value(pin, 0),
    );
    assert_eq!(
        sim.settle_compiled().unwrap_err(),
        Error {
            path: vec![instance],
            reason: Failure::UnsupportedComponent(unsupported)
        }
    );
    assert_eq!(
        (
            sim.steps(),
            sim.pending_count(),
            sim.view(&[]).unwrap().port_value(pin, 0)
        ),
        before
    );
}

#[test]
fn state_only_conflicts_resolve_but_feedback_and_level_triggers_are_rejected() {
    let mut c = Circuit::default();
    let ff = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
    let pin = driven(&mut c, (ff, 2));
    let conflicting = add(&mut c, Kind::Pin, 1);
    link(&mut c, (pin, 0), (conflicting, 0));
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.set_input(conflicting, 1);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(ff, 2), Some(Value::filled(1, Bit::Error)));
    assert!(
        sim.diagnostics()
            .iter()
            .any(|d| d.problem == crate::Problem::ErrorSignal)
    );
    c.remove(&BTreeSet::from([conflicting]));
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    let mut comp = c.component(ff).unwrap().clone();
    comp.sequential.trigger = Trigger::High;
    c.replace_component(comp);
    sim.synchronize(&c);
    assert_eq!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::LevelTriggered(ff)
    );
    let mut c = Circuit::default();
    let inv = add(&mut c, Kind::Gate(Gate::Not), 1);
    link(&mut c, (inv, 0), (inv, 1));
    sim.synchronize(&c);
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        }
    ));
}

#[test]
fn asynchronous_state_oscillation_is_bounded_and_can_recover_after_edit() {
    let mut c = Circuit::default();
    let ff = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
    let inv = add(&mut c, Kind::Gate(Gate::Not), 1);
    link(&mut c, (ff, 0), (inv, 1));
    link(&mut c, (ff, 0), (ff, 5)); // Q clears itself.
    link(&mut c, (inv, 0), (ff, 4)); // /Q presets it.
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    assert_eq!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::PropagationLimit
    );
    assert!(sim.limited());
    assert!(!sim.enabled());
    assert!(
        sim.diagnostics()
            .iter()
            .any(|d| d.problem == crate::Problem::PropagationLimit)
    );
    c.remove(&BTreeSet::from([inv]));
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    assert!(!sim.limited());
    assert_eq!(sim.pending_count(), 0);
}

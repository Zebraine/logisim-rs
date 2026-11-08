use super::*;
use crate::ProjectSimulator;
use logisim_core::{Circuit, Component, Facing, FlipFlop, Gate, Latch, Point, Project};

fn add(c: &mut Circuit, kind: Kind, bits: u8) -> Id {
    let mut component =
        Component::new(kind, Point::new(200 + c.components.len() as i32 * 200, 300));
    component.bits = bits;
    c.add_component(component)
}
fn link(c: &mut Circuit, from: Port, to: Port) {
    let label = format!("{}:{}", from.0, from.1);
    for port in [from, to] {
        let component = c.component(port.0).unwrap();
        let mut tunnel = Component::new(Kind::Tunnel, component.ports()[port.1]);
        tunnel.bits = component.port_width(port.1).unwrap();
        tunnel.label = label.clone();
        c.add_component(tunnel);
    }
}
fn driven(c: &mut Circuit, port: Port) -> Id {
    let pin = add(
        c,
        Kind::Pin,
        c.component(port.0).unwrap().port_width(port.1).unwrap(),
    );
    link(c, (pin, 0), port);
    pin
}
fn chain(c: &mut Circuit, input: Id, count: usize) -> Id {
    let mut previous = input;
    let width = c.component(input).unwrap().port_width(0).unwrap();
    for _ in 0..count {
        let gate = add(c, Kind::Gate(Gate::Buffer), width);
        link(c, (previous, 0), (gate, 1));
        previous = gate;
    }
    previous
}
fn stored(sim: &Simulator, id: Id) -> u32 {
    sim.stored_value(id).unwrap().number().unwrap()
}
fn output_pin(c: &mut Circuit) -> Id {
    let mut pin = Component::new(Kind::Pin, Point::new(5000, 2000));
    pin.output = true;
    pin.facing = Facing::West;
    c.add_component(pin)
}

#[test]
fn same_edge_chain_reads_old_state_in_both_document_orders() {
    for reverse in [false, true] {
        let mut c = Circuit::default();
        let a = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
        let b = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
        let clock = driven(&mut c, (a, 2));
        link(&mut c, (clock, 0), (b, 2));
        let data = driven(&mut c, (a, 3));
        link(&mut c, (a, 0), (b, 3));
        if reverse {
            c.components.reverse();
        }
        let mut sim = Simulator::default();
        sim.synchronize(&c);
        sim.set_enabled(false);
        sim.settle_compiled().unwrap();
        sim.set_input(data, 1);
        sim.set_input(clock, 1);
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, a), stored(&sim, b)), (1, 0));
        sim.set_input(clock, 0);
        sim.settle_compiled().unwrap();
        sim.set_input(clock, 1);
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, a), stored(&sim, b)), (1, 1));
        assert!(!sim.enabled());
        assert_eq!(sim.pending_count(), 0);
        let mut renamed = c.component(a).unwrap().clone();
        renamed.label = "renamed state".into();
        c.replace_component(renamed);
        assert!(!sim.synchronize(&c));
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, a), stored(&sim, b)), (1, 1));
    }
}

#[test]
fn stable_sampling_ignores_combinational_depth_but_step_preserves_old_batches() {
    for depth in [1, 2, 32] {
        let mut c = Circuit::default();
        let ff = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
        let clock = add(&mut c, Kind::Pin, 1);
        let data = add(&mut c, Kind::Pin, 1);
        let clk_out = chain(&mut c, clock, 1);
        let data_out = chain(&mut c, data, depth);
        link(&mut c, (clk_out, 0), (ff, 2));
        link(&mut c, (data_out, 0), (ff, 3));
        for compiled in [false, true] {
            let mut sim = Simulator::default();
            sim.synchronize(&c);
            sim.advance(256);
            sim.set_input(data, 1);
            sim.set_input(clock, 1);
            if compiled {
                sim.settle_compiled().unwrap();
            } else {
                sim.advance(256);
            }
            assert_eq!(stored(&sim, ff), u32::from(compiled || depth == 1));
            assert_eq!(sim.port_value(ff, 3), Some(Value::known(1, 1)));
            assert_eq!(sim.pending_count(), 0);
            // Resume exact stepping after a compiled settle, without stale pending.
            sim.set_input(clock, 0);
            sim.advance(256);
            sim.set_input(data, 0);
            sim.advance(256);
            sim.set_input(clock, 1);
            sim.advance(256);
            assert_eq!(stored(&sim, ff), 0);
        }
    }
}

#[test]
fn asynchronous_controls_and_four_state_register_data_match_settled_reference() {
    for trigger in [Trigger::Rising, Trigger::Falling] {
        for width in [1, 4, 32] {
            let mut c = Circuit::default();
            let reg = add(&mut c, Kind::Register, width);
            let mut comp = c.component(reg).unwrap().clone();
            comp.sequential.trigger = trigger;
            c.replace_component(comp);
            let pins: Vec<_> = (1..7).map(|p| driven(&mut c, (reg, p))).collect();
            let mut slow = Simulator::default();
            let mut fast = Simulator::default();
            slow.synchronize(&c);
            fast.synchronize(&c);
            let mut seed = 0x98ab_7123u32;
            for i in 0..128 {
                for (port, &pin) in pins.iter().enumerate() {
                    // Set data/async controls first, then independently apply a clock
                    // transition: both models share this explicit stimulus ordering.
                    if port == 1 {
                        continue;
                    }
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let bits = c.component(pin).unwrap().bits;
                    let value = if port == 0 && i % 4 >= 2 {
                        Value::filled(bits, if i % 2 == 0 { Bit::Unknown } else { Bit::Error })
                    } else {
                        Value::known(bits, seed)
                    };
                    slow.external_input(pin, value);
                    fast.external_input(pin, value);
                }
                slow.advance(128);
                fast.settle_compiled().unwrap();
                for sim in [&mut slow, &mut fast] {
                    sim.set_input(pins[1], i & 1);
                }
                slow.advance(128);
                fast.settle_compiled().unwrap();
                assert_eq!(fast.stored_value(reg), slow.stored_value(reg));
                for port in 0..7 {
                    assert_eq!(fast.port_value(reg, port), slow.port_value(reg, port));
                }
            }
        }
    }
}

fn buffered_child(p: &mut Project, depth: usize) -> (Id, Id, Id) {
    let id = p.add(format!("buffer{depth}")).unwrap();
    let c = &mut p.circuit_mut(id).unwrap().circuit;
    let input = add(c, Kind::Pin, 1);
    let buffer = chain(c, input, depth);
    let output = output_pin(c);
    link(c, (buffer, 0), (output, 0));
    (id, input, output)
}
fn instance(
    p: &mut Project,
    definition: Id,
    parent: Id,
    position: Point,
) -> (Id, Vec<(Id, usize)>) {
    let comp = p.instance(definition, position).unwrap();
    let pins = comp
        .interface
        .ports
        .iter()
        .enumerate()
        .map(|(i, p)| (p.pin, i))
        .collect();
    (
        p.circuit_mut(parent).unwrap().circuit.add_component(comp),
        pins,
    )
}

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
fn state_only_conflicts_feedback_and_level_triggers_are_rejected() {
    let mut c = Circuit::default();
    let ff = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
    let pin = driven(&mut c, (ff, 2));
    let conflicting = add(&mut c, Kind::Pin, 1);
    link(&mut c, (pin, 0), (conflicting, 0));
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::MultipleDrivers,
            ..
        }
    ));
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
fn electrical_edits_invalidate_plans_while_visual_edits_preserve_state() {
    let mut c = Circuit::default();
    let input = add(&mut c, Kind::Pin, 4);
    let gate = chain(&mut c, input, 2);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.set_input(input, 5);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 5)));
    let mut edited = c.component(gate).unwrap().clone();
    edited.kind = Kind::Gate(Gate::Not);
    c.replace_component(edited);
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 10)));
    // A new text item changes component indexing without invalidating electricity.
    let text = Component::new(Kind::Text, Point::new(700, 800));
    let text = c.add_component(text);
    c.components.rotate_right(1);
    assert_eq!(c.components[0].id, text);
    assert!(!sim.synchronize(&c));
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(gate, 0), Some(Value::known(4, 10)));
    let mut c = Circuit::default();
    let rom = add(&mut c, Kind::Rom, 4);
    for p in [1, 2] {
        driven(&mut c, (rom, p));
    }
    for number in [3, 12] {
        let mut edited = c.component(rom).unwrap().clone();
        edited.memory.contents.set(0, number);
        c.replace_component(edited);
        sim.synchronize(&c);
        sim.settle_compiled().unwrap();
        assert_eq!(sim.port_value(rom, 0), Some(Value::known(4, number)));
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

#[test]
fn cached_outputs_are_rematerialized_after_partial_step_and_reset() {
    let mut c = Circuit::default();
    let input = add(&mut c, Kind::Pin, 1);
    let first = chain(&mut c, input, 1);
    let last = chain(&mut c, first, 4);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    sim.set_input(input, 1);
    assert!(sim.step());
    sim.set_input(input, 0);
    assert!(sim.step());
    assert_eq!(sim.port_value(first, 0), Some(Value::known(1, 1)));
    sim.settle_compiled().unwrap();
    for id in [first, last] {
        assert_eq!(sim.port_value(id, 0), Some(Value::known(1, 0)));
    }
    assert_eq!(sim.pending_count(), 0);
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(last, 0), Some(Value::known(1, 0)));
}

#[test]
fn supported_edge_state_families_keep_transitions_and_clock_ticks() {
    for kind in [
        Kind::FlipFlop(FlipFlop::D),
        Kind::FlipFlop(FlipFlop::T),
        Kind::FlipFlop(FlipFlop::Jk),
        Kind::FlipFlop(FlipFlop::Sr),
        Kind::Register,
        Kind::Counter,
        Kind::ShiftRegister,
        Kind::Random,
    ] {
        for trigger in [Trigger::Rising, Trigger::Falling] {
            let mut c = Circuit::default();
            let id = add(
                &mut c,
                kind,
                if matches!(kind, Kind::FlipFlop(_)) {
                    1
                } else {
                    4
                },
            );
            let mut component = c.component(id).unwrap().clone();
            component.sequential.trigger = trigger;
            component.sequential.counter_max = 15;
            component.sequential.random_seed = 17;
            c.replace_component(component);
            let clock = add(&mut c, Kind::Clock, 1);
            link(&mut c, (clock, 0), (id, 2));
            let component = c.component(id).unwrap().clone();
            let pins: Vec<_> = (0..component.ports().len())
                .filter(|&i| i != 2 && !component.drives_port(i))
                .map(|i| driven(&mut c, (id, i)))
                .collect();
            let mut a = Simulator::default();
            let mut b = Simulator::default();
            a.synchronize(&c);
            b.synchronize(&c);
            a.advance(128);
            b.settle_compiled().unwrap();
            let mut seed = 0x8123_1235u32;
            for _ in 0..64 {
                for &pin in &pins {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    a.set_input(pin, seed);
                    b.set_input(pin, seed);
                }
                a.advance(128);
                b.settle_compiled().unwrap();
                assert!(a.tick());
                assert!(b.tick());
                a.advance(128);
                b.settle_compiled().unwrap();
                assert_eq!(
                    a.stored_value(id),
                    b.stored_value(id),
                    "{kind:?} {trigger:?}"
                );
                for i in 0..component.ports().len() {
                    assert_eq!(
                        a.port_value(id, i),
                        b.port_value(id, i),
                        "{kind:?} {trigger:?} {i}"
                    );
                }
            }
        }
    }
}

#[test]
fn stable_sampling_skips_idle_state_and_recovers_after_steps_reset_and_reindexing() {
    let mut c = Circuit::default();
    let mut ffs = vec![];
    for _ in 0..64 {
        ffs.push(add(&mut c, Kind::FlipFlop(FlipFlop::D), 1));
    }
    let active = ffs[31];
    let data = driven(&mut c, (active, 3));
    let clock = driven(&mut c, (active, 2));
    let unrelated = add(&mut c, Kind::Pin, 1);
    chain(&mut c, unrelated, 8);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    assert!(sim.settle_compiled().unwrap().sampled_components >= 64);
    let report = sim.settle_compiled().unwrap();
    assert_eq!(
        (report.evaluated_instructions, report.sampled_components),
        (0, 0)
    );
    sim.set_input(unrelated, 1);
    let report = sim.settle_compiled().unwrap();
    assert_eq!(
        (report.evaluated_instructions, report.sampled_components),
        (8, 0)
    );
    sim.set_input(data, 1);
    assert_eq!(sim.settle_compiled().unwrap().sampled_components, 1);
    // Reindexing state entries must not invalidate topology-based dirty membership.
    add(&mut c, Kind::Text, 1);
    c.components.rotate_right(1);
    assert!(!sim.synchronize(&c));
    sim.set_input(clock, 1);
    assert_eq!(sim.settle_compiled().unwrap().sampled_components, 2);
    for id in ffs {
        assert_eq!(stored(&sim, id), u32::from(id == active));
    }
    sim.set_input(clock, 0);
    assert!(sim.step());
    assert!(sim.settle_compiled().unwrap().sampled_components >= 64);
    sim.reset();
    assert!(sim.settle_compiled().unwrap().sampled_components >= 64);
    assert_eq!(stored(&sim, active), 0);
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
fn selective_state_sampling_matches_full_sampling_over_sparse_four_state_stimuli() {
    let mut c = Circuit::default();
    let mut ids = vec![];
    let mut pins = vec![];
    for kind in [
        Kind::FlipFlop(FlipFlop::D),
        Kind::Register,
        Kind::Counter,
        Kind::ShiftRegister,
        Kind::Random,
    ] {
        let id = add(
            &mut c,
            kind,
            if matches!(kind, Kind::FlipFlop(_)) {
                1
            } else {
                4
            },
        );
        let mut comp = c.component(id).unwrap().clone();
        comp.sequential.random_seed = 17;
        c.replace_component(comp.clone());
        for port in 0..comp.ports().len() {
            if !comp.drives_port(port) {
                let pin = driven(&mut c, (id, port));
                pins.push((pin, comp.port_width(port).unwrap()));
            }
        }
        ids.push(id);
    }
    let mut fast = Simulator::default();
    let mut full = Simulator::default();
    fast.synchronize(&c);
    full.synchronize(&c);
    full.prepare_stable().unwrap();
    let mut seed = 0x8912_3456u32;
    for i in 0..512 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let (pin, width) = pins[seed as usize % pins.len()];
        let bit = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error][i % 4];
        for sim in [&mut fast, &mut full] {
            sim.external_input(pin, Value::filled(width, bit));
        }
        fast.settle_compiled().unwrap();
        for round in 0..PROPAGATION_LIMIT {
            full.stable_combinational_pass();
            full.schedule_storage(None);
            if full.pending.is_empty() {
                break;
            }
            assert!(round + 1 < PROPAGATION_LIMIT);
        }
        assert_eq!(fast.drives, full.drives);
        assert_eq!(fast.values, full.values);
        for &id in &ids {
            assert_eq!(fast.stored_value(id), full.stored_value(id));
        }
    }
}

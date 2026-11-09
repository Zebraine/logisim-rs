use crate::stable::*;
use logisim_core::{Circuit, Component, FlipFlop, Gate, Point};

use super::support::*;

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

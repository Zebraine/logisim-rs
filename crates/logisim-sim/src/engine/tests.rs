use crate::{Bit, Simulator, Value};
use logisim_core::{
    Arithmetic, Circuit, Component, FlipFlop, Gate, Kind, Latch, Plexer, Point, Trigger,
};

// Former scheduling rule: resolve all nets and evaluate all entries after each batch.
// Keep this independent of the incremental resolver and its dirty-reader selection.
fn full_step(s: &mut Simulator) -> bool {
    if s.pending.is_empty() || s.limited {
        return false;
    }
    s.drives.extend(std::mem::take(&mut s.pending));
    s.values = s.network.resolve(&s.drives);
    s.schedule_components(false, None);
    if s.progress.record_batch(!s.pending.is_empty()) {
        s.limited = true;
        s.enabled = false;
    }
    true
}
fn compare(a: &Simulator, b: &Simulator) {
    assert_eq!(a.values, b.values, "resolved nets");
    assert_eq!(a.pending, b.pending, "next batch");
    assert_eq!(a.drives, b.drives, "committed drivers");
    assert_eq!(a.inputs, b.inputs);
    assert_eq!(a.steps(), b.steps());
    assert_eq!(a.limited(), b.limited());
    for c in &a.circuit.components {
        assert_eq!(a.stored_value(c.id), b.stored_value(c.id));
        assert_eq!(a.memory_contents(c.id), b.memory_contents(c.id));
        assert_eq!(a.tty(c.id), b.tty(c.id));
        if c.kind == Kind::DotMatrix {
            assert_eq!(a.matrix_cells(c.id), b.matrix_cells(c.id));
        }
    }
}
fn settle(a: &mut Simulator, b: &mut Simulator) {
    compare(a, b);
    for _ in 0..=crate::PROPAGATION_LIMIT {
        let advanced = a.step();
        assert_eq!(advanced, full_step(b));
        compare(a, b);
        if !advanced {
            return;
        }
    }
    panic!("unbounded propagation");
}

#[test]
fn selective_scheduling_matches_full_batches_through_state_events_and_rebuilds() {
    let mut circuit = Circuit::default();
    let mut pins = vec![];
    let mut ids = vec![];
    let kinds = [
        Kind::Gate(Gate::Not),
        Kind::Gate(Gate::And),
        Kind::ControlledBuffer,
        Kind::Arithmetic(Arithmetic::Adder),
        Kind::Plexer(Plexer::DisplayDecoder),
        Kind::FlipFlop(FlipFlop::D),
        Kind::Latch(Latch::D),
        Kind::Register,
        Kind::Counter,
        Kind::ShiftRegister,
        Kind::Random,
        Kind::Ram,
        Kind::Rom,
        Kind::Keyboard,
        Kind::Tty,
        Kind::Switch,
        Kind::DipSwitch,
        Kind::Joystick,
        Kind::DotMatrix,
        Kind::Clock,
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        let mut c = Component::new(kind, Point::new(index as i32 * 1000, 500));
        c.sequential.random_seed = 17;
        c.memory.address_bits = 2;
        c.memory.contents.set(0, 1);
        c.io.matrix_persistence = 2;
        for (port, at) in c.ports().into_iter().enumerate() {
            if !c.drives_port(port) {
                let mut pin = Component::new(Kind::Pin, at);
                pin.bits = c.port_width(port).unwrap();
                let bits = pin.bits;
                pins.push((circuit.add_component(pin), bits));
            }
        }
        ids.push((kind, circuit.add_component(c)));
    }
    let mut a = Simulator::default();
    let mut b = Simulator::default();
    for s in [&mut a, &mut b] {
        s.synchronize(&circuit);
    }
    settle(&mut a, &mut b);
    let mut seed = 19_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as u32
    };
    for round in 0..160 {
        for &(pin, bits) in &pins {
            if next() % 5 != 0 {
                continue;
            }
            let value = match next() % 6 {
                0 => Value::filled(bits, Bit::Unknown),
                1 => Value::filled(bits, Bit::Error),
                _ => Value::known(bits, next()),
            };
            for s in [&mut a, &mut b] {
                s.external_input(pin, value);
            }
        }
        // Local state events have no prerequisite net change and must bypass filtering.
        for s in [&mut a, &mut b] {
            for &(kind, id) in &ids {
                match kind {
                    Kind::Ram => s.set_memory_word(id, 0, round % 2),
                    Kind::Keyboard => s.type_character(id, 65 + round as u16 % 26),
                    Kind::FlipFlop(_) if round % 7 == 0 => s.toggle_stored(id),
                    Kind::Switch | Kind::DipSwitch => s.toggle_input(id, 0),
                    Kind::Joystick => s.set_joystick(id, Point::new(round as i32 % 41 - 20, 3)),
                    _ => {}
                }
            }
        }
        settle(&mut a, &mut b);
        for s in [&mut a, &mut b] {
            assert!(s.tick());
        }
        settle(&mut a, &mut b);
        if round % 20 == 0 {
            // Both non-electrical index shifts and electrical edits must invalidate correctly.
            let text = circuit.add_component(Component::new(Kind::Text, Point::ZERO));
            circuit.components.rotate_right(1);
            for s in [&mut a, &mut b] {
                assert!(!s.synchronize(&circuit));
            }
            circuit.remove(&[text].into());
            circuit.components.reverse();
            for s in [&mut a, &mut b] {
                s.synchronize(&circuit);
            }
            settle(&mut a, &mut b);
            for s in [&mut a, &mut b] {
                s.reset();
            }
            settle(&mut a, &mut b);
        }
    }
}

#[test]
fn unchanged_networks_do_not_schedule_unrelated_readers() {
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for i in 0..256 {
        let gate = Component::new(Kind::Gate(Gate::Not), Point::new(i * 100, 100));
        inputs.push(circuit.add_component(Component::new(Kind::Pin, gate.ports()[1])));
        circuit.add_component(gate);
    }
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    sim.advance(16);
    sim.set_input(inputs[17], 1);
    assert!(sim.step());
    assert_eq!(sim.changed_nets.items.len(), 1);
    let affected: Vec<_> = sim
        .plan
        .single
        .iter()
        .filter(|e| e.affected(Some(&sim.changed_nets)))
        .map(|e| e.component)
        .collect();
    assert_eq!(affected, [35]);
    assert_eq!(sim.pending_count(), 1);
    sim.advance(16);
    sim.set_input(inputs[17], 0);
    sim.set_input(inputs[17], 1);
    assert!(!sim.step(), "cancelled changes must not invent a batch");
}

#[test]
fn rebuilding_held_outputs_samples_even_when_only_unrelated_nets_change() {
    let mut circuit = Circuit::default();
    let decoder = Component::new(Kind::Plexer(Plexer::DisplayDecoder), Point::ZERO);
    let mut sources = std::collections::BTreeSet::new();
    for (i, at) in decoder.ports().into_iter().enumerate() {
        if !decoder.drives_port(i) {
            let mut source = Component::new(Kind::Constant, at);
            source.bits = decoder.port_width(i).unwrap();
            source.constant = 1;
            sources.insert(circuit.add_component(source));
        }
    }
    let id = circuit.add_component(decoder);
    circuit.add_component(Component::new(Kind::Constant, Point::new(1000, 1000)));
    let mut a = Simulator::default();
    let mut b = Simulator::default();
    for s in [&mut a, &mut b] {
        s.synchronize(&circuit);
    }
    settle(&mut a, &mut b);
    assert!(a.port_value(id, 0).unwrap().number().is_some());
    circuit.remove(&sources);
    for s in [&mut a, &mut b] {
        s.synchronize(&circuit);
    }
    assert!(
        a.port_value(id, 0).unwrap().number().is_some(),
        "restore held segments first"
    );
    settle(&mut a, &mut b);
    assert_eq!(a.port_value(id, 0), Some(Value::filled(1, Bit::Unknown)));
}

#[test]
fn masked_level_triggered_state_keeps_existing_per_batch_semantics_and_limit() {
    let mut circuit = Circuit::default();
    let mut flip = Component::new(Kind::FlipFlop(FlipFlop::T), Point::ZERO);
    flip.sequential.trigger = Trigger::High;
    let ports = flip.ports();
    let id = circuit.add_component(flip);
    // Mask both Q outputs with E, so internal toggles no longer change any network.
    for (i, at) in ports.into_iter().enumerate() {
        let mut constant = Component::new(Kind::Constant, at);
        constant.constant = u32::from(matches!(i, 2 | 3));
        circuit.add_component(constant);
        if i < 2 {
            let mut conflicting = Component::new(Kind::Constant, at);
            conflicting.constant = 1;
            circuit.add_component(conflicting);
        }
    }
    let mut a = Simulator::default();
    let mut b = Simulator::default();
    for s in [&mut a, &mut b] {
        s.synchronize(&circuit);
    }
    settle(&mut a, &mut b);
    assert!(a.limited());
    assert_eq!(a.port_value(id, 0), Some(Value::filled(1, Bit::Error)));
    assert!(a.changed_nets.items.is_empty());
}

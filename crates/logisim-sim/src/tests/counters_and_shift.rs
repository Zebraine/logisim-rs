use super::super::*;
use logisim_core::{Component, CounterOverflow, Kind, Point, Trigger};

// Controls are real input pins on the observed terminals; output/carry never get input drivers.
fn fixture(
    kind: Kind,
    bits: u8,
    max: u32,
    old: bool,
    overflow: CounterOverflow,
    trigger: Trigger,
) -> (Circuit, Id, Vec<Option<Id>>, Simulator) {
    let mut c = Component::new(kind, Point::new(200, 200));
    c.bits = bits;
    c.sequential.counter_max = max;
    c.sequential.counter_old = old;
    c.sequential.counter_overflow = overflow;
    c.sequential.trigger = trigger;
    let mut circuit = Circuit::default();
    let pins = c
        .ports()
        .into_iter()
        .enumerate()
        .map(|(i, at)| {
            if c.drives_port(i) {
                return None;
            }
            let mut pin = Component::new(Kind::Pin, at);
            pin.bits = c.port_width(i).unwrap();
            Some(circuit.add_component(pin))
        })
        .collect();
    let id = circuit.add_component(c);
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    settle(&mut sim);
    (circuit, id, pins, sim)
}
fn settle(s: &mut Simulator) {
    s.advance(128);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
fn drive(s: &mut Simulator, p: &[Option<Id>], port: usize, value: Value) {
    s.external_input(p[port].unwrap(), value);
    settle(s);
}
fn bit(s: &mut Simulator, p: &[Option<Id>], port: usize, value: u32) {
    drive(s, p, port, Value::known(1, value));
}
fn edge(s: &mut Simulator, p: &[Option<Id>], trigger: Trigger) {
    let start = u32::from(trigger == Trigger::Falling);
    bit(s, p, 2, start);
    bit(s, p, 2, 1 - start);
}
fn number(s: &Simulator, id: Id) -> u32 {
    s.stored_value(id).unwrap().number().unwrap()
}

#[test]
fn counter_overflow_carry_and_both_edges_follow_runtime_matrix() {
    for trigger in [Trigger::Rising, Trigger::Falling] {
        for (action, up, down) in [
            (CounterOverflow::Wrap, 0, 5),
            (CounterOverflow::Stay, 5, 0),
            (CounterOverflow::Continue, 6, 15),
            (CounterOverflow::Load, 3, 3),
        ] {
            for reverse in [false, true] {
                let (_, id, p, mut s) = fixture(Kind::Counter, 4, 5, false, action, trigger);
                bit(&mut s, &p, 8, 1);
                if !reverse {
                    bit(&mut s, &p, 3, 1);
                    drive(&mut s, &p, 1, Value::known(4, 5));
                    edge(&mut s, &p, trigger);
                    bit(&mut s, &p, 3, 0);
                }
                bit(&mut s, &p, 4, u32::from(reverse));
                assert_eq!(s.port_value(id, 5).unwrap().number(), Some(1));
                drive(&mut s, &p, 1, Value::known(4, 3));
                edge(&mut s, &p, trigger);
                assert_eq!(
                    number(&s, id),
                    if reverse { down } else { up },
                    "{action:?} {reverse} {trigger:?}"
                );
                assert_eq!(
                    s.port_value(id, 5).unwrap().number(),
                    Some(u32::from(action == CounterOverflow::Stay))
                );
            }
        }
    }
}

#[test]
fn counter_conditional_load_mask_unknown_data_and_async_priority() {
    for (max, expected) in [
        (5, [0, 1, 2, 3, 4, 5, 4, 5, 0, 1, 0, 1, 4, 5, 4, 5]),
        (9, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 8, 9, 8, 9, 8, 9]),
    ] {
        let (_, id, p, mut s) = fixture(
            Kind::Counter,
            4,
            max,
            false,
            CounterOverflow::Wrap,
            Trigger::Rising,
        );
        bit(&mut s, &p, 3, 1); // Loads even with EN=0.
        for (data, expected) in expected.into_iter().enumerate() {
            drive(&mut s, &p, 1, Value::known(4, data as u32));
            edge(&mut s, &p, Trigger::Rising);
            assert_eq!(number(&s, id), expected, "max={max} D={data}");
        }
        for unknown in [Bit::Unknown, Bit::Error] {
            bit(&mut s, &p, 6, 1);
            assert_eq!(number(&s, id), 15); // PRE uses width, not maximum.
            bit(&mut s, &p, 7, 1);
            assert_eq!(number(&s, id), 0);
            bit(&mut s, &p, 7, 0);
            assert_eq!(number(&s, id), 15);
            bit(&mut s, &p, 6, 0);
            drive(
                &mut s,
                &p,
                1,
                Value::from_bits([Bit::One, unknown, Bit::Zero, Bit::One].into_iter()),
            );
            edge(&mut s, &p, Trigger::Rising);
            assert_eq!(number(&s, id), 0);
        }
        bit(&mut s, &p, 3, 0);
        bit(&mut s, &p, 6, 1);
        bit(&mut s, &p, 6, 0);
        bit(&mut s, &p, 8, 1);
        edge(&mut s, &p, Trigger::Rising);
        assert_eq!(number(&s, id), 0); // 15 > max: natural overflow, not equality at max.
    }
}

#[test]
fn counter_old_new_four_state_controls_and_width_boundaries() {
    let states = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error];
    for old in [false, true] {
        for load in states {
            for count in states {
                let (_, id, p, mut s) = fixture(
                    Kind::Counter,
                    4,
                    5,
                    old,
                    CounterOverflow::Wrap,
                    Trigger::Rising,
                );
                drive(&mut s, &p, 1, Value::known(4, 3));
                drive(&mut s, &p, 3, Value::filled(1, load));
                drive(&mut s, &p, 4, Value::filled(1, count));
                if !old {
                    bit(&mut s, &p, 8, 1);
                }
                edge(&mut s, &p, Trigger::Rising);
                let expected = if old {
                    if count == Bit::Zero {
                        if load == Bit::One { 3 } else { 0 }
                    } else if load == Bit::One {
                        5
                    } else {
                        1
                    }
                } else if load == Bit::One {
                    3
                } else if count == Bit::One {
                    5
                } else {
                    1
                };
                assert_eq!(
                    number(&s, id),
                    expected,
                    "old={old} LD={load:?} CT={count:?}"
                );
            }
        }
    }
    for enable in states {
        let (_, id, p, mut s) = fixture(
            Kind::Counter,
            4,
            5,
            false,
            CounterOverflow::Wrap,
            Trigger::Rising,
        );
        drive(&mut s, &p, 8, Value::filled(1, enable));
        for control in [6, 7] {
            drive(&mut s, &p, control, Value::filled(1, Bit::Error));
        }
        edge(&mut s, &p, Trigger::Rising);
        assert_eq!(number(&s, id), u32::from(enable != Bit::Zero));
    }
    for (width, max, data) in [(1, 1, 1), (32, u32::MAX, 0x80000001), (4, 0, 0)] {
        let (_, id, p, mut s) = fixture(
            Kind::Counter,
            width,
            max,
            false,
            CounterOverflow::Wrap,
            Trigger::Falling,
        );
        drive(&mut s, &p, 1, Value::known(width, data));
        bit(&mut s, &p, 3, 1);
        edge(&mut s, &p, Trigger::Falling);
        assert_eq!(number(&s, id), data);
        if max == 0 {
            bit(&mut s, &p, 3, 0);
            bit(&mut s, &p, 8, 1);
            for reverse in [0, 1] {
                bit(&mut s, &p, 4, reverse);
                edge(&mut s, &p, Trigger::Falling);
                assert_eq!(number(&s, id), 0);
                assert_eq!(s.port_value(id, 5).unwrap().number(), Some(1));
            }
        }
    }
}

#[test]
fn universal_register_modes_shift_only_selected_serial_bit_and_preserve_four_states() {
    for trigger in [Trigger::Rising, Trigger::Falling] {
        for (mode, data, expected) in [(1, 8, [8, 12, 14]), (2, 1, [1, 3, 7])] {
            let (_, id, p, mut s) = fixture(
                Kind::ShiftRegister,
                4,
                15,
                false,
                CounterOverflow::Wrap,
                trigger,
            );
            drive(&mut s, &p, 3, Value::known(2, mode));
            drive(&mut s, &p, 1, Value::known(4, data));
            for value in expected {
                edge(&mut s, &p, trigger);
                assert_eq!(number(&s, id), value);
            }
            for mode in [
                Value::known(2, 0),
                Value::filled(2, Bit::Unknown),
                Value::from_bits([Bit::One, Bit::Error].into_iter()),
            ] {
                drive(&mut s, &p, 3, mode);
                edge(&mut s, &p, trigger);
                assert_eq!(number(&s, id), expected[2]);
            }
            bit(&mut s, &p, 4, 1);
            assert_eq!(number(&s, id), 0);
        }
    }
    let (_, id, p, mut s) = fixture(
        Kind::ShiftRegister,
        4,
        15,
        false,
        CounterOverflow::Wrap,
        Trigger::Rising,
    );
    let mixed = Value::from_bits([Bit::Error, Bit::One, Bit::Unknown, Bit::Zero].into_iter());
    drive(&mut s, &p, 1, mixed);
    drive(&mut s, &p, 3, Value::known(2, 3));
    edge(&mut s, &p, Trigger::Rising);
    assert_eq!(s.stored_value(id), Some(mixed));
    drive(&mut s, &p, 3, Value::known(2, 1));
    edge(&mut s, &p, Trigger::Rising);
    assert_eq!(s.stored_value(id).unwrap().binary(), "00X1");
    drive(&mut s, &p, 3, Value::known(2, 2));
    edge(&mut s, &p, Trigger::Rising);
    assert_eq!(s.stored_value(id).unwrap().binary(), "0X1E");
    for (bits, data, shifted) in [(1, 1, 1), (32, 0x80000001, 0xc0000000)] {
        let (_, id, p, mut s) = fixture(
            Kind::ShiftRegister,
            bits,
            0,
            false,
            CounterOverflow::Wrap,
            Trigger::Rising,
        );
        drive(&mut s, &p, 1, Value::known(bits, data));
        for (mode, expected) in [(3, data), (1, shifted), (2, data)] {
            drive(&mut s, &p, 3, Value::known(2, mode));
            edge(&mut s, &p, Trigger::Rising);
            assert_eq!(number(&s, id), expected);
        }
    }
}

#[test]
fn storage_resize_preserves_low_bits_and_startup_high_has_no_edge() {
    for kind in [Kind::Counter, Kind::ShiftRegister] {
        let (mut c, id, p, mut s) =
            fixture(kind, 4, 15, false, CounterOverflow::Wrap, Trigger::Rising);
        drive(&mut s, &p, 1, Value::known(4, 9));
        drive(
            &mut s,
            &p,
            3,
            Value::known(
                if kind == Kind::Counter { 1 } else { 2 },
                if kind == Kind::Counter { 1 } else { 3 },
            ),
        );
        edge(&mut s, &p, Trigger::Rising);
        assert_eq!(number(&s, id), 9);
        drive(
            &mut s,
            &p,
            3,
            Value::known(if kind == Kind::Counter { 1 } else { 2 }, 0),
        );
        for (bits, expected) in [(32, 9), (1, 1)] {
            c.components.iter_mut().find(|c| c.id == id).unwrap().bits = bits;
            s.synchronize(&c);
            settle(&mut s);
            assert_eq!(number(&s, id), expected);
        }
        let clock = c
            .components
            .iter_mut()
            .find(|c| Some(c.id) == p[2])
            .unwrap();
        clock.kind = Kind::Constant;
        clock.constant = 1;
        for (port, value) in [(1, 1), (3, if kind == Kind::Counter { 1 } else { 3 })] {
            let pin = c
                .components
                .iter_mut()
                .find(|c| Some(c.id) == p[port])
                .unwrap();
            pin.kind = Kind::Constant;
            pin.constant = value;
            pin.bits = if port == 1 || kind == Kind::Counter {
                1
            } else {
                2
            };
        }
        let mut startup = Simulator::default();
        startup.synchronize(&c);
        settle(&mut startup);
        assert_eq!(number(&startup, id), 0);
    }
}

#[test]
fn counter_and_shift_subcircuits_keep_independent_runtime_state() {
    for kind in [Kind::Counter, Kind::ShiftRegister] {
        let (c, stored_id, pins, _) =
            fixture(kind, 8, 255, false, CounterOverflow::Wrap, Trigger::Rising);
        let mut project = logisim_core::Project::default();
        let root = project.main;
        let cell = project.add("cell".into()).unwrap();
        project.circuit_mut(cell).unwrap().circuit = c;
        let mut instances = vec![];
        let mut controls = vec![];
        for y in [100, 500] {
            let instance = project.instance(cell, Point::new(500, y)).unwrap();
            let mut input = std::collections::BTreeMap::new();
            for (i, port) in instance.interface.ports.iter().enumerate() {
                let mut pin = Component::new(Kind::Pin, instance.ports()[i]);
                pin.bits = port.bits;
                input.insert(
                    port.pin,
                    project
                        .circuit_mut(root)
                        .unwrap()
                        .circuit
                        .add_component(pin),
                );
            }
            controls.push(input);
            instances.push(
                project
                    .circuit_mut(root)
                    .unwrap()
                    .circuit
                    .add_component(instance),
            );
        }
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&project, root);
        sim.advance(128);
        for (i, input) in controls.iter().enumerate() {
            sim.set_input(input[&pins[1].unwrap()], 7 + i as u32);
            sim.set_input(
                input[&pins[3].unwrap()],
                if kind == Kind::Counter { 1 } else { 3 },
            );
        }
        sim.advance(128);
        sim.set_input(controls[0][&pins[2].unwrap()], 1);
        sim.advance(128);
        assert_eq!(number(sim.view(&[instances[0]]).unwrap(), stored_id), 7);
        assert_eq!(number(sim.view(&[instances[1]]).unwrap(), stored_id), 0);
        sim.set_input(controls[1][&pins[2].unwrap()], 1);
        sim.advance(128);
        assert_eq!(number(sim.view(&[instances[1]]).unwrap(), stored_id), 8);
        sim.synchronize(&project, cell);
        sim.advance(128);
        sim.synchronize(&project, root);
        sim.advance(128);
        assert_eq!(number(sim.view(&[instances[0]]).unwrap(), stored_id), 7);
        assert_eq!(number(sim.view(&[instances[1]]).unwrap(), stored_id), 8);
    }
}

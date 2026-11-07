use super::*;
use logisim_core::{Component, ControlPosition, FlipFlop, Kind, Point, Trigger};

fn fixture(kind: Kind, trigger: Trigger) -> (Circuit, Id, Vec<Id>) {
    let mut circuit = Circuit::default();
    let mut c = Component::new(kind, Point::new(200, 200));
    c.sequential.trigger = trigger;
    if kind == Kind::Register {
        c.bits = 4;
    }
    let ports = c.ports();
    let first_input = if matches!(kind, Kind::FlipFlop(_)) {
        2
    } else {
        1
    };
    let inputs: Vec<_> = (first_input..ports.len())
        .map(|index| {
            let mut pin = Component::new(Kind::Pin, ports[index]);
            pin.bits = c.port_width(index).unwrap();
            circuit.add_component(pin)
        })
        .collect();
    let id = circuit.add_component(c);
    (circuit, id, inputs)
}
fn settle(sim: &mut Simulator) {
    sim.advance(64);
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
}
fn drive(sim: &mut Simulator, id: Id, n: u32) {
    sim.set_input(id, n);
    settle(sim);
}
fn stored(sim: &Simulator, id: Id) -> u32 {
    sim.stored_value(id).unwrap().number().unwrap()
}

#[test]
fn flip_flop_matrices_hold_undefined_inputs_and_keep_complement_outputs() {
    // Rows are J/S=0,1; columns K/R=0,1. Collected for both initial stored states.
    for (ff, tables) in [
        (FlipFlop::Jk, [[0, 0, 1, 1], [1, 0, 1, 0]]),
        (FlipFlop::Sr, [[0, 0, 1, 0], [1, 0, 1, 1]]),
    ] {
        for old in 0..=1 {
            for (ai, a) in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error]
                .into_iter()
                .enumerate()
            {
                for (bi, b) in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error]
                    .into_iter()
                    .enumerate()
                {
                    let (c, id, inputs) = fixture(Kind::FlipFlop(ff), Trigger::Rising);
                    let mut s = Simulator::default();
                    s.synchronize(&c);
                    settle(&mut s);
                    s.external_input(inputs[1], Value::filled(1, a));
                    s.external_input(inputs[2], Value::filled(1, b));
                    settle(&mut s);
                    if old == 1 {
                        s.toggle_stored(id);
                        settle(&mut s);
                    }
                    drive(&mut s, inputs[0], 1);
                    let expected = if ai < 2 && bi < 2 {
                        tables[old as usize][ai * 2 + bi]
                    } else {
                        old
                    };
                    assert_eq!(stored(&s, id), expected, "{ff:?} old={old} a={a:?} b={b:?}");
                    assert_eq!(s.port_value(id, 1).unwrap().number(), Some(expected ^ 1));
                }
            }
        }
    }
    for ff in [FlipFlop::D, FlipFlop::T] {
        for bit in [Bit::Unknown, Bit::Error] {
            let (c, id, inputs) = fixture(Kind::FlipFlop(ff), Trigger::Rising);
            let mut s = Simulator::default();
            s.synchronize(&c);
            settle(&mut s);
            s.toggle_stored(id);
            s.external_input(inputs[1], Value::filled(1, bit));
            settle(&mut s);
            drive(&mut s, inputs[0], 1);
            assert_eq!(stored(&s, id), 1);
        }
    }
}

#[test]
fn edges_async_priority_polarity_release_and_startup_high() {
    for ff in FlipFlop::ALL {
        for trigger in [Trigger::Rising, Trigger::Falling] {
            for negate in [false, true] {
                let (mut c, id, inputs) = fixture(Kind::FlipFlop(ff), trigger);
                c.components
                    .iter_mut()
                    .find(|c| c.id == id)
                    .unwrap()
                    .sequential
                    .negate_controls = negate;
                let mut s = Simulator::default();
                s.synchronize(&c);
                settle(&mut s);
                let pre = inputs[inputs.len() - 2];
                let clr = inputs[inputs.len() - 1];
                drive(&mut s, pre, u32::from(negate));
                drive(&mut s, clr, u32::from(negate));
                drive(&mut s, inputs[1], 1);
                assert_eq!(stored(&s, id), 0);
                drive(&mut s, inputs[0], 1);
                assert_eq!(stored(&s, id), u32::from(trigger == Trigger::Rising));
                drive(&mut s, inputs[0], 0);
                assert_eq!(stored(&s, id), 1);
                drive(&mut s, pre, u32::from(!negate));
                drive(&mut s, clr, u32::from(!negate));
                assert_eq!(stored(&s, id), 0);
                drive(&mut s, clr, u32::from(negate));
                assert_eq!(stored(&s, id), 1);
                drive(&mut s, pre, u32::from(negate));
                assert_eq!(stored(&s, id), 1);
            }
        }
    }
    let mut c = Circuit::default();
    let ff = Component::new(Kind::FlipFlop(FlipFlop::D), Point::new(100, 100));
    for index in [2, 3] {
        c.add_component(Component::new(Kind::Constant, ff.ports()[index]));
    }
    let id = c.add_component(ff);
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(stored(&s, id), 0); // Unknown -> 1 is not a rising edge.
    s.toggle_stored(id);
    settle(&mut s);
    s.reset();
    settle(&mut s);
    assert_eq!(stored(&s, id), 0);
}

#[test]
fn registers_distinguish_enable_chip_select_async_controls_and_level_triggers() {
    for trigger in Trigger::ALL {
        for select_high in [false, true] {
            let (mut c, id, inputs) = fixture(Kind::Register, trigger);
            c.components
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .sequential
                .select_high = select_high;
            let [data, clock, enable, select, pre, clear] = inputs.as_slice() else {
                panic!("register ports")
            };
            let mut s = Simulator::default();
            s.synchronize(&c);
            settle(&mut s);
            drive(&mut s, *select, u32::from(select_high));
            drive(&mut s, *data, 10);
            drive(&mut s, *clock, 1);
            assert_eq!(stored(&s, id), 0);
            drive(&mut s, *enable, 1);
            assert_eq!(
                stored(&s, id),
                if trigger == Trigger::High { 10 } else { 0 }
            );
            drive(&mut s, *clock, 0);
            assert_eq!(
                stored(&s, id),
                if trigger == Trigger::Rising { 0 } else { 10 }
            );
            drive(&mut s, *clock, 1);
            assert_eq!(stored(&s, id), 10);
            drive(&mut s, *select, u32::from(!select_high));
            assert!(s.port_value(id, 0).unwrap().contains(Bit::Unknown));
            drive(&mut s, *data, 5);
            drive(&mut s, *pre, 1);
            drive(&mut s, *clock, 0);
            drive(&mut s, *clock, 1);
            assert_eq!(stored(&s, id), 10);
            drive(&mut s, *pre, 0);
            drive(&mut s, *enable, 0);
            drive(&mut s, *select, u32::from(select_high));
            assert_eq!(stored(&s, id), 10);
            drive(&mut s, *pre, 1);
            assert_eq!(stored(&s, id), 15);
            drive(&mut s, *clear, 1);
            assert_eq!(stored(&s, id), 0);
            drive(&mut s, *clear, 0);
            assert_eq!(stored(&s, id), 15);
            drive(&mut s, *pre, 0);
            s.external_input(*data, Value::filled(4, Bit::Unknown));
            drive(&mut s, *enable, 1);
            drive(&mut s, *clock, 0);
            drive(&mut s, *clock, 1);
            assert_eq!(stored(&s, id), 15);
        }
    }
}

#[test]
fn one_edge_samples_old_outputs_through_a_register_chain() {
    let (mut c, first, inputs) = fixture(Kind::FlipFlop(FlipFlop::D), Trigger::Rising);
    let second_c = Component::new(Kind::FlipFlop(FlipFlop::D), Point::new(400, 160));
    c.add_wire(c.component(first).unwrap().ports()[0], second_c.ports()[3]);
    let path = [
        c.component(first).unwrap().ports()[2],
        Point::new(120, 200),
        Point::new(120, 120),
        Point::new(340, 120),
        second_c.ports()[2],
    ];
    for pair in path.windows(2) {
        c.add_wire(pair[0], pair[1]);
    }
    let second = c.add_component(second_c);
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    drive(&mut s, inputs[1], 1);
    drive(&mut s, inputs[0], 1);
    assert_eq!((stored(&s, first), stored(&s, second)), (1, 0));
    drive(&mut s, inputs[0], 0);
    drive(&mut s, inputs[0], 1);
    assert_eq!((stored(&s, first), stored(&s, second)), (1, 1));
}

#[test]
fn clock_durations_poke_phase_paused_batches_and_state_preserving_edits() {
    let mut c = Circuit::default();
    let first = c.add_component(Component::new(Kind::Clock, Point::new(100, 100)));
    let mut clock = Component::new(Kind::Clock, Point::new(200, 100));
    clock.sequential.high_ticks = 2;
    clock.sequential.low_ticks = 3;
    let second = c.add_component(clock);
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    for expected in [0, 0, 1, 1, 0, 0, 0, 1] {
        assert!(s.tick());
        settle(&mut s);
        assert_eq!(s.port_value(second, 0).unwrap().number(), Some(expected));
    }
    s.reset();
    settle(&mut s);
    s.tick();
    settle(&mut s);
    s.tick();
    settle(&mut s);
    s.poke_clock(second);
    settle(&mut s);
    assert_eq!(s.port_value(first, 0).unwrap().number(), Some(0));
    for expected in [0, 0, 1, 1] {
        s.tick();
        settle(&mut s);
        assert_eq!(s.port_value(second, 0).unwrap().number(), Some(expected));
    }
    s.set_enabled(false);
    let old = s.port_value(first, 0);
    assert!(s.tick());
    assert!(!s.tick());
    s.advance(100);
    assert_eq!(s.port_value(first, 0), old);
    assert!(s.step());
    assert_ne!(s.port_value(first, 0), old);
    c.components
        .iter_mut()
        .find(|c| c.id == second)
        .unwrap()
        .label = "clock label".into();
    let old = s.port_value(second, 0);
    assert!(!s.synchronize(&c));
    assert_eq!(s.port_value(second, 0), old);
    s.set_enabled(true);
    c.add_component(Component::new(Kind::Probe, Point::new(500, 500)));
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(s.port_value(second, 0), old);
    let (mut c, id, inputs) = fixture(Kind::FlipFlop(FlipFlop::D), Trigger::Rising);
    s.synchronize(&c);
    settle(&mut s);
    drive(&mut s, inputs[1], 1);
    drive(&mut s, inputs[0], 1);
    c.components
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .sequential
        .controls = ControlPosition::BelowAbove;
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(stored(&s, id), 1);
}

#[test]
fn hierarchy_storage_and_clocks_are_isolated_and_survive_navigation() {
    let mut p = logisim_core::Project::default();
    let root = p.main;
    let cell = p.add("cell".into()).unwrap();
    let c = &mut p.circuit_mut(cell).unwrap().circuit;
    let clock = c.add_component(Component::new(Kind::Pin, Point::new(0, 0)));
    let data = c.add_component(Component::new(Kind::Pin, Point::new(0, 40)));
    let stored_id = c.add_component(Component::new(
        Kind::FlipFlop(FlipFlop::D),
        Point::new(100, 0),
    ));
    let local_clock = c.add_component(Component::new(Kind::Clock, Point::new(200, 100)));
    c.add_wire(Point::new(0, 0), Point::new(40, 0));
    c.add_wire(Point::new(0, 40), Point::new(40, 40));
    let mut out = Component::new(Kind::Pin, Point::new(160, 0));
    out.output = true;
    c.add_component(out);
    c.add_wire(Point::new(100, 0), Point::new(160, 0));
    let mut ids = vec![];
    let mut controls = vec![];
    for pos in [Point::new(300, 100), Point::new(300, 300)] {
        let instance = p.instance(cell, pos).unwrap();
        let mut pins = vec![];
        for target in [clock, data] {
            let index = instance
                .interface
                .ports
                .iter()
                .position(|port| port.pin == target)
                .unwrap();
            pins.push(
                p.circuit_mut(root)
                    .unwrap()
                    .circuit
                    .add_component(Component::new(Kind::Pin, instance.ports()[index])),
            );
        }
        ids.push(p.circuit_mut(root).unwrap().circuit.add_component(instance));
        controls.push(pins);
    }
    let mut s = ProjectSimulator::default();
    s.synchronize(&p, root);
    s.advance(64);
    s.set_input(controls[0][1], 1);
    s.advance(64);
    for pins in &controls {
        s.set_input(pins[0], 1);
    }
    s.advance(64);
    assert_eq!(stored(s.view(&[ids[0]]).unwrap(), stored_id), 1);
    assert_eq!(stored(s.view(&[ids[1]]).unwrap(), stored_id), 0);
    s.poke_clock(&[ids[0]], local_clock);
    s.advance(64);
    assert_eq!(
        s.view(&[ids[0]])
            .unwrap()
            .port_value(local_clock, 0)
            .unwrap()
            .number(),
        Some(1)
    );
    assert_eq!(
        s.view(&[ids[1]])
            .unwrap()
            .port_value(local_clock, 0)
            .unwrap()
            .number(),
        Some(0)
    );
    assert!(s.tick());
    s.advance(64);
    assert_eq!(s.ticks(), 1);
    assert_eq!(
        s.view(&[ids[0]])
            .unwrap()
            .port_value(local_clock, 0)
            .unwrap()
            .number(),
        Some(0)
    );
    assert_eq!(
        s.view(&[ids[1]])
            .unwrap()
            .port_value(local_clock, 0)
            .unwrap()
            .number(),
        Some(1)
    );
    s.synchronize(&p, cell);
    s.advance(64);
    assert_eq!(stored(s.view(&[]).unwrap(), stored_id), 0);
    s.set_input(data, 1);
    s.advance(64);
    s.set_input(clock, 1);
    s.advance(64);
    s.synchronize(&p, root);
    s.advance(64);
    assert_eq!(s.ticks(), 1);
    assert_eq!(stored(s.view(&[ids[0]]).unwrap(), stored_id), 1);
    assert_eq!(stored(s.view(&[ids[1]]).unwrap(), stored_id), 0);
    s.synchronize(&p, cell);
    s.advance(64);
    assert_eq!(stored(s.view(&[]).unwrap(), stored_id), 1);
    s.reset();
    s.advance(64);
    s.synchronize(&p, root);
    s.advance(64);
    assert_eq!(s.ticks(), 0);
    assert_eq!(stored(s.view(&[ids[0]]).unwrap(), stored_id), 0);
}

#[test]
fn register_error_controls_allow_capture_but_error_data_holds_storage() {
    for error_port in [1, 3, 4, 5, 6] {
        let (c, id, inputs) = fixture(Kind::Register, Trigger::Rising);
        let mut s = Simulator::default();
        s.synchronize(&c);
        settle(&mut s);
        drive(&mut s, inputs[0], 10);
        drive(&mut s, inputs[2], 1);
        s.external_input(
            inputs[error_port - 1],
            Value::filled(if error_port == 1 { 4 } else { 1 }, Bit::Error),
        );
        settle(&mut s);
        drive(&mut s, inputs[1], 1);
        let expected = if error_port == 1 { 0 } else { 10 };
        assert_eq!(stored(&s, id), expected, "error port {error_port}");
        assert_eq!(s.port_value(id, 0).unwrap().number(), Some(expected));
    }
}

use super::tests::{FOUR, component, fixture, link};
use super::*;
use logisim_core::{Component, Gate, Kind, Point, Pull};

fn pull(c: &mut Circuit, point: Point, value: Pull) {
    let mut r = Component::new(Kind::PullResistor, point);
    r.bus.pull = value;
    c.add_component(r);
}

fn compare(event: &Simulator, stable: &mut Simulator, evaluator: &mut Evaluator) {
    assert_eq!(event.pending_count(), 0);
    evaluator
        .evaluate_cached(|(id, port)| event.drive_value(id, port))
        .unwrap();
    for ((id, port), value) in evaluator.values() {
        assert_eq!(Some(value), event.drive_value(id, port));
    }
    let incremental: Vec<_> = evaluator.values().collect();
    evaluator
        .evaluate(|(id, port)| event.drive_value(id, port))
        .unwrap();
    assert_eq!(incremental, evaluator.values().collect::<Vec<_>>());
    stable.settle_compiled().unwrap();
    assert_eq!(event.drives, stable.drives);
    assert_eq!(event.values, stable.values);
    let diagnostics = |sim: &Simulator| {
        sim.diagnostics()
            .into_iter()
            .map(|d| (d.problem, d.components, d.wires))
            .collect::<Vec<_>>()
    };
    assert_eq!(diagnostics(event), diagnostics(stable));
    assert_eq!(stable.pending_count(), 0);
}

#[test]
fn shared_drivers_and_conflicting_pulls_match_four_state_resolution() {
    for width in [1, 4, 32] {
        for pulls in [
            vec![],
            vec![Pull::Zero],
            vec![Pull::One],
            vec![Pull::Error],
            vec![Pull::Zero, Pull::One],
            vec![Pull::Zero, Pull::Error],
        ] {
            let (mut c, gate, first) = fixture(component(Kind::Gate(Gate::Buffer), width));
            let position = c.component(first[0].0).unwrap().position;
            let mut pin = Component::new(Kind::Pin, position);
            pin.bits = width;
            let second = c.add_component(pin);
            for value in pulls {
                pull(&mut c, position, value);
            }
            let plan = CompiledCircuit::new(&c);
            assert_eq!(plan.stats().nodes, 1);
            let mut evaluator = plan.evaluator();
            let mut event = Simulator::default();
            let mut stable = Simulator::default();
            event.synchronize(&c);
            stable.synchronize(&c);
            for a in 0..4 {
                for b in 0..4 {
                    let left = Value::from_bits((0..width).map(|i| FOUR[(a + usize::from(i)) % 4]));
                    let right =
                        Value::from_bits((0..width).map(|i| FOUR[(b + usize::from(i)) % 4]));
                    for sim in [&mut event, &mut stable] {
                        sim.external_input(first[0].0, left);
                        sim.external_input(second, right);
                    }
                    event.advance(128);
                    compare(&event, &mut stable, &mut evaluator);
                    assert_eq!(stable.port_value(gate, 0), event.port_value(gate, 1));
                }
            }
        }
    }
}

#[test]
fn folded_bus_values_and_output_pulls_do_not_become_component_drives() {
    let (mut c, controlled, inputs) = fixture(component(Kind::ControlledBuffer, 4));
    let position = c_position(&c, controlled, 0);
    pull(&mut c, position, Pull::One);
    let mut buffer = component(Kind::Gate(Gate::Buffer), 4);
    buffer.position.x += 400;
    let buffer = c.add_component(buffer);
    link(&mut c, (controlled, 0), (buffer, 1), "resolved-output");
    let plan = CompiledCircuit::new(&c);
    let mut evaluator = plan.evaluator();
    let mut event = Simulator::default();
    let mut stable = Simulator::default();
    event.synchronize(&c);
    stable.synchronize(&c);
    for enable in [0, 1, 0, 1] {
        for sim in [&mut event, &mut stable] {
            sim.set_input(inputs[0].0, 5);
            sim.set_input(inputs[1].0, enable);
        }
        event.advance(128);
        compare(&event, &mut stable, &mut evaluator);
        assert_eq!(
            stable.drive_value(controlled, 0),
            Some(if enable == 0 {
                Value::filled(4, Bit::Unknown)
            } else {
                Value::known(4, 5)
            })
        );
        assert_eq!(
            stable.port_value(controlled, 0),
            Some(Value::known(4, if enable == 0 { 15 } else { 5 }))
        );
        assert_eq!(
            stable.drive_value(buffer, 0),
            stable.port_value(controlled, 0)
        );
    }
    // Replace boundaries with constants, then change the pull: folding and edit
    // invalidation must keep a released raw drive separate from its resolved bus.
    for &(id, _) in &inputs {
        let mut constant = c.component(id).unwrap().clone();
        constant.kind = Kind::Constant;
        constant.constant = 0;
        c.replace_component(constant);
    }
    for value in [Pull::One, Pull::Zero, Pull::Error] {
        let id = c
            .components
            .iter()
            .find(|c| c.kind == Kind::PullResistor)
            .unwrap()
            .id;
        let mut changed = c.component(id).unwrap().clone();
        changed.bus.pull = value;
        c.replace_component(changed);
        let plan = CompiledCircuit::new(&c);
        assert_eq!(plan.stats().instructions, 0);
        let mut evaluator = plan.evaluator();
        event.synchronize(&c);
        stable.synchronize(&c);
        event.advance(128);
        compare(&event, &mut stable, &mut evaluator);
        assert_eq!(
            evaluator.value((controlled, 0)),
            Some(Value::filled(4, Bit::Unknown))
        );
        assert_eq!(
            evaluator.value((buffer, 0)),
            Some(Value::filled(4, crate::bus::pull_bit(value)))
        );
    }
}

fn c_position(c: &Circuit, id: logisim_core::Id, port: usize) -> Point {
    c.component(id).unwrap().ports()[port]
}

#[test]
fn shorted_source_bits_and_splitter_pulls_keep_distinct_dependencies() {
    let mut c = Circuit::default();
    let mut pin = Component::new(Kind::Pin, Point::ZERO);
    pin.bits = 32;
    let pin = c.add_component(pin);
    let mut split = Component::new(Kind::Splitter, Point::new(600, 500));
    split.bits = 32;
    split.splitter.fanout = 2;
    split.splitter.mapping = [None; 32];
    split.splitter.mapping[0] = Some(0);
    split.splitter.mapping[31] = Some(1);
    let split = c.add_component(split);
    link(&mut c, (pin, 0), (split, 0), "bus");
    link(&mut c, (split, 1), (split, 2), "short");
    let position = c_position(&c, split, 1);
    pull(&mut c, position, Pull::One);
    let buffer = c.add_component(component(Kind::Gate(Gate::Buffer), 1));
    link(&mut c, (split, 1), (buffer, 1), "short");
    let mut evaluator = CompiledCircuit::new(&c).evaluator();
    let mut event = Simulator::default();
    let mut stable = Simulator::default();
    event.synchronize(&c);
    stable.synchronize(&c);
    for lo in FOUR {
        for hi in FOUR {
            let value = Value::from_bits((0..32).map(|i| {
                if i == 0 {
                    lo
                } else if i == 31 {
                    hi
                } else {
                    Bit::Zero
                }
            }));
            for sim in [&mut event, &mut stable] {
                sim.external_input(pin, value);
            }
            event.advance(128);
            compare(&event, &mut stable, &mut evaluator);
            if lo == Bit::Zero && hi == Bit::One {
                assert_ne!(event.drive_value(pin, 0), event.port_value(pin, 0));
            }
            assert_eq!(
                evaluator.value((buffer, 0)).unwrap().bit(0),
                event.port_value(split, 0).unwrap().bit(31)
            );
        }
    }
}

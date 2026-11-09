use super::super::*;
use logisim_core::{
    Arithmetic, BitSearch, Component, Extension, Facing, FlipFlop, Gate, Kind, Parity,
    ParityOutput, Pla, PlaInput, Plexer, Point, Shift,
};

use super::support::*;

fn settle(sim: &mut Simulator) {
    sim.advance(1024);
    assert!(!sim.limited());
    assert_eq!(sim.pending_count(), 0);
}

fn compare(sim: &Simulator, evaluator: &mut Evaluator) {
    evaluator
        .evaluate_cached(|(id, port)| sim.drive_value(id, port))
        .unwrap();
    assert!(evaluator.values().count() > 0);
    for (port, value) in evaluator.values() {
        assert_eq!(Some(value), sim.drive_value(port.0, port.1), "{port:?}");
    }
    let incremental: Vec<_> = evaluator.values().collect();
    evaluator
        .evaluate(|(id, port)| sim.drive_value(id, port))
        .unwrap();
    assert_eq!(incremental, evaluator.values().collect::<Vec<_>>());
}

#[test]
fn gates_preserve_four_states_negation_and_physically_absent_inputs() {
    for gate in Gate::ALL {
        for width in [1, 4, 32] {
            for odd in [false, true] {
                for negated in [0, 1] {
                    // 0..4 are driven values, 4 is absent, 5 is a floating wire.
                    for a in 0..6 {
                        for b in 0..6 {
                            let mut comp = component(Kind::Gate(gate), width);
                            comp.gate_options.xor_odd = odd;
                            comp.gate_options.negated = negated;
                            let mut c = Circuit::default();
                            let mut drives = vec![];
                            for (port, state) in comp.ports()[1..].iter().zip([a, b]) {
                                if state < 4 {
                                    let mut pin = Component::new(Kind::Pin, *port);
                                    pin.bits = width;
                                    drives.push((
                                        c.add_component(pin),
                                        Value::filled(width, FOUR[state]),
                                    ));
                                } else if state == 5 {
                                    c.add_wire(*port, *port + Point::new(-100, 0));
                                }
                            }
                            c.add_component(comp);
                            let mut sim = Simulator::default();
                            sim.synchronize(&c);
                            for (id, value) in drives {
                                sim.external_input(id, value);
                            }
                            settle(&mut sim);
                            compare(&sim, &mut sim.compile_combinational().evaluator());
                        }
                    }
                }
            }
        }
    }
}

fn variants(width: u8) -> Vec<Component> {
    let mut result = vec![];
    for kind in [Kind::ControlledBuffer, Kind::ControlledInverter] {
        for negate in [false, true] {
            for left in [false, true] {
                let mut c = component(kind, width);
                c.bus.negate_control = negate;
                c.bus.control_left = left;
                result.push(c);
            }
        }
    }
    for kind in Arithmetic::ALL {
        for mode in 0..5 {
            let mut c = component(Kind::Arithmetic(kind), width);
            c.arithmetic.signed = mode % 2 == 0;
            c.arithmetic.shift = Shift::ALL[mode];
            c.arithmetic.search = BitSearch::ALL[mode % 4];
            c.arithmetic.bit_inputs = if mode == 4 { 32 } else { 3 };
            result.push(c);
        }
    }
    for kind in Plexer::ALL
        .into_iter()
        .filter(|k| *k != Plexer::DisplayDecoder)
    {
        for mode in 0..8 {
            let mut c = component(Kind::Plexer(kind), width);
            c.plexing.select_bits = if mode == 7 { 5 } else { 2 };
            c.plexing.enable = mode & 1 != 0;
            c.plexing.disabled_zero = mode & 2 != 0;
            c.plexing.three_state = mode & 4 != 0;
            c.plexing.no_input_zero = mode & 4 != 0;
            c.plexing.group_bits = [1, 3, 32][mode % 3];
            c.plexing.top_right = mode & 2 != 0;
            result.push(c);
        }
    }
    for parity in [Parity::Odd, Parity::Even] {
        for output in ParityOutput::ALL {
            for inputs in [2, 32] {
                let mut c = component(Kind::Parity(parity), width);
                c.parity.inputs = inputs;
                c.parity.output = output;
                c.parity.negated = 0xaaaa_aaaa;
                result.push(c);
            }
        }
    }
    for extension in Extension::ALL {
        let mut c = component(Kind::BitExtender, width);
        c.extension = extension;
        c.input_bits = if width == 1 { 4 } else { 2 };
        result.push(c);
    }
    for high in [true, false] {
        let mut c = component(Kind::Rom, width);
        c.memory.address_bits = 2;
        c.memory.select_high = high;
        for (a, v) in [0, u32::MAX, 0xaaaa_aaaa, 0x5555_5555]
            .into_iter()
            .enumerate()
        {
            c.memory.contents.set(a as u32, v);
        }
        result.push(c);
        let mut c = component(Kind::PlaRom, width);
        c.pla = Pla::new(4, 3, width);
        c.pla.select_high = high;
        c.pla.set_input(0, 0, PlaInput::Direct);
        c.pla.set_input(0, 3, PlaInput::Inverted);
        c.pla.set_input(1, 2, PlaInput::Direct);
        for output in 0..width {
            c.pla.set_output(output % 3, output, true);
        }
        result.push(c);
    }
    result
}

#[test]
fn all_supported_word_operations_match_settled_engine_with_mixed_buses() {
    let mut random = 0x9375_1234u32;
    for width in [1, 4, 32] {
        for (index, mut comp) in variants(width).into_iter().enumerate() {
            comp.facing = Facing::ALL[index % 4];
            let kind = comp.kind;
            let (circuit, _, inputs) = fixture(comp);
            let mut sim = Simulator::default();
            sim.synchronize(&circuit);
            let compiled = sim.compile_combinational();
            assert_eq!(compiled.stats().nodes, 1, "{kind:?}");
            let mut evaluator = compiled.evaluator();
            for sample in 0..96 {
                for &(id, bits) in &inputs {
                    let v = Value::from_bits((0..bits).map(|_| {
                        random ^= random << 13;
                        random ^= random >> 17;
                        random ^= random << 5;
                        match sample {
                            0 => Bit::Zero,
                            1 => Bit::One,
                            2 => Bit::Unknown,
                            3 => Bit::Error,
                            4..=47 => FOUR[(random & 1) as usize],
                            _ => FOUR[(random & 3) as usize],
                        }
                    }));
                    sim.external_input(id, v);
                }
                settle(&mut sim);
                compare(&sim, &mut evaluator);
            }
        }
    }
}

#[test]
fn splitter_gathers_reordered_dynamic_constant_and_floating_bits() {
    let mut c = Circuit::default();
    let mut split = Component::new(Kind::Splitter, Point::new(600, 300));
    split.bits = 4;
    split.splitter.fanout = 3;
    split.splitter.mapping = [None; 32];
    split.splitter.mapping[..4].copy_from_slice(&[Some(1), Some(0), Some(1), Some(2)]);
    let split = c.add_component(split);
    let mut input = Component::new(Kind::Pin, Point::ZERO);
    input.bits = 2;
    let input = c.add_component(input);
    let power = c.add_component(Component::new(Kind::Power, Point::new(0, 500)));
    let gate = c.add_component(component(Kind::Gate(Gate::Buffer), 4));
    link(&mut c, (input, 0), (split, 2), "dynamic");
    link(&mut c, (power, 0), (split, 1), "constant");
    link(&mut c, (split, 0), (gate, 1), "assembled");
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    let mut evaluator = sim.compile_combinational().evaluator();
    for low in FOUR {
        for high in FOUR {
            sim.external_input(input, Value::from_bits([low, high].into_iter()));
            settle(&mut sim);
            compare(&sim, &mut evaluator);
            assert_eq!(
                evaluator.value((gate, 0)),
                Some(Value::from_bits(
                    [low, Bit::One, high, Bit::Unknown].into_iter()
                ))
            );
        }
    }
}

#[test]
fn fanout_is_a_shared_linear_plan_and_xor_is_not_boolean_simplified() {
    let mut c = Circuit::default();
    let input = c.add_component(Component::new(Kind::Pin, Point::ZERO));
    let mut previous = input;
    for layer in 0..24 {
        let mut ids = vec![];
        for (column, kind) in [Gate::Buffer, Gate::Buffer, Gate::Xor]
            .into_iter()
            .enumerate()
        {
            let mut comp = component(Kind::Gate(kind), 1);
            comp.position = Point::new(300 + column as i32 * 300, layer * 200);
            ids.push(c.add_component(comp));
        }
        link(&mut c, (previous, 0), (ids[0], 1), &format!("{layer}-a"));
        link(&mut c, (previous, 0), (ids[1], 1), &format!("{layer}-b"));
        link(&mut c, (ids[0], 0), (ids[2], 1), &format!("{layer}-c"));
        link(&mut c, (ids[1], 0), (ids[2], 2), &format!("{layer}-d"));
        previous = ids[2];
    }
    let plan = CompiledCircuit::new(&c);
    assert_eq!(plan.stats().instructions, 72);
    assert_eq!(plan.stats().boundary_ports, 1);
    let mut evaluator = plan.evaluator();
    for input_bit in FOUR {
        let mut reads = 0;
        evaluator
            .evaluate(|p| {
                assert_eq!(p, (input, 0));
                reads += 1;
                Some(Value::filled(1, input_bit))
            })
            .unwrap();
        assert_eq!(reads, 1);
        let expected = if matches!(input_bit, Bit::Zero | Bit::One) {
            Bit::Zero
        } else {
            Bit::Error
        };
        assert_eq!(
            evaluator.value((previous, 0)),
            Some(Value::filled(1, expected))
        );
    }
}

#[test]
fn constant_folding_retains_internal_values_and_non_boolean_rules() {
    let mut c = Circuit::default();
    let absent = c.add_component(component(Kind::Gate(Gate::Or), 1));
    let mut invert = component(Kind::Gate(Gate::Not), 1);
    invert.position.y += 200;
    let invert = c.add_component(invert);
    link(&mut c, (absent, 0), (invert, 1), "error");
    let mut xor = component(Kind::Gate(Gate::Xor), 4);
    xor.gate_options.inputs = 3;
    xor.position.y += 400;
    let ports = xor.ports();
    let xor = c.add_component(xor);
    for port in &ports[1..] {
        let mut value = Component::new(Kind::Constant, *port);
        value.bits = 4;
        value.constant = 1;
        c.add_component(value);
    }
    let plan = CompiledCircuit::new(&c);
    assert_eq!(plan.stats().instructions, 0);
    assert_eq!(plan.stats().folded_nodes, 6);
    let mut evaluator = plan.evaluator();
    evaluator
        .evaluate(|_| panic!("constant plan has no boundary"))
        .unwrap();
    assert_eq!(evaluator.value((xor, 0)), Some(Value::known(4, 0)));
    assert_eq!(
        evaluator.value((absent, 0)),
        Some(Value::filled(1, Bit::Error))
    );
    assert_eq!(
        evaluator.value((invert, 0)),
        Some(Value::filled(1, Bit::Error))
    );
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    settle(&mut sim);
    compare(&sim, &mut evaluator);
}

#[test]
fn evaluation_failure_hides_stale_values_and_instances_are_independent() {
    let (c, output, pins) = fixture(component(Kind::Gate(Gate::And), 4));
    let plan = CompiledCircuit::new(&c);
    let mut first = plan.evaluator();
    let mut second = plan.clone().evaluator();
    assert_eq!(first.value((output, 0)), None);
    first.evaluate(|_| Some(Value::known(4, 15))).unwrap();
    second.evaluate(|_| Some(Value::known(4, 0))).unwrap();
    assert_eq!(first.value((output, 0)), Some(Value::known(4, 15)));
    assert_eq!(second.value((output, 0)), Some(Value::known(4, 0)));
    assert_eq!(
        first.evaluate(|p| (p.0 == pins[0].0).then(|| Value::known(4, 1))),
        Err(EvaluationError::MissingInput {
            port: (pins[1].0, 0)
        })
    );
    assert_eq!(first.values().count(), 0);
    assert_eq!(first.value((output, 0)), None);
    assert_eq!(
        first.evaluate(|_| Some(Value::known(1, 1))),
        Err(EvaluationError::WidthMismatch {
            port: (pins[0].0, 0),
            expected: 4,
            actual: 1
        })
    );
    first.evaluate(|_| Some(Value::known(4, 3))).unwrap();
    assert_eq!(first.value((output, 0)), Some(Value::known(4, 3)));
    assert_eq!(second.value((output, 0)), Some(Value::known(4, 0)));
    assert_eq!(first.value((pins[0].0, 0)), None);
}

#[test]
fn compiled_rom_is_an_immutable_document_snapshot() {
    let mut rom = component(Kind::Rom, 4);
    rom.memory.address_bits = 2;
    rom.memory.contents.set(0, 5);
    let (mut c, id, _) = fixture(rom);
    let original = CompiledCircuit::new(&c);
    let mut edited = c.component(id).unwrap().clone();
    edited.memory.contents.set(0, 10);
    c.replace_component(edited);
    for (plan, expected) in [(original, 5), (CompiledCircuit::new(&c), 10)] {
        let mut evaluator = plan.evaluator();
        evaluator
            .evaluate(|p| Some(Value::known(c.component(p.0).unwrap().bits, 0)))
            .unwrap();
        // CS defaults active low. Address and select are both zero.
        assert_eq!(evaluator.value((id, 0)), Some(Value::known(4, expected)));
    }
}

#[test]
fn cached_evaluation_checks_every_four_state_input_and_recovers_after_errors() {
    let (c, output, pins) = fixture(component(Kind::Gate(Gate::Or), 4));
    let mut evaluator = CompiledCircuit::new(&c).evaluator();
    for bit in FOUR {
        assert!(
            evaluator
                .evaluate_cached(|_| Some(Value::filled(4, bit)))
                .unwrap()
        );
        let expected = evaluator.value((output, 0));
        let mut reads = 0;
        assert!(
            !evaluator
                .evaluate_cached(|_| {
                    reads += 1;
                    Some(Value::filled(4, bit))
                })
                .unwrap()
        );
        assert_eq!(reads, pins.len());
        assert_eq!(evaluator.value((output, 0)), expected);
        assert!(
            evaluator
                .evaluate_cached(|p| { (p.0 == pins[0].0).then(|| Value::known(4, 0)) })
                .is_err()
        );
        assert_eq!(evaluator.value((output, 0)), None);
        assert!(
            evaluator
                .evaluate_cached(|_| Some(Value::filled(4, bit)))
                .unwrap()
        );
        assert_eq!(evaluator.value((output, 0)), expected);
    }
}

#[test]
fn compilation_and_evaluation_do_not_sample_or_advance_state() {
    let mut c = Circuit::default();
    let state = c.add_component(Component::new(Kind::FlipFlop(FlipFlop::D), Point::ZERO));
    let gate = c.add_component(component(Kind::Gate(Gate::Not), 1));
    link(&mut c, (state, 0), (gate, 1), "q");
    link(&mut c, (gate, 0), (state, 3), "d");
    let mut decoder = component(Kind::Plexer(Plexer::DisplayDecoder), 4);
    decoder.position.x += 600;
    let decoder = c.add_component(decoder);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    let before = (
        sim.pending_count(),
        sim.stored_value(state),
        sim.port_value(gate, 0),
    );
    let plan = sim.compile_combinational();
    assert_eq!(plan.stats().nodes, 1);
    let mut evaluator = plan.evaluator();
    for bit in FOUR {
        evaluator.evaluate(|_| Some(Value::filled(1, bit))).unwrap();
    }
    assert_eq!(evaluator.value((state, 0)), None);
    assert_eq!(evaluator.value((decoder, 0)), None);
    assert_eq!(
        (
            sim.pending_count(),
            sim.stored_value(state),
            sim.port_value(gate, 0)
        ),
        before
    );
}

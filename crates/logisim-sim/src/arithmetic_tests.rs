use super::*;
use logisim_core::{Arithmetic, Component, Point, Shift};

fn value(text: &str) -> Value {
    Value::from_bits(text.chars().rev().map(|b| match b {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        'E' => Bit::Error,
        _ => panic!("bad test signal"),
    }))
}
fn fixture(kind: Arithmetic, width: u8) -> (Circuit, Id, Vec<Id>) {
    let mut c = Component::new(Kind::Arithmetic(kind), Point::new(300, 300));
    c.bits = width;
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for (i, point) in c
        .ports()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !c.drives_port(*i))
    {
        let mut pin = Component::new(Kind::Pin, point);
        pin.bits = c.port_width(i).unwrap();
        inputs.push(circuit.add_component(pin));
    }
    let id = circuit.add_component(c);
    (circuit, id, inputs)
}
fn run(s: &mut Simulator, circuit: &Circuit, pins: &[Id], values: &[Value]) {
    s.synchronize(circuit);
    for (&id, &v) in pins.iter().zip(values) {
        s.external_input(id, v);
    }
    s.advance(100);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
fn output(s: &Simulator, id: Id, port: usize) -> String {
    s.port_value(id, port).unwrap().binary()
}

#[test]
fn observed_single_bit_add_subtract_four_state_matrices() {
    for (kind, cin, sum, carry) in [
        (
            Arithmetic::Adder,
            '0',
            "01XE10XEXXXEEEEE",
            "00XE01XEXXXEEEEE",
        ),
        (
            Arithmetic::Adder,
            '1',
            "10XE01XEXXXEEEEE",
            "01XE11XEXXXEEEEE",
        ),
        (
            Arithmetic::Adder,
            'X',
            "01XE10XEXXXEEEEE",
            "00XE01XEXXXEEEEE",
        ),
        (
            Arithmetic::Adder,
            'E',
            "EEEEEEEEEEEEEEEE",
            "EEEEEEEEEEEEEEEE",
        ),
        (
            Arithmetic::Subtractor,
            '0',
            "01EE10EEXXEEEEEE",
            "01EE00EEEEEEEEEE",
        ),
        (
            Arithmetic::Subtractor,
            '1',
            "10EE01EEXXEEEEEE",
            "11EE01EEEEEEEEEE",
        ),
        (
            Arithmetic::Subtractor,
            'X',
            "01EE10EEXXEEEEEE",
            "01EE00EEEEEEEEEE",
        ),
        (
            Arithmetic::Subtractor,
            'E',
            "EEEEEEEEEEEEEEEE",
            "EEEEEEEEEEEEEEEE",
        ),
    ] {
        let (c, id, pins) = fixture(kind, 1);
        let mut s = Simulator::default();
        for (i, a) in ["0", "1", "X", "E"].into_iter().enumerate() {
            for (j, b) in ["0", "1", "X", "E"].into_iter().enumerate() {
                run(
                    &mut s,
                    &c,
                    &pins,
                    &[value(a), value(b), value(&cin.to_string())],
                );
                assert_eq!(
                    output(&s, id, 0),
                    &sum[i * 4 + j..i * 4 + j + 1],
                    "{kind:?} {a} {b} {cin}"
                );
                assert_eq!(
                    output(&s, id, 4),
                    &carry[i * 4 + j..i * 4 + j + 1],
                    "{kind:?} {a} {b} {cin}"
                );
            }
        }
    }
}
#[test]
fn observed_partial_buses_preserve_low_bits_and_first_uncertain_column() {
    for (a, b, add, cout, sub, bout) in [
        ("000X", "0000", "XXXX", "X", "XXXX", "E"),
        ("X000", "0000", "X000", "X", "X000", "E"),
        ("0000", "000X", "XXXX", "X", "EEEE", "E"),
        ("00E0", "0000", "EEE0", "E", "EEE0", "E"),
        ("00X0", "0E00", "XXX0", "X", "XXX0", "E"),
        ("X000", "000X", "XXXX", "X", "EEEE", "E"),
        ("0X00", "000E", "EEEE", "E", "EEEE", "E"),
        ("00E0", "000X", "XXXX", "X", "EEEE", "E"),
        ("000E", "000X", "EEEE", "E", "EEEE", "E"),
    ] {
        for (kind, result, carry) in [
            (Arithmetic::Adder, add, cout),
            (Arithmetic::Subtractor, sub, bout),
        ] {
            let (c, id, pins) = fixture(kind, 4);
            let mut s = Simulator::default();
            run(&mut s, &c, &pins, &[value(a), value(b), value("0")]);
            assert_eq!(output(&s, id, 0), result, "{kind:?} {a} {b}");
            assert_eq!(output(&s, id, 4), carry);
        }
    }
}
#[test]
fn arithmetic_known_carry_borrow_and_32_bit_overflow() {
    for (kind, width, a, b, cin, result, carry) in [
        (Arithmetic::Adder, 4, 15, 1, 0, 0, 1),
        (Arithmetic::Adder, 4, 7, 7, 1, 15, 0),
        (Arithmetic::Subtractor, 4, 15, 1, 0, 14, 0),
        (Arithmetic::Subtractor, 4, 0, 1, 0, 15, 1),
        (Arithmetic::Subtractor, 4, 7, 7, 1, 15, 1),
        (Arithmetic::Adder, 32, u32::MAX, 1, 0, 0, 1),
        (Arithmetic::Adder, 32, u32::MAX, 0, 1, 0, 1),
        (Arithmetic::Adder, 32, 0x7fffffff, 1, 0, 0x80000000, 0),
    ] {
        let (c, id, pins) = fixture(kind, width);
        let mut s = Simulator::default();
        run(
            &mut s,
            &c,
            &pins,
            &[
                Value::known(width, a),
                Value::known(width, b),
                Value::known(1, cin),
            ],
        );
        assert_eq!(s.port_value(id, 0), Some(Value::known(width, result)));
        assert_eq!(s.port_value(id, 4), Some(Value::known(1, carry)));
    }
}
#[test]
fn comparator_observed_priority_signedness_and_property_invalidation() {
    let (mut c, id, pins) = fixture(Arithmetic::Comparator, 4);
    let mut s = Simulator::default();
    for (signed, a, b, want) in [
        (true, "1000", "0111", "001"),
        (false, "1000", "0111", "100"),
        (true, "0111", "1000", "100"),
        (true, "1000", "1000", "010"),
        (true, "100X", "0000", "001"),
        (false, "100E", "0000", "100"),
        (true, "100E", "0000", "001"),
        (true, "010X", "0000", "100"),
        (true, "000X", "0000", "XXX"),
        (true, "000X", "000E", "EEE"),
        (true, "X000", "000E", "XXX"),
        (true, "E000", "000X", "EEE"),
        (true, "00XE", "0000", "XXX"),
        (true, "00EX", "0000", "EEE"),
        (true, "1100", "1011", "100"),
        (true, "1111", "0000", "001"),
    ] {
        c.components
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .arithmetic
            .signed = signed;
        run(&mut s, &c, &pins, &[value(a), value(b)]);
        assert_eq!(
            [3, 0, 4].map(|p| output(&s, id, p)).concat(),
            want,
            "signed={signed} {a} {b}"
        );
    }
}
#[test]
fn five_shift_modes_preserve_each_data_state_and_reject_unknown_amount() {
    let (mut c, id, pins) = fixture(Arithmetic::Shifter, 4);
    let mut s = Simulator::default();
    for (mode, results, partial, floating, errors) in [
        (
            Shift::LogicalLeft,
            ["1001", "0010", "1000"],
            "0XE0",
            "XXX0",
            "EEE0",
        ),
        (
            Shift::LogicalRight,
            ["1001", "0100", "0001"],
            "010X",
            "0XXX",
            "0EEE",
        ),
        (
            Shift::ArithmeticRight,
            ["1001", "1100", "1111"],
            "110X",
            "XXXX",
            "EEEE",
        ),
        (
            Shift::RotateLeft,
            ["1001", "0011", "1100"],
            "0XE1",
            "XXXX",
            "EEEE",
        ),
        (
            Shift::RotateRight,
            ["1001", "1100", "0011"],
            "E10X",
            "XXXX",
            "EEEE",
        ),
    ] {
        c.components
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .arithmetic
            .shift = mode;
        for (amount, expected) in [0, 1, 3].into_iter().zip(results) {
            run(&mut s, &c, &pins, &[value("1001"), Value::known(2, amount)]);
            assert_eq!(output(&s, id, 0), expected);
        }
        for (data, want) in [("10XE", partial), ("XXXX", floating), ("EEEE", errors)] {
            run(&mut s, &c, &pins, &[value(data), value("01")]);
            assert_eq!(output(&s, id, 0), want);
        }
        for amount in ["XX", "EE", "0X", "E0"] {
            run(&mut s, &c, &pins, &[value("0000"), value(amount)]);
            assert_eq!(output(&s, id, 0), "EEEE");
        }
    }
}
#[test]
fn shift_one_bit_non_power_of_two_and_32_bit_boundaries() {
    for (width, data, amount, results) in [
        (1, 1, 1, [0, 0, 1, 1, 1]),
        (3, 5, 3, [0, 0, 7, 5, 5]),
        (5, 17, 7, [0, 0, 31, 6, 12]),
        (32, 1, 31, [0x80000000, 0, 0, 0x80000000, 2]),
    ] {
        let (mut c, id, pins) = fixture(Arithmetic::Shifter, width);
        let mut s = Simulator::default();
        for (mode, want) in Shift::ALL.into_iter().zip(results) {
            let component = c.components.iter_mut().find(|c| c.id == id).unwrap();
            component.arithmetic.shift = mode;
            let abits = component.shift_amount_bits();
            run(
                &mut s,
                &c,
                &pins,
                &[Value::known(width, data), Value::known(abits, amount)],
            );
            assert_eq!(
                s.port_value(id, 0),
                Some(Value::known(width, want)),
                "{width} {mode:?}"
            );
        }
    }
}

#[test]
fn two_arithmetic_instances_propagate_carry_between_words() {
    use logisim_core::{Facing, Project};
    let mut p = Project::default();
    let sub = p.add("word".into()).unwrap();
    let (mut circuit, adder, inputs) = fixture(Arithmetic::Adder, 4);
    let body = circuit.component(adder).unwrap().clone();
    let mut output_ids = vec![];
    for i in [0, 4] {
        let mut pin = Component::new(Kind::Pin, body.ports()[i]);
        pin.bits = body.port_width(i).unwrap();
        pin.output = true;
        pin.facing = Facing::West;
        output_ids.push(circuit.add_component(pin));
    }
    p.circuit_mut(sub).unwrap().circuit = circuit;
    let mut words = vec![];
    let mut low_b = 0;
    for (i, operand_b) in [1, 0].into_iter().enumerate() {
        let instance = p
            .instance(sub, Point::new(300 + i as i32 * 300, 200))
            .unwrap();
        let points: Vec<_> = inputs
            .iter()
            .chain(&output_ids)
            .map(|pin| {
                let index = instance
                    .interface
                    .ports
                    .iter()
                    .position(|p| p.pin == *pin)
                    .unwrap();
                (index, instance.ports()[index])
            })
            .collect();
        let root = &mut p.main_circuit_mut().circuit;
        let id = root.add_component(instance);
        for (j, v) in [15, operand_b, 0].into_iter().enumerate() {
            if i == 1 && j == 2 {
                continue;
            }
            let mut source = Component::new(Kind::Constant, points[j].1);
            source.bits = if j == 2 { 1 } else { 4 };
            source.constant = v;
            let source_id = root.add_component(source);
            if i == 0 && j == 1 {
                low_b = source_id;
            }
        }
        words.push((id, points));
    }
    let from = words[0].1[4].1;
    let to = words[1].1[2].1;
    let corner = Point::new(from.x + 30, from.y);
    let root = &mut p.main_circuit_mut().circuit;
    root.add_wire(from, corner);
    root.add_wire(corner, Point::new(corner.x, to.y));
    root.add_wire(Point::new(corner.x, to.y), to);
    let mut s = ProjectSimulator::default();
    for (b, expected) in [(1, [(0, 1), (0, 1)]), (0, [(15, 0), (15, 0)])] {
        p.main_circuit_mut()
            .circuit
            .components
            .iter_mut()
            .find(|c| c.id == low_b)
            .unwrap()
            .constant = b;
        s.synchronize(&p, p.main);
        s.advance(128);
        assert!(s.error().is_none());
        let root = s.view(&[]).unwrap();
        for ((id, points), (sum, carry)) in words.iter().zip(expected) {
            assert_eq!(
                root.port_value(*id, points[3].0),
                Some(Value::known(4, sum))
            );
            assert_eq!(
                root.port_value(*id, points[4].0),
                Some(Value::known(1, carry))
            );
        }
    }
}

#[path = "arithmetic_extra_tests.rs"]
mod extra;

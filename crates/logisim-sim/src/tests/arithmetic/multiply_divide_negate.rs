//! Runtime-observed multiplication, division and negation regressions.
use super::support::*;
use crate::*;
use logisim_core::{Arithmetic, Component, Kind, Point};

#[test]
fn wired_product_words_feed_dividend_and_remainder_feeds_negator() {
    let (mut c, mul, pins) = fixture(Arithmetic::Multiplier, 4);
    let mut div = Component::new(Kind::Arithmetic(Arithmetic::Divider), Point::new(550, 350));
    div.bits = 4;
    let mut neg = Component::new(Kind::Arithmetic(Arithmetic::Negator), Point::new(700, 450));
    neg.bits = 4;
    let mut divisor = Component::new(Kind::Constant, div.ports()[2]);
    divisor.bits = 4;
    divisor.constant = 2;
    c.add_component(divisor);
    let m = c.component(mul).unwrap().ports();
    let d = div.ports();
    for (from, to, x) in [
        (m[0], d[1], 480),
        (m[4], d[3], 430),
        (d[4], neg.ports()[1], 610),
    ] {
        c.add_wire(from, Point::new(x, from.y));
        c.add_wire(Point::new(x, from.y), Point::new(x, to.y));
        c.add_wire(Point::new(x, to.y), to);
    }
    let div = c.add_component(div);
    let neg = c.add_component(neg);
    let mut s = Simulator::default();
    run(
        &mut s,
        &c,
        &pins,
        &[Value::known(4, 3), Value::known(4, 5), Value::known(4, 2)],
    );
    for (id, port, result) in [
        (mul, 0, 1),
        (mul, 4, 1),
        (div, 0, 8),
        (div, 4, 1),
        (neg, 0, 15),
    ] {
        assert_eq!(s.port_value(id, port), Some(Value::known(4, result)));
    }
}

#[test]
fn multiply_and_divide_single_bit_four_state_matrices() {
    for (kind, extra, low, high) in [
        (
            Arithmetic::Multiplier,
            "0",
            "00XE01XEXXXEEEEE",
            "00XE00XEXXXEEEEE",
        ),
        (
            Arithmetic::Multiplier,
            "1",
            "11XE10XEXXXEEEEE",
            "00XE01XEXXXEEEEE",
        ),
        (
            Arithmetic::Multiplier,
            "X",
            "00XE01XEXXXEEEEE",
            "00XE00XEXXXEEEEE",
        ),
        (
            Arithmetic::Multiplier,
            "E",
            "EEEEEEEEEEEEEEEE",
            "EEEEEEEEEEEEEEEE",
        ),
        (
            Arithmetic::Divider,
            "0",
            "00XE11XEXXXEEEEE",
            "00XE00XEXXXEEEEE",
        ),
        (
            Arithmetic::Divider,
            "1",
            "00XE11XEXXXEEEEE",
            "00XE00XEXXXEEEEE",
        ),
        (
            Arithmetic::Divider,
            "X",
            "00XE11XEXXXEEEEE",
            "00XE00XEXXXEEEEE",
        ),
        (
            Arithmetic::Divider,
            "E",
            "EEEEEEEEEEEEEEEE",
            "EEEEEEEEEEEEEEEE",
        ),
    ] {
        let (c, id, pins) = fixture(kind, 1);
        let mut s = Simulator::default();
        for (i, a) in ["0", "1", "X", "E"].into_iter().enumerate() {
            for (j, b) in ["0", "1", "X", "E"].into_iter().enumerate() {
                run(&mut s, &c, &pins, &[value(a), value(b), value(extra)]);
                for (port, expected) in [(0, low), (4, high)] {
                    assert_eq!(
                        output(&s, id, port),
                        &expected[i * 4 + j..i * 4 + j + 1],
                        "{kind:?} {a} {b} {extra} port {port}"
                    );
                }
            }
        }
    }
}

#[test]
fn multiplication_partial_buses_and_auxiliary_input_preserve_observed_prefix() {
    let (c, id, pins) = fixture(Arithmetic::Multiplier, 4);
    let mut s = Simulator::default();
    for (a, b, extra, low, high) in [
        ("0011", "0101", "XXXX", "1111", "0000"),
        ("X000", "0000", "0000", "X000", "XXXX"),
        ("00E0", "0000", "0000", "EEE0", "EEEE"),
        ("00X0", "0E00", "0000", "EEX0", "EEEE"),
        ("000X", "00E0", "0000", "EEEX", "EEEE"),
        ("0E00", "000X", "0000", "EEXX", "EEEE"),
        ("10E1", "0001", "0000", "EEE1", "EEEE"),
        ("0X01", "0011", "0010", "XX01", "XXXX"),
        ("0011", "0101", "X000", "X111", "XXXX"),
        ("0011", "0101", "000X", "XXXX", "XXXX"),
        ("0000", "0000", "00EX", "EEEX", "EEEE"),
        ("0000", "0000", "00XE", "EEEE", "EEEE"),
    ] {
        run(&mut s, &c, &pins, &[value(a), value(b), value(extra)]);
        assert_eq!(output(&s, id, 0), low, "{a} * {b} + {extra}");
        assert_eq!(output(&s, id, 4), high);
    }
}

#[test]
fn division_unknown_upper_word_defaults_only_when_all_bits_float() {
    let (c, id, pins) = fixture(Arithmetic::Divider, 4);
    let mut s = Simulator::default();
    for (a, b, upper, q, r) in [
        ("0011", "0010", "XXXX", "0001", "0001"),
        ("0011", "0010", "X000", "XXXX", "XXXX"),
        ("0011", "0010", "000X", "XXXX", "XXXX"),
        ("000X", "0001", "0000", "XXXX", "XXXX"),
        ("0000", "X000", "0000", "XXXX", "XXXX"),
        ("0000", "0000", "00EX", "EEEE", "EEEE"),
        ("00X0", "0E00", "0000", "EEEE", "EEEE"),
        ("0011", "0010", "00E0", "EEEE", "EEEE"),
        ("EEEE", "XXXX", "0000", "EEEE", "EEEE"),
    ] {
        run(&mut s, &c, &pins, &[value(a), value(b), value(upper)]);
        assert_eq!(output(&s, id, 0), q, "{upper}:{a} / {b}");
        assert_eq!(output(&s, id, 4), r);
    }
}

#[test]
fn known_multiply_divide_full_width_auxiliary_ports_and_32_bit_dividend_boundary() {
    for (kind, width, a, b, extra, low, high) in [
        (Arithmetic::Multiplier, 4, 3, 5, 2, 1, 1),
        (Arithmetic::Multiplier, 4, 15, 15, 15, 0, 15),
        (Arithmetic::Multiplier, 4, 8, 2, 0, 0, 1),
        (
            Arithmetic::Multiplier,
            32,
            u32::MAX,
            u32::MAX,
            0,
            1,
            0xfffffffe,
        ),
        (
            Arithmetic::Multiplier,
            32,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            0,
            u32::MAX,
        ),
        (Arithmetic::Multiplier, 32, 0x80000000, 2, 0, 0, 1),
        (
            Arithmetic::Multiplier,
            32,
            u32::MAX,
            2,
            0x80000000,
            0x7ffffffe,
            2,
        ),
        (Arithmetic::Divider, 4, 3, 2, 1, 9, 1),
        (Arithmetic::Divider, 4, 15, 3, 0, 5, 0),
        (Arithmetic::Divider, 4, 15, 2, 15, 15, 1),
        (Arithmetic::Divider, 4, 15, 0, 0, 15, 0),
        (Arithmetic::Divider, 4, 3, 0, 1, 3, 0),
        (
            Arithmetic::Divider,
            32,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            0xfffffffe,
        ),
        (Arithmetic::Divider, 32, 1, 3, 0x80000000, 0x55555555, 2),
        (
            Arithmetic::Divider,
            32,
            0,
            0x80000000,
            u32::MAX,
            0xfffffffe,
            0,
        ),
        (
            Arithmetic::Divider,
            32,
            u32::MAX,
            2,
            0x80000000,
            0x7fffffff,
            1,
        ),
        (Arithmetic::Divider, 32, 0, u32::MAX, 1, 1, 1),
        (Arithmetic::Divider, 32, 1, 0x80000000, 1, 2, 1),
        (Arithmetic::Divider, 32, u32::MAX, 0, u32::MAX, u32::MAX, 0),
        (Arithmetic::Divider, 32, u32::MAX, u32::MAX, 0, 1, 0),
        (Arithmetic::Divider, 32, 0x80000000, 2, 0, 0x40000000, 0),
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
                Value::known(width, extra),
            ],
        );
        for (port, result) in [(0, low), (4, high)] {
            assert_eq!(
                s.port_value(id, port),
                Some(Value::known(width, result)),
                "{kind:?} width {width} {a:x} {b:x} {extra:x} port {port}"
            );
        }
    }
}

#[test]
fn negation_uncertainty_stops_propagating_after_first_known_one() {
    let (c, id, pins) = fixture(Arithmetic::Negator, 4);
    let mut s = Simulator::default();
    for (input, expected) in [
        ("0000", "0000"),
        ("0001", "1111"),
        ("0111", "1001"),
        ("1000", "1000"),
        ("1111", "0001"),
        ("XXXX", "XXXX"),
        ("EEEE", "EEEE"),
        ("000X", "XXXX"),
        ("X000", "X000"),
        ("00E0", "EEE0"),
        ("00EX", "EEEX"),
        ("00XE", "EEEE"),
        ("01X0", "1XX0"),
        ("01E0", "1EE0"),
        ("10X1", "01X1"),
        ("10E1", "01E1"),
        ("1EX0", "EEX0"),
        ("1XE0", "EEE0"),
        ("XX10", "XX10"),
        ("EE10", "EE10"),
    ] {
        run(&mut s, &c, &pins, &[value(input)]);
        assert_eq!(output(&s, id, 0), expected, "-{input}");
    }
    let (c, id, pins) = fixture(Arithmetic::Negator, 32);
    for (input, expected) in [
        (0, 0),
        (1, u32::MAX),
        (u32::MAX, 1),
        (0x80000000, 0x80000000),
        (0x7fffffff, 0x80000001),
    ] {
        run(&mut s, &c, &pins, &[Value::known(32, input)]);
        assert_eq!(s.port_value(id, 0), Some(Value::known(32, expected)));
    }
}

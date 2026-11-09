//! Expected values from runtime matrices, including partially determined counts.
use super::support::*;
use crate::*;
use logisim_core::BitSearch;
use logisim_core::{Arithmetic, Component, Kind, Point};

fn bit_adder(width: u8, inputs: u8) -> (Circuit, Id, Vec<Id>) {
    let mut c = Component::new(Kind::Arithmetic(Arithmetic::BitAdder), Point::new(400, 400));
    c.bits = width;
    c.arithmetic.bit_inputs = inputs;
    component_fixture(c)
}

#[test]
fn bit_adder_counts_ones_and_preserves_the_known_count_prefix() {
    let (c, id, pins) = bit_adder(4, 1);
    let mut s = Simulator::default();
    for (v, count) in [0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4]
        .into_iter()
        .enumerate()
    {
        run(&mut s, &c, &pins, &[Value::known(4, v as u32)]);
        assert_eq!(s.port_value(id, 0), Some(Value::known(3, count)));
    }
    for (v, expected) in [
        ("XXXX", "EEE"),
        ("EEEE", "EEE"),
        ("000X", "00E"),
        ("000E", "00E"),
        ("X000", "00E"),
        ("E000", "00E"),
        ("00X1", "0EE"),
        ("00E1", "0EE"),
        ("01X1", "01E"),
        ("01E1", "01E"),
        ("XX11", "EEE"),
        ("EE11", "EEE"),
        ("00XE", "0EE"),
        ("00EX", "0EE"),
        ("1X11", "EEE"),
        ("1E11", "EEE"),
    ] {
        run(&mut s, &c, &pins, &[value(v)]);
        assert_eq!(output(&s, id, 0), expected, "{v}");
    }
}

#[test]
fn multiple_bit_adder_inputs_combine_known_and_uncertain_counts() {
    let (c, id, pins) = bit_adder(4, 2);
    let mut s = Simulator::default();
    for (a, b, want) in [
        ("0000", "0000", "0000"),
        ("1111", "1111", "1000"),
        ("1111", "XXXX", "EEEE"),
        ("0000", "XXXX", "0EEE"),
        ("0011", "XXXX", "0EEE"),
        ("0111", "000X", "0EEE"),
        ("0011", "000X", "001E"),
        ("0011", "000E", "001E"),
        ("001X", "000X", "00EE"),
        ("001E", "000E", "00EE"),
        ("00XX", "00EE", "0EEE"),
        ("XXXX", "EEEE", "EEEE"),
    ] {
        run(&mut s, &c, &pins, &[value(a), value(b)]);
        assert_eq!(output(&s, id, 0), want, "{a} + {b}");
    }
}

#[test]
fn all_input_counts_and_32_word_buses_have_enough_output_bits() {
    for n in 1..=32 {
        let (c, id, pins) = bit_adder(1, n);
        let mut s = Simulator::default();
        run(&mut s, &c, &pins, &vec![Value::known(1, 1); usize::from(n)]);
        assert_eq!(s.port_value(id, 0).unwrap().number(), Some(u32::from(n)));
    }
    for (width, n, out_width, count) in [
        (3, 1, 2, 3),
        (4, 1, 3, 4),
        (8, 1, 4, 8),
        (32, 1, 6, 32),
        (32, 8, 9, 256),
        (8, 32, 9, 256),
        (32, 32, 11, 1024),
    ] {
        let (c, id, pins) = bit_adder(width, n);
        let mut s = Simulator::default();
        run(
            &mut s,
            &c,
            &pins,
            &vec![Value::known(width, u32::MAX); usize::from(n)],
        );
        assert_eq!(s.port_value(id, 0), Some(Value::known(out_width, count)));
    }
}

#[test]
fn finder_modes_and_property_changes_select_the_observed_bit() {
    let (mut c, id, pins) = fixture(Arithmetic::BitFinder, 4);
    let mut s = Simulator::default();
    for (mode, indices, missing) in [
        (
            BitSearch::LowestOne,
            [0, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0],
            0,
        ),
        (
            BitSearch::HighestOne,
            [0, 0, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3],
            0,
        ),
        (
            BitSearch::LowestZero,
            [0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0, 0],
            15,
        ),
        (
            BitSearch::HighestZero,
            [3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 1, 1, 0, 0],
            15,
        ),
    ] {
        c.components
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .arithmetic
            .search = mode;
        for (input, index) in indices.into_iter().enumerate() {
            run(&mut s, &c, &pins, &[Value::known(4, input as u32)]);
            assert_eq!(
                s.port_value(id, 0),
                Some(Value::known(2, index)),
                "{mode:?} {input}"
            );
            assert_eq!(
                s.port_value(id, 2),
                Some(Value::known(1, u32::from(input != missing)))
            );
        }
    }
}

#[test]
fn finder_stops_at_the_first_match_or_uncertain_bit_in_search_order() {
    let (mut c, id, pins) = fixture(Arithmetic::BitFinder, 4);
    let mut s = Simulator::default();
    for (input, expected) in [
        ("XXXX", ["EE/E", "EE/E", "EE/E", "EE/E"]),
        ("EEEE", ["EE/E", "EE/E", "EE/E", "EE/E"]),
        ("000X", ["EE/E", "EE/E", "EE/E", "11/1"]),
        ("000E", ["EE/E", "EE/E", "EE/E", "11/1"]),
        ("X000", ["EE/E", "EE/E", "00/1", "EE/E"]),
        ("E000", ["EE/E", "EE/E", "00/1", "EE/E"]),
        ("00X1", ["00/1", "EE/E", "EE/E", "11/1"]),
        ("00E1", ["00/1", "EE/E", "EE/E", "11/1"]),
        ("01X1", ["00/1", "10/1", "EE/E", "11/1"]),
        ("01E1", ["00/1", "10/1", "EE/E", "11/1"]),
        ("XX11", ["00/1", "EE/E", "EE/E", "EE/E"]),
        ("EE11", ["00/1", "EE/E", "EE/E", "EE/E"]),
        ("00XE", ["EE/E", "EE/E", "EE/E", "11/1"]),
        ("00EX", ["EE/E", "EE/E", "EE/E", "11/1"]),
        ("1X11", ["00/1", "11/1", "EE/E", "EE/E"]),
        ("1E11", ["00/1", "11/1", "EE/E", "EE/E"]),
    ] {
        for (mode, want) in BitSearch::ALL.into_iter().zip(expected) {
            c.components
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .arithmetic
                .search = mode;
            run(&mut s, &c, &pins, &[value(input)]);
            assert_eq!(
                format!("{}/{}", output(&s, id, 0), output(&s, id, 2)),
                want,
                "{mode:?} {input}"
            );
        }
    }
}

#[test]
fn finder_one_bit_and_non_power_of_two_and_32_bit_boundaries() {
    let (mut c, id, pins) = fixture(Arithmetic::BitFinder, 1);
    let mut s = Simulator::default();
    for mode in BitSearch::ALL {
        c.components
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .arithmetic
            .search = mode;
        for input in ["0", "1", "X", "E"] {
            run(&mut s, &c, &pins, &[value(input)]);
            let (index, found) = if matches!(input, "X" | "E") {
                ("E", "E")
            } else {
                (
                    "0",
                    if (input == "1") == mode.find_one() {
                        "1"
                    } else {
                        "0"
                    },
                )
            };
            assert_eq!(output(&s, id, 0), index);
            assert_eq!(output(&s, id, 2), found);
        }
    }
    for (width, input, mode, result, found, out_width) in [
        (3, 4, BitSearch::LowestOne, 2, 1, 2),
        (5, 16, BitSearch::LowestOne, 4, 1, 3),
        (32, 0x80000002, BitSearch::LowestOne, 1, 1, 5),
        (32, 0x80000002, BitSearch::HighestOne, 31, 1, 5),
        (32, 0x7ffffffd, BitSearch::LowestZero, 1, 1, 5),
        (32, 0x7ffffffd, BitSearch::HighestZero, 31, 1, 5),
        (32, 0, BitSearch::LowestOne, 0, 0, 5),
        (32, u32::MAX, BitSearch::LowestZero, 0, 0, 5),
        (32, 1, BitSearch::HighestOne, 0, 1, 5),
    ] {
        let (mut c, id, pins) = fixture(Arithmetic::BitFinder, width);
        c.components
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .arithmetic
            .search = mode;
        let mut s = Simulator::default();
        run(&mut s, &c, &pins, &[Value::known(width, input)]);
        assert_eq!(s.port_value(id, 0), Some(Value::known(out_width, result)));
        assert_eq!(s.port_value(id, 2), Some(Value::known(1, found)));
    }
}

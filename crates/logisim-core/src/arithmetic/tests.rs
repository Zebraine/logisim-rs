use super::*;
#[test]
fn bit_adder_geometry_and_count_edits_keep_observed_wire_positions() {
    let mut c = Component::new(Kind::Arithmetic(Arithmetic::BitAdder), Point::new(200, 200));
    for (n, first, last, top, bottom) in [
        (1, 0, 0, -20, 20),
        (2, -10, 10, -20, 20),
        (3, -10, 10, -20, 20),
        (4, -10, 20, -15, 25),
        (5, -20, 20, -25, 25),
        (8, -30, 40, -35, 45),
        (32, -150, 160, -155, 165),
    ] {
        c.arithmetic.bit_inputs = n;
        let ports = c.local_ports();
        assert_eq!(ports.len(), usize::from(n) + 1);
        assert_eq!(ports[1], Point::new(-40, first));
        assert_eq!(*ports.last().unwrap(), Point::new(-40, last));
        assert_eq!(
            c.arithmetic_body(),
            [Point::new(-40, top), Point::new(0, bottom)]
        );
        for i in 1..ports.len() {
            assert_eq!(c.port_width(i), Some(8));
            assert!(!c.drives_port(i));
        }
    }
    for (w, n, result) in [
        (1, 1, 1),
        (1, 2, 2),
        (3, 1, 2),
        (4, 1, 3),
        (8, 1, 4),
        (32, 1, 6),
        (32, 8, 9),
        (8, 32, 9),
        (32, 32, 11),
    ] {
        c.bits = w;
        c.arithmetic.bit_inputs = n;
        assert_eq!(c.port_width(0), Some(result));
    }
    c.bits = 1;
    c.arithmetic.bit_inputs = 2;
    let mut circuit = crate::Circuit::default();
    let id = circuit.add_component(c);
    for y in [190, 210] {
        circuit.add_wire(Point::new(80, y), Point::new(160, y));
    }
    let wires = circuit.wires.clone();
    for n in [4, 1, 32, 2] {
        let mut replacement = circuit.component(id).unwrap().clone();
        replacement.arithmetic.bit_inputs = n;
        circuit.replace_component(replacement);
        assert_eq!(circuit.wires, wires);
    }
}
#[test]
fn fixed_terminals_and_bus_width_boundaries() {
    for kind in Arithmetic::ALL {
        let mut c = Component::new(Kind::Arithmetic(kind), Point::new(100, 100));
        assert_eq!(c.bits, 8);
        assert!(!c.kind.rotatable());
        assert!(!c.kind.has_label());
        if matches!(kind, Arithmetic::Negator | Arithmetic::BitAdder) {
            assert_eq!(c.ports(), [Point::new(100, 100), Point::new(60, 100)]);
        } else if kind == Arithmetic::BitFinder {
            assert_eq!(
                c.ports(),
                [
                    Point::new(100, 100),
                    Point::new(60, 100),
                    Point::new(80, 120)
                ]
            );
        } else {
            assert_eq!(c.ports()[1..3], [Point::new(60, 90), Point::new(60, 110)]);
        }
        if kind == Arithmetic::Comparator {
            assert_eq!(c.ports()[3..], [Point::new(100, 90), Point::new(100, 110)]);
        } else if matches!(
            kind,
            Arithmetic::Adder
                | Arithmetic::Subtractor
                | Arithmetic::Multiplier
                | Arithmetic::Divider
        ) {
            assert_eq!(c.ports()[3..], [Point::new(80, 80), Point::new(80, 120)]);
        }
        for (width, amount) in [(1, 1), (3, 2), (4, 2), (5, 3), (8, 3), (32, 5)] {
            c.bits = width;
            assert_eq!(c.shift_amount_bits(), amount);
            for i in 0..c.ports().len() {
                let output = i == 0
                    || i == 4
                    || (i == 3 && kind == Arithmetic::Comparator)
                    || (i == 2 && kind == Arithmetic::BitFinder);
                assert_eq!(c.drives_port(i), output);
                let bits = if i == 0 && kind == Arithmetic::BitAdder {
                    match width {
                        1 => 1,
                        3 => 2,
                        4 | 5 => 3,
                        8 => 4,
                        32 => 6,
                        _ => unreachable!(),
                    }
                } else if i == 0 && kind == Arithmetic::BitFinder {
                    amount
                } else if (i == 2 && kind == Arithmetic::BitFinder)
                    || (i >= 3 && !matches!(kind, Arithmetic::Multiplier | Arithmetic::Divider))
                    || (kind == Arithmetic::Comparator && i == 0)
                {
                    1
                } else if kind == Arithmetic::Shifter && i == 2 {
                    amount
                } else {
                    width
                };
                assert_eq!(c.port_width(i), Some(bits));
            }
        }
    }
}

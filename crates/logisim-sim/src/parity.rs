use crate::{Bit, Value};
use logisim_core::{Component, Kind, Parity, ParityOutput};

fn invert(bit: Bit) -> Bit {
    match bit {
        Bit::Zero => Bit::One,
        Bit::One => Bit::Zero,
        _ => Bit::Error,
    }
}
pub(crate) fn evaluate(c: &Component, inputs: &[Option<Value>]) -> Value {
    Value::from_bits((0..c.bits.clamp(1, 32)).map(|bit| {
        let mut values = inputs.iter().enumerate().filter_map(|(i, input)| {
            input.map(|v| {
                if c.parity_negated(i as u8) {
                    invert(v.bit(bit))
                } else {
                    v.bit(bit)
                }
            })
        });
        // A single connected input is passed through; two or more use four-state XOR.
        let mut result = values.next().unwrap_or(Bit::Error);
        for next in values {
            result = match (result, next) {
                (Bit::Zero, Bit::Zero) | (Bit::One, Bit::One) => Bit::Zero,
                (Bit::Zero, Bit::One) | (Bit::One, Bit::Zero) => Bit::One,
                _ => Bit::Error,
            };
        }
        if c.kind == Kind::Parity(Parity::Even) {
            result = invert(result);
        }
        match (c.parity.output, result) {
            (ParityOutput::ZeroFloating, Bit::One) | (ParityOutput::FloatingOne, Bit::Zero) => {
                Bit::Unknown
            }
            _ => result,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Simulator;
    use logisim_core::{Circuit, Facing, Id, Point};
    fn connected(
        c: &Component,
        values: &[Option<Value>],
    ) -> (Circuit, Id, Vec<Option<Id>>, Simulator) {
        let mut circuit = Circuit::default();
        let ports = c.ports();
        let id = circuit.add_component(c.clone());
        let ids: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.map(|_| {
                    let mut pin = Component::new(Kind::Pin, ports[i + 1]);
                    pin.bits = c.bits;
                    circuit.add_component(pin)
                })
            })
            .collect();
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        for (id, value) in ids.iter().zip(values) {
            if let (Some(id), Some(v)) = (id, value) {
                sim.external_input(*id, *v);
            }
        }
        sim.advance(64);
        assert!(!sim.limited());
        assert_eq!(sim.pending_count(), 0);
        (circuit, id, ids, sim)
    }
    #[test]
    fn parity_networks_match_observed_five_by_five_four_state_matrices() {
        use Bit::{Error as E, One as H, Unknown as X, Zero as L};
        let states = [Some(L), Some(H), Some(X), Some(E), None];
        for (kind, negated, rows) in [
            (
                Parity::Odd,
                0,
                ["01EE0", "10EE1", "EEEEX", "EEEEE", "01XEE"],
            ),
            (
                Parity::Even,
                0,
                ["10EE1", "01EE0", "EEEEE", "EEEEE", "10EEE"],
            ),
            (
                Parity::Odd,
                1,
                ["10EE1", "01EE0", "EEEEE", "EEEEE", "01XEE"],
            ),
        ] {
            for mode in ParityOutput::ALL {
                for (a, va) in states.iter().enumerate() {
                    for (b, vb) in states.iter().enumerate() {
                        let mut c = Component::new(Kind::Parity(kind), Point::new(300, 300));
                        c.parity.negated = negated;
                        c.parity.output = mode;
                        let (_, id, _, sim) = connected(
                            &c,
                            &[
                                va.map(|v| Value::filled(1, v)),
                                vb.map(|v| Value::filled(1, v)),
                            ],
                        );
                        let expected = match rows[a].as_bytes()[b] {
                            b'0' => {
                                if mode == ParityOutput::FloatingOne {
                                    X
                                } else {
                                    L
                                }
                            }
                            b'1' => {
                                if mode == ParityOutput::ZeroFloating {
                                    X
                                } else {
                                    H
                                }
                            }
                            b'X' => X,
                            _ => E,
                        };
                        assert_eq!(
                            sim.port_value(id, 0),
                            Some(Value::filled(1, expected)),
                            "{kind:?} {negated} {mode:?} {a} {b}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn parity_counts_ones_instead_of_testing_exactly_one_and_supports_32_inputs() {
        for kind in Parity::ALL {
            for facing in Facing::ALL {
                for (count, bits, values, odd) in [
                    (3, 1, vec![0, 0, 0], 0),
                    (3, 1, vec![1, 0, 0], 1),
                    (3, 1, vec![1, 1, 0], 0),
                    (3, 1, vec![1, 1, 1], 1),
                    (3, 4, vec![0xa, 0xc, 0xf], 9),
                    (31, 32, vec![0x12345678; 31], 0x12345678),
                    (32, 32, vec![0x12345678; 32], 0),
                ] {
                    let mut c = Component::new(Kind::Parity(kind), Point::new(400, 400));
                    c.facing = facing;
                    c.inputs = count;
                    c.bits = bits;
                    let inputs: Vec<_> = values
                        .iter()
                        .map(|v| Some(Value::known(bits, *v)))
                        .collect();
                    let (_, id, _, sim) = connected(&c, &inputs);
                    assert_eq!(
                        sim.port_value(id, 0),
                        Some(Value::known(
                            bits,
                            if kind == Parity::Odd { odd } else { !odd }
                        ))
                    );
                }
                let mut c = Component::new(Kind::Parity(kind), Point::new(400, 400));
                c.facing = facing;
                c.inputs = 3;
                c.bits = 4;
                c.parity.negated = 1;
                let (_, id, _, sim) = connected(
                    &c,
                    &[
                        Some(Value::known(4, 10)),
                        Some(Value::known(4, 12)),
                        Some(Value::known(4, 15)),
                    ],
                );
                assert_eq!(
                    sim.port_value(id, 0),
                    Some(Value::known(4, if kind == Parity::Odd { 6 } else { 9 }))
                );
            }
        }
    }
    #[test]
    fn parity_preserves_partial_single_input_and_resynchronizes_output_edits() {
        use Bit::{Error as E, One as H, Unknown as X, Zero as L};
        let partial = Value::from_bits([L, E, X, H].into_iter());
        for (kind, negated, second, expected) in [
            (Parity::Odd, 0, None, "1XE0"),
            (Parity::Even, 0, None, "0EE1"),
            (Parity::Odd, 0, Some(Value::known(4, 0)), "1EE0"),
            (Parity::Even, 0, Some(Value::known(4, 0)), "0EE1"),
            (Parity::Odd, 1, None, "0EE1"),
            (Parity::Even, 1, None, "1EE0"),
        ] {
            let mut c = Component::new(Kind::Parity(kind), Point::new(300, 300));
            c.bits = 4;
            c.parity.negated = negated;
            let (_, id, _, sim) = connected(&c, &[Some(partial), second]);
            assert_eq!(sim.port_value(id, 0).unwrap().binary(), expected);
        }
        let c = Component::new(Kind::Parity(Parity::Odd), Point::new(300, 300));
        let (mut circuit, id, _, mut sim) =
            connected(&c, &[Some(Value::known(1, 1)), Some(Value::known(1, 0))]);
        for (mode, expected) in [
            (ParityOutput::ZeroFloating, X),
            (ParityOutput::FloatingOne, H),
            (ParityOutput::Normal, H),
        ] {
            let mut c = circuit.component(id).unwrap().clone();
            c.parity.output = mode;
            circuit.replace_component(c);
            assert!(sim.synchronize(&circuit));
            sim.advance(32);
            assert_eq!(sim.port_value(id, 0), Some(Value::filled(1, expected)));
        }
    }
}

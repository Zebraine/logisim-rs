use crate::{Bit, Value};
use logisim_core::{Component, Kind, TransistorType};

pub(crate) fn evaluate(c: &Component, input: Value, gate: Bit, n_gate: Bit) -> Value {
    let (on, off) = if c.kind == Kind::TransmissionGate {
        (
            (gate, n_gate) == (Bit::Zero, Bit::One),
            (gate, n_gate) == (Bit::One, Bit::Zero),
        )
    } else {
        let (active, inactive) = match c.transistor.transistor_type {
            TransistorType::P => (Bit::Zero, Bit::One),
            TransistorType::N => (Bit::One, Bit::Zero),
        };
        (gate == active, gate == inactive)
    };
    if on {
        input
    } else if off {
        Value::filled(c.bits, Bit::Unknown)
    } else {
        Value::from_bits((0..c.bits.clamp(1, 32)).map(|i| {
            if input.bit(i) == Bit::Unknown {
                Bit::Unknown
            } else {
                Bit::Error
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Simulator;
    use logisim_core::{Circuit, Facing, Id, Point};
    fn pin(c: &mut Circuit, p: Point, bits: u8) -> Id {
        let mut pin = Component::new(Kind::Pin, p);
        pin.bits = bits;
        c.add_component(pin)
    }
    fn run(c: &Circuit) -> Simulator {
        let mut sim = Simulator::default();
        sim.synchronize(c);
        sim.advance(64);
        sim
    }
    #[test]
    fn transistor_four_state_matrix_matches_both_types_and_all_orientations() {
        use Bit::{Error as E, One as H, Unknown as X, Zero as L};
        let states = [L, H, X, E];
        // Rows are control 0/1/X/E, columns are data 0/1/X/E.
        let p_expected = [[L, H, X, E], [X; 4], [E, E, X, E], [E, E, X, E]];
        let n_expected = [[X; 4], [L, H, X, E], [E, E, X, E], [E, E, X, E]];
        for typ in TransistorType::ALL {
            for facing in Facing::ALL {
                for right in [false, true] {
                    for bits in [1, 4, 32] {
                        let mut circuit = Circuit::default();
                        let mut c = Component::new(Kind::Transistor, Point::new(300, 300));
                        c.facing = facing;
                        c.bits = bits;
                        c.transistor.transistor_type = typ;
                        c.transistor.gate_bottom_right = right;
                        let ports = c.ports();
                        let id = circuit.add_component(c);
                        let input = pin(&mut circuit, ports[1], bits);
                        let control = pin(&mut circuit, ports[2], 1);
                        let mut sim = run(&circuit);
                        let expected = if typ == TransistorType::P {
                            p_expected
                        } else {
                            n_expected
                        };
                        for (g, gate) in states.into_iter().enumerate() {
                            sim.external_input(control, Value::filled(1, gate));
                            for (d, data) in states.into_iter().enumerate() {
                                sim.external_input(input, Value::filled(bits, data));
                                sim.advance(16);
                                assert_eq!(
                                    sim.port_value(id, 0),
                                    Some(Value::filled(bits, expected[g][d])),
                                    "{typ:?} {facing:?} {right} {bits} {g} {d}"
                                );
                                assert!(!sim.limited());
                            }
                        }
                        sim.set_input(control, u32::from(typ == TransistorType::N));
                        sim.set_input(input, 0x12345678);
                        sim.advance(16);
                        assert_eq!(sim.port_value(id, 0), Some(Value::known(bits, 0x12345678)));
                        let mut edited = circuit.component(id).unwrap().clone();
                        edited.transistor.transistor_type = if typ == TransistorType::P {
                            TransistorType::N
                        } else {
                            TransistorType::P
                        };
                        circuit.replace_component(edited);
                        assert!(sim.synchronize(&circuit));
                        sim.advance(16);
                        assert_eq!(sim.port_value(id, 0), Some(Value::filled(bits, X)));
                    }
                }
            }
        }
    }
    #[test]
    fn transmission_controls_and_partial_data_follow_the_observed_matrix() {
        use Bit::{Error as E, One as H, Unknown as X, Zero as L};
        let states = [L, H, X, E];
        // Controls indexed N then P. 0=invalid, 1=pass, 2=release.
        let modes = [[0, 2, 0, 0], [1, 0, 0, 0], [0; 4], [0; 4]];
        for facing in Facing::ALL {
            for right in [false, true] {
                for bits in [1, 4, 32] {
                    let mut circuit = Circuit::default();
                    let mut c = Component::new(Kind::TransmissionGate, Point::new(300, 300));
                    c.facing = facing;
                    c.bits = bits;
                    c.transistor.gate_bottom_right = right;
                    let ports = c.ports();
                    let id = circuit.add_component(c);
                    let input = pin(&mut circuit, ports[1], bits);
                    let p = pin(&mut circuit, ports[2], 1);
                    let n = pin(&mut circuit, ports[3], 1);
                    let mut sim = run(&circuit);
                    for (ni, nv) in states.into_iter().enumerate() {
                        for (pi, pv) in states.into_iter().enumerate() {
                            sim.external_input(p, Value::filled(1, pv));
                            sim.external_input(n, Value::filled(1, nv));
                            for data in states {
                                sim.external_input(input, Value::filled(bits, data));
                                sim.advance(16);
                                let expected = match modes[ni][pi] {
                                    1 => data,
                                    2 => X,
                                    _ if data == X => X,
                                    _ => E,
                                };
                                assert_eq!(
                                    sim.port_value(id, 0),
                                    Some(Value::filled(bits, expected))
                                );
                            }
                        }
                    }
                    sim.set_input(p, 0);
                    sim.set_input(n, 1);
                    sim.set_input(input, 0x12345678);
                    sim.advance(16);
                    assert_eq!(sim.port_value(id, 0), Some(Value::known(bits, 0x12345678)));
                }
            }
        }
        for (kind, typ) in [
            (Kind::Transistor, TransistorType::P),
            (Kind::Transistor, TransistorType::N),
            (Kind::TransmissionGate, TransistorType::P),
        ] {
            let mut circuit = Circuit::default();
            let mut c = Component::new(kind, Point::new(200, 200));
            c.bits = 4;
            c.transistor.transistor_type = typ;
            let ports = c.ports();
            let id = circuit.add_component(c);
            // Floating control and data pins stay unknown, not error.
            assert_eq!(run(&circuit).port_value(id, 0).unwrap().binary(), "XXXX");
            let input = pin(&mut circuit, ports[1], 4);
            let control = pin(&mut circuit, ports[2], 1);
            let n = (kind == Kind::TransmissionGate).then(|| pin(&mut circuit, ports[3], 1));
            let mut sim = run(&circuit);
            sim.external_input(input, Value::from_bits([L, E, X, H].into_iter()));
            let on = if typ == TransistorType::P { L } else { H };
            let off = if on == L { H } else { L };
            for (g, ng, expected) in [
                (on, H, "1XE0"),
                (off, L, "XXXX"),
                (X, L, "EXEE"),
                (E, H, "EXEE"),
            ] {
                sim.external_input(control, Value::filled(1, g));
                if let Some(n) = n {
                    sim.external_input(n, Value::filled(1, ng));
                }
                sim.advance(16);
                assert_eq!(sim.port_value(id, 0).unwrap().binary(), expected);
            }
        }
    }
    #[test]
    fn output_drivers_do_not_feed_back_to_transistor_inputs() {
        for (kind, typ) in [
            (Kind::Transistor, TransistorType::P),
            (Kind::Transistor, TransistorType::N),
            (Kind::TransmissionGate, TransistorType::P),
        ] {
            let mut circuit = Circuit::default();
            let mut c = Component::new(kind, Point::new(200, 200));
            c.transistor.transistor_type = typ;
            let ports = c.ports();
            let id = circuit.add_component(c);
            let output = pin(&mut circuit, ports[0], 1);
            let control = pin(&mut circuit, ports[2], 1);
            let n = (kind == Kind::TransmissionGate).then(|| pin(&mut circuit, ports[3], 1));
            let probe =
                circuit.add_component(Component::new(Kind::Probe, ports[1] + Point::new(-100, 0)));
            circuit.add_wire(ports[1], ports[1] + Point::new(-100, 0));
            let mut sim = run(&circuit);
            sim.set_input(output, 1);
            sim.set_input(control, u32::from(typ == TransistorType::N));
            if let Some(n) = n {
                sim.set_input(n, 1);
            }
            sim.advance(16);
            assert_eq!(sim.port_value(id, 0), Some(Value::known(1, 1)));
            assert_eq!(
                sim.port_value(probe, 0),
                Some(Value::filled(1, Bit::Unknown))
            );
            assert!(!sim.limited());
        }
    }
}

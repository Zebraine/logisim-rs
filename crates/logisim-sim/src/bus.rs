use crate::{Bit, Value};
use logisim_core::{Component, Gate, Kind, Pull};
pub(crate) fn pull_bit(pull: Pull) -> Bit {
    match pull {
        Pull::Zero => Bit::Zero,
        Pull::One => Bit::One,
        Pull::Error => Bit::Error,
    }
}
pub(crate) fn evaluate(c: &Component, input: Value, enable: Bit) -> Value {
    let active = if c.bus.negate_control {
        Bit::Zero
    } else {
        Bit::One
    };
    if matches!(enable, Bit::Unknown | Bit::Error) {
        Value::filled(c.bits, Bit::Error)
    } else if enable != active {
        Value::filled(c.bits, Bit::Unknown)
    } else if c.kind == Kind::ControlledInverter {
        crate::value::evaluate(Gate::Not, c.bits, &[input])
    } else {
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Problem, Simulator};
    use logisim_core::{Circuit, Facing, GateSize, Id, Point};
    fn pin(c: &mut Circuit, at: Point, bits: u8) -> Id {
        let mut p = Component::new(Kind::Pin, at);
        p.bits = bits;
        c.add_component(p)
    }
    fn run(c: &Circuit) -> Simulator {
        let mut s = Simulator::default();
        s.synchronize(c);
        s.advance(64);
        s
    }
    fn pull(c: &mut Circuit, at: Point, p: Pull) -> Id {
        let mut r = Component::new(Kind::PullResistor, at);
        r.bus.pull = p;
        c.add_component(r)
    }
    #[test]
    fn controlled_four_state_matrix_and_full_width_rotated_buses() {
        let states = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error];
        for kind in [Kind::ControlledBuffer, Kind::ControlledInverter] {
            for bits in [1, 4, 32] {
                for facing in Facing::ALL {
                    for negated in [false, true] {
                        let mut c = Circuit::default();
                        let mut gate = Component::new(kind, Point::new(300, 200));
                        gate.bits = bits;
                        gate.facing = facing;
                        gate.bus.negate_control = negated;
                        gate.bus.control_left = negated;
                        gate.gate_size = GateSize::Wide;
                        let ports = gate.ports();
                        let id = c.add_component(gate);
                        let data = pin(&mut c, ports[1], bits);
                        let control = pin(&mut c, ports[2], 1);
                        let mut sim = run(&c);
                        let enabled = if kind == Kind::ControlledBuffer {
                            states
                        } else {
                            [Bit::One, Bit::Zero, Bit::Error, Bit::Error]
                        };
                        for (j, enable) in states.into_iter().enumerate() {
                            for (i, input) in states.into_iter().enumerate() {
                                sim.external_input(data, Value::filled(bits, input));
                                sim.external_input(control, Value::filled(1, enable));
                                sim.advance(16);
                                let expected = if j >= 2 {
                                    Bit::Error
                                } else if (j == 1) != negated {
                                    enabled[i]
                                } else {
                                    Bit::Unknown
                                };
                                assert_eq!(
                                    sim.port_value(id, 0),
                                    Some(Value::filled(bits, expected)),
                                    "{kind:?} {bits} {facing:?} {negated} {input:?} {enable:?}"
                                );
                            }
                        }
                        sim.external_input(data, Value::known(bits, 0x12345678));
                        sim.set_input(control, u32::from(!negated));
                        sim.advance(16);
                        assert_eq!(
                            sim.port_value(id, 0),
                            Some(Value::known(
                                bits,
                                if kind == Kind::ControlledBuffer {
                                    0x12345678
                                } else {
                                    0xedcba987
                                }
                            ))
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn open_inputs_and_partial_bits_follow_control_polarity_and_reset() {
        for (kind, expected) in [
            (Kind::ControlledBuffer, "1XE0"),
            (Kind::ControlledInverter, "0EE1"),
        ] {
            let mut c = Circuit::default();
            let mut gate = Component::new(kind, Point::new(200, 100));
            gate.bits = 4;
            let ports = gate.ports();
            let id = c.add_component(gate);
            assert_eq!(run(&c).port_value(id, 0).unwrap().binary(), "EEEE");
            let enable = pin(&mut c, ports[2], 1);
            let mut sim = run(&c);
            assert_eq!(sim.port_value(id, 0).unwrap().binary(), "XXXX");
            sim.set_input(enable, 1);
            sim.advance(16);
            assert_eq!(
                sim.port_value(id, 0).unwrap().binary(),
                if kind == Kind::ControlledBuffer {
                    "XXXX"
                } else {
                    "EEEE"
                }
            );
            let data = pin(&mut c, ports[1], 4);
            sim.synchronize(&c);
            sim.external_input(
                data,
                Value::from_bits([Bit::Zero, Bit::Error, Bit::Unknown, Bit::One].into_iter()),
            );
            sim.advance(16);
            assert_eq!(sim.port_value(id, 0).unwrap().binary(), expected);
            let mut edited = c.component(id).unwrap().clone();
            edited.bus.negate_control = true;
            c.replace_component(edited);
            assert!(sim.synchronize(&c));
            sim.advance(16);
            assert_eq!(sim.port_value(id, 0).unwrap().binary(), "XXXX");
            sim.set_input(enable, 0);
            sim.advance(16);
            assert_eq!(sim.port_value(id, 0).unwrap().binary(), expected);
            sim.reset();
            sim.advance(16);
            assert_eq!(
                sim.port_value(id, 0).unwrap().binary(),
                if kind == Kind::ControlledBuffer {
                    "0000"
                } else {
                    "1111"
                }
            );
        }
    }
    #[test]
    fn weak_pulls_fill_only_undriven_bits_and_conflicts_do_not_override_drives() {
        for (pulls, floating) in [
            (vec![Pull::Zero], "10E0"),
            (vec![Pull::One], "11E0"),
            (vec![Pull::Error], "1EE0"),
            (vec![Pull::Zero, Pull::One], "1EE0"),
            (vec![Pull::Zero, Pull::Error], "1EE0"),
            (vec![Pull::One, Pull::One], "11E0"),
        ] {
            let mut c = Circuit::default();
            let at = Point::new(200, 100);
            let input = pin(&mut c, at, 4);
            for p in pulls {
                pull(&mut c, at, p);
            }
            let probe = c.add_component(Component::new(Kind::Probe, at + Point::new(100, 0)));
            c.add_wire(at, at + Point::new(100, 0));
            let mut sim = run(&c);
            sim.external_input(
                input,
                Value::from_bits([Bit::Zero, Bit::Error, Bit::Unknown, Bit::One].into_iter()),
            );
            sim.advance(16);
            assert_eq!(sim.port_value(probe, 0).unwrap().binary(), floating);
            for value in [0, 1, 15] {
                sim.set_input(input, value);
                sim.advance(16);
                assert_eq!(sim.port_value(probe, 0), Some(Value::known(4, value)));
            }
            sim.external_input(input, Value::filled(4, Bit::Error));
            sim.advance(16);
            assert_eq!(sim.port_value(probe, 0).unwrap().binary(), "EEEE");
        }
    }
    #[test]
    fn pulls_cross_splitters_and_tunnels_without_imposing_a_bus_width() {
        let mut c = Circuit::default();
        let at = Point::new(100, 100);
        let resistor = pull(&mut c, at, Pull::One);
        let probe = c.add_component(Component::new(Kind::Probe, at));
        assert_eq!(run(&c).port_value(probe, 0).unwrap().width(), 0);
        let mut splitter = Component::new(Kind::Splitter, at + Point::new(-20, 30));
        splitter.bits = 4;
        let ports = splitter.ports();
        let split = c.add_component(splitter);
        assert_eq!(ports[2], at);
        assert_eq!(run(&c).port_value(split, 0).unwrap().binary(), "XX1X");
        let common_pull = pull(&mut c, ports[0], Pull::Zero);
        let sim = run(&c);
        assert_eq!(sim.port_value(split, 0).unwrap().binary(), "00E0");
        assert_eq!(sim.port_value(resistor, 0).unwrap().binary(), "E");
        let mut edited = c.component(common_pull).unwrap().clone();
        edited.bus.pull = Pull::One;
        c.replace_component(edited);
        let mut sim = run(&c);
        assert_eq!(sim.port_value(split, 0).unwrap().binary(), "1111");
        let t1 = Component::new(Kind::Tunnel, ports[0]);
        let t2 = Component::new(Kind::Tunnel, Point::new(500, 300));
        let mut t1 = t1;
        let mut t2 = t2;
        t1.bits = 4;
        t2.bits = 4;
        c.add_component(t1);
        let tunnel = c.add_component(t2);
        sim.synchronize(&c);
        sim.advance(16);
        assert_eq!(sim.port_value(tunnel, 0).unwrap().binary(), "1111");
        // Width conflicts remain visible; a pull does not turn one-bit sources into buses.
        pin(&mut c, ports[0], 1);
        sim.synchronize(&c);
        sim.advance(16);
        assert!(sim.port_mismatch(tunnel, 0));
        assert!(
            sim.diagnostics()
                .iter()
                .any(|d| d.problem == Problem::WidthMismatch)
        );
    }
    #[test]
    fn shared_bus_release_poke_pause_reset_and_pull_attribute_edits() {
        let mut c = Circuit::default();
        let mut controls = vec![];
        let bus = Point::new(300, 100);
        for (y, value) in [(100, 0), (220, 1)] {
            let g = Component::new(Kind::ControlledBuffer, Point::new(200, y));
            let ports = g.ports();
            c.add_component(g);
            let mut data = Component::new(Kind::Constant, ports[1]);
            data.constant = value;
            c.add_component(data);
            controls.push(pin(&mut c, ports[2], 1));
            c.add_wire(ports[0], Point::new(300, y));
        }
        c.add_wire(bus, Point::new(300, 220));
        let resistor = pull(&mut c, bus, Pull::One);
        let mut sim = run(&c);
        for (a, b, expected) in [
            (0, 0, "1"),
            (1, 0, "0"),
            (0, 1, "1"),
            (1, 1, "E"),
            (0, 0, "1"),
        ] {
            sim.set_input(controls[0], a);
            sim.set_input(controls[1], b);
            sim.advance(16);
            assert_eq!(sim.port_value(resistor, 0).unwrap().binary(), expected);
            assert!(!sim.limited());
        }
        sim.set_enabled(false);
        sim.set_input(controls[0], 1);
        sim.advance(16);
        assert_eq!(sim.port_value(resistor, 0).unwrap().binary(), "1");
        while sim.step() {}
        assert_eq!(sim.port_value(resistor, 0).unwrap().binary(), "0");
        sim.set_enabled(true);
        sim.reset();
        sim.advance(16);
        assert_eq!(sim.port_value(resistor, 0).unwrap().binary(), "1");
        let mut edited = c.component(resistor).unwrap().clone();
        edited.bus.pull = Pull::Zero;
        c.replace_component(edited);
        assert!(sim.synchronize(&c));
        sim.advance(16);
        assert_eq!(sim.port_value(resistor, 0).unwrap().binary(), "0");
    }
}

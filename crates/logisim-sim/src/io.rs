use crate::{Bit, Simulator};
use logisim_core::{Component, Id, Kind};
impl Simulator {
    pub fn set_button(&mut self, id: Id, pressed: bool) {
        if self
            .circuit
            .component(id)
            .is_some_and(|c| c.kind == Kind::Button)
        {
            self.set_input(id, u32::from(pressed));
        }
    }
    pub fn release_buttons(&mut self) {
        let ids: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Button)
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.set_button(id, false);
        }
    }
    /// Segment order: middle, upper-left, top, upper-right, lower-left, bottom, lower-right, dot.
    pub fn indicator(&self, id: Id) -> Option<[bool; 8]> {
        let c = self.circuit.component(id)?;
        let ports = (0..c.ports().len())
            .map(|i| self.port_value(id, i))
            .collect::<Option<Vec<_>>>()?;
        match c.kind {
            Kind::Led | Kind::SevenSegment | Kind::HexDisplay => Some(evaluate(c, &ports)),
            _ => None,
        }
    }
}
fn evaluate(c: &Component, ports: &[crate::Value]) -> [bool; 8] {
    let mut lights = [false; 8];
    match c.kind {
        Kind::Led => {
            lights[0] = ports[0].bit(0)
                == if c.io.active_high {
                    Bit::One
                } else {
                    Bit::Zero
                }
        }
        Kind::SevenSegment => {
            for (light, value) in lights.iter_mut().zip(ports) {
                *light = (value.bit(0) == Bit::One) == c.io.active_high;
            }
        }
        Kind::HexDisplay => {
            let mask = ports[0].number().filter(|v| *v < 16).map_or(1, |v| {
                [
                    0x7e, 0x48, 0x3d, 0x6d, 0x4b, 0x67, 0x77, 0x4c, 0x7f, 0x4f, 0x5f, 0x73, 0x36,
                    0x79, 0x37, 0x17,
                ][v as usize]
            });
            for (i, light) in lights.iter_mut().enumerate().take(7) {
                *light = mask & (1 << i) != 0;
            }
            lights[7] = ports[1].bit(0) == Bit::One;
        }
        _ => {}
    }
    lights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectSimulator, Value};
    use logisim_core::{Circuit, Point, Project};
    #[test]
    fn indicator_four_states_and_each_segment_follow_observed_polarities() {
        for kind in [Kind::Led, Kind::SevenSegment] {
            for high in [true, false] {
                let mut c = Component::new(kind, Point::ZERO);
                c.io.active_high = high;
                for bit in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
                    let p = vec![Value::filled(1, bit); c.ports().len()];
                    let expected = if kind == Kind::Led {
                        bit == if high { Bit::One } else { Bit::Zero }
                    } else {
                        (bit == Bit::One) == high
                    };
                    assert!(
                        evaluate(&c, &p)[..c.ports().len()]
                            .iter()
                            .all(|on| *on == expected)
                    );
                }
                if kind == Kind::SevenSegment {
                    for index in 0..8 {
                        let mut p = vec![Value::known(1, u32::from(!high)); 8];
                        p[index] = Value::known(1, u32::from(high));
                        assert_eq!(evaluate(&c, &p), std::array::from_fn(|i| i == index));
                    }
                }
            }
        }
    }
    #[test]
    fn all_hex_glyphs_unknown_data_and_independent_decimal_point() {
        let c = Component::new(Kind::HexDisplay, Point::ZERO);
        // Lit segment names transcribed from the sixteen runtime displays, in a..g order.
        let names = [
            "abcdef", "bc", "abdeg", "abcdg", "bcfg", "acdfg", "acdefg", "abc", "abcdefg", "abcfg",
            "abcefg", "cdefg", "adef", "bcdeg", "adefg", "aefg",
        ];
        let roles = ['g', 'f', 'a', 'b', 'e', 'd', 'c'];
        for (n, names) in names.iter().enumerate() {
            for bit in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
                let output = evaluate(&c, &[Value::known(4, n as u32), Value::filled(1, bit)]);
                for (i, name) in roles.iter().enumerate() {
                    assert_eq!(
                        output[i],
                        names.contains(*name),
                        "digit {n}, segment {name}"
                    );
                }
                assert_eq!(output[7], bit == Bit::One);
            }
        }
        for bit in [Bit::Unknown, Bit::Error] {
            for value in [
                Value::filled(4, bit),
                Value::from_bits([Bit::One, Bit::Zero, Bit::One, bit].into_iter()),
            ] {
                assert_eq!(
                    evaluate(&c, &[value, Value::known(1, 1)]),
                    [true, false, false, false, false, false, false, true]
                );
            }
        }
    }
    fn settle(sim: &mut Simulator) {
        sim.advance(64);
        assert_eq!(sim.pending_count(), 0);
        assert!(!sim.limited());
    }
    #[test]
    fn button_press_release_pause_reset_and_display_edits_do_not_latch() {
        let mut circuit = Circuit::default();
        let button = circuit.add_component(Component::new(Kind::Button, Point::new(100, 100)));
        let led = circuit.add_component(Component::new(Kind::Led, Point::new(200, 100)));
        circuit.add_wire(Point::new(100, 100), Point::new(200, 100));
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert!(!sim.indicator(led).unwrap()[0]);
        sim.set_button(button, true);
        settle(&mut sim);
        assert!(sim.indicator(led).unwrap()[0]);
        sim.reset();
        settle(&mut sim);
        assert_eq!(sim.input(button).unwrap().number(), Some(0));
        assert!(!sim.indicator(led).unwrap()[0]);
        sim.set_button(button, true);
        settle(&mut sim);
        let c = circuit.components.iter_mut().find(|c| c.id == led).unwrap();
        c.io.active_high = false;
        c.io.on_color = [0, 255, 0, 128];
        assert!(!sim.synchronize(&circuit));
        assert!(!sim.indicator(led).unwrap()[0]);
        sim.set_enabled(false);
        sim.set_button(button, false);
        sim.advance(64);
        assert_eq!(sim.input(button).unwrap().number(), Some(0));
        assert_eq!(sim.port_value(button, 0).unwrap().number(), Some(1));
        sim.step();
        assert_eq!(sim.port_value(button, 0).unwrap().number(), Some(0));
        assert!(sim.indicator(led).unwrap()[0]);
        sim.set_enabled(true);
        sim.set_button(button, true);
        settle(&mut sim);
        sim.release_buttons();
        settle(&mut sim);
        assert_eq!(sim.port_value(button, 0).unwrap().number(), Some(0));
    }
    #[test]
    fn buttons_and_indicators_are_instance_local_and_release_across_cached_roots() {
        let mut project = Project::default();
        let root = project.main;
        let cell = project.add("cell".into()).unwrap();
        let c = &mut project.circuit_mut(cell).unwrap().circuit;
        let button = c.add_component(Component::new(Kind::Button, Point::new(100, 100)));
        let led = c.add_component(Component::new(Kind::Led, Point::new(200, 100)));
        c.add_wire(Point::new(100, 100), Point::new(200, 100));
        let mut output = Component::new(Kind::Pin, Point::new(200, 100));
        output.output = true;
        output.facing = logisim_core::Facing::West;
        c.add_component(output);
        let mut ids = vec![];
        let mut parent_leds = vec![];
        for x in [100, 300] {
            let instance = project.instance(cell, Point::new(x, 300)).unwrap();
            let port = instance.ports()[0];
            parent_leds.push(
                project
                    .circuit_mut(root)
                    .unwrap()
                    .circuit
                    .add_component(Component::new(Kind::Led, port)),
            );
            ids.push(
                project
                    .circuit_mut(root)
                    .unwrap()
                    .circuit
                    .add_component(instance),
            );
        }
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&project, root);
        sim.advance(64);
        sim.set_button(&[ids[0]], button, true);
        sim.advance(64);
        assert!(sim.view(&[ids[0]]).unwrap().indicator(led).unwrap()[0]);
        assert!(!sim.view(&[ids[1]]).unwrap().indicator(led).unwrap()[0]);
        assert!(sim.view(&[]).unwrap().indicator(parent_leds[0]).unwrap()[0]);
        assert!(!sim.view(&[]).unwrap().indicator(parent_leds[1]).unwrap()[0]);
        sim.synchronize(&project, cell);
        sim.advance(64);
        assert!(!sim.view(&[]).unwrap().indicator(led).unwrap()[0]);
        sim.release_buttons();
        sim.synchronize(&project, root);
        sim.advance(64);
        assert!(!sim.view(&[ids[0]]).unwrap().indicator(led).unwrap()[0]);
        assert!(!sim.view(&[]).unwrap().indicator(parent_leds[0]).unwrap()[0]);
    }
}

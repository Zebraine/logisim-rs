use crate::{Bit, Simulator, Value};
use logisim_core::{Component, Id, Kind};
pub(crate) fn control_bits(c: &Component) -> Option<u8> {
    match c.kind {
        Kind::Pin if !c.output => Some(c.bits.clamp(1, 32)),
        Kind::Button | Kind::Switch => Some(1),
        Kind::DipSwitch => Some(c.io.switch_count.clamp(1, 32)),
        _ => None,
    }
}
impl Simulator {
    pub(crate) fn schedule_controls(&mut self) {
        let outputs: Vec<_> = self
            .circuit
            .components
            .iter()
            .flat_map(|c| {
                let state = self.inputs.get(&c.id).and_then(|v| v.number()).unwrap_or(0);
                match c.kind {
                    Kind::Switch => vec![(
                        (c.id, 0),
                        if state != 0 {
                            self.port_value(c.id, 1).expect("compiled switch input")
                        } else {
                            Value::filled(c.bits, Bit::Unknown)
                        },
                    )],
                    Kind::DipSwitch => (0..c.io.switch_count.clamp(1, 32))
                        .map(|i| ((c.id, usize::from(i)), Value::known(1, (state >> i) & 1)))
                        .collect(),
                    Kind::Slider => vec![(
                        (c.id, 0),
                        Value::known(c.bits, c.io.slider_value.min(c.slider_max())),
                    )],
                    _ => vec![],
                }
            })
            .collect();
        for (port, value) in outputs {
            self.drive_port(port, value);
        }
    }
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

#[cfg(test)]
mod input_tests {
    use super::*;
    use crate::ProjectSimulator;
    use logisim_core::{Circuit, Point, Project};
    fn settle(s: &mut Simulator) {
        s.advance(128);
        assert_eq!(s.pending_count(), 0);
        assert!(!s.limited());
    }
    #[test]
    fn switch_transmits_four_states_only_forward_with_pause_reset_and_width_edits() {
        for width in [1, 4, 32] {
            for bit in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
                for reverse in [false, true] {
                    let mut circuit = Circuit::default();
                    let mut c = Component::new(Kind::Switch, Point::new(200, 100));
                    c.bits = width;
                    let id = circuit.add_component(c);
                    let mut pin =
                        Component::new(Kind::Pin, Point::new(if reverse { 200 } else { 180 }, 100));
                    pin.bits = width;
                    let source_id = circuit.add_component(pin);
                    let mut sim = Simulator::default();
                    sim.synchronize(&circuit);
                    settle(&mut sim);
                    let source = Value::from_bits((0..width).map(|i| {
                        if i == width - 1 {
                            bit
                        } else if i % 2 == 0 {
                            Bit::One
                        } else {
                            Bit::Zero
                        }
                    }));
                    let driven = if reverse { 0 } else { 1 };
                    sim.external_input(source_id, source);
                    settle(&mut sim);
                    if !reverse {
                        assert_eq!(
                            sim.port_value(id, 0),
                            Some(Value::filled(width, Bit::Unknown))
                        );
                    }
                    sim.set_enabled(false);
                    sim.toggle_input(id, 0);
                    assert_eq!(sim.input(id).unwrap().number(), Some(1));
                    sim.set_enabled(true);
                    settle(&mut sim);
                    assert_eq!(
                        sim.port_value(id, 1 - driven),
                        Some(if reverse {
                            Value::filled(width, Bit::Unknown)
                        } else {
                            source
                        })
                    );
                    circuit.components[0].bits = if width == 32 { 4 } else { 32 };
                    sim.synchronize(&circuit);
                    settle(&mut sim);
                    assert_eq!(sim.input(id), Some(Value::known(1, 1)));
                    sim.reset();
                    settle(&mut sim);
                    assert_eq!(sim.input(id), Some(Value::known(1, 0)));
                    assert_eq!(
                        sim.port_value(id, 0),
                        Some(Value::filled(circuit.components[0].bits, Bit::Unknown))
                    );
                }
            }
        }
    }
    #[test]
    fn dip_outputs_are_separate_and_resize_truncates_then_zero_extends() {
        let mut circuit = Circuit::default();
        let mut c = Component::new(Kind::DipSwitch, Point::ZERO);
        c.io.switch_count = 32;
        let id = circuit.add_component(c);
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        settle(&mut sim);
        sim.toggle_input(id, 0);
        sim.toggle_input(id, 31);
        settle(&mut sim);
        for i in 0..32 {
            assert_eq!(
                sim.port_value(id, i),
                Some(Value::known(1, u32::from(i == 0 || i == 31)))
            );
        }
        circuit.components[0].io.on_color = [1, 2, 3, 128];
        assert!(!sim.synchronize(&circuit));
        circuit.components[0].io.switch_count = 1;
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert_eq!(sim.input(id), Some(Value::known(1, 1)));
        circuit.components[0].io.switch_count = 32;
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert_eq!(sim.input(id), Some(Value::known(32, 1)));
        assert_eq!(sim.port_value(id, 31), Some(Value::known(1, 0)));
        sim.set_enabled(false);
        sim.toggle_input(id, 31);
        sim.advance(64);
        assert_eq!(sim.port_value(id, 31), Some(Value::known(1, 0)));
        sim.step();
        assert_eq!(sim.port_value(id, 31), Some(Value::known(1, 1)));
        sim.reset();
        sim.set_enabled(true);
        settle(&mut sim);
        assert_eq!(sim.input(id), Some(Value::known(32, 0)));
    }
    #[test]
    fn slider_is_document_state_shared_across_instances_switches_are_independent() {
        let mut p = Project::default();
        let root = p.main;
        let cell = p.add("Cell".into()).unwrap();
        let c = &mut p.circuit_mut(cell).unwrap().circuit;
        let sw = c.add_component(Component::new(Kind::Switch, Point::new(200, 100)));
        let dip = c.add_component(Component::new(Kind::DipSwitch, Point::new(200, 200)));
        let mut slider = Component::new(Kind::Slider, Point::new(200, 400));
        slider.bits = 32;
        slider.io.slider_value = 0x80000000;
        let sl = c.add_component(slider);
        let mut ids = vec![];
        for x in [100, 300] {
            let instance = p.instance(cell, Point::new(x, 100)).unwrap();
            ids.push(p.circuit_mut(root).unwrap().circuit.add_component(instance));
        }
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, root);
        sim.advance(128);
        sim.toggle_input_at(&[ids[0]], sw, 0);
        sim.toggle_input_at(&[ids[1]], dip, 3);
        sim.advance(128);
        for (i, id) in ids.iter().enumerate() {
            let s = sim.view(&[*id]).unwrap();
            assert_eq!(s.input(sw).unwrap().number(), Some(u32::from(i == 0)));
            assert_eq!(
                s.input(dip).unwrap().number(),
                Some(if i == 1 { 8 } else { 0 })
            );
            assert_eq!(s.port_value(sl, 0).unwrap().number(), Some(0x80000000));
            assert!(s.input(sl).is_none());
        }
        p.circuit_mut(cell)
            .unwrap()
            .circuit
            .components
            .iter_mut()
            .find(|c| c.id == sl)
            .unwrap()
            .io
            .slider_value = u32::MAX;
        sim.synchronize(&p, root);
        sim.advance(128);
        for id in &ids {
            assert_eq!(
                sim.view(&[*id])
                    .unwrap()
                    .port_value(sl, 0)
                    .unwrap()
                    .number(),
                Some(u32::MAX)
            );
        }
        assert_eq!(
            sim.view(&[ids[0]]).unwrap().input(sw).unwrap().number(),
            Some(1)
        );
        sim.reset();
        sim.advance(128);
        for id in ids {
            let s = sim.view(&[id]).unwrap();
            assert_eq!(s.input(sw).unwrap().number(), Some(0));
            assert_eq!(s.input(dip).unwrap().number(), Some(0));
            assert_eq!(s.port_value(sl, 0).unwrap().number(), Some(u32::MAX));
        }
    }
}

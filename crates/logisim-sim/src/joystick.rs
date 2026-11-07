//! Momentary, instance-local two-axis input.
use crate::Simulator;
use logisim_core::{Id, Point};

// Measured outputs at each integer displacement -14..=14. Keep the observed
// quantization, including the skipped codes around the five-bit midpoint.
const AXES: [[u8; 29]; 4] = [
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    ],
    [
        1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7,
    ],
    [
        1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14,
        14, 15,
    ],
    [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
        28, 29, 30, 31,
    ],
];
pub(crate) fn axis_value(bits: u8, displacement: i32) -> u32 {
    u32::from(AXES[usize::from(bits.clamp(2, 5) - 2)][(displacement.clamp(-14, 14) + 14) as usize])
}

impl Simulator {
    pub fn joystick(&self, id: Id) -> Option<Point> {
        self.joysticks.get(&id).copied()
    }
    pub fn set_joystick(&mut self, id: Id, offset: Point) {
        let Some(current) = self.joysticks.get_mut(&id) else {
            return;
        };
        let offset = Point::new(offset.x.clamp(-14, 14), offset.y.clamp(-14, 14));
        if *current == offset {
            return;
        }
        *current = offset;
        self.limited = false;
        self.progress.restart();
        self.schedule_controls();
    }
    pub fn release_joysticks(&mut self) {
        let ids: Vec<_> = self.joysticks.keys().copied().collect();
        for id in ids {
            self.set_joystick(id, Point::ZERO);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectSimulator, Value};
    use logisim_core::{Circuit, Component, Facing, Kind, Project};
    fn settle(s: &mut Simulator) {
        s.advance(128);
        assert_eq!(s.pending_count(), 0);
        assert!(!s.limited());
    }
    #[test]
    fn two_axes_drive_observed_codes_and_clamp_independently_in_every_facing() {
        for bits in 2..=5 {
            for facing in Facing::ALL {
                let mut circuit = Circuit::default();
                let mut c = Component::new(Kind::Joystick, Point::new(200, 200));
                c.bits = bits;
                c.facing = facing;
                let ports = c.ports();
                let id = circuit.add_component(c);
                let probes: Vec<_> = ports
                    .iter()
                    .map(|p| circuit.add_component(Component::new(Kind::Probe, *p)))
                    .collect();
                let mut s = Simulator::default();
                s.synchronize(&circuit);
                settle(&mut s);
                for p in &probes {
                    assert_eq!(
                        s.port_value(*p, 0),
                        Some(Value::known(bits, 1 << (bits - 1)))
                    );
                }
                for (offset, expected) in [
                    (Point::new(-100, 100), [1, (1 << bits) - 1]),
                    (Point::new(100, -100), [(1 << bits) - 1, 1]),
                    (
                        Point::new(-1, 1),
                        match bits {
                            2 => [2, 2],
                            3 => [4, 4],
                            4 => [7, 8],
                            _ => [14, 18],
                        },
                    ),
                    (
                        Point::new(-10, 10),
                        match bits {
                            2 => [1, 3],
                            3 => [1, 6],
                            4 => [3, 13],
                            _ => [5, 27],
                        },
                    ),
                ] {
                    s.set_joystick(id, offset);
                    settle(&mut s);
                    for (p, v) in probes.iter().zip(expected) {
                        assert_eq!(s.port_value(*p, 0), Some(Value::known(bits, v)));
                    }
                }
                s.release_joysticks();
                settle(&mut s);
                assert_eq!(s.joystick(id), Some(Point::ZERO));
                assert_eq!(
                    s.port_value(id, 0),
                    Some(Value::known(bits, 1 << (bits - 1)))
                );
                assert!(s.input(id).is_none());
            }
        }
    }
    #[test]
    fn pause_reset_recompile_and_deleted_controls_do_not_leak_state() {
        let mut c = Circuit::default();
        let id = c.add_component(Component::new(Kind::Joystick, Point::ZERO));
        let mut s = Simulator::default();
        s.synchronize(&c);
        settle(&mut s);
        s.set_joystick(id, Point::new(14, -14));
        settle(&mut s);
        s.set_enabled(false);
        s.release_joysticks();
        s.advance(128);
        assert_eq!(s.joystick(id), Some(Point::ZERO));
        assert_eq!(s.port_value(id, 0), Some(Value::known(4, 15)));
        s.step();
        assert_eq!(s.port_value(id, 0), Some(Value::known(4, 8)));
        s.set_enabled(true);
        s.set_joystick(id, Point::new(-14, 14));
        settle(&mut s);
        c.components[0].io.on_color = [1, 2, 3, 128];
        assert!(!s.synchronize(&c));
        assert_eq!(s.joystick(id), Some(Point::new(-14, 14)));
        c.components[0].bits = 5;
        s.synchronize(&c);
        settle(&mut s);
        assert_eq!(s.port_value(id, 1), Some(Value::known(5, 31)));
        s.reset();
        settle(&mut s);
        assert_eq!(s.port_value(id, 1), Some(Value::known(5, 16)));
        c.components.clear();
        s.synchronize(&c);
        s.set_joystick(id, Point::new(14, 14));
        assert!(s.joystick(id).is_none());
    }
    #[test]
    fn child_axes_propagate_independently_and_release_in_cached_roots() {
        let mut p = Project::default();
        let root = p.main;
        let child = p.add("stick".into()).unwrap();
        let c = &mut p.circuit_mut(child).unwrap().circuit;
        let joystick = c.add_component(Component::new(Kind::Joystick, Point::new(200, 200)));
        for pos in [Point::new(200, 200), Point::new(200, 210)] {
            let mut pin = Component::new(Kind::Pin, pos);
            pin.bits = 4;
            pin.output = true;
            c.add_component(pin);
        }
        let mut ids = vec![];
        for x in [100, 300] {
            let instance = p.instance(child, Point::new(x, 100)).unwrap();
            ids.push(p.circuit_mut(root).unwrap().circuit.add_component(instance));
        }
        let mut s = ProjectSimulator::default();
        s.synchronize(&p, root);
        s.advance(128);
        s.set_joystick(&[ids[0]], joystick, Point::new(14, -14));
        s.advance(128);
        for (id, expected) in [(ids[0], [15, 1]), (ids[1], [8, 8])] {
            for (i, v) in expected.into_iter().enumerate() {
                assert_eq!(
                    s.view(&[id]).unwrap().port_value(joystick, i),
                    Some(Value::known(4, v))
                );
                assert_eq!(
                    s.view(&[]).unwrap().port_value(id, i),
                    Some(Value::known(4, v))
                );
            }
        }
        s.synchronize(&p, child);
        s.advance(128);
        s.release_joysticks();
        s.synchronize(&p, root);
        s.advance(128);
        for id in ids {
            assert_eq!(s.view(&[id]).unwrap().joystick(joystick), Some(Point::ZERO));
        }
        s.set_joystick(&[], joystick, Point::new(14, 14)); // Not a control in this circuit.
        assert_eq!(s.pending_count(), 0);
    }
}

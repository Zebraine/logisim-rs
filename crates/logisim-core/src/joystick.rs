//! Joystick geometry uses world axes independently of its facing.
use crate::{Component, Facing, Point};

impl Component {
    pub fn joystick_center(&self) -> Point {
        self.position
            + match self.facing {
                Facing::East => Point::new(-15, 5),
                Facing::West => Point::new(15, 5),
                Facing::South => Point::new(-5, 15),
                Facing::North => Point::new(-5, -15),
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circuit, Kind};
    #[test]
    fn joystick_world_axes_ports_and_attribute_edits_keep_wires() {
        for (facing, center, y) in [
            (Facing::East, (-15, 5), (0, 10)),
            (Facing::South, (-5, 15), (-10, 0)),
            (Facing::West, (15, 5), (0, 10)),
            (Facing::North, (-5, -15), (-10, 0)),
        ] {
            let mut c = Component::new(Kind::Joystick, Point::new(200, 200));
            c.facing = facing;
            assert_eq!(
                c.joystick_center(),
                Point::new(200 + center.0, 200 + center.1)
            );
            assert_eq!(
                c.ports(),
                [Point::new(200, 200), Point::new(200 + y.0, 200 + y.1)]
            );
            assert_eq!(c.io.on_color, [255, 0, 0, 255]);
            assert_eq!(c.color, [0; 3]);
            assert!(!c.kind.has_label());
            let body = c.io_body().map(|p| c.position + c.facing.transform(p));
            assert_eq!(
                body,
                [
                    c.joystick_center() - Point::new(15, 15),
                    c.joystick_center() + Point::new(15, 15)
                ]
            );
            for i in 0..2 {
                assert!(c.drives_port(i));
                assert_eq!(c.port_width(i), Some(4));
            }
            assert!(!c.drives_port(2));
            assert_eq!(c.port_width(2), None);
            let mut circuit = Circuit::default();
            let id = circuit.add_component(c);
            for port in circuit.component(id).unwrap().ports() {
                circuit.add_wire(port, port + Point::new(100, 0));
            }
            let wires = circuit.wires.clone();
            let mut next = circuit.component(id).unwrap().clone();
            next.facing = facing.next();
            next.bits = 5;
            circuit.replace_component(next);
            assert_eq!(circuit.wires, wires);
            assert!(
                circuit
                    .component(id)
                    .unwrap()
                    .ports()
                    .iter()
                    .enumerate()
                    .all(|(i, _)| circuit.component(id).unwrap().port_width(i) == Some(5))
            );
        }
    }
}

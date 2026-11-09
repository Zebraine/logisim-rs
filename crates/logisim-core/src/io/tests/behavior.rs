use super::super::*;
use crate::{Circuit, Facing};
#[test]
fn measured_ports_widths_and_rotations_preserve_connections() {
    let seven = Component::new(Kind::SevenSegment, Point::new(200, 100));
    assert_eq!(
        seven.ports(),
        vec![
            Point::new(200, 100),
            Point::new(210, 100),
            Point::new(220, 100),
            Point::new(230, 100),
            Point::new(200, 160),
            Point::new(210, 160),
            Point::new(220, 160),
            Point::new(230, 160)
        ]
    );
    for i in 0..8 {
        assert_eq!(seven.port_width(i), Some(1));
        assert!(!seven.drives_port(i));
    }
    let hex = Component::new(Kind::HexDisplay, Point::ZERO);
    assert_eq!(hex.ports(), vec![Point::ZERO, Point::new(10, 0)]);
    assert_eq!((hex.port_width(0), hex.port_width(1)), (Some(4), Some(1)));
    assert!(!hex.kind.rotatable());
    assert!(!seven.kind.rotatable());
    assert_eq!(Component::new(Kind::Led, Point::ZERO).facing, Facing::West);
    for kind in [Kind::Button, Kind::Led] {
        for facing in Facing::ALL {
            let mut c = Component::new(kind, Point::new(100, 100));
            c.facing = facing;
            assert_eq!(c.ports(), vec![Point::new(100, 100)]);
            assert_eq!(c.port_width(0), Some(1));
            assert_eq!(c.drives_port(0), kind == Kind::Button);
            let mut circuit = Circuit::default();
            let id = circuit.add_component(c);
            circuit.add_wire(Point::new(100, 100), Point::new(200, 100));
            let mut next = circuit.component(id).unwrap().clone();
            next.facing = facing.next();
            circuit.replace_component(next);
            assert_eq!(circuit.wires[0].a, Point::new(100, 100));
            let mut moved = circuit.component(id).unwrap().clone();
            moved.position = Point::new(100, 150);
            circuit.replace_component(moved);
            assert!(
                circuit
                    .wires
                    .iter()
                    .any(|w| w.a == Point::new(100, 150) || w.b == Point::new(100, 150))
            );
        }
    }
}

//! Input/output geometry and appearance; pressed and illuminated states live in the simulator.
use crate::{Component, Kind, Point};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Io {
    pub on_color: [u8; 4],
    pub off_color: [u8; 4],
    pub background: [u8; 4],
    pub active_high: bool,
}
impl Default for Io {
    fn default() -> Self {
        Self {
            on_color: [240, 0, 0, 255],
            off_color: [220, 220, 220, 255],
            background: [255; 4],
            active_high: true,
        }
    }
}
impl Io {
    pub fn for_kind(kind: Kind) -> Self {
        let mut io = Self::default();
        if kind == Kind::Button {
            io.on_color = [255; 4];
        }
        if kind == Kind::Led {
            io.off_color = [64, 64, 64, 255];
        }
        io
    }
}
impl Kind {
    pub fn is_io(self) -> bool {
        matches!(
            self,
            Self::Button | Self::Led | Self::SevenSegment | Self::HexDisplay
        )
    }
}
impl Component {
    pub fn io_body(&self) -> [Point; 2] {
        match self.kind {
            Kind::SevenSegment => [Point::new(-5, 0), Point::new(35, 60)],
            Kind::HexDisplay => [Point::new(-15, -60), Point::new(25, 0)],
            _ => [Point::new(-20, -10), Point::new(0, 10)],
        }
    }
    pub fn io_ports(&self) -> Vec<Point> {
        match self.kind {
            Kind::SevenSegment => (0..8)
                .map(|i| Point::new((i % 4) * 10, (i / 4) * 60))
                .collect(),
            Kind::HexDisplay => vec![Point::ZERO, Point::new(10, 0)],
            Kind::Button | Kind::Led => vec![Point::ZERO],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}

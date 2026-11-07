//! DIP geometry and pin assignments observed in the reference application.
use crate::{Component, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ttl {
    Ic7400,
    Ic7402,
    Ic7404,
    Ic7408,
    Ic7432,
    Ic7486,
    Ic747266,
}
impl Ttl {
    pub const ALL: [Self; 7] = [
        Self::Ic7400,
        Self::Ic7402,
        Self::Ic7404,
        Self::Ic7408,
        Self::Ic7432,
        Self::Ic7486,
        Self::Ic747266,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ic7400 => "7400",
            Self::Ic7402 => "7402",
            Self::Ic7404 => "7404",
            Self::Ic7408 => "7408",
            Self::Ic7432 => "7432",
            Self::Ic7486 => "7486",
            Self::Ic747266 => "747266",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Ic7400 => "Quad 2-input NAND",
            Self::Ic7402 => "Quad 2-input NOR",
            Self::Ic7404 => "Hex inverter",
            Self::Ic7408 => "Quad 2-input AND",
            Self::Ic7432 => "Quad 2-input OR",
            Self::Ic7486 => "Quad 2-input XOR",
            Self::Ic747266 => "Quad 2-input XNOR",
        }
    }
    /// Physical pin numbers [input A, input B, output]. Inverters repeat their input.
    pub fn gates(self) -> &'static [[u8; 3]] {
        match self {
            Self::Ic7402 => &[[2, 3, 1], [5, 6, 4], [9, 8, 10], [12, 11, 13]],
            Self::Ic7404 => &[
                [1, 1, 2],
                [3, 3, 4],
                [5, 5, 6],
                [9, 9, 8],
                [11, 11, 10],
                [13, 13, 12],
            ],
            _ => &[[1, 2, 3], [4, 5, 6], [10, 9, 8], [13, 12, 11]],
        }
    }
    pub fn output(self, pin: u8) -> bool {
        self.gates().iter().any(|g| g[2] == pin)
    }
    pub fn pin_position(pin: u8) -> Point {
        assert!((1..=14).contains(&pin));
        if pin <= 7 {
            Point::new(10 + 20 * i32::from(pin - 1), 30)
        } else {
            Point::new(10 + 20 * i32::from(14 - pin), -30)
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TtlOptions {
    pub power_pins: bool,
    pub internal: bool,
}
impl Component {
    /// Signal indices stay stable when the two supply terminals are appended.
    pub fn ttl_pins(&self) -> &'static [u8] {
        if self.ttl.power_pins {
            &[1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 7, 14]
        } else {
            &[1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13]
        }
    }
    pub fn ttl_index(&self, pin: u8) -> Option<usize> {
        self.ttl_pins().iter().position(|p| *p == pin)
    }
    pub fn ttl_output(&self, index: usize) -> bool {
        matches!(self.kind, Kind::Ttl(t) if self.ttl_pins().get(index).is_some_and(|p| t.output(*p)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circuit, Facing};
    #[test]
    fn dip_pin_geometry_roles_and_attribute_edits_preserve_wires() {
        let physical = [
            (10, 30),
            (30, 30),
            (50, 30),
            (70, 30),
            (90, 30),
            (110, 30),
            (130, 30),
            (130, -30),
            (110, -30),
            (90, -30),
            (70, -30),
            (50, -30),
            (30, -30),
            (10, -30),
        ];
        for kind in Ttl::ALL {
            for facing in Facing::ALL {
                for power in [false, true] {
                    let mut c = Component::new(Kind::Ttl(kind), Point::new(300, 300));
                    c.facing = facing;
                    c.ttl.power_pins = power;
                    assert_eq!(c.ports().len(), if power { 14 } else { 12 });
                    let outputs: &[u8] = match kind {
                        Ttl::Ic7402 => &[1, 4, 10, 13],
                        Ttl::Ic7404 => &[2, 4, 6, 8, 10, 12],
                        _ => &[3, 6, 8, 11],
                    };
                    for n in 1..=14 {
                        let index = c.ttl_index(n);
                        if !power && matches!(n, 7 | 14) {
                            assert!(index.is_none());
                            continue;
                        }
                        let i = index.unwrap();
                        let (x, y) = physical[usize::from(n - 1)];
                        assert_eq!(
                            c.ports()[i],
                            c.position + facing.transform(Point::new(x, y))
                        );
                        assert_eq!(c.drives_port(i), outputs.contains(&n));
                        assert_eq!(c.port_width(i), Some(1));
                    }
                    assert_eq!(c.port_width(c.ports().len()), None);
                    assert!(!c.drives_port(c.ports().len()));
                    let mut circuit = Circuit::default();
                    let id = circuit.add_component(c.clone());
                    for point in c.ports() {
                        circuit.add_wire(
                            point,
                            point
                                + facing
                                    .transform(Point::new(0, if point.y > 300 { 40 } else { -40 })),
                        );
                    }
                    let wires = circuit.wires.clone();
                    c.id = id;
                    c.facing = facing.next();
                    c.ttl.power_pins = !power;
                    c.ttl.internal = true;
                    circuit.replace_component(c);
                    assert_eq!(circuit.wires, wires);
                }
            }
        }
    }
}

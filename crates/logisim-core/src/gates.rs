use crate::Component;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Not,
    Buffer,
    And,
    Or,
    Nand,
    Nor,
    Xor,
    Xnor,
}
impl Gate {
    /// Input counts whose port geometry is supported by ordinary logic gates.
    pub const MAX_INPUTS: u8 = 8;

    pub const ALL: [Self; 8] = [
        Self::Not,
        Self::Buffer,
        Self::And,
        Self::Or,
        Self::Nand,
        Self::Nor,
        Self::Xor,
        Self::Xnor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Not => "NOT Gate",
            Self::Buffer => "Buffer",
            Self::And => "AND Gate",
            Self::Or => "OR Gate",
            Self::Nand => "NAND Gate",
            Self::Nor => "NOR Gate",
            Self::Xor => "XOR Gate",
            Self::Xnor => "XNOR Gate",
        }
    }
    pub fn unary(self) -> bool {
        matches!(self, Self::Not | Self::Buffer)
    }
    pub fn inverted(self) -> bool {
        matches!(self, Self::Not | Self::Nand | Self::Nor | Self::Xnor)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GateSize {
    #[default]
    Narrow,
    Medium,
    Wide,
}
impl GateSize {
    pub fn name(self) -> &'static str {
        match self {
            Self::Narrow => "Narrow",
            Self::Medium => "Medium",
            Self::Wide => "Wide",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateOptions {
    /// Indexed in world order: top to bottom, or left to right.
    pub negated: u8,
    pub xor_odd: bool,
}
impl Component {
    pub fn gate_negated(&self, input: u8) -> bool {
        input < Gate::MAX_INPUTS && self.gate_options.negated & (1 << input) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circuit, Facing, Gate, Kind, Point};
    #[test]
    fn inversion_extends_the_world_ordered_input_without_rerouting_wires() {
        for facing in Facing::ALL {
            let mut c = Component::new(Kind::Gate(Gate::And), Point::new(300, 300));
            c.facing = facing;
            c.gate_options.negated = 1;
            let mut circuit = Circuit::default();
            let port = c.ports()[1];
            let id = circuit.add_component(c);
            circuit.add_wire(port, port + facing.transform(Point::new(-40, 0)));
            let before = circuit.wires.clone();
            let mut c = circuit.component(id).unwrap().clone();
            c.gate_options.negated = 0;
            let plain = c.ports()[1];
            assert_eq!(port, plain + facing.transform(Point::new(-10, 0)));
            let points = c.ports();
            let axis = |p: Point| {
                if matches!(facing, Facing::East | Facing::West) {
                    p.y
                } else {
                    p.x
                }
            };
            assert!(axis(points[1]) < axis(points[2]));
            circuit.replace_component(c);
            assert_eq!(circuit.wires, before);
        }
    }
}

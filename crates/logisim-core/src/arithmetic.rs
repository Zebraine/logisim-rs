//! Fixed arithmetic terminals and editable options, measured from runtime behavior.
use crate::{Component, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    Adder,
    Subtractor,
    Multiplier,
    Divider,
    Negator,
    Comparator,
    Shifter,
}
impl Arithmetic {
    pub const ALL: [Self; 7] = [
        Self::Adder,
        Self::Subtractor,
        Self::Multiplier,
        Self::Divider,
        Self::Negator,
        Self::Comparator,
        Self::Shifter,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Adder => "Adder",
            Self::Subtractor => "Subtractor",
            Self::Multiplier => "Multiplier",
            Self::Divider => "Divider",
            Self::Negator => "Negator",
            Self::Comparator => "Comparator",
            Self::Shifter => "Shifter",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shift {
    #[default]
    LogicalLeft,
    LogicalRight,
    ArithmeticRight,
    RotateLeft,
    RotateRight,
}
impl Shift {
    pub const ALL: [Self; 5] = [
        Self::LogicalLeft,
        Self::LogicalRight,
        Self::ArithmeticRight,
        Self::RotateLeft,
        Self::RotateRight,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::LogicalLeft => "Logical left",
            Self::LogicalRight => "Logical right",
            Self::ArithmeticRight => "Arithmetic right",
            Self::RotateLeft => "Rotate left",
            Self::RotateRight => "Rotate right",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArithmeticOptions {
    pub signed: bool,
    pub shift: Shift,
}
impl Default for ArithmeticOptions {
    fn default() -> Self {
        Self {
            signed: true,
            shift: Shift::LogicalLeft,
        }
    }
}

impl Component {
    pub fn shift_amount_bits(&self) -> u8 {
        (8 - (self.bits.clamp(1, 32) - 1).leading_zeros() as u8).max(1)
    }
    /// 0 = result/equal; 1,2 = operands; 3,4 = auxiliary in/out or greater/less.
    pub fn arithmetic_ports(&self) -> Option<Vec<Point>> {
        let Kind::Arithmetic(kind) = self.kind else {
            return None;
        };
        if kind == Arithmetic::Negator {
            return Some(vec![Point::ZERO, Point::new(-40, 0)]);
        }
        let mut ports = vec![Point::ZERO, Point::new(-40, -10), Point::new(-40, 10)];
        match kind {
            Arithmetic::Adder
            | Arithmetic::Subtractor
            | Arithmetic::Multiplier
            | Arithmetic::Divider => {
                ports.extend([Point::new(-20, -20), Point::new(-20, 20)]);
            }
            Arithmetic::Comparator => {
                ports.extend([Point::new(0, -10), Point::new(0, 10)]);
            }
            Arithmetic::Shifter | Arithmetic::Negator => {}
        }
        Some(ports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_terminals_and_bus_width_boundaries() {
        for kind in Arithmetic::ALL {
            let mut c = Component::new(Kind::Arithmetic(kind), Point::new(100, 100));
            assert_eq!(c.bits, 8);
            assert!(!c.kind.rotatable());
            assert!(!c.kind.has_label());
            if kind == Arithmetic::Negator {
                assert_eq!(c.ports(), [Point::new(100, 100), Point::new(60, 100)]);
            } else {
                assert_eq!(c.ports()[1..3], [Point::new(60, 90), Point::new(60, 110)]);
            }
            if kind == Arithmetic::Comparator {
                assert_eq!(c.ports()[3..], [Point::new(100, 90), Point::new(100, 110)]);
            } else if !matches!(kind, Arithmetic::Shifter | Arithmetic::Negator) {
                assert_eq!(c.ports()[3..], [Point::new(80, 80), Point::new(80, 120)]);
            }
            for (width, amount) in [(1, 1), (3, 2), (4, 2), (5, 3), (8, 3), (32, 5)] {
                c.bits = width;
                assert_eq!(c.shift_amount_bits(), amount);
                for i in 0..c.ports().len() {
                    let output = i == 0 || i == 4 || (i == 3 && kind == Arithmetic::Comparator);
                    assert_eq!(c.drives_port(i), output);
                    let bits = if (i >= 3
                        && !matches!(kind, Arithmetic::Multiplier | Arithmetic::Divider))
                        || (kind == Arithmetic::Comparator && i == 0)
                    {
                        1
                    } else if kind == Arithmetic::Shifter && i == 2 {
                        amount
                    } else {
                        width
                    };
                    assert_eq!(c.port_width(i), Some(bits));
                }
            }
        }
    }
}

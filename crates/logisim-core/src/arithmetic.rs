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
    BitAdder,
    BitFinder,
}
impl Arithmetic {
    pub const ALL: [Self; 9] = [
        Self::Adder,
        Self::Subtractor,
        Self::Multiplier,
        Self::Divider,
        Self::Negator,
        Self::Comparator,
        Self::Shifter,
        Self::BitAdder,
        Self::BitFinder,
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
            Self::BitAdder => "Bit Adder",
            Self::BitFinder => "Bit Finder",
        }
    }
    pub fn file_name(self) -> &'static str {
        match self {
            Self::BitAdder => "BitAdder",
            Self::BitFinder => "BitFinder",
            _ => self.name(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BitSearch {
    #[default]
    LowestOne,
    HighestOne,
    LowestZero,
    HighestZero,
}
impl BitSearch {
    pub const ALL: [Self; 4] = [
        Self::LowestOne,
        Self::HighestOne,
        Self::LowestZero,
        Self::HighestZero,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::LowestOne => "Lowest-order 1",
            Self::HighestOne => "Highest-order 1",
            Self::LowestZero => "Lowest-order 0",
            Self::HighestZero => "Highest-order 0",
        }
    }
    pub fn from_high(self) -> bool {
        matches!(self, Self::HighestOne | Self::HighestZero)
    }
    pub fn find_one(self) -> bool {
        matches!(self, Self::LowestOne | Self::HighestOne)
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
    pub bit_inputs: u8,
    pub search: BitSearch,
}
impl Default for ArithmeticOptions {
    fn default() -> Self {
        Self {
            signed: true,
            shift: Shift::LogicalLeft,
            bit_inputs: 1,
            search: BitSearch::LowestOne,
        }
    }
}

impl Component {
    pub fn bit_adder_inputs(&self) -> u8 {
        self.arithmetic.bit_inputs.clamp(1, 32)
    }
    pub fn arithmetic_output_bits(&self) -> u8 {
        match self.kind {
            Kind::Arithmetic(Arithmetic::BitAdder) => {
                let maximum =
                    u32::from(self.bits.clamp(1, 32)) * u32::from(self.bit_adder_inputs());
                (32 - maximum.leading_zeros()) as u8
            }
            Kind::Arithmetic(Arithmetic::BitFinder) => self.shift_amount_bits(),
            Kind::Arithmetic(Arithmetic::Comparator) => 1,
            _ => self.bits.clamp(1, 32),
        }
    }
    pub fn arithmetic_body(&self) -> [Point; 2] {
        let n = self.bit_adder_inputs();
        if self.kind == Kind::Arithmetic(Arithmetic::BitAdder) && n >= 4 {
            let top = -i32::from((n - 1) / 2) * 10 - 5;
            [Point::new(-40, top), Point::new(0, top + i32::from(n) * 10)]
        } else {
            [Point::new(-40, -20), Point::new(0, 20)]
        }
    }
    pub fn shift_amount_bits(&self) -> u8 {
        (8 - (self.bits.clamp(1, 32) - 1).leading_zeros() as u8).max(1)
    }
    /// 0 = result/equal; 1,2 = operands; 3,4 = auxiliary in/out or greater/less.
    pub fn arithmetic_ports(&self) -> Option<Vec<Point>> {
        let Kind::Arithmetic(kind) = self.kind else {
            return None;
        };
        if kind == Arithmetic::BitAdder {
            let n = self.bit_adder_inputs();
            let mut ports = vec![Point::ZERO];
            ports.extend((0..n).map(|i| {
                Point::new(
                    -40,
                    if n == 2 {
                        i32::from(i) * 20 - 10
                    } else {
                        (i32::from(i) - i32::from((n - 1) / 2)) * 10
                    },
                )
            }));
            return Some(ports);
        }
        if kind == Arithmetic::BitFinder {
            return Some(vec![Point::ZERO, Point::new(-40, 0), Point::new(-20, 20)]);
        }
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
            Arithmetic::Shifter
            | Arithmetic::Negator
            | Arithmetic::BitAdder
            | Arithmetic::BitFinder => {}
        }
        Some(ports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bit_adder_geometry_and_count_edits_keep_observed_wire_positions() {
        let mut c = Component::new(Kind::Arithmetic(Arithmetic::BitAdder), Point::new(200, 200));
        for (n, first, last, top, bottom) in [
            (1, 0, 0, -20, 20),
            (2, -10, 10, -20, 20),
            (3, -10, 10, -20, 20),
            (4, -10, 20, -15, 25),
            (5, -20, 20, -25, 25),
            (8, -30, 40, -35, 45),
            (32, -150, 160, -155, 165),
        ] {
            c.arithmetic.bit_inputs = n;
            let ports = c.local_ports();
            assert_eq!(ports.len(), usize::from(n) + 1);
            assert_eq!(ports[1], Point::new(-40, first));
            assert_eq!(*ports.last().unwrap(), Point::new(-40, last));
            assert_eq!(
                c.arithmetic_body(),
                [Point::new(-40, top), Point::new(0, bottom)]
            );
            for i in 1..ports.len() {
                assert_eq!(c.port_width(i), Some(8));
                assert!(!c.drives_port(i));
            }
        }
        for (w, n, result) in [
            (1, 1, 1),
            (1, 2, 2),
            (3, 1, 2),
            (4, 1, 3),
            (8, 1, 4),
            (32, 1, 6),
            (32, 8, 9),
            (8, 32, 9),
            (32, 32, 11),
        ] {
            c.bits = w;
            c.arithmetic.bit_inputs = n;
            assert_eq!(c.port_width(0), Some(result));
        }
        c.bits = 1;
        c.arithmetic.bit_inputs = 2;
        let mut circuit = crate::Circuit::default();
        let id = circuit.add_component(c);
        for y in [190, 210] {
            circuit.add_wire(Point::new(80, y), Point::new(160, y));
        }
        let wires = circuit.wires.clone();
        for n in [4, 1, 32, 2] {
            let mut replacement = circuit.component(id).unwrap().clone();
            replacement.arithmetic.bit_inputs = n;
            circuit.replace_component(replacement);
            assert_eq!(circuit.wires, wires);
        }
    }
    #[test]
    fn fixed_terminals_and_bus_width_boundaries() {
        for kind in Arithmetic::ALL {
            let mut c = Component::new(Kind::Arithmetic(kind), Point::new(100, 100));
            assert_eq!(c.bits, 8);
            assert!(!c.kind.rotatable());
            assert!(!c.kind.has_label());
            if matches!(kind, Arithmetic::Negator | Arithmetic::BitAdder) {
                assert_eq!(c.ports(), [Point::new(100, 100), Point::new(60, 100)]);
            } else if kind == Arithmetic::BitFinder {
                assert_eq!(
                    c.ports(),
                    [
                        Point::new(100, 100),
                        Point::new(60, 100),
                        Point::new(80, 120)
                    ]
                );
            } else {
                assert_eq!(c.ports()[1..3], [Point::new(60, 90), Point::new(60, 110)]);
            }
            if kind == Arithmetic::Comparator {
                assert_eq!(c.ports()[3..], [Point::new(100, 90), Point::new(100, 110)]);
            } else if matches!(
                kind,
                Arithmetic::Adder
                    | Arithmetic::Subtractor
                    | Arithmetic::Multiplier
                    | Arithmetic::Divider
            ) {
                assert_eq!(c.ports()[3..], [Point::new(80, 80), Point::new(80, 120)]);
            }
            for (width, amount) in [(1, 1), (3, 2), (4, 2), (5, 3), (8, 3), (32, 5)] {
                c.bits = width;
                assert_eq!(c.shift_amount_bits(), amount);
                for i in 0..c.ports().len() {
                    let output = i == 0
                        || i == 4
                        || (i == 3 && kind == Arithmetic::Comparator)
                        || (i == 2 && kind == Arithmetic::BitFinder);
                    assert_eq!(c.drives_port(i), output);
                    let bits = if i == 0 && kind == Arithmetic::BitAdder {
                        match width {
                            1 => 1,
                            3 => 2,
                            4 | 5 => 3,
                            8 => 4,
                            32 => 6,
                            _ => unreachable!(),
                        }
                    } else if i == 0 && kind == Arithmetic::BitFinder {
                        amount
                    } else if (i == 2 && kind == Arithmetic::BitFinder)
                        || (i >= 3 && !matches!(kind, Arithmetic::Multiplier | Arithmetic::Divider))
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

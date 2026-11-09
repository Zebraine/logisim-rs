//! Fixed arithmetic terminals and editable options, measured from runtime behavior.
use crate::{Component, Kind, Point};
mod logic;
pub use logic::ArithmeticLogic;

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
        self.arithmetic.input_count()
    }
    pub fn arithmetic_output_bits(&self) -> u8 {
        self.arithmetic_logic()
            .map_or(self.bits.clamp(1, 32), |logic| logic.output_bits())
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
        logic::index_bits(self.bits)
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
mod tests;

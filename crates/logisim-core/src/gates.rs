use crate::{Component, Facing, GRID, Kind};

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
    pub(crate) fn width(self) -> i32 {
        match self {
            Self::Narrow => 30,
            Self::Medium => 50,
            Self::Wide => 70,
        }
    }

    /// Shared by ordinary and parity gates; numbering stays in world order.
    pub(crate) fn input_y(self, count: u8, input: u8, facing: Facing) -> i32 {
        if count == 1 {
            return 0;
        }
        let mut slot = i32::from(input) - i32::from(count / 2);
        if count.is_multiple_of(2) && slot >= 0 {
            slot += 1;
        }
        let y = match (self, count) {
            (Self::Medium | Self::Wide, 2) | (Self::Medium, 3) => slot * 20,
            (Self::Wide, 3) => slot * 30,
            (Self::Wide, 4) => [-30, -10, 10, 30][usize::from(input)],
            _ => slot * GRID,
        };
        if matches!(facing, Facing::South | Facing::West) {
            -y
        } else {
            y
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Narrow => "Narrow",
            Self::Medium => "Medium",
            Self::Wide => "Wide",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateOptions {
    pub inputs: u8,
    pub size: GateSize,
    /// Indexed in world order: top to bottom, or left to right.
    pub negated: u8,
    pub xor_odd: bool,
}
impl Default for GateOptions {
    fn default() -> Self {
        Self {
            inputs: 2,
            size: GateSize::Narrow,
            negated: 0,
            xor_odd: false,
        }
    }
}

/// Electrical snapshot shared by event and compiled evaluation; no document geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateLogic {
    pub gate: Gate,
    pub bits: u8,
    pub negated: u8,
    pub xor_odd: bool,
}
impl GateLogic {
    pub fn input_negated(&self, input: u8) -> bool {
        input < Gate::MAX_INPUTS && self.negated & (1 << input) != 0
    }
}
impl Component {
    pub fn gate_logic(&self) -> Option<GateLogic> {
        let Kind::Gate(gate) = self.kind else {
            return None;
        };
        Some(GateLogic {
            gate,
            bits: self.bits,
            negated: self.gate_options.negated,
            xor_odd: self.gate_options.xor_odd,
        })
    }
    pub fn gate_negated(&self, input: u8) -> bool {
        self.gate_logic()
            .is_some_and(|logic| logic.input_negated(input))
    }
}

#[cfg(test)]
mod tests;

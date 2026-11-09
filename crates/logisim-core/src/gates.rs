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
mod tests;

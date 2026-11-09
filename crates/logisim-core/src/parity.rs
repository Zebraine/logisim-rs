//! Parity gate attributes and geometry, independent of signal evaluation.
use crate::{Component, GateSize, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parity {
    Odd,
    Even,
}
impl Parity {
    pub const ALL: [Self; 2] = [Self::Odd, Self::Even];
    pub fn name(self) -> &'static str {
        match self {
            Self::Odd => "Odd Parity",
            Self::Even => "Even Parity",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParityOutput {
    #[default]
    Normal,
    ZeroFloating,
    FloatingOne,
}
impl ParityOutput {
    pub const ALL: [Self; 3] = [Self::Normal, Self::ZeroFloating, Self::FloatingOne];
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "0/1",
            Self::ZeroFloating => "0/floating",
            Self::FloatingOne => "floating/1",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParityOptions {
    pub inputs: u8,
    pub size: GateSize,
    /// Hidden inputs retain their inversion while editing; only active inputs are serialized.
    pub negated: u32,
    pub output: ParityOutput,
}
impl Default for ParityOptions {
    fn default() -> Self {
        Self {
            inputs: 2,
            size: GateSize::Narrow,
            negated: 0,
            output: ParityOutput::Normal,
        }
    }
}

/// Electrical snapshot; input presence is supplied separately by the topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParityLogic {
    pub parity: Parity,
    pub bits: u8,
    pub negated: u32,
    pub output: ParityOutput,
}
impl ParityLogic {
    pub fn input_negated(&self, input: u8) -> bool {
        input < 32 && self.negated & (1 << input) != 0
    }
}
impl Component {
    pub fn parity_logic(&self) -> Option<ParityLogic> {
        let Kind::Parity(parity) = self.kind else {
            return None;
        };
        Some(ParityLogic {
            parity,
            bits: self.bits,
            negated: self.parity.negated,
            output: self.parity.output,
        })
    }
    pub fn parity_negated(&self, input: u8) -> bool {
        self.parity_logic()
            .is_some_and(|logic| logic.input_negated(input))
    }
    pub fn parity_ports(&self) -> Vec<Point> {
        let count = self.parity.inputs.clamp(2, 32);
        let mut ports = vec![Point::ZERO];
        for i in 0..count {
            let y = self.parity.size.input_y(count, i, self.facing);
            ports.push(Point::new(
                -self.gate_width() - if self.parity_negated(i) { 10 } else { 0 },
                y,
            ));
        }
        ports
    }
    pub fn parity_half_height(&self) -> i32 {
        (self.gate_width() / 2).max(i32::from(self.parity.inputs.clamp(2, 32) / 2) * 10 + 5)
    }
}

#[cfg(test)]
mod tests;

//! Parity gate attributes and geometry, independent of signal evaluation.
use crate::{Component, Facing, GateSize, Point};

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParityOptions {
    /// Hidden inputs retain their inversion while editing; only active inputs are serialized.
    pub negated: u32,
    pub output: ParityOutput,
}
impl Component {
    pub fn parity_negated(&self, input: u8) -> bool {
        input < 32 && self.parity.negated & (1 << input) != 0
    }
    pub fn parity_ports(&self) -> Vec<Point> {
        let count = self.inputs.clamp(2, 32);
        let mut ports = vec![Point::ZERO];
        for i in 0..count {
            let mut slot = i32::from(i) - i32::from(count / 2);
            if count.is_multiple_of(2) && slot >= 0 {
                slot += 1;
            }
            let mut y = match (self.gate_size, count) {
                (GateSize::Medium | GateSize::Wide, 2) | (GateSize::Medium, 3) => slot * 20,
                (GateSize::Wide, 3) => slot * 30,
                (GateSize::Wide, 4) => [-30, -10, 10, 30][usize::from(i)],
                _ => slot * 10,
            };
            if matches!(self.facing, Facing::South | Facing::West) {
                y = -y;
            }
            ports.push(Point::new(
                -self.gate_width() - if self.parity_negated(i) { 10 } else { 0 },
                y,
            ));
        }
        ports
    }
    pub fn parity_half_height(&self) -> i32 {
        (self.gate_width() / 2).max(i32::from(self.inputs.clamp(2, 32) / 2) * 10 + 5)
    }
}

#[cfg(test)]
mod tests;

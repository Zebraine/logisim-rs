//! Arithmetic configuration shared by document widths and both evaluation modes.
use super::{Arithmetic, ArithmeticOptions};
use crate::{Component, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArithmeticLogic {
    pub kind: Arithmetic,
    pub bits: u8,
    pub options: ArithmeticOptions,
}

pub(super) fn index_bits(bits: u8) -> u8 {
    (8 - (bits.clamp(1, 32) - 1).leading_zeros() as u8).max(1)
}

impl ArithmeticOptions {
    pub fn input_count(&self) -> u8 {
        self.bit_inputs.clamp(1, 32)
    }
}

impl ArithmeticLogic {
    pub fn output_bits(&self) -> u8 {
        match self.kind {
            Arithmetic::BitAdder => {
                let maximum =
                    u32::from(self.bits.clamp(1, 32)) * u32::from(self.options.input_count());
                (32 - maximum.leading_zeros()) as u8
            }
            Arithmetic::BitFinder => index_bits(self.bits),
            Arithmetic::Comparator => 1,
            _ => self.bits.clamp(1, 32),
        }
    }
}

impl Component {
    pub fn arithmetic_logic(&self) -> Option<ArithmeticLogic> {
        let Kind::Arithmetic(kind) = self.kind else {
            return None;
        };
        Some(ArithmeticLogic {
            kind,
            bits: self.bits.clamp(1, 32),
            options: self.arithmetic,
        })
    }
}

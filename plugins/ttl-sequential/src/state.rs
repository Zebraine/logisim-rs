use crate::model::{Chip, DATA};
use Bit::{Error, One, Unknown, Zero};
use logisim_core::{Bit, Value};
use logisim_plugin_api::Signal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub bits: [Bit; 8],
    clock: Bit,
}
impl Default for State {
    fn default() -> Self {
        Self {
            bits: [Zero; 8],
            clock: Unknown,
        }
    }
}
pub fn powered(chip: Chip, inputs: &[Signal]) -> bool {
    !chip.power || (bit(chip, inputs, 16) == One && bit(chip, inputs, 8) == Zero)
}
fn bit(chip: Chip, inputs: &[Signal], pin: u8) -> Bit {
    Value::from(inputs[chip.index(pin)]).bit(0)
}
impl State {
    pub fn sample(&mut self, chip: Chip, inputs: &[Signal]) {
        if !powered(chip, inputs) {
            return;
        }
        let clock = bit(chip, inputs, 2);
        let rising = self.clock == Zero && clock == One;
        self.clock = clock;
        if rising && bit(chip, inputs, 15) != One {
            match bit(chip, inputs, 1) {
                Zero => self.bits = DATA.map(|pin| bit(chip, inputs, pin)),
                One => {
                    self.bits.copy_within(1..8, 0);
                    self.bits[7] = bit(chip, inputs, 10);
                }
                Unknown | Error => {}
            }
        }
    }
    pub fn outputs(&self, chip: Chip, inputs: &[Signal]) -> [Bit; 2] {
        if !powered(chip, inputs) {
            return [Unknown; 2];
        }
        let q = self.bits[7];
        [
            q,
            match q {
                Zero => One,
                One => Zero,
                Unknown | Error => Error,
            },
        ]
    }
    /// Versioned bytes, independent of Rust layout. Empty means a fresh instance.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        if bytes.len() != 7 || &bytes[..4] != b"L65\x01" || bytes[4] > 3 {
            return Err("Invalid 74165 state encoding".into());
        }
        const BITS: [Bit; 4] = [Zero, One, Unknown, Error];
        Ok(Self {
            clock: BITS[usize::from(bytes[4])],
            bits: std::array::from_fn(|i| {
                BITS[usize::from((bytes[5 + i / 4] >> (2 * (i % 4))) & 3)]
            }),
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let code = |bit| match bit {
            Zero => 0,
            One => 1,
            Unknown => 2,
            Error => 3,
        };
        let mut bytes = b"L65\x01".to_vec();
        bytes.push(code(self.clock));
        for bits in self.bits.as_chunks::<4>().0 {
            bytes.push(
                bits.iter()
                    .enumerate()
                    .fold(0, |v, (i, b)| v | (code(*b) << (i * 2))),
            );
        }
        bytes
    }
}

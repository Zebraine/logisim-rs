//! Observed software four-state semantics, including non-hardware treatment of X/E.
use crate::model::{Chip, Kind};
use Bit::{One, Unknown, Zero};
use logisim_core::{Bit, Value};

pub fn evaluate(chip: Chip, ports: &[Value]) -> Vec<Bit> {
    let at = |pin| ports[chip.index(pin)].bit(0);
    let n = chip.kind.count();
    if chip.power && (at(n) != One || at(n / 2) != Zero) {
        return vec![Unknown; chip.kind.outputs().len()];
    }
    let input: Vec<_> = chip.kind.inputs().iter().map(|p| at(*p)).collect();
    let number = |bits: &[Bit]| {
        bits.iter()
            .enumerate()
            .fold(0u8, |v, (i, b)| v | (u8::from(*b == One) << i))
    };
    let binary = |v: u8, width: usize| {
        (0..width)
            .map(|i| if v & (1 << i) != 0 { One } else { Zero })
            .collect()
    };
    match chip.kind {
        Kind::Buffer74125 => input
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| if p[0] == One { Unknown } else { p[1] })
            .collect(),
        Kind::Adder74283 => binary(
            number(&input[..4]) + number(&input[4..8]) + u8::from(input[8] == One),
            5,
        ),
        Kind::Comparator7485 => {
            let a = number(&input[..4]);
            let b = number(&input[4..8]);
            let (lt, eq, gt) = (input[8] == One, input[9] == One, input[10] == One);
            let code = if a > b {
                1
            } else if a < b {
                4
            } else if eq {
                2
            } else if lt && gt {
                0
            } else if gt {
                1
            } else if lt {
                4
            } else {
                5
            };
            binary(code, 3)
        }
        Kind::Decoder7447 => {
            if input[5] == Zero {
                return vec![Zero; 7];
            }
            if input[4] == Zero {
                return vec![One; 7];
            }
            if input[..4].contains(&Unknown) {
                return vec![Unknown; 7];
            }
            let n = number(&input[..4]);
            // a..g in bit order, including the reference's hexadecimal glyphs.
            const SEGMENTS: [u8; 16] = [
                0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x67, 0x77, 0x7c, 0x39, 0x5e,
                0x79, 0x71,
            ];
            binary(
                if n == 0 && input[6] == Zero {
                    0
                } else {
                    SEGMENTS[usize::from(n)]
                },
                7,
            )
        }
    }
}

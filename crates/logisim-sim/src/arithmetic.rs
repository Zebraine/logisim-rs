use crate::{Bit, Value};
use logisim_core::{Arithmetic, Component, Kind, Shift};

pub(crate) fn evaluate(c: &Component, ports: &[Value]) -> Vec<(usize, Value)> {
    let Kind::Arithmetic(kind) = c.kind else {
        return vec![];
    };
    let a = ports[1];
    let b = ports[2];
    let width = c.bits.clamp(1, 32);
    match kind {
        Arithmetic::Adder | Arithmetic::Subtractor => {
            let subtract = kind == Arithmetic::Subtractor;
            let mut carry = match ports[3].bit(0) {
                Bit::Unknown => Bit::Zero,
                other => other,
            };
            let result = Value::from_bits((0..width).map(|i| {
                // Once uncertain, the carry determines all remaining columns.
                if matches!(carry, Bit::Unknown | Bit::Error) {
                    return carry;
                }
                let (a, b) = (a.bit(i), b.bit(i));
                carry = if a == Bit::Error || b == Bit::Error || (subtract && b == Bit::Unknown) {
                    Bit::Error
                } else if a == Bit::Unknown || b == Bit::Unknown {
                    Bit::Unknown
                } else {
                    let a = i8::from(a == Bit::One);
                    let b = i8::from(b == Bit::One);
                    let cin = i8::from(carry == Bit::One);
                    let total = if subtract { a - b - cin } else { a + b + cin };
                    carry = if if subtract { total < 0 } else { total >= 2 } {
                        Bit::One
                    } else {
                        Bit::Zero
                    };
                    return if total & 1 == 1 { Bit::One } else { Bit::Zero };
                };
                carry
            }));
            if subtract && carry == Bit::Unknown {
                carry = Bit::Error;
            }
            vec![(0, result), (4, Value::filled(1, carry))]
        }
        Arithmetic::Comparator => {
            let mut relation = std::cmp::Ordering::Equal;
            let mut uncertain = None;
            for i in (0..width).rev() {
                let (a, b) = (a.bit(i), b.bit(i));
                if a == Bit::Error || b == Bit::Error {
                    uncertain = Some(Bit::Error);
                    break;
                }
                if a == Bit::Unknown || b == Bit::Unknown {
                    uncertain = Some(Bit::Unknown);
                    break;
                }
                relation = (a == Bit::One).cmp(&(b == Bit::One));
                if c.arithmetic.signed && i == width - 1 {
                    relation = relation.reverse();
                }
                if relation != std::cmp::Ordering::Equal {
                    break;
                }
            }
            [
                (0, std::cmp::Ordering::Equal),
                (3, std::cmp::Ordering::Greater),
                (4, std::cmp::Ordering::Less),
            ]
            .into_iter()
            .map(|(i, expected)| {
                (
                    i,
                    uncertain.map_or_else(
                        || Value::known(1, u32::from(relation == expected)),
                        |bit| Value::filled(1, bit),
                    ),
                )
            })
            .collect()
        }
        Arithmetic::Shifter => {
            let Some(distance) = b.number() else {
                return vec![(0, Value::filled(width, Bit::Error))];
            };
            let distance = distance as i32;
            let w = i32::from(width);
            let result = Value::from_bits((0..w).map(|i| {
                let source = match c.arithmetic.shift {
                    Shift::LogicalLeft => i - distance,
                    Shift::LogicalRight | Shift::ArithmeticRight => i + distance,
                    Shift::RotateLeft => (i - distance).rem_euclid(w),
                    Shift::RotateRight => (i + distance) % w,
                };
                if (0..w).contains(&source) {
                    a.bit(source as u8)
                } else if c.arithmetic.shift == Shift::ArithmeticRight {
                    a.bit(width - 1)
                } else {
                    Bit::Zero
                }
            }));
            vec![(0, result)]
        }
    }
}

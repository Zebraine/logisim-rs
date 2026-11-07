use crate::{Bit, Value};
use logisim_core::{Arithmetic, Component, Kind, Shift};

pub(crate) fn evaluate(c: &Component, ports: &[Value]) -> Vec<(usize, Value)> {
    let Kind::Arithmetic(kind) = c.kind else {
        return vec![];
    };
    let a = ports[1];
    let width = c.bits.clamp(1, 32);
    if kind == Arithmetic::BitAdder {
        let mut minimum = 0_u32;
        let mut uncertain = 0_u32;
        for input in &ports[1..] {
            for i in 0..width {
                match input.bit(i) {
                    Bit::One => minimum += 1,
                    Bit::Unknown | Bit::Error => uncertain += 1,
                    Bit::Zero => {}
                }
            }
        }
        let maximum = minimum + uncertain;
        // All possible counts form an interval; only its common high prefix is known.
        let uncertain_bits = 32 - (minimum ^ maximum).leading_zeros();
        let count = Value::from_bits((0..c.arithmetic_output_bits()).map(|i| {
            if u32::from(i) < uncertain_bits {
                Bit::Error
            } else if minimum & (1 << i) != 0 {
                Bit::One
            } else {
                Bit::Zero
            }
        }));
        return vec![(0, count)];
    }
    if kind == Arithmetic::BitFinder {
        let result_width = c.arithmetic_output_bits();
        let target = if c.arithmetic.search.find_one() {
            Bit::One
        } else {
            Bit::Zero
        };
        for offset in 0..width {
            let i = if c.arithmetic.search.from_high() {
                width - 1 - offset
            } else {
                offset
            };
            match a.bit(i) {
                Bit::Unknown | Bit::Error => {
                    return vec![
                        (0, Value::filled(result_width, Bit::Error)),
                        (2, Value::filled(1, Bit::Error)),
                    ];
                }
                bit if bit == target => {
                    return vec![
                        (0, Value::known(result_width, u32::from(i))),
                        (2, Value::known(1, 1)),
                    ];
                }
                _ => {}
            }
        }
        return vec![(0, Value::known(result_width, 0)), (2, Value::known(1, 0))];
    }
    if kind == Arithmetic::Negator {
        return vec![(0, negate(a))];
    }
    let b = ports[2];
    match kind {
        Arithmetic::Multiplier | Arithmetic::Divider => {
            let extra = if (0..width).all(|i| ports[3].bit(i) == Bit::Unknown) {
                Value::known(width, 0)
            } else {
                ports[3]
            };
            let (result, auxiliary) = if kind == Arithmetic::Multiplier {
                multiply(a, b, extra)
            } else {
                divide(a, b, extra)
            };
            vec![(0, result), (4, auxiliary)]
        }
        Arithmetic::Negator | Arithmetic::BitAdder | Arithmetic::BitFinder => {
            unreachable!("handled before reading a second operand")
        }
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

fn multiply(a: Value, b: Value, extra: Value) -> (Value, Value) {
    let width = a.width();
    let inputs = [a, b, extra];
    let first_uncertain = (0..width).find(|i| {
        inputs
            .iter()
            .any(|v| matches!(v.bit(*i), Bit::Unknown | Bit::Error))
    });
    let first_error = (0..width).find(|i| inputs.iter().any(|v| v.bit(*i) == Bit::Error));
    // Unknown high bits do not affect the exact low prefix of a product or sum.
    let known_bits =
        |v: Value| (0..width).fold(0_u64, |n, i| n | (u64::from(v.bit(i) == Bit::One) << i));
    let product = known_bits(a) * known_bits(b) + known_bits(extra);
    let low = Value::from_bits((0..width).map(|i| {
        if first_error.is_some_and(|start| i >= start) {
            Bit::Error
        } else if first_uncertain.is_some_and(|start| i >= start) {
            Bit::Unknown
        } else if product & (1 << i) != 0 {
            Bit::One
        } else {
            Bit::Zero
        }
    }));
    let high = if first_error.is_some() {
        Value::filled(width, Bit::Error)
    } else if first_uncertain.is_some() {
        Value::filled(width, Bit::Unknown)
    } else {
        Value::known(width, (product >> width) as u32)
    };
    (low, high)
}

fn divide(low: Value, divisor: Value, upper: Value) -> (Value, Value) {
    let width = low.width();
    if [low, divisor, upper].iter().any(|v| v.contains(Bit::Error)) {
        return (
            Value::filled(width, Bit::Error),
            Value::filled(width, Bit::Error),
        );
    }
    let (Some(low), Some(divisor), Some(upper)) = (low.number(), divisor.number(), upper.number())
    else {
        return (
            Value::filled(width, Bit::Unknown),
            Value::filled(width, Bit::Unknown),
        );
    };
    // Runtime observations at width 32 use a signed 64-bit dividend and nonnegative
    // remainder. Keep that boundary behavior, including when the divisor is zero.
    let dividend = ((u64::from(upper) << width) | u64::from(low)) as i64;
    let divisor = i64::from(divisor.max(1));
    (
        Value::known(width, dividend.div_euclid(divisor) as u32),
        Value::known(width, dividend.rem_euclid(divisor) as u32),
    )
}

fn negate(input: Value) -> Value {
    // Before the first known 1, uncertainty propagates upward; after it, invert
    // defined bits independently and preserve X/E at their own positions.
    let mut carry = Bit::One;
    Value::from_bits((0..input.width()).map(|i| {
        let bit = input.bit(i);
        if carry == Bit::Zero {
            return match bit {
                Bit::Zero => Bit::One,
                Bit::One => Bit::Zero,
                other => other,
            };
        }
        if bit == Bit::One {
            let result = carry;
            carry = Bit::Zero;
            return result;
        }
        if bit == Bit::Error {
            carry = Bit::Error;
        } else if bit == Bit::Unknown && carry != Bit::Error {
            carry = Bit::Unknown;
        }
        if carry == Bit::One { Bit::Zero } else { carry }
    }))
}

use crate::{Bit, Value};
use logisim_core::{Parity, ParityLogic, ParityOutput};

fn invert(bit: Bit) -> Bit {
    match bit {
        Bit::Zero => Bit::One,
        Bit::One => Bit::Zero,
        _ => Bit::Error,
    }
}
pub(crate) fn evaluate(c: &ParityLogic, inputs: &[Option<Value>]) -> Value {
    Value::from_bits((0..c.bits.clamp(1, 32)).map(|bit| {
        let mut values = inputs.iter().enumerate().filter_map(|(i, input)| {
            input.map(|v| {
                if c.input_negated(i as u8) {
                    invert(v.bit(bit))
                } else {
                    v.bit(bit)
                }
            })
        });
        // A single connected input is passed through; two or more use four-state XOR.
        let mut result = values.next().unwrap_or(Bit::Error);
        for next in values {
            result = match (result, next) {
                (Bit::Zero, Bit::Zero) | (Bit::One, Bit::One) => Bit::Zero,
                (Bit::Zero, Bit::One) | (Bit::One, Bit::Zero) => Bit::One,
                _ => Bit::Error,
            };
        }
        if c.parity == Parity::Even {
            result = invert(result);
        }
        match (c.output, result) {
            (ParityOutput::ZeroFloating, Bit::One) | (ParityOutput::FloatingOne, Bit::Zero) => {
                Bit::Unknown
            }
            _ => result,
        }
    }))
}

#[cfg(test)]
mod tests;

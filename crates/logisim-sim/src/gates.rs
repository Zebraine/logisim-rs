use crate::{Bit, Value};
use logisim_core::{Component, Gate, Kind};

pub(crate) fn evaluate(c: &Component, inputs: &[Option<Value>]) -> Value {
    let Kind::Gate(gate) = c.kind else {
        unreachable!("gate component");
    };
    let inputs: Vec<_> = inputs
        .iter()
        .enumerate()
        .filter_map(|(i, value)| {
            value.map(|v| {
                if c.gate_negated(i as u8) {
                    crate::value::evaluate(Gate::Not, c.bits, &[v])
                } else {
                    v
                }
            })
        })
        .collect();
    // Ordinary gates ignore absent terminals. PLA terms have separate four-state behavior.
    if matches!(gate, Gate::And | Gate::Or) && inputs.len() == 1 {
        return inputs[0];
    }
    if matches!(gate, Gate::Xor | Gate::Xnor) && c.gate_options.xor_odd {
        return Value::from_bits((0..c.bits).map(|i| {
            let mut bits = inputs.iter().map(|v| v.bit(i));
            let mut result = bits.next().unwrap_or(Bit::Error);
            for next in bits {
                result = match (result, next) {
                    (Bit::Zero, Bit::Zero) | (Bit::One, Bit::One) => Bit::Zero,
                    (Bit::Zero, Bit::One) | (Bit::One, Bit::Zero) => Bit::One,
                    _ => Bit::Error,
                };
            }
            if gate == Gate::Xnor {
                match result {
                    Bit::Zero => Bit::One,
                    Bit::One => Bit::Zero,
                    _ => Bit::Error,
                }
            } else {
                result
            }
        }));
    }
    crate::value::evaluate(gate, c.bits, &inputs)
}

#[cfg(test)]
mod tests;

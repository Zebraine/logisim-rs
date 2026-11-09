use crate::{Bit, Value};
use logisim_core::{Gate, Pla, PlaInput};

pub(crate) fn evaluate(pla: &Pla, input: Value, select: Bit) -> Value {
    let (inputs, terms, outputs) = pla.dimensions();
    if select == if pla.select_high { Bit::Zero } else { Bit::One } {
        return Value::filled(outputs, Bit::Unknown);
    }
    let terms: Vec<_> = (0..terms)
        .map(|row| {
            let literals: Vec<_> = (0..inputs)
                .filter_map(|column| {
                    let bit = input.bit(inputs - 1 - column);
                    let bit = match pla.input(row, column) {
                        PlaInput::Disconnected => return None,
                        PlaInput::Direct => bit,
                        PlaInput::Inverted => match bit {
                            Bit::Zero => Bit::One,
                            Bit::One => Bit::Zero,
                            _ => Bit::Error,
                        },
                    };
                    Some(Value::filled(1, bit))
                })
                .collect();
            crate::value::evaluate(Gate::And, 1, &literals)
        })
        .collect();
    Value::from_bits((0..outputs).rev().map(|column| {
        let selected: Vec<_> = terms
            .iter()
            .enumerate()
            .filter_map(|(row, value)| pla.output(row as u8, column).then_some(*value))
            .collect();
        crate::value::evaluate(Gate::Or, 1, &selected).bit(0)
    }))
}

#[cfg(test)]
mod tests;

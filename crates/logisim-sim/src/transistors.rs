use crate::{Bit, Value};
use logisim_core::{Component, Kind, TransistorType};

pub(crate) fn evaluate(c: &Component, input: Value, gate: Bit, n_gate: Bit) -> Value {
    let (on, off) = if c.kind == Kind::TransmissionGate {
        (
            (gate, n_gate) == (Bit::Zero, Bit::One),
            (gate, n_gate) == (Bit::One, Bit::Zero),
        )
    } else {
        let (active, inactive) = match c.transistor.transistor_type {
            TransistorType::P => (Bit::Zero, Bit::One),
            TransistorType::N => (Bit::One, Bit::Zero),
        };
        (gate == active, gate == inactive)
    };
    if on {
        input
    } else if off {
        Value::filled(c.bits, Bit::Unknown)
    } else {
        Value::from_bits((0..c.bits.clamp(1, 32)).map(|i| {
            if input.bit(i) == Bit::Unknown {
                Bit::Unknown
            } else {
                Bit::Error
            }
        }))
    }
}

#[cfg(test)]
mod tests;

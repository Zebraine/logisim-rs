use logisim_core::{Bit, Gate, Value};

/// Inputs omitted by the caller are physically unconnected, not floating wires.
pub(crate) fn evaluate(gate: Gate, width: u8, inputs: &[Value]) -> Value {
    Value::from_bits((0..width.min(32)).map(|i| {
        let bits: Vec<_> = inputs.iter().map(|v| v.bit(i)).collect();
        let base = match gate {
            Gate::Buffer => bits.first().copied().unwrap_or(Bit::Unknown),
            Gate::Not => match bits.first() {
                Some(Bit::Zero) => Bit::One,
                Some(Bit::One) => Bit::Zero,
                _ => Bit::Error,
            },
            Gate::And | Gate::Nand if bits.contains(&Bit::Zero) => Bit::Zero,
            Gate::Or | Gate::Nor if bits.contains(&Bit::One) => Bit::One,
            _ if bits.is_empty() || bits.iter().any(|b| matches!(b, Bit::Unknown | Bit::Error)) => {
                Bit::Error
            }
            Gate::And | Gate::Nand => Bit::One,
            Gate::Or | Gate::Nor => Bit::Zero,
            Gate::Xor | Gate::Xnor => {
                if bits.iter().filter(|b| **b == Bit::One).count() == 1 {
                    Bit::One
                } else {
                    Bit::Zero
                }
            }
        };
        if matches!(gate, Gate::Nand | Gate::Nor | Gate::Xnor) {
            match base {
                Bit::Zero => Bit::One,
                Bit::One => Bit::Zero,
                other => other,
            }
        } else {
            base
        }
    }))
}

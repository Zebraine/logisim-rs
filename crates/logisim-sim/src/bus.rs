use crate::{Bit, Value};
use logisim_core::{Component, Gate, Kind, Pull};
pub(crate) fn pull_bit(pull: Pull) -> Bit {
    match pull {
        Pull::Zero => Bit::Zero,
        Pull::One => Bit::One,
        Pull::Error => Bit::Error,
    }
}
pub(crate) fn evaluate(c: &Component, input: Value, enable: Bit) -> Value {
    let active = if c.bus.negate_control {
        Bit::Zero
    } else {
        Bit::One
    };
    if matches!(enable, Bit::Unknown | Bit::Error) {
        Value::filled(c.bits, Bit::Error)
    } else if enable != active {
        Value::filled(c.bits, Bit::Unknown)
    } else if c.kind == Kind::ControlledInverter {
        crate::value::evaluate(Gate::Not, c.bits, &[input])
    } else {
        input
    }
}

#[cfg(test)]
mod tests;

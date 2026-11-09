use crate::{Bit, Simulator, Value};
use logisim_core::{Component, Id, Kind, PinPull};

pub(crate) fn initial(c: &Component, bits: u8) -> Value {
    Value::filled(
        bits,
        if c.kind == Kind::Pin && c.pin.three_state {
            Bit::Unknown
        } else {
            Bit::Zero
        },
    )
}

pub(crate) fn pulled(c: &Component, value: Value) -> Value {
    let pull = match c.pin.pull {
        PinPull::None => return value,
        PinPull::Up => Bit::One,
        PinPull::Down => Bit::Zero,
    };
    Value::from_bits((0..value.width()).map(|i| match value.bit(i) {
        Bit::Unknown | Bit::Error => pull,
        bit => bit,
    }))
}

impl Simulator {
    pub fn set_input_unknown(&mut self, id: Id) {
        if let Some(c) = self.circuit.component(id)
            && c.kind == Kind::Pin
            && !c.output
            && c.pin.three_state
        {
            self.set_input_value(id, Value::filled(c.bits, Bit::Unknown));
        }
    }

    pub fn toggle_input(&mut self, id: Id, bit: u8) {
        if let Some(value) = self.input(id)
            && bit < value.width()
        {
            let tri = self
                .circuit
                .component(id)
                .is_some_and(|c| c.kind == Kind::Pin && c.pin.three_state);
            let next = match value.bit(bit) {
                Bit::Zero => Bit::One,
                Bit::One if tri => Bit::Unknown,
                _ => Bit::Zero,
            };
            let updated = Value::from_bits(
                (0..value.width()).map(|i| if i == bit { next } else { value.bit(i) }),
            );
            self.set_input_value(id, updated);
        }
    }
}

#[cfg(test)]
mod tests;

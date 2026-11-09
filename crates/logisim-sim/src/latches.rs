use crate::{Bit, Value};
use logisim_core::{Component, Kind, Latch, LatchEnable, LatchRole};

pub(super) fn evaluate(stored: &mut Value, c: &Component, ports: &[Value]) -> Value {
    let terminals = c.latch_terminals();
    let value = |role| {
        terminals
            .iter()
            .position(|(r, _)| *r == role)
            .map_or(Bit::Unknown, |i| ports[i].bit(0))
    };
    let active = if c.sequential.negate_controls {
        Bit::Zero
    } else {
        Bit::One
    };
    let enabled = match c.latch_enable {
        LatchEnable::High => value(LatchRole::Enable) == Bit::One,
        LatchEnable::Low => value(LatchRole::Enable) == Bit::Zero,
        LatchEnable::None => c.kind == Kind::Latch(Latch::Sr),
    };
    if value(LatchRole::Clear) == active {
        *stored = Value::known(1, 0);
    } else if value(LatchRole::Preset) == active {
        *stored = Value::known(1, 1);
    } else if enabled {
        let next = if c.kind == Kind::Latch(Latch::D) {
            match value(LatchRole::D) {
                Bit::Zero => Some(0),
                Bit::One => Some(1),
                _ => None,
            }
        } else {
            match (value(LatchRole::S), value(LatchRole::R)) {
                (Bit::One, Bit::Zero) => Some(1),
                (Bit::Zero, Bit::One) => Some(0),
                _ => None,
            }
        };
        if let Some(next) = next {
            *stored = Value::known(1, next);
        }
    }
    *stored
}

#[cfg(test)]
mod tests;

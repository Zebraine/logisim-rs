use crate::{Bit, Value};
use logisim_core::{Component, Kind, Ttl};

pub(crate) fn evaluate(c: &Component, ports: &[Value]) -> Vec<(usize, Value)> {
    let Kind::Ttl(kind) = c.kind else {
        return vec![];
    };
    let at = |pin| ports[c.ttl_index(pin).expect("TTL pin")].bit(0);
    let powered = !c.ttl.power_pins || (at(14) == Bit::One && at(7) == Bit::Zero);
    kind.gates()
        .iter()
        .map(|&[a, b, out]| {
            let bit = if powered {
                gate(kind, at(a), at(b))
            } else {
                Bit::Unknown
            };
            (c.ttl_index(out).expect("TTL output"), Value::filled(1, bit))
        })
        .collect()
}
fn gate(kind: Ttl, a: Bit, b: Bit) -> Bit {
    use Bit::{Error, One, Zero};
    let invert = |v| match v {
        Zero => One,
        One => Zero,
        _ => Error,
    };
    let and = || {
        if a == Zero || b == Zero {
            Zero
        } else if a == One && b == One {
            One
        } else {
            Error
        }
    };
    let or = || {
        if a == One || b == One {
            One
        } else if a == Zero && b == Zero {
            Zero
        } else {
            Error
        }
    };
    let xor = || match (a, b) {
        (Zero, Zero) | (One, One) => Zero,
        (Zero, One) | (One, Zero) => One,
        _ => Error,
    };
    match kind {
        Ttl::Ic7400 => invert(and()),
        Ttl::Ic7402 => invert(or()),
        Ttl::Ic7404 => invert(a),
        Ttl::Ic7408 => and(),
        Ttl::Ic7432 => or(),
        Ttl::Ic7486 => xor(),
        Ttl::Ic747266 => invert(xor()),
    }
}

#[cfg(test)]
mod tests;

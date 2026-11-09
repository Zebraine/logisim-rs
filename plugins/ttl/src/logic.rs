use crate::model::{Chip, Ttl};
use logisim_core::{Bit, Value};

pub(crate) fn evaluate(c: Chip, ports: &[Value]) -> Vec<(usize, Value)> {
    let kind = c.kind;
    let at = |pin| ports[c.index(pin)].bit(0);
    let powered = !c.power || (at(14) == Bit::One && at(7) == Bit::Zero);
    kind.gates()
        .iter()
        .map(|&[a, b, out]| {
            let bit = if powered {
                gate(kind, at(a), at(b))
            } else {
                Bit::Unknown
            };
            (c.index(out), Value::filled(1, bit))
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

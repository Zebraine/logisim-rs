use crate::{Error, Result};
use logisim_core::{Arithmetic, BitSearch, Component, Facing, Kind, Shift};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Arithmetic, key: &str) -> bool {
    key == "width"
        || (kind == Arithmetic::Comparator && key == "mode")
        || (kind == Arithmetic::Shifter && key == "shift")
        || (kind == Arithmetic::BitAdder && key == "inputs")
        || (kind == Arithmetic::BitFinder && key == "type")
}
pub(crate) fn read(c: &mut Component, attrs: &BTreeMap<String, String>) -> Result<()> {
    match c.kind {
        Kind::Arithmetic(Arithmetic::BitAdder) => {
            c.arithmetic.bit_inputs = crate::attributes::number(attrs, "inputs", 1, 1..=32)?;
        }
        Kind::Arithmetic(Arithmetic::BitFinder) => {
            c.arithmetic.search = match attrs.get("type").map(String::as_str) {
                None => BitSearch::LowestOne,
                Some("high1") => BitSearch::HighestOne,
                Some("low0") => BitSearch::LowestZero,
                Some("high0") => BitSearch::HighestZero,
                Some(v) => return Err(Error(format!("Unsupported bit search: {v}"))),
            };
        }
        Kind::Arithmetic(Arithmetic::Comparator) => {
            c.arithmetic.signed = match attrs.get("mode").map(String::as_str) {
                None => true,
                Some("unsigned") => false,
                Some(v) => return Err(Error(format!("Unsupported numeric type: {v}"))),
            };
        }
        Kind::Arithmetic(Arithmetic::Shifter) => {
            c.arithmetic.shift = match attrs.get("shift").map(String::as_str) {
                None => Shift::LogicalLeft,
                Some("lr") => Shift::LogicalRight,
                Some("ar") => Shift::ArithmeticRight,
                Some("rl") => Shift::RotateLeft,
                Some("rr") => Shift::RotateRight,
                Some(v) => return Err(Error(format!("Unsupported shift type: {v}"))),
            };
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute;
    let Kind::Arithmetic(kind) = c.kind else {
        return Ok(());
    };
    if c.facing != Facing::East {
        return Err(Error(
            "Arithmetic components have a fixed orientation".into(),
        ));
    }
    if kind == Arithmetic::BitAdder {
        if !(1..=32).contains(&c.arithmetic.bit_inputs) {
            return Err(Error("Bit Adder input count must be 1–32".into()));
        }
        attribute(out, "inputs", c.arithmetic.bit_inputs)?;
    }
    if kind == Arithmetic::BitFinder {
        let mode = match c.arithmetic.search {
            BitSearch::LowestOne => None,
            BitSearch::HighestOne => Some("high1"),
            BitSearch::LowestZero => Some("low0"),
            BitSearch::HighestZero => Some("high0"),
        };
        if let Some(mode) = mode {
            attribute(out, "type", mode)?;
        }
    }
    if kind == Arithmetic::Comparator && !c.arithmetic.signed {
        attribute(out, "mode", "unsigned")?;
    }
    if kind == Arithmetic::Shifter {
        let mode = match c.arithmetic.shift {
            Shift::LogicalLeft => None,
            Shift::LogicalRight => Some("lr"),
            Shift::ArithmeticRight => Some("ar"),
            Shift::RotateLeft => Some("rl"),
            Shift::RotateRight => Some("rr"),
        };
        if let Some(mode) = mode {
            attribute(out, "shift", mode)?;
        }
    }
    Ok(())
}

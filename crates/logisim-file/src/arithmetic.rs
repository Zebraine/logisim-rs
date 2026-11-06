use crate::{Error, Result};
use logisim_core::{Arithmetic, Component, Facing, Kind, Shift};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Arithmetic, key: &str) -> bool {
    key == "width"
        || (kind == Arithmetic::Comparator && key == "mode")
        || (kind == Arithmetic::Shifter && key == "shift")
}
pub(crate) fn read(c: &mut Component, attrs: &BTreeMap<String, String>) -> Result<()> {
    match c.kind {
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
    use crate::write::attribute;
    let Kind::Arithmetic(kind) = c.kind else {
        return Ok(());
    };
    if c.facing != Facing::East {
        return Err(Error(
            "Arithmetic components have a fixed orientation".into(),
        ));
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

use crate::{Error, Result};
use logisim_core::{Component, GateSize, Kind, Pull};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    if kind == Kind::PullResistor {
        matches!(key, "facing" | "pull")
    } else {
        matches!(
            key,
            "facing" | "width" | "control" | "negatecontrol" | "label" | "labelfont" | "labelcolor"
        ) || (kind == Kind::ControlledInverter && key == "size")
    }
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if c.kind == Kind::PullResistor {
        c.bus.pull = match a.get("pull").map(String::as_str) {
            None | Some("0") => Pull::Zero,
            Some("1") => Pull::One,
            Some("X") => Pull::Error,
            _ => return Err(Error("Unsupported pull direction".into())),
        };
    }
    if c.kind.is_controlled() {
        c.bus.control_left = match a.get("control").map(String::as_str) {
            None | Some("right") => false,
            Some("left") => true,
            _ => return Err(Error("Unsupported control location".into())),
        };
        c.bus.negate_control = match a.get("negatecontrol").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(Error("Invalid control inversion".into())),
        };
        c.gate_size = match a.get("size").map(String::as_str) {
            None | Some("20") => GateSize::Narrow,
            Some("30") if c.kind == Kind::ControlledInverter => GateSize::Wide,
            _ => return Err(Error("Unsupported controlled gate size".into())),
        };
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute as a;
    if c.kind == Kind::PullResistor {
        a(
            out,
            "pull",
            match c.bus.pull {
                Pull::Zero => "0",
                Pull::One => "1",
                Pull::Error => "X",
            },
        )?;
    }
    if c.kind.is_controlled() {
        a(
            out,
            "control",
            if c.bus.control_left { "left" } else { "right" },
        )?;
        a(out, "negatecontrol", c.bus.negate_control)?;
        if c.kind == Kind::ControlledInverter {
            if c.gate_size == GateSize::Medium {
                return Err(Error("Unsupported controlled gate size".into()));
            }
            a(out, "size", c.gate_width())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

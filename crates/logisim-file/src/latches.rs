use crate::{Error, Result};
use logisim_core::{Component, ControlPosition, Facing, Kind, Latch, LatchEnable};
use std::collections::BTreeMap;

pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "trigger"
            | "Pre/Clr Positions"
            | "NegatePresetClear"
            | "NewFFLayout"
            | "label"
            | "labelfont"
            | "labelcolor"
    )
}

pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !matches!(c.kind, Kind::Latch(_)) {
        return Ok(());
    }
    c.latch_enable = match a.get("trigger").map(String::as_str) {
        None | Some("high") => LatchEnable::High,
        Some("low") => LatchEnable::Low,
        Some("latch") if c.kind == Kind::Latch(Latch::Sr) => LatchEnable::None,
        _ => return Err(Error("Unsupported latch enable mode".into())),
    };
    c.sequential.controls = match a.get("Pre/Clr Positions").map(String::as_str) {
        None => ControlPosition::AboveBelow,
        Some("BELOW_ABOVE") => ControlPosition::BelowAbove,
        Some("LEGACY") => ControlPosition::Legacy,
        _ => return Err(Error("Unsupported latch preset/clear layout".into())),
    };
    for (key, target) in [
        ("NewFFLayout", &mut c.sequential.new_layout),
        ("NegatePresetClear", &mut c.sequential.negate_controls),
    ] {
        if let Some(v) = a.get(key) {
            *target = match v.as_str() {
                "true" => true,
                "false" => false,
                _ => return Err(Error(format!("Invalid latch {key}"))),
            };
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute as a;
    if !matches!(c.kind, Kind::Latch(_)) {
        return Ok(());
    }
    if c.bits != 1
        || c.facing != Facing::East
        || (c.kind == Kind::Latch(Latch::D) && c.latch_enable == LatchEnable::None)
    {
        return Err(Error(
            "Unsupported latch width, orientation or enable mode".into(),
        ));
    }
    match c.latch_enable {
        LatchEnable::High => {}
        LatchEnable::Low => a(out, "trigger", "low")?,
        LatchEnable::None => a(out, "trigger", "latch")?,
    }
    match c.sequential.controls {
        ControlPosition::AboveBelow => {}
        ControlPosition::BelowAbove => a(out, "Pre/Clr Positions", "BELOW_ABOVE")?,
        ControlPosition::Legacy => a(out, "Pre/Clr Positions", "LEGACY")?,
    }
    a(out, "NewFFLayout", c.sequential.new_layout)?;
    a(out, "NegatePresetClear", c.sequential.negate_controls)?;
    Ok(())
}

#[cfg(test)]
mod tests;

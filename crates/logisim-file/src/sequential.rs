use crate::{Error, Result};
use logisim_core::{Component, ControlPosition, Kind, Trigger};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    if matches!(key, "label" | "labelfont" | "labelcolor") {
        return true;
    }
    match kind {
        Kind::Clock => matches!(key, "facing" | "labelloc" | "highDuration" | "lowDuration"),
        Kind::Register => matches!(key, "width" | "trigger" | "Select"),
        Kind::FlipFlop(_) => matches!(
            key,
            "trigger" | "Pre/Clr Positions" | "NegatePresetClear" | "NewFFLayout"
        ),
        _ => false,
    }
}
pub(crate) fn read(c: &mut Component, attrs: &BTreeMap<String, String>) -> Result<()> {
    let get = |key, default| attrs.get(key).map(String::as_str).unwrap_or(default);
    let boolean = |key, default| match get(key, default) {
        "true" => Ok(true),
        "false" => Ok(false),
        v => Err(Error(format!("Invalid {key}: {v}"))),
    };
    let s = &mut c.sequential;
    if c.kind.is_storage() {
        s.trigger = match get("trigger", "rising") {
            "rising" => Trigger::Rising,
            "falling" => Trigger::Falling,
            "high" if c.kind == Kind::Register => Trigger::High,
            "low" if c.kind == Kind::Register => Trigger::Low,
            v => return Err(Error(format!("Unsupported trigger: {v}"))),
        };
    }
    match c.kind {
        Kind::Clock => {
            for (key, target) in [
                ("highDuration", &mut s.high_ticks),
                ("lowDuration", &mut s.low_ticks),
            ] {
                *target = get(key, "1")
                    .parse()
                    .ok()
                    .filter(|v| (1..=65_535).contains(v))
                    .ok_or_else(|| Error(format!("Unsupported {key}: expected 1–65535 ticks")))?;
            }
        }
        Kind::Register => {
            s.select_high = match get("Select", "low") {
                "low" => false,
                "high" => true,
                v => return Err(Error(format!("Unsupported chip select: {v}"))),
            }
        }
        Kind::FlipFlop(_) => {
            s.new_layout = boolean("NewFFLayout", "true")?;
            s.negate_controls = boolean("NegatePresetClear", "false")?;
            s.controls = match get(
                "Pre/Clr Positions",
                if s.new_layout { "" } else { "LEGACY" },
            ) {
                "" => ControlPosition::AboveBelow,
                "BELOW_ABOVE" => ControlPosition::BelowAbove,
                "LEGACY" => ControlPosition::Legacy,
                v => return Err(Error(format!("Unsupported preset/clear layout: {v}"))),
            };
            if !s.new_layout && s.controls != ControlPosition::Legacy {
                return Err(Error("Unverified legacy preset/clear layout".into()));
            }
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::write::attribute as a;
    let s = c.sequential;
    if c.kind.is_storage() {
        if c.facing != logisim_core::Facing::East
            || (matches!(c.kind, Kind::FlipFlop(_)) && c.bits != 1)
        {
            return Err(Error(
                "Unsupported sequential orientation or bit width".into(),
            ));
        }
        a(
            out,
            "trigger",
            match s.trigger {
                Trigger::Rising => "rising",
                Trigger::Falling => "falling",
                Trigger::High if c.kind == Kind::Register => "high",
                Trigger::Low if c.kind == Kind::Register => "low",
                _ => return Err(Error("Flip-flops require an edge trigger".into())),
            },
        )?;
    }
    match c.kind {
        Kind::Clock => {
            if c.bits != 1
                || !(1..=65_535).contains(&s.high_ticks)
                || !(1..=65_535).contains(&s.low_ticks)
            {
                return Err(Error("Invalid clock duration or width".into()));
            }
            a(out, "highDuration", s.high_ticks)?;
            a(out, "lowDuration", s.low_ticks)?;
            a(out, "labelloc", c.label_facing.name().to_ascii_lowercase())?;
        }
        Kind::Register => a(out, "Select", if s.select_high { "high" } else { "low" })?,
        Kind::FlipFlop(_) => {
            if !s.new_layout && s.controls != ControlPosition::Legacy {
                return Err(Error(
                    "Legacy flip-flops require the legacy control layout".into(),
                ));
            }
            if s.controls != ControlPosition::AboveBelow {
                a(
                    out,
                    "Pre/Clr Positions",
                    if s.controls == ControlPosition::BelowAbove {
                        "BELOW_ABOVE"
                    } else {
                        "LEGACY"
                    },
                )?;
            }
            a(out, "NewFFLayout", s.new_layout)?;
            a(out, "NegatePresetClear", s.negate_controls)?;
        }
        _ => {}
    }
    Ok(())
}

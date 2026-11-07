use crate::{Error, Result};
use logisim_core::{Component, ControlPosition, CounterOverflow, Kind, Trigger};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    if matches!(key, "label" | "labelfont" | "labelcolor") {
        return true;
    }
    match kind {
        Kind::Clock => matches!(key, "facing" | "labelloc" | "highDuration" | "lowDuration"),
        Kind::Random => matches!(key, "width" | "seed" | "trigger"),
        Kind::Counter => matches!(key, "width" | "max" | "ongoal" | "trigger" | "behavior"),
        Kind::ShiftRegister => matches!(key, "length" | "trigger"),
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
    if c.kind == Kind::ShiftRegister {
        c.bits = crate::read::number(attrs, "length", 8, 1..=32)?;
    }
    let s = &mut c.sequential;
    if c.kind.is_storage() && !matches!(c.kind, Kind::Latch(_)) {
        s.trigger = match get("trigger", "rising") {
            "rising" => Trigger::Rising,
            "falling" => Trigger::Falling,
            "high" if c.kind == Kind::Register => Trigger::High,
            "low" if c.kind == Kind::Register => Trigger::Low,
            v => return Err(Error(format!("Unsupported trigger: {v}"))),
        };
    }
    match c.kind {
        Kind::Random => {
            s.random_seed = get("seed", "0").parse().map_err(|_| {
                Error("Invalid random seed: expected a signed 32-bit integer".into())
            })?;
        }
        Kind::Counter => {
            let mask = u32::MAX >> (32 - c.bits);
            s.counter_max = match attrs.get("max") {
                None => mask,
                Some(v) => v
                    .strip_prefix("0x")
                    .and_then(|v| u32::from_str_radix(v, 16).ok())
                    .filter(|v| *v <= mask)
                    .ok_or_else(|| Error("Counter maximum exceeds its bit width".into()))?,
            };
            s.counter_old = match attrs.get("behavior").map(String::as_str) {
                None => false,
                Some("old") => true,
                Some(v) => return Err(Error(format!("Unsupported counter behavior: {v}"))),
            };
            s.counter_overflow = match attrs.get("ongoal").map(String::as_str) {
                None => CounterOverflow::Wrap,
                Some("stay") => CounterOverflow::Stay,
                Some("continue") => CounterOverflow::Continue,
                Some("load") => CounterOverflow::Load,
                Some(v) => return Err(Error(format!("Unsupported counter overflow: {v}"))),
            };
        }
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
    if c.kind.is_storage() && !matches!(c.kind, Kind::Latch(_)) {
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
                _ => return Err(Error("This component requires an edge trigger".into())),
            },
        )?;
    }
    match c.kind {
        Kind::ShiftRegister => a(out, "length", c.bits)?,
        Kind::Random => a(out, "seed", s.random_seed)?,
        Kind::Counter => {
            if s.counter_max > (u32::MAX >> (32 - c.bits)) {
                return Err(Error("Counter maximum exceeds its bit width".into()));
            }
            a(out, "max", format!("0x{:x}", s.counter_max))?;
            if s.counter_old {
                a(out, "behavior", "old")?;
            }
            if s.counter_overflow != CounterOverflow::Wrap {
                a(
                    out,
                    "ongoal",
                    match s.counter_overflow {
                        CounterOverflow::Stay => "stay",
                        CounterOverflow::Continue => "continue",
                        CounterOverflow::Load => "load",
                        CounterOverflow::Wrap => unreachable!(),
                    },
                )?;
            }
        }
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

#[cfg(test)]
mod random_tests {
    use super::*;
    use crate::{parse, serialize};
    use logisim_core::{Facing, Point};
    fn xml(attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="5" desc="#Memory"/><main name="main"/><circuit name="main"><comp lib="5" name="Random" loc="(200,200)">{attrs}</comp></circuit></project>"##
        )
    }
    #[test]
    fn random_native_defaults_signed_seeds_and_labels_roundtrip() {
        let default = parse(&xml("")).unwrap();
        assert!(default.warnings.is_empty());
        let c = &default.document.main_circuit().circuit.components[0];
        assert_eq!(c.kind, Kind::Random);
        assert_eq!(c.bits, 8);
        assert_eq!(c.sequential.random_seed, 0);
        for seed in [i32::MIN, -1, 0, 1, 12345, i32::MAX] {
            for bits in [1, 8, 32] {
                for trigger in ["rising", "falling"] {
                    let attrs = format!(
                        r##"<a name="seed" val="{seed}"/><a name="width" val="{bits}"/><a name="trigger" val="{trigger}"/><a name="label" val="用户 &amp; Random"/><a name="labelfont" val="Dialog plain 16"/><a name="labelcolor" val="#123456"/>"##
                    );
                    let imported = parse(&xml(&attrs)).unwrap();
                    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                    let c = &imported.document.main_circuit().circuit.components[0];
                    assert_eq!(c.sequential.random_seed, seed);
                    assert_eq!(c.bits, bits);
                    assert_eq!(c.label, "用户 & Random");
                    assert_eq!(c.color, [0x12, 0x34, 0x56]);
                    let saved = serialize(&imported.document).unwrap();
                    assert!(saved.contains("name=\"Random\""));
                    let again = parse(&saved).unwrap();
                    assert!(again.warnings.is_empty());
                    assert_eq!(saved, serialize(&again.document).unwrap());
                    assert_eq!(again.document.main_circuit().circuit.components[0], *c);
                }
            }
        }
    }
    #[test]
    fn random_unobserved_attributes_and_invalid_values_are_protected() {
        for (key, val) in [
            ("seed", "2147483648"),
            ("seed", "-2147483649"),
            ("seed", "0x10"),
            ("seed", "NaN"),
            ("width", "0"),
            ("width", "33"),
            ("trigger", "high"),
            ("trigger", "low"),
            ("facing", "west"),
            ("algorithm", "other"),
        ] {
            let imported = parse(&xml(&format!(r#"<a name="{key}" val="{val}"/>"#))).unwrap();
            assert!(!imported.warnings.is_empty(), "{key}={val}");
            assert!(
                imported
                    .document
                    .main_circuit()
                    .circuit
                    .components
                    .is_empty()
            );
        }
        for (facing, trigger) in [
            (Facing::West, Trigger::Rising),
            (Facing::East, Trigger::High),
        ] {
            let mut c = Component::new(Kind::Random, Point::ZERO);
            c.facing = facing;
            c.sequential.trigger = trigger;
            assert!(write(&mut String::new(), &c).is_err());
        }
    }
}

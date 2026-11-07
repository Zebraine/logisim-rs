use crate::{Error, Result};
use logisim_core::{Component, Kind, Plexer};
use std::collections::BTreeMap;
pub(crate) fn allowed(kind: Plexer, key: &str) -> bool {
    if kind == Plexer::BitSelector {
        return matches!(key, "facing" | "selloc" | "width" | "group");
    }
    if kind == Plexer::DisplayDecoder {
        return matches!(key, "facing" | "multibit");
    }
    matches!(key, "facing" | "select" | "disabled")
        || (kind == Plexer::Multiplexer && key == "size")
        || match kind {
            Plexer::PriorityEncoder => key == "noInputState",
            _ => {
                matches!(key, "selloc" | "enable")
                    || (kind.has_data_bits() && key == "width")
                    || (kind != Plexer::Multiplexer && key == "tristate")
            }
        }
}
pub(crate) fn read(c: &mut Component, attrs: &BTreeMap<String, String>) -> Result<()> {
    let Kind::Plexer(kind) = c.kind else {
        return Ok(());
    };
    if kind == Plexer::Multiplexer && attrs.get("size").is_some_and(|v| v != "30") {
        return Err(Error("Unsupported multiplexer size".into()));
    }
    let get = |key, default| attrs.get(key).map(String::as_str).unwrap_or(default);
    let boolean = |key, default| match get(key, default) {
        "true" => Ok(true),
        "false" => Ok(false),
        v => Err(Error(format!("Invalid {key}: {v}"))),
    };
    let zero = |key| match attrs.get(key).map(String::as_str) {
        None => Ok(false),
        Some("0") => Ok(true),
        Some(v) => Err(Error(format!("Unsupported {key}: {v}"))),
    };
    let p = &mut c.plexing;
    if kind == Plexer::DisplayDecoder {
        p.multibit = boolean("multibit", "true")?;
        return Ok(());
    }
    if kind == Plexer::BitSelector {
        p.group_bits = crate::read::number(attrs, "group", 1, 1..=32)?;
        p.top_right = match attrs.get("selloc").map(String::as_str) {
            None => false,
            Some("tr") => true,
            Some(v) => return Err(Error(format!("Unsupported select location: {v}"))),
        };
        return Ok(());
    }
    p.select_bits = get(
        "select",
        if kind == Plexer::PriorityEncoder {
            "3"
        } else {
            "1"
        },
    )
    .parse()
    .ok()
    .filter(|v| (1..=5).contains(v))
    .ok_or_else(|| Error("Select bits must be 1–5".into()))?;
    p.disabled_zero = zero("disabled")?;
    if kind == Plexer::PriorityEncoder {
        p.no_input_zero = zero("noInputState")?;
    } else {
        p.enable = boolean("enable", "true")?;
        p.top_right = match attrs.get("selloc").map(String::as_str) {
            None => false,
            Some("tr") => true,
            Some(v) => return Err(Error(format!("Unsupported select location: {v}"))),
        };
        if kind != Plexer::Multiplexer {
            p.three_state = boolean("tristate", "false")?;
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::write::attribute as a;
    let Kind::Plexer(kind) = c.kind else {
        return Ok(());
    };
    let p = c.plexing;
    if kind == Plexer::DisplayDecoder {
        if c.bits != 1 {
            return Err(Error("Display Decoder has fixed terminal widths".into()));
        }
        a(out, "multibit", p.multibit)?;
        return Ok(());
    }
    if kind == Plexer::BitSelector {
        if !(1..=32).contains(&p.group_bits) {
            return Err(Error("Bit Selector output bits must be 1–32".into()));
        }
        a(out, "group", p.group_bits)?;
        if p.top_right {
            a(out, "selloc", "tr")?;
        }
        return Ok(());
    }
    if !(1..=5).contains(&p.select_bits) || (!kind.has_data_bits() && c.bits != 1) {
        return Err(Error("Invalid plexer bit widths".into()));
    }
    a(out, "select", p.select_bits)?;
    if p.disabled_zero {
        a(out, "disabled", "0")?;
    }
    if kind == Plexer::PriorityEncoder {
        if p.no_input_zero {
            a(out, "noInputState", "0")?;
        }
    } else {
        a(out, "enable", p.enable)?;
        if p.top_right {
            a(out, "selloc", "tr")?;
        }
        if kind != Plexer::Multiplexer {
            a(out, "tristate", p.three_state)?;
        }
    }
    Ok(())
}

use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind, Trigger};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    matches!(key, "Char bit width" | "trigger")
        || (kind == Kind::Keyboard && key == "buflen")
        || (kind == Kind::Tty && matches!(key, "rows" | "cols" | "color" | "bg"))
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    c.bits = crate::attributes::number(a, "Char bit width", 7, 7..=16)?;
    if !matches!(c.bits, 7 | 16) {
        return Err(Error("Unsupported character width".into()));
    }
    c.terminal.trigger = match a.get("trigger").map(String::as_str) {
        None | Some("rising") => Trigger::Rising,
        Some("falling") => Trigger::Falling,
        _ => return Err(Error("Unsupported terminal trigger".into())),
    };
    if c.kind == Kind::Keyboard {
        c.terminal.buffer_len = match a.get("buflen") {
            None => 32,
            Some(s) => s
                .parse::<u16>()
                .ok()
                .filter(|v| (1..=256).contains(v))
                .ok_or_else(|| Error("Invalid keyboard buffer length".into()))?,
        };
    } else {
        c.terminal.rows = crate::attributes::number(a, "rows", 8, 1..=48)?;
        c.terminal.columns = crate::attributes::number(a, "cols", 32, 1..=120)?;
        for (key, color) in [("color", &mut c.io.on_color), ("bg", &mut c.io.background)] {
            if let Some(text) = a.get(key) {
                *color = crate::io::rgba(text)?;
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    if !matches!(c.bits, 7 | 16)
        || c.facing != Facing::East
        || !matches!(c.terminal.trigger, Trigger::Rising | Trigger::Falling)
        || (c.kind == Kind::Keyboard && !(1..=256).contains(&c.terminal.buffer_len))
        || (c.kind == Kind::Tty
            && (!(1..=48).contains(&c.terminal.rows) || !(1..=120).contains(&c.terminal.columns)))
    {
        return Err(Error("Invalid character terminal attributes".into()));
    }
    use crate::xml::attribute as a;
    if c.bits != 7 {
        a(out, "Char bit width", c.bits)?;
    }
    if c.terminal.trigger == Trigger::Falling {
        a(out, "trigger", "falling")?;
    }
    if c.kind == Kind::Keyboard {
        a(out, "buflen", c.terminal.buffer_len)?;
    } else {
        a(out, "rows", c.terminal.rows)?;
        a(out, "cols", c.terminal.columns)?;
        a(out, "color", crate::io::color_text(c.io.on_color))?;
        a(out, "bg", crate::io::color_text(c.io.background))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

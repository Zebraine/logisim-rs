use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind, MatrixInput, Radix};
use std::collections::BTreeMap;
pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    if kind.is_terminal() {
        return crate::terminal::allowed(kind, key);
    }
    if kind == Kind::Joystick {
        return matches!(key, "facing" | "bits" | "color" | "labelcolor");
    }
    (kind != Kind::RgbLed && key == "color")
        || (kind == Kind::RgbLed && key == "multibit")
        || (kind == Kind::DotMatrix
            && matches!(
                key,
                "inputtype" | "matrixcols" | "matrixrows" | "persist" | "dotshape"
            ))
        || (matches!(kind, Kind::Switch | Kind::Slider) && key == "width")
        || (kind == Kind::DipSwitch && key == "NSwitches")
        || (kind == Kind::Slider && matches!(key, "radix" | "Direction" | "value"))
        || (matches!(
            kind,
            Kind::Led | Kind::SevenSegment | Kind::HexDisplay | Kind::DotMatrix
        ) && key == "offcolor")
        || (matches!(kind, Kind::Led | Kind::SevenSegment) && key == "active")
        || (matches!(kind, Kind::SevenSegment | Kind::HexDisplay) && key == "bg")
        || (matches!(
            kind,
            Kind::Button | Kind::Switch | Kind::DipSwitch | Kind::Slider | Kind::Led | Kind::RgbLed
        ) && matches!(
            key,
            "facing" | "label" | "labelloc" | "labelfont" | "labelcolor"
        ))
}
pub(crate) fn rgba(text: &str) -> Result<[u8; 4]> {
    let hex = text
        .strip_prefix('#')
        .ok_or_else(|| Error("Invalid I/O color".into()))?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error("Invalid I/O color".into()));
    }
    let mut color = [255; 4];
    for (i, pair) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        color[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    Ok(color)
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !c.kind.is_io() {
        return Ok(());
    }
    if c.kind.is_terminal() {
        return crate::terminal::read(c, a);
    }
    c.bits = match c.kind {
        Kind::Switch | Kind::Slider => c.bits,
        Kind::Joystick => crate::attributes::number(a, "bits", 4, 2..=5)?,
        Kind::HexDisplay => 4,
        _ => 1,
    };
    if c.kind == Kind::Joystick
        && let Some(text) = a.get("labelcolor")
    {
        if text.len() != 7 {
            return Err(Error("Invalid joystick label color".into()));
        }
        let [r, g, b, _] = rgba(text)?;
        c.color = [r, g, b];
    }
    if c.kind == Kind::RgbLed {
        c.io.rgb_multibit = match a.get("multibit").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(Error("Invalid RGB input mode".into())),
        };
    }
    if c.kind == Kind::DotMatrix {
        c.io.matrix_columns = crate::attributes::number(a, "matrixcols", 5, 1..=32)?;
        c.io.matrix_rows = crate::attributes::number(a, "matrixrows", 7, 1..=32)?;
        c.io.matrix_input = match a.get("inputtype").map(String::as_str) {
            None | Some("column") => MatrixInput::Columns,
            Some("row") => MatrixInput::Rows,
            Some("select") => MatrixInput::Select,
            _ => return Err(Error("Unsupported matrix input format".into())),
        };
        c.io.matrix_circular = match a.get("dotshape").map(String::as_str) {
            None | Some("square") => false,
            Some("circle") => true,
            _ => return Err(Error("Unsupported matrix dot shape".into())),
        };
        c.io.matrix_persistence = match a.get("persist") {
            None => 0,
            Some(v) => v
                .parse::<u32>()
                .ok()
                .filter(|v| *v <= i32::MAX as u32)
                .ok_or_else(|| Error("Invalid matrix persistence".into()))?,
        };
    }
    if c.kind == Kind::DipSwitch {
        c.io.switch_count = crate::attributes::number(a, "NSwitches", 4, 1..=32)?;
    }
    if c.kind == Kind::Slider {
        c.io.slider_reverse = match a.get("Direction").map(String::as_str) {
            None => false,
            Some("right_to_left") => true,
            _ => return Err(Error("Unsupported slider direction".into())),
        };
        c.io.slider_value = match a.get("value") {
            None => 0,
            Some(v) => v
                .strip_prefix("0x")
                .and_then(|n| u32::from_str_radix(n, 16).ok())
                .filter(|v| *v <= c.slider_max())
                .ok_or_else(|| Error("Invalid slider value".into()))?,
        };
        c.radix = match a.get("radix").map(String::as_str) {
            None | Some("2") => Radix::Binary,
            Some("8") => Radix::Octal,
            Some("10signed") => Radix::Signed,
            Some("10unsigned") => Radix::Unsigned,
            Some("16") => Radix::Hex,
            _ => return Err(Error("Unsupported slider radix".into())),
        };
    }
    for (name, value) in [
        ("color", &mut c.io.on_color),
        ("offcolor", &mut c.io.off_color),
        ("bg", &mut c.io.background),
    ] {
        if let Some(text) = a.get(name) {
            *value = rgba(text)?;
        }
    }
    c.io.active_high = match a.get("active").map(String::as_str) {
        None | Some("true") => true,
        Some("false") => false,
        _ => return Err(Error("Invalid indicator polarity".into())),
    };
    Ok(())
}
pub(crate) fn color_text(color: [u8; 4]) -> String {
    let [r, g, b, a] = color;
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    if !c.kind.is_io() {
        return Ok(());
    }
    if c.kind.is_terminal() {
        return crate::terminal::write(out, c);
    }
    if (match c.kind {
        Kind::Switch | Kind::Slider => !(1..=32).contains(&c.bits),
        Kind::Joystick => !(2..=5).contains(&c.bits),
        Kind::HexDisplay => c.bits != 4,
        _ => c.bits != 1,
    }) || (!c.kind.rotatable() && c.facing != Facing::East)
    {
        return Err(Error("Invalid I/O width or orientation".into()));
    }
    use crate::xml::attribute as a;
    if c.kind == Kind::Joystick {
        a(out, "bits", c.bits)?;
        let [r, g, b] = c.color;
        a(out, "labelcolor", format!("#{r:02x}{g:02x}{b:02x}"))?;
    }
    if c.kind == Kind::DipSwitch {
        if !(1..=32).contains(&c.io.switch_count) {
            return Err(Error("Invalid switch count".into()));
        }
        a(out, "NSwitches", c.io.switch_count)?;
    }
    if c.kind == Kind::Slider {
        if c.io.slider_value > c.slider_max() {
            return Err(Error("Invalid slider value".into()));
        }
        if c.io.slider_reverse {
            a(out, "Direction", "right_to_left")?;
        }
        a(out, "value", format!("0x{:x}", c.io.slider_value))?;
        a(
            out,
            "radix",
            match c.radix {
                Radix::Binary => "2",
                Radix::Octal => "8",
                Radix::Signed => "10signed",
                Radix::Unsigned => "10unsigned",
                Radix::Hex => "16",
            },
        )?;
    }
    if c.kind == Kind::RgbLed {
        a(out, "multibit", c.io.rgb_multibit)?;
    } else {
        a(out, "color", color_text(c.io.on_color))?;
    }
    if c.kind == Kind::DotMatrix {
        if !(1..=32).contains(&c.io.matrix_columns)
            || !(1..=32).contains(&c.io.matrix_rows)
            || c.io.matrix_persistence > i32::MAX as u32
        {
            return Err(Error("Invalid matrix dimensions or persistence".into()));
        }
        a(out, "matrixcols", c.io.matrix_columns)?;
        a(out, "matrixrows", c.io.matrix_rows)?;
        match c.io.matrix_input {
            MatrixInput::Columns => {}
            MatrixInput::Rows => a(out, "inputtype", "row")?,
            MatrixInput::Select => a(out, "inputtype", "select")?,
        }
        if c.io.matrix_circular {
            a(out, "dotshape", "circle")?;
        }
        a(out, "persist", c.io.matrix_persistence)?;
    }
    if matches!(
        c.kind,
        Kind::Led | Kind::SevenSegment | Kind::HexDisplay | Kind::DotMatrix
    ) {
        a(out, "offcolor", color_text(c.io.off_color))?;
    }
    if matches!(c.kind, Kind::SevenSegment | Kind::HexDisplay) {
        a(out, "bg", color_text(c.io.background))?;
    }
    if matches!(c.kind, Kind::Led | Kind::SevenSegment) {
        a(out, "active", c.io.active_high)?;
    }
    if matches!(
        c.kind,
        Kind::Button | Kind::Switch | Kind::DipSwitch | Kind::Slider | Kind::Led | Kind::RgbLed
    ) {
        a(
            out,
            "labelloc",
            if c.kind == Kind::Button && c.label_center {
                "center".into()
            } else {
                c.label_facing.name().to_ascii_lowercase()
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

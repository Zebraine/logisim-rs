//! Font strings and package labels observed through runtime load/save.
use crate::{Error, Result, write::attribute};
use logisim_core::{ArtworkFont, CircuitLabel, Facing};

pub(crate) fn parse(text: &str) -> Result<ArtworkFont> {
    let (rest, size) = text
        .rsplit_once(' ')
        .ok_or_else(|| Error("Invalid font".into()))?;
    let (family, style) = rest
        .rsplit_once(' ')
        .ok_or_else(|| Error("Invalid font".into()))?;
    let (bold, italic) = match style {
        "plain" => (false, false),
        "bold" => (true, false),
        "italic" => (false, true),
        "bolditalic" => (true, true),
        _ => return Err(Error("Unsupported font style".into())),
    };
    let font = ArtworkFont {
        family: family.into(),
        size: size
            .parse()
            .map_err(|_| Error("Invalid font size".into()))?,
        bold,
        italic,
    };
    serialize(&font)?;
    Ok(font)
}
pub(crate) fn serialize(font: &ArtworkFont) -> Result<String> {
    if font.size == 0
        || font.family.trim().is_empty()
        || font.family.len() > 256
        || font.family.chars().any(char::is_control)
    {
        return Err(Error("Invalid font".into()));
    }
    let style = match (font.bold, font.italic) {
        (false, false) => "plain",
        (true, false) => "bold",
        (false, true) => "italic",
        (true, true) => "bolditalic",
    };
    Ok(format!("{} {style} {}", font.family, font.size))
}
pub(crate) fn read_label(label: &mut CircuitLabel, key: &str, val: &str) -> Result<()> {
    match key {
        "clabel" if val.len() <= 65_536 => label.text = val.into(),
        "clabelfont" => label.font = parse(val)?,
        "clabelup" => {
            label.up = match val {
                "east" => Facing::East,
                "south" => Facing::South,
                "west" => Facing::West,
                "north" => Facing::North,
                _ => return Err(Error("Unsupported circuit label direction".into())),
            }
        }
        "clabelcolor" if val.len() == 7 => {
            let c = crate::io::rgba(val)?;
            label.color = [c[0], c[1], c[2]];
        }
        _ => return Err(Error(format!("Unsupported circuit attribute {key}"))),
    }
    Ok(())
}
pub(crate) fn write_label(out: &mut String, label: &CircuitLabel) -> Result<()> {
    if label.text.len() > 65_536 {
        return Err(Error("Circuit label exceeds 64 KiB".into()));
    }
    if label == &CircuitLabel::default() {
        return Ok(());
    }
    attribute(out, "clabel", &label.text)?;
    attribute(out, "clabelfont", serialize(&label.font)?)?;
    attribute(out, "clabelup", label.up.name().to_ascii_lowercase())?;
    attribute(
        out,
        "clabelcolor",
        format!(
            "#{:02x}{:02x}{:02x}",
            label.color[0], label.color[1], label.color[2]
        ),
    )
}

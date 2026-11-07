use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind};
use std::collections::BTreeMap;
pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    key == "color"
        || (kind != Kind::Button && key == "offcolor")
        || (matches!(kind, Kind::Led | Kind::SevenSegment) && key == "active")
        || (matches!(kind, Kind::SevenSegment | Kind::HexDisplay) && key == "bg")
        || (matches!(kind, Kind::Button | Kind::Led)
            && matches!(
                key,
                "facing" | "label" | "labelloc" | "labelfont" | "labelcolor"
            ))
}
fn rgba(text: &str) -> Result<[u8; 4]> {
    let hex = text
        .strip_prefix('#')
        .ok_or_else(|| Error("Invalid I/O color".into()))?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error("Invalid I/O color".into()));
    }
    let mut color = [255; 4];
    for (i, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        color[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    Ok(color)
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !c.kind.is_io() {
        return Ok(());
    }
    c.bits = if c.kind == Kind::HexDisplay { 4 } else { 1 };
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
fn color_text(color: [u8; 4]) -> String {
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
    if c.bits != if c.kind == Kind::HexDisplay { 4 } else { 1 }
        || (!c.kind.rotatable() && c.facing != Facing::East)
    {
        return Err(Error("Invalid I/O width or orientation".into()));
    }
    use crate::write::attribute as a;
    a(out, "color", color_text(c.io.on_color))?;
    if c.kind != Kind::Button {
        a(out, "offcolor", color_text(c.io.off_color))?;
    }
    if matches!(c.kind, Kind::SevenSegment | Kind::HexDisplay) {
        a(out, "bg", color_text(c.io.background))?;
    }
    if matches!(c.kind, Kind::Led | Kind::SevenSegment) {
        a(out, "active", c.io.active_high)?;
    }
    if matches!(c.kind, Kind::Button | Kind::Led) {
        a(out, "labelloc", c.label_facing.name().to_ascii_lowercase())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Point};
    fn xml(kind: &str, attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="io" desc="#I/O"/><main name="main"/><circuit name="main"><comp lib="io" name="{kind}" loc="(200,100)">{attrs}</comp></circuit></project>"##
        )
    }
    #[test]
    fn io_defaults_colors_transparency_and_attributes_roundtrip() {
        for kind in [
            Kind::Button,
            Kind::Led,
            Kind::SevenSegment,
            Kind::HexDisplay,
        ] {
            let imported = parse(&xml(kind.name(), "")).unwrap();
            assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
            let c = &imported.document.main_circuit().circuit.components[0];
            let default = Component::new(kind, Point::new(200, 100));
            assert_eq!(c.io, default.io);
            assert_eq!(c.ports(), default.ports());
            assert_eq!(c.bits, default.bits);
            for facing in Facing::ALL {
                if !kind.rotatable() && facing != Facing::East {
                    continue;
                }
                for high in [true, false] {
                    let mut c = default.clone();
                    c.facing = facing;
                    c.io.on_color = [18, 171, 52, 128];
                    if kind != Kind::Button {
                        c.io.off_color = [10, 27, 44, 255];
                    }
                    if matches!(kind, Kind::Led | Kind::SevenSegment) {
                        c.io.active_high = high;
                    }
                    if matches!(kind, Kind::SevenSegment | Kind::HexDisplay) {
                        c.io.background = [16, 32, 48, 64];
                    }
                    if kind.has_label() {
                        c.label = "输入 & 输出".into();
                        c.label_facing = facing;
                        c.font_size = 18;
                        c.color = [1, 2, 3];
                    }
                    let mut circuit = Circuit::default();
                    let id = circuit.add_component(c);
                    let document = Document::single("main", circuit);
                    let text = serialize(&document).unwrap();
                    assert!(text.contains("#12ab3480"));
                    assert!(!text.contains("name=\"width\""));
                    let imported = parse(&text).unwrap();
                    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                    let mut after = imported.document.main_circuit().circuit.components[0].clone();
                    after.id = id;
                    assert_eq!(
                        &after,
                        document.main_circuit().circuit.component(id).unwrap()
                    );
                    assert_eq!(serialize(&imported.document).unwrap(), text);
                }
            }
        }
    }
    #[test]
    fn unobserved_io_options_invalid_colors_and_widths_are_protected() {
        for (kind, attr) in [
            ("Button", r##"<a name="pressed" val="true"/>"##),
            ("Button", r##"<a name="active" val="false"/>"##),
            ("LED", r##"<a name="width" val="8"/>"##),
            ("LED", r##"<a name="active" val="unknown"/>"##),
            ("LED", r##"<a name="color" val="#11223"/>"##),
            ("LED", r##"<a name="offcolor" val="#xxxxxx"/>"##),
            ("7-Segment Display", r##"<a name="facing" val="north"/>"##),
            (
                "7-Segment Display",
                r##"<a name="label" val="unobserved"/>"##,
            ),
            ("Hex Digit Display", r##"<a name="active" val="false"/>"##),
            ("Hex Digit Display", r##"<a name="bg" val="#1234567890"/>"##),
        ] {
            let imported = parse(&xml(kind, attr)).unwrap();
            assert!(!imported.warnings.is_empty(), "{kind} {attr}");
            assert!(
                imported
                    .document
                    .main_circuit()
                    .circuit
                    .components
                    .is_empty()
            );
        }
        let mut c = Component::new(Kind::SevenSegment, Point::ZERO);
        c.facing = Facing::North;
        assert!(write(&mut String::new(), &c).is_err());
        c.facing = Facing::East;
        c.bits = 8;
        assert!(write(&mut String::new(), &c).is_err());
    }
}

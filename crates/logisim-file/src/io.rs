use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind, MatrixInput, Radix};
use std::collections::BTreeMap;
pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
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
fn rgba(text: &str) -> Result<[u8; 4]> {
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
    c.bits = match c.kind {
        Kind::Switch | Kind::Slider => c.bits,
        Kind::HexDisplay => 4,
        _ => 1,
    };
    if c.kind == Kind::RgbLed {
        c.io.rgb_multibit = match a.get("multibit").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(Error("Invalid RGB input mode".into())),
        };
    }
    if c.kind == Kind::DotMatrix {
        c.io.matrix_columns = crate::read::number(a, "matrixcols", 5, 1..=32)?;
        c.io.matrix_rows = crate::read::number(a, "matrixrows", 7, 1..=32)?;
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
        c.io.switch_count = crate::read::number(a, "NSwitches", 4, 1..=32)?;
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
    if (match c.kind {
        Kind::Switch | Kind::Slider => !(1..=32).contains(&c.bits),
        Kind::HexDisplay => c.bits != 4,
        _ => c.bits != 1,
    }) || (!c.kind.rotatable() && c.facing != Facing::East)
    {
        return Err(Error("Invalid I/O width or orientation".into()));
    }
    use crate::write::attribute as a;
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

#[cfg(test)]
mod input_tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Point};
    fn xml(kind: &str, attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="io" desc="#I/O"/><main name="main"/><circuit name="main"><comp lib="io" name="{kind}" loc="(200,100)">{attrs}</comp></circuit></project>"##
        )
    }
    #[test]
    fn input_defaults_and_observed_attributes_roundtrip_without_runtime_state() {
        for (kind, name, bits) in [
            (Kind::Switch, "Switch", 1),
            (Kind::DipSwitch, "DipSwitch", 1),
            (Kind::Slider, "Slider", 8),
        ] {
            let i = parse(&xml(name, "")).unwrap();
            assert!(i.warnings.is_empty(), "{:?}", i.warnings);
            let c = &i.document.main_circuit().circuit.components[0];
            assert_eq!(c.bits, bits);
            assert_eq!(c.io, Component::new(kind, Point::ZERO).io);
            for facing in Facing::ALL {
                for n in [1, 4, 32] {
                    for radix in Radix::ALL {
                        for reverse in [false, true] {
                            let mut c = Component::new(kind, Point::new(200, 100));
                            c.facing = facing;
                            c.label_facing = facing;
                            c.label = "输入 & Input".into();
                            c.font_size = 18;
                            c.color = [24, 53, 70];
                            c.io.on_color = [18, 52, 86, 128];
                            match kind {
                                Kind::DipSwitch => c.io.switch_count = n,
                                Kind::Slider => {
                                    c.bits = n;
                                    c.radix = radix;
                                    c.io.slider_reverse = reverse;
                                    c.io.slider_value = c.slider_max();
                                }
                                _ => c.bits = n,
                            }
                            let mut circuit = Circuit::default();
                            let id = circuit.add_component(c);
                            let d = Document::single("main", circuit);
                            let s = serialize(&d).unwrap();
                            assert!(s.contains("#12345680"));
                            assert!(!s.contains("offcolor"));
                            assert!(!s.contains("pressed"));
                            if kind == Kind::DipSwitch {
                                assert!(!s.contains("name=\"width\""));
                                assert!(s.contains("NSwitches"));
                            }
                            if kind == Kind::Slider {
                                assert_eq!(s.contains("right_to_left"), reverse);
                            } else {
                                assert!(!s.contains("name=\"value\""));
                            }
                            let i = parse(&s).unwrap();
                            assert!(i.warnings.is_empty(), "{:?}", i.warnings);
                            let mut after = i.document.main_circuit().circuit.components[0].clone();
                            after.id = id;
                            assert_eq!(&after, d.main_circuit().circuit.component(id).unwrap());
                            assert_eq!(serialize(&i.document).unwrap(), s);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn unsupported_input_options_and_invalid_ranges_protect_originals() {
        for (kind, attrs) in [
            ("Switch", r##"<a name="state" val="1"/>"##),
            ("Switch", r##"<a name="width" val="33"/>"##),
            ("DipSwitch", r##"<a name="NSwitches" val="0"/>"##),
            ("DipSwitch", r##"<a name="NSwitches" val="33"/>"##),
            ("DipSwitch", r##"<a name="width" val="4"/>"##),
            ("Slider", r##"<a name="value" val="0x100"/>"##),
            ("Slider", r##"<a name="value" val="-1"/>"##),
            ("Slider", r##"<a name="Direction" val="unobserved"/>"##),
            ("Slider", r##"<a name="radix" val="unknown"/>"##),
            ("Slider", r##"<a name="offcolor" val="#000000"/>"##),
        ] {
            let i = parse(&xml(kind, attrs)).unwrap();
            assert!(!i.warnings.is_empty(), "{kind} {attrs}");
            assert!(i.document.main_circuit().circuit.components.is_empty());
        }
        let mut c = Component::new(Kind::Slider, Point::ZERO);
        c.io.slider_value = 256;
        assert!(write(&mut String::new(), &c).is_err());
        c = Component::new(Kind::DipSwitch, Point::ZERO);
        c.io.switch_count = 33;
        assert!(write(&mut String::new(), &c).is_err());
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Point};
    fn xml(kind: &str, attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="io" desc="#I/O"/><main name="main"/><circuit name="main"><comp lib="io" name="{kind}" loc="(200,100)">{attrs}</comp></circuit></project>"##
        )
    }
    #[test]
    fn display_defaults_and_all_modes_roundtrip_without_runtime_lights() {
        for (kind, name) in [(Kind::RgbLed, "RGBLed"), (Kind::DotMatrix, "DotMatrix")] {
            let loaded = parse(&xml(name, "")).unwrap();
            assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
            let original = Component::new(kind, Point::new(200, 100));
            let read = &loaded.document.main_circuit().circuit.components[0];
            assert_eq!(read.io, original.io);
            assert_eq!(read.facing, original.facing);
            for mode in MatrixInput::ALL {
                for n in [1, 5, 32] {
                    for facing in Facing::ALL {
                        if !kind.rotatable() && facing != Facing::East {
                            continue;
                        }
                        for flag in [false, true] {
                            let mut c = original.clone();
                            if kind == Kind::RgbLed {
                                c.facing = facing;
                                c.io.rgb_multibit = flag;
                                c.label = "红 & Green".into();
                                c.label_facing = facing;
                                c.font_size = 18;
                                c.color = [1, 2, 3];
                            } else {
                                c.io.matrix_input = mode;
                                c.io.matrix_columns = n;
                                c.io.matrix_rows = 33 - n;
                                c.io.matrix_circular = flag;
                                c.io.matrix_persistence = 3;
                                c.io.on_color = [18, 52, 86, 128];
                                c.io.off_color = [171, 205, 239, 64];
                            }
                            let mut circuit = Circuit::default();
                            let id = circuit.add_component(c);
                            let document = Document::single("main", circuit);
                            let text = serialize(&document).unwrap();
                            assert!(!text.contains("name=\"width\""));
                            let read = parse(&text).unwrap();
                            assert!(read.warnings.is_empty(), "{:?}", read.warnings);
                            let mut c = read.document.main_circuit().circuit.components[0].clone();
                            c.id = id;
                            assert_eq!(&c, document.main_circuit().circuit.component(id).unwrap());
                            assert_eq!(serialize(&read.document).unwrap(), text);
                        }
                    }
                }
            }
        }
        let explicit = parse(&xml(
            "DotMatrix",
            r#"<a name="inputtype" val="column"/><a name="dotshape" val="square"/>"#,
        ))
        .unwrap();
        assert!(explicit.warnings.is_empty());
    }
    #[test]
    fn unknown_display_options_and_invalid_ranges_are_reported_without_silent_defaults() {
        for (kind, attrs) in [
            ("RGBLed", r#"<a name="multibit" val="unknown"/>"#),
            ("RGBLed", r#"<a name="width" val="8"/>"#),
            ("RGBLed", r##"<a name="color" val="#112233"/>"##),
            ("DotMatrix", r#"<a name="inputtype" val="col"/>"#),
            ("DotMatrix", r#"<a name="dotshape" val="triangle"/>"#),
            ("DotMatrix", r#"<a name="matrixcols" val="0"/>"#),
            ("DotMatrix", r#"<a name="matrixrows" val="33"/>"#),
            ("DotMatrix", r#"<a name="persist" val="-1"/>"#),
            ("DotMatrix", r#"<a name="persist" val="4294967295"/>"#),
            ("DotMatrix", r#"<a name="facing" val="west"/>"#),
            ("DotMatrix", r#"<a name="label" val="unsupported"/>"#),
        ] {
            let read = parse(&xml(kind, attrs)).unwrap();
            assert!(!read.warnings.is_empty(), "{kind}: {attrs}");
            assert!(read.document.main_circuit().circuit.components.is_empty());
        }
        for n in [0, 33] {
            let mut c = Component::new(Kind::DotMatrix, Point::ZERO);
            c.io.matrix_rows = n;
            assert!(write(&mut String::new(), &c).is_err());
        }
    }
}

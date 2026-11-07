use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind, Pla, PlaInput};
use std::collections::BTreeMap;

pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "inputs" | "and" | "outputs" | "Contents" | "Select" | "label" | "labelfont" | "labelcolor"
    )
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if c.kind != Kind::PlaRom {
        return Ok(());
    }
    let inputs = crate::read::number(a, "inputs", 4, 1..=32)?;
    let terms = crate::read::number(a, "and", 4, 1..=32)?;
    let outputs = crate::read::number(a, "outputs", 4, 1..=32)?;
    c.pla = Pla::new(inputs, terms, outputs);
    c.pla.select_high = match a.get("Select").map(String::as_str) {
        None | Some("low") => false,
        Some("high") => true,
        _ => return Err(Error("Unsupported PLA select polarity".into())),
    };
    if let Some(text) = a.get("Contents") {
        let invalid = || Error("Invalid PLA connection table or dimensions".into());
        let input_len = usize::from(inputs) * usize::from(terms);
        let total = input_len + usize::from(outputs) * usize::from(terms);
        let mut cells = Vec::with_capacity(total);
        for token in text.split_whitespace() {
            let (value, count) = token.split_once('*').unwrap_or((token, "1"));
            if !value.bytes().all(|v| v.is_ascii_digit())
                || !count.bytes().all(|v| v.is_ascii_digit())
            {
                return Err(invalid());
            }
            let value = value
                .parse::<u8>()
                .ok()
                .filter(|v| *v <= 2)
                .ok_or_else(invalid)?;
            let count = count
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0 && *n <= total - cells.len())
                .ok_or_else(invalid)?;
            cells.extend(std::iter::repeat_n(value, count));
        }
        if cells.len() != total {
            return Err(invalid());
        }
        for row in 0..terms {
            for col in 0..inputs {
                c.pla.set_input(
                    row,
                    col,
                    match cells[usize::from(row) * usize::from(inputs) + usize::from(col)] {
                        0 => PlaInput::Disconnected,
                        1 => PlaInput::Inverted,
                        2 => PlaInput::Direct,
                        _ => unreachable!(),
                    },
                );
            }
            for col in 0..outputs {
                let value =
                    cells[input_len + usize::from(row) * usize::from(outputs) + usize::from(col)];
                if value > 1 {
                    return Err(invalid());
                }
                c.pla.set_output(row, col, value == 1);
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    if c.kind != Kind::PlaRom {
        return Ok(());
    }
    if c.facing != Facing::East {
        return Err(Error("Unsupported PLA orientation".into()));
    }
    use crate::write::attribute as a;
    let (inputs, terms, outputs) = c.pla.dimensions();
    for (name, value) in [("inputs", inputs), ("and", terms), ("outputs", outputs)] {
        a(out, name, value)?;
    }
    if c.pla.select_high {
        a(out, "Select", "high")?;
    }
    let input: Vec<_> = (0..terms)
        .flat_map(|row| {
            (0..inputs).map(move |col| match c.pla.input(row, col) {
                PlaInput::Disconnected => 0,
                PlaInput::Inverted => 1,
                PlaInput::Direct => 2,
            })
        })
        .collect();
    let output: Vec<_> = (0..terms)
        .flat_map(|row| (0..outputs).map(move |col| u8::from(c.pla.output(row, col))))
        .collect();
    if input.iter().chain(&output).any(|v| *v != 0) {
        let mut text = String::new();
        for plane in [&input, &output] {
            let mut start = 0;
            while start < plane.len() {
                let value = plane[start];
                let count = plane[start..].iter().take_while(|v| **v == value).count();
                if count >= 3 {
                    text.push_str(&format!("{value}*{count} "));
                } else {
                    for _ in 0..count {
                        text.push_str(&format!("{value} "));
                    }
                }
                start += count;
            }
        }
        a(out, "Contents", text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Point};
    fn xml(attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="5" desc="#Memory"/><main name="main"/><circuit name="main"><comp lib="5" name="PlaRom" loc="(300,250)">{attrs}</comp></circuit></project>"##
        )
    }
    #[test]
    fn native_defaults_and_value_times_count_programs_roundtrip() {
        for attrs in [
            "",
            r#"<a name="Contents" val="2 0*4 2 0*10 1 0*4 1 0*10 "/>"#,
            r#"<a name="inputs" val="2"/><a name="and" val="2"/><a name="outputs" val="2"/><a name="Contents" val="2 0*3 0*4 "/><a name="Select" val="high"/>"#,
        ] {
            let imported = parse(&xml(attrs)).unwrap();
            assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
            let before = &imported.document.main_circuit().circuit.components[0];
            assert_eq!(before.kind, Kind::PlaRom);
            if attrs.contains("0*10") {
                assert_eq!(before.pla.input(0, 0), PlaInput::Direct);
                assert_eq!(before.pla.input(1, 1), PlaInput::Direct);
                assert_eq!(before.pla.input(3, 3), PlaInput::Disconnected);
                assert!(before.pla.output(0, 0) && before.pla.output(1, 1));
                assert!(!before.pla.output(3, 3));
            }
            let text = serialize(&imported.document).unwrap();
            assert!(text.contains("name=\"PlaRom\""));
            let again = parse(&text).unwrap();
            assert!(again.warnings.is_empty());
            assert_eq!(before, &again.document.main_circuit().circuit.components[0]);
            assert_eq!(serialize(&again.document).unwrap(), text);
        }
    }
    #[test]
    fn maximum_tables_and_unicode_labels_keep_every_connection() {
        for size in [1, 32] {
            let mut c = Component::new(Kind::PlaRom, Point::new(300, 250));
            c.pla.resize(size, size, size);
            c.pla.select_high = true;
            c.label = "逻辑 & <PLA>".into();
            c.color = [0x12, 0x34, 0x56];
            c.font_size = 16;
            for row in 0..size {
                for col in 0..size {
                    c.pla.set_input(
                        row,
                        col,
                        match (row + col) % 3 {
                            0 => PlaInput::Disconnected,
                            1 => PlaInput::Direct,
                            _ => PlaInput::Inverted,
                        },
                    );
                    c.pla.set_output(row, col, (row + col) % 2 == 0);
                }
            }
            let mut circuit = Circuit::default();
            circuit.add_component(c);
            let doc = Document::single("main", circuit);
            let text = serialize(&doc).unwrap();
            let import = parse(&text).unwrap();
            assert!(import.warnings.is_empty());
            assert_eq!(
                doc.main_circuit().circuit.components,
                import.document.main_circuit().circuit.components
            );
            let component = text.split("<comp ").nth(1).unwrap();
            assert!(!component.contains("name=\"width\""));
            assert!(!component.contains("name=\"facing\""));
        }
    }
    #[test]
    fn unknown_and_malformed_programs_are_omitted_with_warnings() {
        for attrs in [
            r#"<a name="inputs" val="0"/>"#,
            r#"<a name="and" val="33"/>"#,
            r#"<a name="outputs" val="33"/>"#,
            r#"<a name="Select" val="rising"/>"#,
            r#"<a name="facing" val="north"/>"#,
            r#"<a name="contents" val="0*32"/>"#,
            r#"<a name="Contents" val=""/>"#,
            r#"<a name="Contents" val="0*31"/>"#,
            r#"<a name="Contents" val="0*33"/>"#,
            r#"<a name="Contents" val="3 0*31"/>"#,
            r#"<a name="Contents" val="0*16 2 0*15"/>"#,
            r#"<a name="Contents" val="0*0 0*32"/>"#,
            r#"<a name="Contents" val="0*999999999999999999999999999999"/>"#,
            r#"<a name="Contents" val="+0*32"/>"#,
            r#"<a name="Contents" val="0*-1"/>"#,
            r#"<a name="Contents" val="0*32*1"/>"#,
        ] {
            let import = parse(&xml(attrs)).unwrap();
            assert!(!import.warnings.is_empty(), "{attrs}");
            assert!(
                import.document.main_circuit().circuit.components.is_empty(),
                "{attrs}"
            );
        }
        let mut c = Component::new(Kind::PlaRom, Point::ZERO);
        c.facing = Facing::North;
        let mut circuit = Circuit::default();
        circuit.add_component(c);
        assert!(serialize(&Document::single("main", circuit)).is_err());
    }
}

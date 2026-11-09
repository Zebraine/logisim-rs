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
        c.font.size = 16;
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

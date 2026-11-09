use super::super::*;
use crate::{Document, parse, serialize};
use logisim_core::{Circuit, Point};
fn xml(kind: &str, attrs: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="io" desc="#I/O"/><main name="main"/><circuit name="main"><comp lib="io" name="{kind}" loc="(200,100)">{attrs}</comp></circuit></project>"##
    )
}
#[test]
fn joystick_native_attributes_colors_ports_and_roundtrip() {
    let imported = parse(&xml("Joystick", "")).unwrap();
    assert!(imported.warnings.is_empty());
    let c = &imported.document.main_circuit().circuit.components[0];
    assert_eq!(c.bits, 4);
    assert_eq!(c.io.on_color, [255, 0, 0, 255]);
    assert_eq!(c.color, [0; 3]);
    for facing in Facing::ALL {
        for bits in 2..=5 {
            let source = xml(
                "Joystick",
                &format!(
                    r##"<a name="facing" val="{}"/><a name="bits" val="{bits}"/><a name="color" val="#12345680"/><a name="labelcolor" val="#234567"/>"##,
                    facing.name().to_ascii_lowercase()
                ),
            );
            let imported = parse(&source).unwrap();
            assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
            let c = &imported.document.main_circuit().circuit.components[0];
            assert_eq!((c.bits, c.facing), (bits, facing));
            assert_eq!(c.io.on_color, [18, 52, 86, 128]);
            assert_eq!(c.color, [35, 69, 103]);
            let saved = serialize(&imported.document).unwrap();
            assert!(!saved.contains("name=\"width\""));
            assert!(!saved.contains("name=\"label\""));
            assert!(!saved.contains("name=\"value\""));
            let again = parse(&saved).unwrap();
            assert!(again.warnings.is_empty());
            assert_eq!(
                again.document.main_circuit().circuit,
                imported.document.main_circuit().circuit
            );
        }
    }
}
#[test]
fn unknown_joystick_modes_are_reported_and_invalid_width_cannot_be_saved() {
    for attrs in [
        r#"<a name="bits" val="1"/>"#,
        r#"<a name="bits" val="6"/>"#,
        r#"<a name="width" val="4"/>"#,
        r#"<a name="label" val="unimplemented"/>"#,
        r#"<a name="value" val="0"/>"#,
        r##"<a name="color" val="#12345"/>"##,
        r##"<a name="labelcolor" val="#23456780"/>"##,
    ] {
        let imported = parse(&xml("Joystick", attrs)).unwrap();
        assert!(!imported.warnings.is_empty(), "{attrs}");
        assert!(
            imported
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
    }
    for bits in [0, 1, 6, 32] {
        let mut c = Component::new(Kind::Joystick, Point::ZERO);
        c.bits = bits;
        assert!(write(&mut String::new(), &c).is_err());
    }
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
                    c.font.size = 18;
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

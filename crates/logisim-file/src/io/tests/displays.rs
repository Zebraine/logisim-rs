use super::super::*;
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
                            c.font.size = 18;
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

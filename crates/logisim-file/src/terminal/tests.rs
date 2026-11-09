use super::*;
use crate::{parse, serialize};
fn xml(kind: &str, attrs: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="io" desc="#I/O"/><main name="main"/><circuit name="main"><comp lib="io" name="{kind}" loc="(200,100)">{attrs}</comp></circuit></project>"##
    )
}
#[test]
fn terminal_native_defaults_limits_colors_and_roundtrip() {
    for kind in ["Keyboard", "TTY"] {
        let imported = parse(&xml(kind, "")).unwrap();
        assert!(imported.warnings.is_empty());
        let c = &imported.document.main_circuit().circuit.components[0];
        assert_eq!(c.bits, 7);
        assert_eq!(c.terminal, Default::default());
        for bits in [7, 16] {
            for trigger in ["rising", "falling"] {
                for maximum in [false, true] {
                    let attrs = format!(
                        r##"<a name="Char bit width" val="{bits}"/><a name="trigger" val="{trigger}"/>{}"##,
                        if kind == "Keyboard" {
                            format!(
                                r#"<a name="buflen" val="{}"/>"#,
                                if maximum { 256 } else { 1 }
                            )
                        } else {
                            format!(
                                r##"<a name="rows" val="{}"/><a name="cols" val="{}"/><a name="color" val="#33669980"/><a name="bg" val="#aaccee40"/>"##,
                                if maximum { 48 } else { 1 },
                                if maximum { 120 } else { 1 }
                            )
                        }
                    );
                    let imported = parse(&xml(kind, &attrs)).unwrap();
                    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                    let saved = serialize(&imported.document).unwrap();
                    for forbidden in ["width", "facing", "label", "value", "contents"] {
                        assert!(
                            !saved
                                .split("<circuit ")
                                .nth(1)
                                .unwrap()
                                .contains(&format!("name=\"{forbidden}\"")),
                            "{forbidden}: {saved}"
                        );
                    }
                    let again = parse(&saved).unwrap();
                    assert!(again.warnings.is_empty());
                    assert_eq!(serialize(&again.document).unwrap(), saved);
                    assert_eq!(
                        again.document.main_circuit().circuit,
                        imported.document.main_circuit().circuit
                    );
                }
            }
        }
    }
}
#[test]
fn unsupported_terminal_attributes_are_omitted_and_bad_model_cannot_save() {
    for (kind, attrs) in [
        ("Keyboard", r#"<a name="buflen" val="0"/>"#),
        ("Keyboard", r#"<a name="buflen" val="257"/>"#),
        ("Keyboard", r#"<a name="Char bit width" val="8"/>"#),
        ("Keyboard", r#"<a name="trigger" val="high"/>"#),
        ("Keyboard", r#"<a name="label" val="lost"/>"#),
        ("Keyboard", r#"<a name="buffer" val="abc"/>"#),
        ("TTY", r#"<a name="rows" val="0"/>"#),
        ("TTY", r#"<a name="rows" val="49"/>"#),
        ("TTY", r#"<a name="cols" val="121"/>"#),
        ("TTY", r#"<a name="cols" val="0"/>"#),
        ("TTY", r##"<a name="bg" val="#123"/>"##),
        ("TTY", r#"<a name="facing" val="west"/>"#),
    ] {
        let imported = parse(&xml(kind, attrs)).unwrap();
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
    let mut c = Component::new(Kind::Tty, logisim_core::Point::ZERO);
    for invalid in 0..4 {
        c.bits = 7;
        c.facing = Facing::East;
        c.terminal = Default::default();
        match invalid {
            0 => c.bits = 8,
            1 => c.facing = Facing::West,
            2 => c.terminal.rows = 49,
            _ => c.terminal.trigger = Trigger::High,
        }
        assert!(write(&mut String::new(), &c).is_err());
    }
}

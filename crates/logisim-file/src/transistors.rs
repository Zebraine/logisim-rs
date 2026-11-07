use crate::{Error, Result};
use logisim_core::{Component, Kind, TransistorType};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    matches!(key, "facing" | "width" | "gate") || (kind == Kind::Transistor && key == "type")
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !c.kind.is_transistor() {
        return Ok(());
    }
    c.transistor.gate_bottom_right = match a.get("gate").map(String::as_str) {
        None | Some("tl") => false,
        Some("br") => true,
        _ => return Err(Error("Unsupported transistor gate location".into())),
    };
    if c.kind == Kind::Transistor {
        c.transistor.transistor_type = match a.get("type").map(String::as_str) {
            None | Some("p") => TransistorType::P,
            Some("n") => TransistorType::N,
            _ => return Err(Error("Unsupported transistor type".into())),
        };
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::write::attribute as a;
    if !c.kind.is_transistor() {
        return Ok(());
    }
    a(
        out,
        "gate",
        if c.transistor.gate_bottom_right {
            "br"
        } else {
            "tl"
        },
    )?;
    if c.kind == Kind::Transistor {
        a(
            out,
            "type",
            match c.transistor.transistor_type {
                TransistorType::P => "p",
                TransistorType::N => "n",
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Facing, Point};
    fn xml(kind: Kind, attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="w" desc="#Wiring"/><main name="main"/><circuit name="main"><comp lib="w" name="{}" loc="(200,100)">{attrs}</comp></circuit></project>"##,
            kind.name()
        )
    }
    #[test]
    fn native_transistor_defaults_and_attributes_roundtrip() {
        for kind in [Kind::Transistor, Kind::TransmissionGate] {
            let default = parse(&xml(kind, "")).unwrap();
            assert!(default.warnings.is_empty());
            let default = &default.document.main_circuit().circuit.components[0];
            assert_eq!(default.transistor, Default::default());
            assert_eq!(default.facing, Facing::East);
            assert_eq!(default.bits, 1);
            let raw = parse(&xml(kind, if kind == Kind::Transistor { r#"<a name="type" val="n"/><a name="facing" val="south"/><a name="width" val="32"/><a name="gate" val="br"/>"# } else { r#"<a name="facing" val="south"/><a name="width" val="32"/><a name="gate" val="br"/>"# })).unwrap();
            assert!(raw.warnings.is_empty());
            let c = &raw.document.main_circuit().circuit.components[0];
            assert!(c.transistor.gate_bottom_right);
            assert_eq!(c.facing, Facing::South);
            assert_eq!(c.bits, 32);
            assert_eq!(
                c.transistor.transistor_type,
                if kind == Kind::Transistor {
                    TransistorType::N
                } else {
                    TransistorType::P
                }
            );
            for facing in Facing::ALL {
                for bits in [1, 4, 32] {
                    for right in [false, true] {
                        for typ in TransistorType::ALL {
                            let mut c = default.clone();
                            c.facing = facing;
                            c.bits = bits;
                            c.transistor.gate_bottom_right = right;
                            if kind == Kind::Transistor {
                                c.transistor.transistor_type = typ;
                            }
                            let mut circuit = Circuit::default();
                            let id = circuit.add_component(c);
                            let document = Document::single("main", circuit);
                            let text = serialize(&document).unwrap();
                            let parsed = parse(&text).unwrap();
                            assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
                            let mut restored =
                                parsed.document.main_circuit().circuit.components[0].clone();
                            restored.id = id;
                            assert_eq!(
                                &restored,
                                document.main_circuit().circuit.component(id).unwrap()
                            );
                            assert_eq!(serialize(&parsed.document).unwrap(), text);
                            assert_eq!(restored.position, Point::new(200, 100));
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn unsupported_transistor_attributes_are_reported_without_silent_loss() {
        for kind in [Kind::Transistor, Kind::TransmissionGate] {
            for attrs in [
                r#"<a name="gate" val="left"/>"#,
                r#"<a name="width" val="0"/>"#,
                r#"<a name="width" val="33"/>"#,
                r#"<a name="label" val="hidden"/>"#,
                r#"<a name="type" val="N"/>"#,
                r#"<a name="facing" val="up"/>"#,
                r#"<a name="control" val="left"/>"#,
            ] {
                let imported = parse(&xml(kind, attrs)).unwrap();
                assert!(!imported.warnings.is_empty(), "{kind:?} {attrs}");
                assert!(
                    imported
                        .document
                        .main_circuit()
                        .circuit
                        .components
                        .is_empty()
                );
            }
        }
        let imported = parse(&xml(Kind::TransmissionGate, r#"<a name="type" val="p"/>"#)).unwrap();
        assert!(!imported.warnings.is_empty());
        assert!(
            imported
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
    }
}

use crate::{Error, Result};
use logisim_core::{Component, ControlPosition, Facing, Kind, Latch, LatchEnable};
use std::collections::BTreeMap;

pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "trigger"
            | "Pre/Clr Positions"
            | "NegatePresetClear"
            | "NewFFLayout"
            | "label"
            | "labelfont"
            | "labelcolor"
    )
}

pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !matches!(c.kind, Kind::Latch(_)) {
        return Ok(());
    }
    c.latch_enable = match a.get("trigger").map(String::as_str) {
        None | Some("high") => LatchEnable::High,
        Some("low") => LatchEnable::Low,
        Some("latch") if c.kind == Kind::Latch(Latch::Sr) => LatchEnable::None,
        _ => return Err(Error("Unsupported latch enable mode".into())),
    };
    c.sequential.controls = match a.get("Pre/Clr Positions").map(String::as_str) {
        None => ControlPosition::AboveBelow,
        Some("BELOW_ABOVE") => ControlPosition::BelowAbove,
        Some("LEGACY") => ControlPosition::Legacy,
        _ => return Err(Error("Unsupported latch preset/clear layout".into())),
    };
    for (key, target) in [
        ("NewFFLayout", &mut c.sequential.new_layout),
        ("NegatePresetClear", &mut c.sequential.negate_controls),
    ] {
        if let Some(v) = a.get(key) {
            *target = match v.as_str() {
                "true" => true,
                "false" => false,
                _ => return Err(Error(format!("Invalid latch {key}"))),
            };
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::write::attribute as a;
    if !matches!(c.kind, Kind::Latch(_)) {
        return Ok(());
    }
    if c.bits != 1
        || c.facing != Facing::East
        || (c.kind == Kind::Latch(Latch::D) && c.latch_enable == LatchEnable::None)
    {
        return Err(Error(
            "Unsupported latch width, orientation or enable mode".into(),
        ));
    }
    match c.latch_enable {
        LatchEnable::High => {}
        LatchEnable::Low => a(out, "trigger", "low")?,
        LatchEnable::None => a(out, "trigger", "latch")?,
    }
    match c.sequential.controls {
        ControlPosition::AboveBelow => {}
        ControlPosition::BelowAbove => a(out, "Pre/Clr Positions", "BELOW_ABOVE")?,
        ControlPosition::Legacy => a(out, "Pre/Clr Positions", "LEGACY")?,
    }
    a(out, "NewFFLayout", c.sequential.new_layout)?;
    a(out, "NegatePresetClear", c.sequential.negate_controls)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Point};
    fn xml(kind: Latch, attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="m" desc="#Memory"/><main name="main"/><circuit name="main"><comp lib="m" name="{}" loc="(300,300)">{attrs}</comp></circuit></project>"##,
            kind.name()
        )
    }
    #[test]
    fn native_latch_defaults_and_attributes_roundtrip_without_runtime_state() {
        for kind in Latch::ALL {
            for old in ["", r#"<a name="NewFFLayout" val="false"/>"#] {
                let parsed = parse(&xml(kind, old)).unwrap();
                assert!(parsed.warnings.is_empty());
                let c = &parsed.document.main_circuit().circuit.components[0];
                assert_eq!(c.latch_enable, LatchEnable::High);
                assert_eq!(c.sequential.controls, ControlPosition::AboveBelow);
                assert_eq!(c.sequential.new_layout, old.is_empty());
                assert!(!c.sequential.negate_controls);
                assert_eq!(c.bits, 1);
            }
            for mode in LatchEnable::ALL {
                if kind == Latch::D && mode == LatchEnable::None {
                    continue;
                }
                for new in [false, true] {
                    for controls in ControlPosition::ALL {
                        for negated in [false, true] {
                            let mut c = Component::new(Kind::Latch(kind), Point::new(300, 300));
                            c.latch_enable = mode;
                            c.sequential.new_layout = new;
                            c.sequential.controls = controls;
                            c.sequential.negate_controls = negated;
                            c.label = "锁存 & latch".into();
                            let mut circuit = Circuit::default();
                            let id = circuit.add_component(c);
                            let doc = Document::single("main", circuit);
                            let out = serialize(&doc).unwrap();
                            let imported = parse(&out).unwrap();
                            assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                            let mut restored =
                                imported.document.main_circuit().circuit.components[0].clone();
                            restored.id = id;
                            assert_eq!(
                                &restored,
                                doc.main_circuit().circuit.component(id).unwrap()
                            );
                            assert_eq!(serialize(&imported.document).unwrap(), out);
                            assert!(
                                !out.contains("name=\"width\"")
                                    || out.contains("name=\"width\" val=\"1\"")
                            );
                            assert_eq!(out.contains("val=\"latch\""), mode == LatchEnable::None);
                            assert!(!out.contains("rising"));
                        }
                    }
                }
            }
        }
        let high = parse(&xml(Latch::D, r#"<a name="trigger" val="high"/>"#)).unwrap();
        assert!(high.warnings.is_empty());
        assert_eq!(
            high.document.main_circuit().circuit.components[0].latch_enable,
            LatchEnable::High
        );
    }
    #[test]
    fn unsupported_latch_attributes_are_reported_and_not_silently_reinterpreted() {
        for kind in Latch::ALL {
            for attrs in [
                r#"<a name="trigger" val="rising"/>"#,
                r#"<a name="trigger" val="falling"/>"#,
                r#"<a name="trigger" val="unknown"/>"#,
                r#"<a name="width" val="8"/>"#,
                r#"<a name="facing" val="south"/>"#,
                r#"<a name="Pre/Clr Positions" val="unknown"/>"#,
                r#"<a name="NewFFLayout" val="yes"/>"#,
                r#"<a name="NegatePresetClear" val="1"/>"#,
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
            for invalid in 0..3 {
                let mut c = Component::new(Kind::Latch(kind), Point::ZERO);
                match invalid {
                    0 => c.bits = 8,
                    1 => c.facing = Facing::South,
                    _ => {
                        c.kind = Kind::Latch(Latch::D);
                        c.latch_enable = LatchEnable::None;
                    }
                }
                let mut circuit = Circuit::default();
                circuit.add_component(c);
                assert!(serialize(&Document::single("main", circuit)).is_err());
            }
        }
        let invalid = parse(&xml(Latch::D, r#"<a name="trigger" val="latch"/>"#)).unwrap();
        assert!(!invalid.warnings.is_empty());
        assert!(
            invalid
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
    }
}

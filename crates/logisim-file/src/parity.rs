use crate::{Error, Result};
use logisim_core::{Component, GateSize, Kind, ParityOutput};
use std::collections::BTreeMap;

fn negated_index(key: &str) -> Option<u8> {
    let suffix = key.strip_prefix("negate")?;
    let n = suffix.parse::<u8>().ok()?;
    (n < 32 && suffix == n.to_string()).then_some(n)
}
pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "facing" | "width" | "size" | "inputs" | "out" | "label" | "labelfont" | "labelcolor"
    ) || negated_index(key).is_some()
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !matches!(c.kind, Kind::Parity(_)) {
        return Ok(());
    }
    c.inputs = crate::attributes::number(a, "inputs", 2, 2..=32)?;
    c.gate_size = match a.get("size").map(String::as_str) {
        None | Some("30") => GateSize::Narrow,
        Some("50") => GateSize::Medium,
        Some("70") => GateSize::Wide,
        _ => return Err(Error("Unsupported parity gate size".into())),
    };
    c.parity.output = match a.get("out").map(String::as_str) {
        None => ParityOutput::Normal,
        Some("0Z") => ParityOutput::ZeroFloating,
        Some("Z1") => ParityOutput::FloatingOne,
        _ => return Err(Error("Unsupported parity output mode".into())),
    };
    for (key, value) in a {
        if let Some(i) = negated_index(key) {
            if i >= c.inputs {
                return Err(Error("Inversion refers to an inactive parity input".into()));
            }
            match value.as_str() {
                "false" => {}
                "true" => c.parity.negated |= 1 << i,
                _ => return Err(Error("Invalid parity input inversion".into())),
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute as a;
    if !matches!(c.kind, Kind::Parity(_)) {
        return Ok(());
    }
    a(out, "size", c.gate_width())?;
    a(out, "inputs", c.inputs)?;
    match c.parity.output {
        ParityOutput::Normal => {}
        ParityOutput::ZeroFloating => a(out, "out", "0Z")?,
        ParityOutput::FloatingOne => a(out, "out", "Z1")?,
    }
    for i in 0..c.input_count() {
        if c.parity_negated(i) {
            a(out, &format!("negate{i}"), true)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Facing, Parity};
    fn xml(kind: Parity, attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="g" desc="#Gates"/><main name="main"/><circuit name="main"><comp lib="g" name="{}" loc="(300,300)">{attrs}</comp></circuit></project>"##,
            kind.name()
        )
    }
    #[test]
    fn native_parity_defaults_and_full_attributes_roundtrip() {
        for kind in Parity::ALL {
            let default = parse(&xml(kind, "")).unwrap();
            assert!(default.warnings.is_empty());
            let c = &default.document.main_circuit().circuit.components[0];
            assert_eq!(c.parity, Default::default());
            assert_eq!(c.bits, 1);
            assert_eq!(c.inputs, 2);
            assert_eq!(c.gate_size, GateSize::Narrow);
            let parsed=parse(&xml(kind,r#"<a name="inputs" val="32"/><a name="width" val="32"/><a name="size" val="70"/><a name="facing" val="south"/><a name="negate0" val="true"/><a name="negate31" val="true"/><a name="out" val="0Z"/>"#)).unwrap();
            assert!(parsed.warnings.is_empty());
            let c = &parsed.document.main_circuit().circuit.components[0];
            assert_eq!(c.parity.negated, 0x80000001);
            assert_eq!(c.parity.output, ParityOutput::ZeroFloating);
            assert_eq!(c.inputs, 32);
            assert_eq!(c.bits, 32);
            assert_eq!(c.facing, Facing::South);
            for facing in Facing::ALL {
                for count in [2, 3, 4, 8, 31, 32] {
                    for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                        for mode in ParityOutput::ALL {
                            let mut c = c.clone();
                            c.facing = facing;
                            c.inputs = count;
                            c.gate_size = size;
                            c.parity.output = mode;
                            c.parity.negated = 1 | (1 << (count - 1));
                            c.label = "校验 & parity".into();
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
                            assert_eq!(out.contains("name=\"out\""), mode != ParityOutput::Normal);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn parity_serializes_only_current_inputs_and_rejects_unobserved_fields() {
        let mut parsed=parse(&xml(Parity::Odd,r#"<a name="inputs" val="32"/><a name="negate0" val="true"/><a name="negate31" val="true"/>"#)).unwrap();
        parsed.document.circuits[0].circuit.components[0].inputs = 2;
        let out = serialize(&parsed.document).unwrap();
        assert!(out.contains("negate0"));
        assert!(!out.contains("negate31"));
        assert_eq!(
            parsed.document.circuits[0].circuit.components[0]
                .parity
                .negated,
            0x80000001
        );
        assert_eq!(
            parse(&out).unwrap().document.circuits[0].circuit.components[0]
                .parity
                .negated,
            1
        );
        for kind in Parity::ALL {
            for attrs in [
                r#"<a name="inputs" val="1"/>"#,
                r#"<a name="inputs" val="33"/>"#,
                r#"<a name="size" val="40"/>"#,
                r#"<a name="width" val="33"/>"#,
                r#"<a name="out" val="01"/>"#,
                r#"<a name="negate32" val="true"/>"#,
                r#"<a name="negate01" val="true"/>"#,
                r#"<a name="negate2" val="true"/>"#,
                r#"<a name="negate0" val="yes"/>"#,
                r#"<a name="xor" val="odd"/>"#,
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
    }
}

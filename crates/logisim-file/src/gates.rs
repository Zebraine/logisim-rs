use crate::{Error, Result};
use logisim_core::{Component, Gate, Kind};
use std::collections::BTreeMap;

pub(crate) fn index(key: &str) -> Option<u8> {
    let suffix = key.strip_prefix("negate")?;
    let i = suffix.parse::<u8>().ok()?;
    (i < Gate::MAX_INPUTS && suffix == i.to_string()).then_some(i)
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    let Kind::Gate(gate) = c.kind else {
        return Ok(());
    };
    if matches!(gate, Gate::Xor | Gate::Xnor) {
        c.gate_options.xor_odd = match a.get("xor").map(String::as_str) {
            None => false,
            Some("odd") => true,
            _ => return Err(Error("Unsupported XOR mode".into())),
        };
    }
    for (key, value) in a {
        if let Some(i) = index(key) {
            if i >= c.input_count() {
                return Err(Error("Inversion refers to an inactive gate input".into()));
            }
            match value.as_str() {
                "false" => {}
                "true" => c.gate_options.negated |= 1 << i,
                _ => return Err(Error("Invalid gate input inversion".into())),
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute;
    let Kind::Gate(gate) = c.kind else {
        return Ok(());
    };
    if matches!(gate, Gate::Xor | Gate::Xnor) && c.gate_options.xor_odd {
        attribute(out, "xor", "odd")?;
    }
    if !gate.unary() {
        for i in 0..c.input_count() {
            if c.gate_negated(i) {
                attribute(out, &format!("negate{i}"), true)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{parse, serialize};
    fn xml(attrs: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="g" desc="#Gates"/><main name="main"/><circuit name="main"><comp lib="g" name="XOR Gate" loc="(300,300)">{attrs}</comp></circuit></project>"##
        )
    }
    #[test]
    fn gate_negations_and_xor_modes_roundtrip_without_saving_inactive_inputs() {
        let mut imported = parse(&xml(r#"<a name="width" val="32"/><a name="inputs" val="8"/><a name="negate0" val="true"/><a name="negate7" val="true"/><a name="xor" val="odd"/>"#)).unwrap();
        assert!(imported.warnings.is_empty());
        let c = &imported.document.main_circuit().circuit.components[0];
        assert_eq!(c.gate_options.negated, 129);
        assert!(c.gate_options.xor_odd);
        let out = serialize(&imported.document).unwrap();
        assert_eq!(parse(&out).unwrap().document, imported.document);
        imported.document.main_circuit_mut().circuit.components[0].inputs = 2;
        let out = serialize(&imported.document).unwrap();
        assert!(out.contains("negate0"));
        assert!(!out.contains("negate7"));
        for bad in [
            r#"<a name="negate2" val="true"/>"#,
            r#"<a name="negate01" val="true"/>"#,
            r#"<a name="negate0" val="yes"/>"#,
            r#"<a name="xor" val="other"/>"#,
        ] {
            let parsed = parse(&xml(bad)).unwrap();
            assert!(!parsed.warnings.is_empty());
            assert!(parsed.document.main_circuit().circuit.components.is_empty());
        }
    }
}

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
    imported.document.main_circuit_mut().circuit.components[0]
        .gate_options
        .inputs = 2;
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

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
        assert_eq!(c.parity.inputs, 2);
        assert_eq!(c.parity.size, GateSize::Narrow);
        let parsed=parse(&xml(kind,r#"<a name="inputs" val="32"/><a name="width" val="32"/><a name="size" val="70"/><a name="facing" val="south"/><a name="negate0" val="true"/><a name="negate31" val="true"/><a name="out" val="0Z"/>"#)).unwrap();
        assert!(parsed.warnings.is_empty());
        let c = &parsed.document.main_circuit().circuit.components[0];
        assert_eq!(c.parity.negated, 0x80000001);
        assert_eq!(c.parity.output, ParityOutput::ZeroFloating);
        assert_eq!(c.parity.inputs, 32);
        assert_eq!(c.bits, 32);
        assert_eq!(c.facing, Facing::South);
        for facing in Facing::ALL {
            for count in [2, 3, 4, 8, 31, 32] {
                for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                    for mode in ParityOutput::ALL {
                        let mut c = c.clone();
                        c.facing = facing;
                        c.parity.inputs = count;
                        c.parity.size = size;
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
                        assert_eq!(&restored, doc.main_circuit().circuit.component(id).unwrap());
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
    parsed.document.circuits[0].circuit.components[0]
        .parity
        .inputs = 2;
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

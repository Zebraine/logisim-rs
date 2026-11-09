use super::*;
use crate::{Document, parse, serialize};
use logisim_core::{Circuit, Facing, Point, Ttl};
fn xml(name: &str, attrs: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="t" desc="#TTL"/><main name="main"/><circuit name="main"><comp lib="t" name="{name}" loc="(280,200)">{attrs}</comp></circuit></project>"##
    )
}
#[test]
fn native_ttl_defaults_and_all_options_roundtrip() {
    for kind in Ttl::ALL {
        let imported = parse(&xml(kind.name(), "")).unwrap();
        assert!(imported.warnings.is_empty());
        let default = Component::new(Kind::Ttl(kind), Point::new(280, 200));
        let mut c = imported.document.main_circuit().circuit.components[0].clone();
        c.id = 0;
        assert_eq!(c, default);
        for facing in Facing::ALL {
            for power in [false, true] {
                for internal in [false, true] {
                    let mut c = default.clone();
                    c.facing = facing;
                    c.ttl.power_pins = power;
                    c.ttl.internal = internal;
                    c.label = "芯片 <&>".into();
                    c.font.size = 18;
                    c.color = [30, 60, 90];
                    let mut circuit = Circuit::default();
                    let id = circuit.add_component(c.clone());
                    c.id = id;
                    let text = serialize(&Document::single("main", circuit)).unwrap();
                    let parsed = parse(&text).unwrap();
                    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
                    assert_eq!(parsed.document.main_circuit().circuit.components[0], c);
                    assert_eq!(serialize(&parsed.document).unwrap(), text);
                    assert!(!text.contains("name=\"width\""));
                }
            }
        }
    }
}
#[test]
fn unsupported_ttl_models_and_attributes_are_protected() {
    for name in ["7447", "7485", "74125", "74165", "74283", "unknown"] {
        let imported = parse(&xml(name, "")).unwrap();
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
    for name in Ttl::ALL {
        for attrs in [
            r#"<a name="VccGndPorts" val="yes"/>"#,
            r#"<a name="ShowInternalStructure" val="1"/>"#,
            r#"<a name="width" val="1"/>"#,
            r#"<a name="inputs" val="2"/>"#,
            r#"<a name="facing" val="up"/>"#,
            r#"<a name="labelloc" val="north"/>"#,
        ] {
            let imported = parse(&xml(name.name(), attrs)).unwrap();
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
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::Ttl(Ttl::Ic7400), Point::ZERO);
    c.bits = 2;
    circuit.add_component(c);
    assert!(serialize(&Document::single("main", circuit)).is_err());
}

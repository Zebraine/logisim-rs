use super::*;
use crate::{Document, parse, serialize};
use logisim_core::{Circuit, Facing, Point};
fn xml(kind: Kind, attrs: &str) -> String {
    let lib = if kind == Kind::PullResistor {
        "#Wiring"
    } else {
        "#Gates"
    };
    format!(
        r#"<project source="2.16.2.2.jar" version="1.0"><lib name="a" desc="{lib}"/><main name="main"/><circuit name="main"><comp lib="a" name="{}" loc="(100,100)">{attrs}</comp></circuit></project>"#,
        kind.name()
    )
}
#[test]
fn bus_defaults_and_observed_attributes_roundtrip() {
    for kind in [
        Kind::ControlledBuffer,
        Kind::ControlledInverter,
        Kind::PullResistor,
    ] {
        let imported = parse(&xml(kind, "")).unwrap();
        assert!(imported.warnings.is_empty());
        let c = &imported.document.main_circuit().circuit.components[0];
        let default = Component::new(kind, Point::new(100, 100));
        assert_eq!(c.bus, default.bus);
        assert_eq!(c.facing, default.facing);
        assert_eq!(c.ports(), default.ports());
        for facing in Facing::ALL {
            for bits in [1, 4, 32] {
                for left in [false, true] {
                    for negated in [false, true] {
                        for pull in Pull::ALL {
                            for size in [GateSize::Narrow, GateSize::Wide] {
                                let mut c = default.clone();
                                c.facing = facing;
                                if kind.is_controlled() {
                                    c.bits = bits;
                                    c.bus.control_left = left;
                                    c.bus.negate_control = negated;
                                    c.label = "总线 & Bus".into();
                                    c.color = [12, 34, 56];
                                    if kind == Kind::ControlledInverter {
                                        c.gate_size = size;
                                    }
                                } else {
                                    c.bus.pull = pull;
                                }
                                let mut circuit = Circuit::default();
                                let id = circuit.add_component(c);
                                let document = Document::single("main", circuit);
                                let text = serialize(&document).unwrap();
                                let after = parse(&text).unwrap();
                                assert!(after.warnings.is_empty(), "{:?}", after.warnings);
                                let mut restored =
                                    after.document.main_circuit().circuit.components[0].clone();
                                restored.id = id;
                                assert_eq!(
                                    &restored,
                                    document.main_circuit().circuit.component(id).unwrap()
                                );
                                assert_eq!(serialize(&after.document).unwrap(), text);
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn invalid_or_unobserved_bus_attributes_protect_the_original() {
    for (kind, key, value) in [
        (Kind::ControlledBuffer, "size", "20"),
        (Kind::ControlledBuffer, "control", "top"),
        (Kind::ControlledBuffer, "negatecontrol", "1"),
        (Kind::ControlledInverter, "size", "50"),
        (Kind::ControlledInverter, "inputs", "2"),
        (Kind::ControlledInverter, "width", "33"),
        (Kind::ControlledBuffer, "width", "0"),
        (Kind::PullResistor, "pull", "E"),
        (Kind::PullResistor, "pull", "x"),
        (Kind::PullResistor, "width", "4"),
        (Kind::PullResistor, "label", "ignored"),
    ] {
        let imported = parse(&xml(kind, &format!(r#"<a name="{key}" val="{value}"/>"#))).unwrap();
        assert!(!imported.warnings.is_empty(), "{kind:?} {key}");
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

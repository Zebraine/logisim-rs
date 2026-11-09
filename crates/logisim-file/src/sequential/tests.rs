use super::*;
use crate::{parse, serialize};
use logisim_core::{Facing, Point};
fn xml(attrs: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="5" desc="#Memory"/><main name="main"/><circuit name="main"><comp lib="5" name="Random" loc="(200,200)">{attrs}</comp></circuit></project>"##
    )
}
#[test]
fn random_native_defaults_signed_seeds_and_labels_roundtrip() {
    let default = parse(&xml("")).unwrap();
    assert!(default.warnings.is_empty());
    let c = &default.document.main_circuit().circuit.components[0];
    assert_eq!(c.kind, Kind::Random);
    assert_eq!(c.bits, 8);
    assert_eq!(c.sequential.random_seed, 0);
    for seed in [i32::MIN, -1, 0, 1, 12345, i32::MAX] {
        for bits in [1, 8, 32] {
            for trigger in ["rising", "falling"] {
                let attrs = format!(
                    r##"<a name="seed" val="{seed}"/><a name="width" val="{bits}"/><a name="trigger" val="{trigger}"/><a name="label" val="用户 &amp; Random"/><a name="labelfont" val="Dialog plain 16"/><a name="labelcolor" val="#123456"/>"##
                );
                let imported = parse(&xml(&attrs)).unwrap();
                assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                let c = &imported.document.main_circuit().circuit.components[0];
                assert_eq!(c.sequential.random_seed, seed);
                assert_eq!(c.bits, bits);
                assert_eq!(c.label, "用户 & Random");
                assert_eq!(c.color, [0x12, 0x34, 0x56]);
                let saved = serialize(&imported.document).unwrap();
                assert!(saved.contains("name=\"Random\""));
                let again = parse(&saved).unwrap();
                assert!(again.warnings.is_empty());
                assert_eq!(saved, serialize(&again.document).unwrap());
                assert_eq!(again.document.main_circuit().circuit.components[0], *c);
            }
        }
    }
}
#[test]
fn random_unobserved_attributes_and_invalid_values_are_protected() {
    for (key, val) in [
        ("seed", "2147483648"),
        ("seed", "-2147483649"),
        ("seed", "0x10"),
        ("seed", "NaN"),
        ("width", "0"),
        ("width", "33"),
        ("trigger", "high"),
        ("trigger", "low"),
        ("facing", "west"),
        ("algorithm", "other"),
    ] {
        let imported = parse(&xml(&format!(r#"<a name="{key}" val="{val}"/>"#))).unwrap();
        assert!(!imported.warnings.is_empty(), "{key}={val}");
        assert!(
            imported
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
    }
    for (facing, trigger) in [
        (Facing::West, Trigger::Rising),
        (Facing::East, Trigger::High),
    ] {
        let mut c = Component::new(Kind::Random, Point::ZERO);
        c.facing = facing;
        c.sequential.trigger = trigger;
        assert!(write(&mut String::new(), &c).is_err());
    }
}

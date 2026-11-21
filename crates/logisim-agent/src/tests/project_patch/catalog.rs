use super::{Fixture, SOURCE};
use crate::{Request, Service};
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn catalog_names_are_unique_and_all_editable_kinds_can_be_placed_by_name_or_alias() {
    let catalog = Service::default().call(Request::ProjectCatalog {}).unwrap();
    assert_eq!(catalog["schema"], 1);
    let kinds = catalog["kinds"].as_array().unwrap();
    assert_eq!(kinds.len(), 67);
    let mut names = BTreeSet::new();
    let mut read_only = BTreeSet::new();
    let mut ops = vec![json!({"op":"add_circuit","name":"child"})];
    let mut f = Fixture::new(SOURCE);
    for entry in kinds {
        let name = entry["name"].as_str().unwrap();
        let instance = name == "Subcircuit";
        assert_eq!(entry["requires_circuit_id"], instance);
        assert_eq!(entry["addable"], entry["editable"]);
        for spelling in std::iter::once(name).chain(
            entry["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap()),
        ) {
            assert!(names.insert(spelling.to_ascii_lowercase()), "{spelling}");
            let mut op = json!({"op":"add_component","circuit":1,"kind":spelling,"position":[100*ops.len(),100]});
            if instance {
                op["circuit_id"] = 2.into();
            }
            if entry["addable"] == true {
                ops.push(op);
            } else {
                read_only.insert(name);
                assert_eq!(
                    f.patch(json!([op]), true).unwrap_err().code,
                    "unsupported_component"
                );
            }
        }
    }
    assert_eq!(
        read_only,
        BTreeSet::from([
            "ROM",
            "RAM",
            "PLA ROM",
            "Plugin component",
            "Slider",
            "Joystick",
            "Keyboard",
            "TTY"
        ])
    );
    assert!(
        names.contains("jk flip-flop")
            && names.contains("sr latch")
            && names.contains("sevensegment")
    );
    f.patch(json!(ops), false).unwrap();
    let snapshot = f.snapshot();
    let components = snapshot["circuits"][0]["components"].as_array().unwrap();
    for (op, c) in ops[1..].iter().zip(&components[2..]) {
        let entry = kinds.iter().find(|e| e["name"] == c["kind"]).unwrap();
        assert!(
            entry["name"] == op["kind"]
                || entry["aliases"].as_array().unwrap().contains(&op["kind"])
        );
    }
}

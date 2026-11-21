use super::{Fixture, SOURCE};
use crate::{Request, Service};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[test]
fn catalog_names_are_unique_and_all_statically_addable_kinds_can_be_placed_by_name_or_alias() {
    let catalog = Service::default().call(Request::ProjectCatalog {}).unwrap();
    assert_eq!(catalog["schema"], 1);
    let kinds = catalog["kinds"].as_array().unwrap();
    assert_eq!(kinds.len(), 67);
    assert_eq!(kinds.iter().filter(|c| c["addable"] == true).count(), 66);
    let mut names = BTreeSet::new();
    let mut dynamic = BTreeSet::new();
    let mut ops = vec![json!({"op":"add_circuit","name":"child"})];
    let mut f = Fixture::new(SOURCE);
    for entry in kinds {
        let name = entry["name"].as_str().unwrap();
        let instance = name == "Subcircuit";
        assert_eq!(entry["requires_circuit_id"], instance);
        assert_eq!(entry["editable"], true);
        assert_eq!(
            entry["requires_plugin_source"] == true,
            name == "Plugin component"
        );
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
                dynamic.insert(name);
                assert_eq!(
                    f.patch(json!([op]), true).unwrap_err().code,
                    "unsupported_component"
                );
            }
        }
    }
    assert_eq!(dynamic, BTreeSet::from(["Plugin component"]));
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
        let schema = entry["properties"].as_object().unwrap();
        assert_eq!(c["property_schema"], entry["properties"], "{}", c["kind"]);
        let keys: BTreeSet<_> = schema.keys().map(String::as_str).collect();
        let writable: BTreeSet<_> = c["patchable_properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field.as_str().unwrap())
            .collect();
        assert_eq!(writable, keys);
        assert_eq!(c["property_types"].as_object().unwrap().len(), keys.len());
        for (key, spec) in schema {
            assert_eq!(c["property_types"][key], spec["type"]);
            assert_schema_value(spec, &c[key]);
        }
        assert!(keys.is_disjoint(&BTreeSet::from(["stored", "ticks", "ports", "circuit_id"])));
        assert_eq!(keys.contains("value"), c["kind"] == "Constant");
        assert!(
            entry["name"] == op["kind"]
                || entry["aliases"].as_array().unwrap().contains(&op["kind"])
        );
    }
}

fn assert_schema_value(spec: &Value, v: &Value) {
    if let Some(choices) = spec["enum"].as_array() {
        assert!(choices.contains(v), "{v} outside {choices:?}");
    }
    if !spec["const"].is_null() {
        assert_eq!(v, &spec["const"]);
    }
    match spec["type"].as_str() {
        Some("integer") => {
            let n = v.as_i64().unwrap();
            if let Some(min) = spec["minimum"].as_i64() {
                assert!(n >= min);
            }
            if let Some(max) = spec["maximum"].as_i64() {
                assert!(n <= max);
            }
        }
        Some("boolean") => assert!(v.is_boolean()),
        Some("string") => {
            let text = v.as_str().unwrap();
            if let Some(limit) = spec["x-maxUtf8Bytes"].as_u64() {
                assert!(text.len() <= limit as usize);
            }
        }
        Some("object") => {
            for (key, value) in v.as_object().unwrap() {
                assert_schema_value(&spec["properties"][key], value);
            }
        }
        Some("array") => {
            let items = v.as_array().unwrap();
            assert!(
                (spec["minItems"].as_u64().unwrap()..=spec["maxItems"].as_u64().unwrap())
                    .contains(&(items.len() as u64))
            );
            for (i, value) in items.iter().enumerate() {
                let item = spec["prefixItems"].get(i).unwrap_or(&spec["items"]);
                assert_schema_value(item, value);
            }
        }
        None if spec["type"] == json!(["integer", "null"]) => {
            assert!(
                v.is_null()
                    || v.as_i64()
                        .is_some_and(|n| (spec["minimum"].as_i64().unwrap()
                            ..=spec["maximum"].as_i64().unwrap())
                            .contains(&n))
            );
        }
        _ => panic!("unsupported field schema {spec}"),
    }
}

#[test]
fn catalog_trigger_vocabularies_match_patches_and_the_file_codec() {
    let catalog = Service::default().call(Request::ProjectCatalog {}).unwrap();
    let kinds = catalog["kinds"].as_array().unwrap();
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    for (kind, expected) in [
        ("D Latch", &["high", "low"][..]),
        ("S-R Latch", &["high", "low", "latch"]),
        ("Register", &["rising", "falling", "high", "low"]),
        ("Counter", &["rising", "falling"]),
        ("D Flip-Flop", &["rising", "falling"]),
        ("Shift Register", &["rising", "falling"]),
        ("Random Generator", &["rising", "falling"]),
    ] {
        let entry = kinds.iter().find(|k| k["name"] == kind).unwrap();
        assert_eq!(entry["properties"]["trigger"]["enum"], json!(expected));
        for trigger in ["high", "low", "latch", "rising", "falling"] {
            let op = json!({"op":"add_component","kind":kind,"position":[150,20],
                "circuit":1,"properties":{"trigger":trigger}});
            let result = f.patch(json!([op]), true);
            if expected.contains(&trigger) {
                result.unwrap();
            } else {
                let err = result.unwrap_err();
                assert_eq!(err.code, "invalid_property", "{kind} {trigger}");
                assert_eq!(err.details["op_index"], 0);
            }
        }
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn schema_exposes_codec_limits_and_validates_partial_font_rgba_and_splitter_mapping() {
    let catalog = Service::default().call(Request::ProjectCatalog {}).unwrap();
    let fields = |name: &str| {
        &catalog["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["name"] == name)
            .unwrap()["properties"]
    };
    let pin = fields("Pin");
    assert_eq!(pin["label"]["x-maxUtf8Bytes"], 65536);
    assert_eq!(pin["font"]["properties"]["family"]["x-maxUtf8Bytes"], 256);
    assert_eq!(
        pin["font"]["properties"]["size"],
        json!({"type":"integer","minimum":1,"maximum":255})
    );
    assert_eq!(pin["font"]["additionalProperties"], false);
    assert_eq!(pin["color"]["prefixItems"][3]["const"], 255);
    assert!(fields("Text")["color"].get("prefixItems").is_none());
    assert_eq!(
        fields("Subcircuit")["position"]["items"],
        json!({"type":"integer","minimum":-1000000,"maximum":1000000})
    );
    assert_eq!(
        fields("Splitter")["mapping"]["items"]["type"],
        json!(["integer", "null"])
    );
    assert_eq!(fields("Counter")["counter_max"]["maximum"], u32::MAX);
    assert_eq!(fields("Bit Selector")["group_bits"]["maximum"], 32);
    assert_eq!(fields("Multiplexer")["select_bits"]["maximum"], 5);
    assert_eq!(
        fields("Random Generator")["random_seed"]["minimum"],
        i32::MIN
    );
    assert_eq!(fields("AND Gate")["inputs"]["maximum"], 8);
    assert_eq!(fields("Odd Parity")["inputs"]["maximum"], 32);
    assert_eq!(
        fields("Controlled Inverter")["size"]["enum"],
        json!(["narrow", "wide"])
    );
    let mut f = Fixture::new(SOURCE);
    let add = |kind, properties| json!([{"op":"add_component","circuit":1,"position":[150,20],"kind":kind,"properties":properties}]);
    let family = "é".repeat(128);
    let label = "é".repeat(32768);
    f.patch(
        add(
            "Pin",
            json!({"font":{"family":family,"size":255},"label":label,"color":[0,255,0,255]}),
        ),
        true,
    )
    .unwrap();
    f.patch(add("Text", json!({"color":[0,10,255,0]})), true)
        .unwrap();
    f.patch(add("Splitter", json!({"mapping":[null,0,1,null]})), true)
        .unwrap();
    let before = f.snapshot();
    for (kind, props) in [
        ("Pin", json!({"label":label+"a"})),
        ("Pin", json!({"font":{"family":family+"a"}})),
        ("Pin", json!({"font":{"family":" "}})),
        ("Pin", json!({"font":{"size":256}})),
        ("Pin", json!({"color":[0,255,0,254]})),
        ("Text", json!({"color":[0,255,0,256]})),
        ("Splitter", json!({"mapping":[0,1,null,32]})),
        ("Controlled Inverter", json!({"size":"medium"})),
    ] {
        let err = f.patch(add(kind, props), false).unwrap_err();
        assert_eq!(err.code, "invalid_property");
        assert_eq!(err.details["op_index"], 0);
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn memory_programming_and_terminal_schemas_describe_only_persistent_fields() {
    let catalog = Service::default().call(Request::ProjectCatalog {}).unwrap();
    let fields = |name: &str| {
        &catalog["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["name"] == name)
            .unwrap()["properties"]
    };
    let rom = &fields("ROM")["contents"];
    assert_eq!(rom["maxItems"], 65536);
    assert_eq!(rom["items"]["minItems"], 2);
    assert_eq!(rom["items"]["maxItems"], 2);
    assert_eq!(rom["items"]["prefixItems"][0]["maximum"], 0xffffff);
    assert_eq!(rom["items"]["prefixItems"][1]["maximum"], u32::MAX);
    assert!(fields("RAM").get("contents").is_none());
    let pla = fields("PLA ROM");
    assert_eq!(
        pla["input_plane"]["items"]["items"]["enum"],
        json!(["disconnected", "direct", "inverted"])
    );
    assert_eq!(pla["output_plane"]["items"]["items"]["type"], "boolean");
    for (key, columns) in [("input_plane", "inputs"), ("output_plane", "outputs")] {
        assert_eq!(pla[key]["x-rows"], "terms");
        assert_eq!(pla[key]["x-columns"], columns);
        assert_eq!(pla[key]["maxItems"], pla["terms"]["maximum"]);
        assert_eq!(pla[key]["items"]["maxItems"], pla[columns]["maximum"]);
    }
    assert_eq!(fields("Slider")["slider_value"]["x-wordBits"], "bits");
    assert_eq!(fields("Joystick").as_object().unwrap().len(), 2);
    for term in ["Keyboard", "TTY"] {
        assert_eq!(fields(term)["bits"]["enum"], json!([7, 16]));
        assert_eq!(
            fields(term)["trigger"]["enum"],
            json!(["rising", "falling"])
        );
        assert!(fields(term).get("font").is_none());
    }
}

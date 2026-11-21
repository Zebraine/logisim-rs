use super::{Fixture, SOURCE, open};
use serde_json::{Value, json};

fn instance(owner: u64, target: u64) -> Value {
    json!({"op":"add_component","circuit":owner,"kind":"subcircuit","position":[200,100],"circuit_id":target,"properties":{"label":"Unit"}})
}

#[test]
fn nested_instances_derive_the_target_interface_and_follow_core_wire_routing() {
    let mut f = Fixture::new(SOURCE);
    let ops = json!([
        {"op":"add_circuit","name":"child"},{"op":"add_circuit","name":"leaf"},
        {"op":"add_component","circuit":2,"kind":"Pin","position":[20,10],"properties":{"label":"A","bits":3}},
        instance(1,2),instance(2,3)
    ]);
    let preview = f.patch(ops.clone(), true).unwrap();
    let changes = f.patch(ops, false).unwrap()["changes"].clone();
    assert_eq!(preview["changes"], changes);
    let parent_id = &changes[3]["id"];
    let pin_id = &changes[2]["id"];
    let snapshot = f.snapshot();
    let parent = &snapshot["circuits"][0]["components"][2];
    assert_eq!(parent["circuit_id"], 2);
    assert_eq!(parent["circuit_name"], "child");
    assert_eq!(
        parent["interface_ports"],
        snapshot["circuits"][1]["interface_ports"]
    );
    assert_eq!(parent["interface_ports"][0]["pin"], *pin_id);
    let at = &parent["ports"][0]["position"];
    let wire_end = json!([at[0].as_i64().unwrap() - 80, at[1]]);
    f.patch(json!([
        {"op":"add_wire","circuit":1,"from":at,"to":wire_end},
        {"op":"update_component","circuit":2,"id":pin_id,"properties":{"bits":7,"label":"DATA"}},
        {"op":"rename_circuit","id":2,"name":"alu"},
        {"op":"update_component","circuit":1,"id":parent_id,"properties":{"position":[260,100],"facing":"South","label":"U1","font":{"bold":true}}},
        {"op":"move_component","circuit":1,"id":parent_id,"position":[260,140]}
    ]),false).unwrap();
    let edited = f.snapshot();
    let parent = &edited["circuits"][0]["components"][2];
    assert_eq!(parent["label"], "U1");
    assert_eq!(parent["font"]["bold"], true);
    assert_eq!(parent["circuit_name"], "alu");
    assert_eq!(parent["position"], json!([260, 140]));
    assert_eq!(parent["facing"], "South");
    assert_eq!(parent["ports"][0]["bits"], 7);
    assert_eq!(parent["interface_ports"][0]["label"], "DATA");
    assert_eq!(
        parent["patchable_properties"],
        json!(["label", "facing", "font", "position"])
    );
    let wires = edited["circuits"][0]["wires"].as_array().unwrap();
    for end in [&wire_end, &parent["ports"][0]["position"]] {
        assert!(wires.iter().any(|w| w["from"] == *end || w["to"] == *end));
    }
    f.call("project_save", json!({})).unwrap();
    f.id = open(&mut f.cli, &f.path);
    let reopened = f.snapshot();
    assert_eq!(reopened["partial"], false);
    let loaded = &reopened["circuits"][0]["components"][2];
    for field in [
        "position",
        "ports",
        "interface_ports",
        "circuit_id",
        "circuit_name",
        "font",
        "facing",
        "label",
    ] {
        assert_eq!(parent[field], loaded[field], "{field}");
    }
    assert_eq!(
        reopened["circuits"][1]["components"][1]["circuit_name"],
        "leaf"
    );
    f.patch(json!([{"op":"remove_component","circuit":2,"id":reopened["circuits"][1]["components"][0]["id"]}]),false).unwrap();
    assert_eq!(
        f.snapshot()["circuits"][0]["components"][2]["ports"],
        json!([])
    );
    f.patch(json!([{"op":"remove_component","circuit":1,"id":loaded["id"]},{"op":"remove_circuit","id":2},{"op":"remove_circuit","id":3}]),false).unwrap();
    assert_eq!(f.snapshot()["circuits"].as_array().unwrap().len(), 1);
}

#[test]
fn invalid_targets_recursion_and_direct_interface_edits_roll_back() {
    let mut f = Fixture::new(SOURCE);
    let result = f.patch(json!([{"op":"add_circuit","name":"child"},{"op":"add_circuit","name":"leaf"},instance(1,2),instance(2,3)]),false).unwrap();
    let id = &result["changes"][2]["id"];
    let before = f.snapshot();
    for op in [
        instance(1, 1),
        instance(3, 1),
        instance(1, 999),
        instance(999, 2),
        json!({"op":"add_component","circuit":1,"kind":"subcircuit","position":[20,20]}),
        json!({"op":"add_component","circuit":1,"kind":"Text","circuit_id":2,"position":[20,20]}),
        json!({"op":"add_component","circuit":1,"kind":"Text","circuit_id":null,"position":[20,20]}),
        json!({"op":"remove_circuit","id":2}),
        json!({"op":"update_component","circuit":1,"id":id,"properties":{"ports":[]}}),
        json!({"op":"update_component","circuit":1,"id":id,"properties":{"circuit_id":3}}),
        json!({"op":"update_component","circuit":1,"id":id,"properties":{"position":[1000001,0]}}),
    ] {
        let error = f
            .patch(
                json!([{"op":"rename_circuit","id":3,"name":"temporary"},op]),
                false,
            )
            .unwrap_err();
        assert_eq!(error.details["op_index"], 1, "{op}");
        assert_eq!(f.snapshot(), before, "{op}");
    }
}

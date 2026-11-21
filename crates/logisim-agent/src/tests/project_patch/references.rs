use super::{Fixture, SOURCE};
use serde_json::json;

#[test]
fn persistent_references_find_components_wires_circuits_and_subcircuit_targets() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    let top = &before["circuits"][0];
    let pin = &top["components"][0]["agent_id"];
    let wire = &top["wires"][0]["agent_id"];
    let add = json!({"op":"add_circuit","name":"child"});
    let preview = f.patch(json!([add]), true).unwrap();
    let child = &preview["changes"][0]["agent_id"];
    let batch = json!([add,
        {"op":"add_component","circuit":1,"kind":"subcircuit","position":[200,140],"circuit_agent_id":child},
        {"op":"rename_circuit","agent_id":child,"name":"unit"},
        {"op":"set_main_circuit","agent_id":child,"circuit":2},
        {"op":"remove_wire","agent_id":wire},
        {"op":"move_component","agent_id":pin,"position":[50,20]},
        {"op":"update_component","agent_id":pin,"circuit":1,"properties":{"label":"DATA"}}
    ]);
    let dry = f.patch(batch.clone(), true).unwrap();
    assert_eq!(f.snapshot(), before);
    assert_eq!(dry["changes"], f.patch(batch, false).unwrap()["changes"]);
    let snapshot = f.snapshot();
    let new_top = &snapshot["circuits"][0];
    assert_eq!(new_top["components"][0]["label"], "DATA");
    assert_eq!(new_top["components"][0]["position"], json!([50, 20]));
    assert_eq!(new_top["wires"], json!([]));
    assert_eq!(snapshot["circuits"][1]["name"], "unit");
    assert_eq!(snapshot["circuits"][1]["agent_id"], *child);
    assert_eq!(snapshot["main"], 2);
    let instance = &dry["changes"][1];
    assert_eq!(new_top["components"][2]["agent_id"], instance["agent_id"]);
    assert_eq!(new_top["components"][2]["circuit_id"], 2);
    let remove = json!([
        {"op":"remove_component","agent_id":instance["agent_id"]},
        {"op":"set_main_circuit","agent_id":top["agent_id"]},
        {"op":"remove_circuit","agent_id":child}
    ]);
    f.patch(remove, false).unwrap();
    assert_eq!(f.snapshot()["circuits"].as_array().unwrap().len(), 1);
}

#[test]
fn invalid_references_report_the_operation_and_roll_back() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    let top = &before["circuits"][0];
    for (name, extra, tag) in [
        (
            "move_component",
            json!({"position":[10,30]}),
            &top["components"][0]["agent_id"],
        ),
        (
            "update_component",
            json!({"properties":{}}),
            &top["components"][0]["agent_id"],
        ),
        (
            "remove_component",
            json!({}),
            &top["components"][0]["agent_id"],
        ),
        (
            "update_plugin",
            json!({}),
            &top["components"][0]["agent_id"],
        ),
        ("remove_wire", json!({}), &top["wires"][0]["agent_id"]),
        ("set_main_circuit", json!({}), &top["agent_id"]),
        ("rename_circuit", json!({"name":"x"}), &top["agent_id"]),
        ("remove_circuit", json!({}), &top["agent_id"]),
    ] {
        for (fields, code) in [
            (json!({}), "invalid_arguments"),
            (json!({"id":1,"agent_id":tag}), "invalid_arguments"),
            (json!({"id":null,"agent_id":tag}), "invalid_arguments"),
            (json!({"id":1,"agent_id":null}), "invalid_arguments"),
            (json!({"agent_id":"bad"}), "invalid_arguments"),
            (json!({"agent_id":tag,"circuit":2}), "invalid_edit"),
        ] {
            let mut op = extra.clone();
            op["op"] = name.into();
            op.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            let error = f
                .patch(json!([{"op":"add_circuit","name":"discard"},op]), false)
                .unwrap_err();
            assert_eq!(error.code, code, "{op}");
            assert_eq!(error.details["op_index"], 1, "{op}");
        }
    }
    let unknown = "c-00000000000000000000000000000000";
    for (op, code) in [
        (
            json!({"op":"remove_component","agent_id":unknown}),
            "invalid_edit",
        ),
        (
            json!({"op":"remove_component","agent_id":top["wires"][0]["agent_id"]}),
            "invalid_arguments",
        ),
        (
            json!({"op":"add_component","kind":"subcircuit","circuit":1,"position":[0,0],"circuit_id":2,"circuit_agent_id":top["agent_id"]}),
            "invalid_arguments",
        ),
        (
            json!({"op":"remove_wire","id":3,"unknown":true}),
            "invalid_arguments",
        ),
    ] {
        let error = f.patch(json!([op]), true).unwrap_err();
        assert_eq!(error.code, code, "{op}");
        assert_eq!(error.details["op_index"], 0);
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn wire_changes_report_survivors_after_merging_splitting_and_duplicate_adds() {
    let mut f = Fixture::new(SOURCE);
    let wire =
        |left, right| json!({"op":"add_wire","circuit":1,"from":[left,100],"to":[right,100]});
    let old = f.patch(json!([wire(200, 300)]), false).unwrap()["changes"][0].clone();
    let extended = f.patch(json!([wire(100, 200)]), true).unwrap()["changes"][0].clone();
    assert_eq!(
        extended,
        f.patch(json!([wire(100, 200)]), false).unwrap()["changes"][0]
    );
    // The core keeps the new leftmost numeric ID. The original wire's persistent identity survives.
    assert_ne!(extended["id"], old["id"]);
    assert_eq!(extended["agent_id"], old["agent_id"]);
    assert_eq!(extended["wire_agent_ids"], json!([old["agent_id"]]));
    let split =
        json!([{"op":"add_component","circuit":1,"kind":"Pin","position":[200,100]},wire(100,300)]);
    let before = f.snapshot();
    let dry = f.patch(split.clone(), true).unwrap()["changes"].clone();
    assert_eq!(dry, f.patch(split.clone(), true).unwrap()["changes"]);
    assert!(
        f.patch(json!([split[0],{"op":"remove_wire","id":99999}]), false)
            .is_err()
    );
    assert_eq!(f.snapshot(), before);
    assert_eq!(dry, f.patch(split, false).unwrap()["changes"]);
    let snap = f.snapshot();
    let change = &dry[1];
    let tags = change["wire_agent_ids"].as_array().unwrap();
    let ids = change["wire_ids"].as_array().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(tags.len(), 2);
    assert_ne!(tags[0], tags[1]);
    assert!(tags.contains(&old["agent_id"]));
    assert_eq!(change["agent_id"], tags[0]);
    assert_eq!(change["id"], ids[0]);
    for (tag, id) in tags.iter().zip(ids) {
        let w = snap["circuits"][0]["wires"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == *id)
            .unwrap();
        assert_eq!(w["agent_id"], *tag);
        let duplicate = f
            .patch(
                json!([{"op":"add_wire","circuit":1,"from":w["to"],"to":w["from"]}]),
                false,
            )
            .unwrap();
        assert_eq!(duplicate["changed"], false);
        assert_eq!(duplicate["changes"][0]["wire_agent_ids"], json!([tag]));
    }
    f.patch(json!([{"op":"remove_wire","agent_id":tags[1]}]), false)
        .unwrap();
    let after = f.snapshot();
    let wires = after["circuits"][0]["wires"].as_array().unwrap();
    assert!(wires.iter().any(|w| w["agent_id"] == tags[0]));
    assert!(!wires.iter().any(|w| w["agent_id"] == tags[1]));
}

#[test]
fn duplicate_document_reference_rejects_even_a_scoped_edit() {
    let mut f = Fixture::new(SOURCE);
    let top = f.snapshot()["circuits"][0].clone();
    let session = f.cli.sessions.get_mut(&f.id).unwrap();
    let circuit = &mut session.project.circuit_mut(1).unwrap().circuit;
    let ids = &mut circuit.agent_ids.items;
    let tag = ids[&circuit.components[0].id];
    ids.insert(circuit.components[1].id, tag);
    let op =
        json!({"op":"remove_component","agent_id":top["components"][0]["agent_id"],"circuit":1});
    let bad = f.patch(json!([op]), false).unwrap_err();
    assert_eq!(bad.code, "ambiguous_id");
    assert_eq!(bad.details["op_index"], 0);
    assert_eq!(
        f.snapshot()["circuits"][0]["components"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn wire_added_then_removed_with_its_circuit_has_no_surviving_segments() {
    let mut f = Fixture::new(SOURCE);
    let ops = json!([
        {"op":"add_circuit","name":"temporary"},
        {"op":"add_wire","circuit":2,"from":[10,10],"to":[50,10]},
        {"op":"remove_circuit","id":2}
    ]);
    let dry = f.patch(ops.clone(), true).unwrap();
    let changes = f.patch(ops, false).unwrap()["changes"].clone();
    assert_eq!(changes, dry["changes"]);
    let wire = &changes[1];
    assert!(wire["id"].is_u64());
    assert!(
        logisim_core::AgentIdKind::Wire
            .decode(wire["agent_id"].as_str().unwrap())
            .is_some()
    );
    assert_eq!(wire["wire_ids"], json!([]));
    assert_eq!(wire["wire_agent_ids"], json!([]));
    assert_eq!(f.snapshot()["circuits"].as_array().unwrap().len(), 1);
}

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
    f.patch(json!([{"op":"remove_component","agent_id":instance["agent_id"]},
        {"op":"set_main_circuit","agent_id":top["agent_id"]},{"op":"remove_circuit","agent_id":child}]),false).unwrap();
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

use super::{Fixture, SOURCE};
use logisim_core::AgentIdKind;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs};

fn tags(circuit: &Value, group: &str) -> BTreeSet<String> {
    circuit[group]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["agent_id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn session_ids_are_stable_through_previews_rollback_moves_updates_and_wire_splits() {
    let mut f = Fixture::new(SOURCE);
    let initial = f.snapshot();
    assert_eq!(f.snapshot(), initial);
    assert_eq!(fs::read(&f.path).unwrap(), SOURCE.as_bytes());
    let before = &initial["circuits"][0];
    assert!(
        AgentIdKind::Circuit
            .decode(before["agent_id"].as_str().unwrap())
            .is_some()
    );
    let pins = tags(before, "components");
    assert_eq!(pins.len(), 2);
    assert!(
        pins.iter()
            .all(|id| AgentIdKind::Component.decode(id).is_some())
    );
    let wire = before["wires"][0]["agent_id"].as_str().unwrap();
    assert!(AgentIdKind::Wire.decode(wire).is_some());
    let op = json!([{"op":"move_component","id":1,"position":[10,50]},
        {"op":"update_component","id":2,"properties":{"label":"reading"}},
        {"op":"add_circuit","name":"child"}]);
    f.call("simulation_open", json!({})).unwrap();
    let runtime = f.call("simulation_state", json!({})).unwrap();
    f.patch(op.clone(), true).unwrap();
    let error = f
        .patch(json!([op[0],{"op":"remove_component","id":999}]), false)
        .unwrap_err();
    assert_eq!(error.details["op_index"], 1);
    assert_eq!(f.snapshot(), initial);
    assert_eq!(f.call("simulation_state", json!({})).unwrap(), runtime);
    f.patch(op, false).unwrap();
    let changed = f.snapshot();
    let top = &changed["circuits"][0];
    assert_eq!(top["agent_id"], before["agent_id"]);
    assert_eq!(tags(top, "components"), pins);
    assert_eq!(
        top["components"][0]["agent_id"],
        before["components"][0]["agent_id"]
    );
    assert_eq!(
        top["components"][1]["agent_id"],
        before["components"][1]["agent_id"]
    );
    assert_ne!(changed["circuits"][1]["agent_id"], top["agent_id"]);
    let wires = tags(top, "wires");
    assert!(wires.contains(wire));
    assert_eq!(wires.len(), 2);
    assert_eq!(fs::read(&f.path).unwrap(), SOURCE.as_bytes());
    let remove_id = &top["wires"][0]["id"];
    let removed = &top["wires"][0]["agent_id"];
    f.patch(json!([{"op":"remove_wire","id":remove_id},
        {"op":"add_wire","circuit":1,"from":[200,200],"to":[300,200]},
        {"op":"remove_component","id":2},{"op":"add_component","circuit":1,"kind":"Probe","position":[70,10]},
        {"op":"remove_circuit","id":2},{"op":"add_circuit","name":"child"}]),false).unwrap();
    let new = f.snapshot();
    assert!(!tags(&new["circuits"][0], "wires").contains(removed.as_str().unwrap()));
    assert_ne!(
        new["circuits"][0]["components"][1]["agent_id"],
        before["components"][1]["agent_id"]
    );
    assert_ne!(
        new["circuits"][1]["agent_id"],
        changed["circuits"][1]["agent_id"]
    );
    assert_eq!(
        new["circuits"][0]["components"][0]["agent_id"],
        before["components"][0]["agent_id"]
    );
}

#[test]
fn repair_only_id_warnings_allow_explicit_save_but_omissions_still_protect_source() {
    let known = "c-0123456789abcdef0123456789abcdef";
    let xml = SOURCE
        .replace(
            r#"loc="(10,10)""#,
            &format!(r#"loc="(10,10)" agent-id="{known}""#),
        )
        .replace(
            r#"loc="(70,10)""#,
            &format!(r#"loc="(70,10)" agent-id="{known}""#),
        )
        .replace("<wire ", r#"<wire agent-id="bad" "#);
    let mut f = Fixture::new(&xml);
    let snapshot = f.snapshot();
    let warnings = snapshot["warnings"].to_string();
    assert!(warnings.contains("Invalid agent-id") && warnings.contains("Duplicate agent-id"));
    assert_eq!(snapshot["partial"], false);
    let top = &snapshot["circuits"][0];
    assert_eq!(top["components"][0]["agent_id"], known);
    assert_eq!(tags(top, "components").len(), 2);
    let wire = top["wires"][0]["agent_id"].as_str().unwrap();
    assert!(AgentIdKind::Wire.decode(wire).is_some());
    assert_eq!(fs::read(&f.path).unwrap(), xml.as_bytes());
    f.call("project_save", json!({})).unwrap();
    assert_eq!(fs::read(f.backup()).unwrap(), xml.as_bytes());
    f.id = super::open(&mut f.cli, &f.path);
    let clean = f.snapshot();
    assert_eq!(clean["warnings"], json!([]));
    assert_eq!(
        tags(&clean["circuits"][0], "components"),
        tags(top, "components")
    );
    assert_eq!(clean["circuits"][0]["wires"][0]["agent_id"], wire);
    let partial = xml.replace("</project>", "<unknown/></project>");
    let mut f = Fixture::new(&partial);
    assert_eq!(f.snapshot()["partial"], true);
    assert_eq!(
        f.call("project_save", json!({})).unwrap_err().code,
        "partial_project"
    );
    assert_eq!(fs::read(&f.path).unwrap(), partial.as_bytes());
    assert!(!f.backup().exists());
}

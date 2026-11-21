use super::{Session, action_project::register_file};
use serde_json::json;

#[test]
fn register_file_actions_change_private_state_and_drive_the_selected_register() {
    for mode in ["stable", "event", "auto"] {
        let mut s = Session::new(&register_file());
        let snapshot = s.call("project_snapshot", json!({})).unwrap();
        let rf = &snapshot["circuits"][0]["components"][0];
        let write = json!({"id":rf["id"],"action":"write","argument":"1 DEADBEEF"});
        assert_eq!(
            s.call("simulation_plugin_action", write.clone())
                .unwrap_err()
                .code,
            "simulation_not_open"
        );
        let initial = s.call("simulation_open", json!({"mode":mode})).unwrap();
        assert_eq!(
            initial["engine"],
            if mode == "event" { "event" } else { "stable" },
            "{initial}"
        );
        assert_eq!(initial["probes"][0]["value"]["number"], 0);
        let first = s.act("simulation_plugin_action", write.clone());
        assert_eq!(first["action"], "plugin_action");
        assert_eq!(first["changed"], true);
        assert_eq!(
            first["state"]["probes"][0]["value"]["number"],
            0xdead_beefu32
        );
        assert_eq!(first["state"]["ticks"], 0);
        assert_eq!(first["state"]["pending_count"], 0);
        assert_eq!(s.act("simulation_plugin_action", write)["changed"], false);
        let hidden = json!({"agent_id":rf["agent_id"],"action":"write","argument":"2 FACE"});
        let second = s.act("simulation_plugin_action", hidden.clone());
        assert_eq!(second["changed"], true);
        assert_eq!(second["state"]["probes"], first["state"]["probes"]);
        assert_eq!(s.act("simulation_plugin_action", hidden)["changed"], false);
        assert_eq!(s.call("project_snapshot", json!({})).unwrap(), snapshot);
    }
}

#[test]
fn paused_actions_keep_outputs_pending_and_guest_errors_can_be_reset() {
    for mode in ["event", "stable", "auto"] {
        let mut s = Session::new(&register_file());
        // First materialize the legacy objects' agent IDs so explicit saves can be compared byte for byte.
        let saved = s.call("project_save", json!({})).unwrap();
        let path = s.temp.0.join("test.circ");
        let bytes = std::fs::read(&path).unwrap();
        let snapshot = s.call("project_snapshot", json!({})).unwrap();
        let rf = &snapshot["circuits"][0]["components"][0];
        let write =
            |word: &str| json!({"agent_id":rf["agent_id"],"action":"write","argument":word});
        s.call("simulation_open", json!({"mode":mode})).unwrap();
        let pause = s.act("simulation_set_enabled", json!({"enabled":false}));
        let queued = s.act("simulation_plugin_action", write("1 DEADBEEF"));
        assert_eq!(queued["changed"], true);
        let state = &queued["state"];
        assert_eq!(state["enabled"], false);
        assert_eq!(state["steps"], pause["state"]["steps"]);
        assert_eq!(state["ticks"], pause["state"]["ticks"]);
        assert_eq!(state["probes"][0]["value"]["number"], 0);
        assert!(state["pending_count"].as_u64().unwrap() > 0);
        // Rejected arguments must not accidentally settle the outstanding write.
        let invalid = write(&"é".repeat(513));
        assert_eq!(
            s.call("simulation_plugin_action", invalid)
                .unwrap_err()
                .code,
            "invalid_argument"
        );
        assert_eq!(s.state(), *state);
        let settled = s.act("simulation_settle", json!({}));
        assert_eq!(settled["state"]["enabled"], false);
        assert_eq!(
            settled["state"]["probes"][0]["value"]["number"],
            0xdead_beefu32
        );
        assert_eq!(settled["state"]["pending_count"], 0);
        assert_eq!(settled["state"]["ticks"], 0);

        // Register 0 is read-only in this Wasm. Its diagnostic must survive a paused action.
        let error = s
            .call("simulation_plugin_action", write("0 FF"))
            .unwrap_err();
        assert_eq!(error.code, "plugin_error");
        assert!(error.message.contains("1–31"), "{}", error.message);
        let failed = s.state();
        assert_eq!(failed, error.details["state"]);
        assert_eq!(failed["enabled"], false);
        assert!(
            failed["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["problem"] == "plugin_error" && d["components"] == json!([rf["id"]]))
        );
        assert_eq!(
            s.call("simulation_plugin_action", write("1 01"))
                .unwrap_err()
                .code,
            "invalid_state"
        );
        assert_eq!(s.state(), failed);
        let reset = s.act("simulation_reset", json!({}));
        assert!(reset["state"]["error"].is_null());
        assert_eq!(reset["state"]["probes"][0]["value"]["number"], 0);
        s.act("simulation_set_enabled", json!({"enabled":true}));
        let repaired = s.act("simulation_plugin_action", write("1 ABC"));
        assert_eq!(repaired["state"]["probes"][0]["value"]["number"], 0xabc);
        assert_eq!(repaired["state"]["ticks"], 0);

        assert_eq!(s.call("project_snapshot", json!({})).unwrap(), snapshot);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let again = s
            .call(
                "project_save",
                json!({"expected_sha256":saved["source_sha256"]}),
            )
            .unwrap();
        assert_eq!(again["source_sha256"], saved["source_sha256"]);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn validation_rejects_nested_targets_keyboard_events_and_invalid_arguments_without_execution() {
    use super::action_project::plugin_tool;
    use logisim_core::{Component, Point};
    let mut project = register_file();
    let child = project.add("child".into()).unwrap();
    let rf = project.main_circuit().circuit.components[0].clone();
    project
        .circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(rf);
    let (source, keyboard) = plugin_tool("input", "keyboard");
    project.plugin_sources.push(source);
    project
        .main_circuit_mut()
        .circuit
        .add_component(Component::from_plugin(keyboard, Point::new(800, 300)));
    let mut s = Session::new(&project);
    let snap = s.call("project_snapshot", json!({})).unwrap();
    let main = &snap["circuits"][0]["components"];
    let keyboard = &main[3];
    let nested = &snap["circuits"][1]["components"][0];
    s.call("simulation_open", json!({"mode":"event"})).unwrap();
    let before = s.state();
    for (args, code) in [
        (
            json!({"id":main[1]["id"],"action":"write"}),
            "invalid_plugin_target",
        ),
        (json!({"id":999,"action":"write"}), "invalid_plugin_target"),
        (
            json!({"agent_id":nested["agent_id"],"action":"write"}),
            "invalid_plugin_target",
        ),
        (
            json!({"agent_id":"not-a-component","action":"write"}),
            "invalid_plugin_target",
        ),
        (
            json!({"id":main[0]["id"],"action":"missing"}),
            "invalid_action",
        ),
        (
            json!({"id":main[0]["id"],"action":"write","argument":"x".repeat(1025)}),
            "invalid_argument",
        ),
        (
            json!({"id":keyboard["id"],"action":"key","argument":r#"{"type":"release_all"}"#}),
            "invalid_action",
        ),
        (
            json!({"id":main[0]["id"],"action":"write","path":[1]}),
            "invalid_arguments",
        ),
    ] {
        assert_eq!(
            s.call("simulation_plugin_action", args).unwrap_err().code,
            code
        );
        assert_eq!(s.state(), before);
    }
    let full = format!("{:1024}", "1 A");
    assert_eq!(
        s.act(
            "simulation_plugin_action",
            json!({"id":main[0]["id"],"action":"write","argument":full})
        )["state"]["probes"][0]["value"]["number"],
        10
    );
}

#[test]
fn action_without_an_argument_rejects_text_and_a_missing_module_never_reports_success() {
    // An offline saved tool still describes its argument contract.
    let mut p = register_file();
    p.plugin_sources[0].path = "missing.wasm".into();
    p.main_circuit_mut().circuit.components[0]
        .plugin
        .as_mut()
        .unwrap()
        .tool
        .actions[0]
        .argument = None;
    let mut s = Session::new(&p);
    s.call("simulation_open", json!({"mode":"event"})).unwrap();
    let before = s.state();
    assert_eq!(before["plugins"][0]["status"], "missing");
    for (args, code) in [
        (
            json!({"id":1,"action":"write","argument":"x"}),
            "invalid_argument",
        ),
        (
            json!({"id":1,"action":"write","argument":""}),
            "plugin_error",
        ),
        (json!({"id":1,"action":"write"}), "plugin_error"),
    ] {
        assert_eq!(
            s.call("simulation_plugin_action", args).unwrap_err().code,
            code
        );
        assert_eq!(s.state(), before);
    }
}

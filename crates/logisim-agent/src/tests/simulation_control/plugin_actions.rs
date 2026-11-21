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

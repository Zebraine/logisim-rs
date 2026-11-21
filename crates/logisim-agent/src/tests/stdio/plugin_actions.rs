use super::{Client, action_project::register_file};
use serde_json::{Value, json};
use std::fs;

#[test]
fn cli_and_mcp_dispatch_root_plugin_actions_and_report_matching_results() {
    let dir = std::env::temp_dir().join(format!("agent-action-{:016x}", getrandom::u64().unwrap()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("registers.circ");
    let xml = logisim_file::serialize(&register_file()).unwrap();
    fs::write(&path, &xml).unwrap();
    let root = dir.to_str().unwrap();
    let mut cli = Client::start(&[
        "--root",
        root,
        "project",
        "open",
        "--path",
        "registers.circ",
    ]);
    let first = cli.read()["data"]["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut mcp = Client::start(&["--root", root, "mcp"]);
    mcp.initialize("2025-11-25");
    let list = mcp.call("tools/list", json!({}));
    let schema = &list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "simulation_plugin_action")
        .unwrap()["inputSchema"];
    assert_eq!(schema["required"], json!(["session", "action"]));
    assert_eq!(
        schema["oneOf"],
        json!([
            {"required":["id"],"properties":{"id":{"type":"integer"}}},
            {"required":["agent_id"],"properties":{"agent_id":{"type":"string"}}}
        ])
    );
    assert_eq!(schema["properties"]["argument"]["x-maxUtf8Bytes"], 1024);
    assert_eq!(schema["properties"]["argument"]["default"], "");
    assert_eq!(schema["additionalProperties"], false);
    let opened = mcp.tool("project_open", json!({"path":"registers.circ"}));
    let second = opened["result"]["structuredContent"]["data"]["session_id"]
        .as_str()
        .unwrap();
    for (command, tool, flags, mut args, expected_error) in [
        (
            "plugin-action",
            "simulation_plugin_action",
            vec!["--id", "1", "--action", "write"],
            json!({"id":1,"action":"write"}),
            Some("simulation_not_open"),
        ),
        ("open", "simulation_open", vec![], json!({}), None),
        (
            "plugin-action",
            "simulation_plugin_action",
            vec!["--id", "1", "--action", "write", "--argument", "1 DEADBEEF"],
            json!({"id":1,"agent_id":null,"action":"write","argument":"1 DEADBEEF"}),
            None,
        ),
        (
            "plugin-action",
            "simulation_plugin_action",
            vec![
                "--agent-id",
                "c-00000000000000000000000000000042",
                "--action",
                "write",
                "--argument",
                "1 D",
            ],
            json!({"agent_id":"c-00000000000000000000000000000042","action":"write","argument":"1 D"}),
            None,
        ),
        (
            "plugin-action",
            "simulation_plugin_action",
            vec!["--id", "1", "--action", "go"],
            json!({"id":1,"action":"go"}),
            Some("invalid_action"),
        ),
        (
            "plugin-action",
            "simulation_plugin_action",
            vec!["--action", "write"],
            json!({"action":"write"}),
            Some("invalid_arguments"),
        ),
        (
            "plugin-action",
            "simulation_plugin_action",
            vec![
                "--id",
                "1",
                "--agent-id",
                "c-00000000000000000000000000000042",
                "--action",
                "write",
            ],
            json!({"id":1,"agent_id":"c-00000000000000000000000000000042","action":"write"}),
            Some("invalid_arguments"),
        ),
        // The ABI receives an empty string for an omitted argument. The course plugin rejects it at execution.
        (
            "plugin-action",
            "simulation_plugin_action",
            vec!["--id", "1", "--action", "write", "--argument", ""],
            json!({"id":1,"action":"write"}),
            Some("plugin_error"),
        ),
        ("reset", "simulation_reset", vec![], json!({}), None),
    ] {
        let mut words = vec!["simulation", command, "--session", &first];
        words.extend(flags);
        cli.send(json!(words));
        let mut a = cli.read();
        args["session"] = second.into();
        let rpc = mcp.tool(tool, args);
        let result = &rpc["result"];
        assert_eq!(result["isError"], expected_error.is_some(), "{rpc}");
        assert_eq!(a["ok"], expected_error.is_none(), "{a}");
        let mut b = result["structuredContent"].clone();
        assert_eq!(
            serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
            b
        );
        if let Some(code) = expected_error {
            assert_eq!(a["error"]["code"], code);
        }
        for reply in [&mut a, &mut b] {
            for pointer in [
                "/data/session_id",
                "/data/state/session_id",
                "/error/details/state/session_id",
            ] {
                if let Some(id) = reply.pointer_mut(pointer) {
                    *id = "session".into();
                }
            }
        }
        assert_eq!(a, b, "{tool}");
    }
    cli.finish();
    mcp.finish();
    assert_eq!(fs::read(&path).unwrap(), xml.as_bytes());
    fs::remove_dir_all(dir).unwrap();
}

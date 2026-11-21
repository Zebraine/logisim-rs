use super::{BIN, Client, tool_error};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

const PROJECT: &str = r#"<project source="logisim-rs-plugin-v1" version="1.0"><main name="top"/><circuit name="top"/></project>"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("agent-rooted-{:016x}", getrandom::u64().unwrap()));
        let root = path.join("work");
        fs::create_dir(&path).unwrap();
        fs::create_dir(&root).unwrap();
        fs::write(root.join("top.circ"), PROJECT).unwrap();
        fs::write(root.join("prefs.json"), r#"{"version":16}"#).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn cli_root_applies_to_single_commands_and_streamed_patch_files() {
    let fixture = Fixture::new();
    let root = fixture.0.join("work");
    fs::write(fixture.0.join("outside.json"), "invalid json").unwrap();
    for (args, error) in [
        (vec!["settings", "get", "--config", "prefs.json"], None),
        (vec!["project", "open", "--path", "top.circ"], None),
        (
            vec![
                "settings",
                "patch",
                "--config",
                "prefs.json",
                "--ops",
                "../outside.json",
                "--apply",
            ],
            Some("path_outside_root"),
        ),
        (
            vec![
                "project",
                "patch",
                "--session",
                "closed",
                "--ops",
                "../outside.json",
                "--apply",
            ],
            Some("path_outside_root"),
        ),
    ] {
        let output = Command::new(BIN)
            .args(["--root", "work"])
            .args(args)
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.success(), error.is_none(), "{envelope}");
        assert_eq!(envelope["ok"], error.is_none());
        assert_eq!(envelope["error"]["code"].as_str(), error);
    }
    let mut cli = Client::start(&["--root", root.to_str().unwrap(), "cli"]);
    cli.send(json!([
        "settings",
        "patch",
        "--config",
        "prefs.json",
        "--ops",
        "../outside.json",
        "--apply"
    ]));
    assert_eq!(cli.read()["error"]["code"], "path_outside_root");
    cli.send(json!({"tool":"settings_get","arguments":{"config":"../outside.json"}}));
    assert_eq!(cli.read()["error"]["code"], "path_outside_root");
    cli.send(json!(["settings", "get", "--config", "prefs.json"]));
    assert_eq!(cli.read()["ok"], true);
    cli.finish();
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    assert_eq!(
        fs::read(root.join("prefs.json")).unwrap(),
        br#"{"version":16}"#
    );
}

#[test]
fn rooted_mcp_returns_domain_errors_and_preserves_the_source() {
    let fixture = Fixture::new();
    let root = fixture.0.join("work");
    let mut mcp = Client::start(&["--root", root.to_str().unwrap(), "mcp"]);
    mcp.initialize("2024-11-05");
    let opened = mcp.tool("project_open", json!({"path":"top.circ"}));
    assert_eq!(opened["result"]["isError"], false);
    let session = &opened["result"]["structuredContent"]["data"]["session_id"];
    let args = json!({"session":session});
    let error = tool_error(&mcp.tool("simulation_state", args.clone()));
    assert_eq!(error["code"], "simulation_not_open");
    let error = tool_error(&mcp.tool(
        "project_patch",
        json!({
            "session":session,"dry_run":false,"ops":[{"op":"add_component","circuit":1,
            "kind":"Pin","position":[0,0],"properties":{"bits":0}}]
        }),
    ));
    assert_eq!(error["code"], "invalid_property");
    assert_eq!(error["details"]["op_index"], 0);
    let error = tool_error(&mcp.tool(
        "project_save",
        json!({
            "session":session,"expected_sha256":"0".repeat(64)
        }),
    ));
    assert_eq!(error["code"], "conflict");
    fs::create_dir(root.join("top.circ.bak")).unwrap();
    assert_eq!(
        tool_error(&mcp.tool("project_save", args.clone()))["code"],
        "backup_failed"
    );
    assert_eq!(fs::read(root.join("top.circ")).unwrap(), PROJECT.as_bytes());
    fs::remove_file(root.join("top.circ")).unwrap();
    assert_eq!(
        tool_error(&mcp.tool("project_save", args))["code"],
        "conflict"
    );

    let partial = PROJECT.replace("</project>", "<unknown/></project>");
    fs::write(root.join("partial.circ"), &partial).unwrap();
    let opened = mcp.tool("project_open", json!({"path":"partial.circ"}));
    let session = &opened["result"]["structuredContent"]["data"]["session_id"];
    assert_eq!(
        tool_error(&mcp.tool("project_save", json!({"session":session})))["code"],
        "partial_project"
    );
    assert_eq!(
        fs::read(root.join("partial.circ")).unwrap(),
        partial.as_bytes()
    );
    assert!(!root.join("partial.circ.lock").exists());
    mcp.finish();
}

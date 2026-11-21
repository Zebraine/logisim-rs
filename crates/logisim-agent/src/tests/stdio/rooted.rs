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

#[test]
fn two_sessions_saving_one_source_keep_the_winning_project_and_original_backup() {
    let fixture = Fixture::new();
    let root = fixture.0.join("work");
    let mut c = Client::start(&["--root", root.to_str().unwrap(), "mcp"]);
    c.initialize("2025-11-25");
    let sessions:Vec<_> = ["first","second"].into_iter().map(|name| {
        let opened = c.tool("project_open",json!({"path":"top.circ"}));
        let session = opened["result"]["structuredContent"]["data"]["session_id"].clone();
        assert_eq!(c.tool("project_patch",json!({"session":session,"dry_run":false,"ops":[{"op":"add_circuit","name":name}]}))["result"]["isError"],false);
        session
    }).collect();
    for (id, session) in sessions.iter().enumerate() {
        c.send(json!({"id":id,"jsonrpc":"2.0","method":"tools/call","params":{"name":"project_save","arguments":{"session":session}}}));
    }
    let mut winner = None;
    let mut loser = None;
    for _ in 0..2 {
        let r = c.read();
        let id = r["id"].as_u64().unwrap() as usize;
        if r["result"]["isError"] == true {
            assert_eq!(tool_error(&r)["code"], "conflict");
            loser = Some(id);
        } else {
            assert_eq!(
                r["result"]["structuredContent"]["data"]["session_id"],
                sessions[id]
            );
            winner = Some(id);
        }
    }
    let names = ["first", "second"];
    let saved = fs::read_to_string(root.join("top.circ")).unwrap();
    assert!(saved.contains(names[winner.unwrap()]));
    assert!(!saved.contains(names[loser.unwrap()]));
    assert_eq!(
        fs::read(root.join("top.circ.bak")).unwrap(),
        PROJECT.as_bytes()
    );
    assert!(logisim_preferences::storage::lock_target(&root.join("top.circ")).is_ok());
    c.finish();
}

#[test]
fn pipelined_settings_writes_and_read_share_one_ordered_disk_revision() {
    let fixture = Fixture::new();
    let root = fixture.0.join("work");
    let mut c = Client::start(&["--root", root.to_str().unwrap(), "mcp"]);
    c.initialize("2025-11-25");
    let before = c.tool("settings_get", json!({"config":"prefs.json"}));
    let hash = &before["result"]["structuredContent"]["data"]["source_sha256"];
    for (id, theme) in [(0, "dark"), (1, "light")] {
        c.send(json!({"id":id,"jsonrpc":"2.0","method":"tools/call","params":{"name":"settings_patch",
            "arguments":{"config":"prefs.json","dry_run":false,"expected_sha256":hash,"ops":[{"key":"appearance.theme","value":theme}]}}}));
    }
    c.send(json!({"id":2,"jsonrpc":"2.0","method":"tools/call","params":{"name":"settings_get","arguments":{"config":"prefs.json"}}}));
    let applied = c.read();
    assert_eq!(applied["id"], 0);
    assert_eq!(applied["result"]["isError"], false);
    let stale = c.read();
    assert_eq!(stale["id"], 1);
    assert_eq!(tool_error(&stale)["code"], "conflict");
    let current = c.read();
    assert_eq!(current["id"], 2);
    assert_eq!(
        current["result"]["structuredContent"]["data"]["settings"]["appearance"]["theme"],
        "dark"
    );
    c.finish();
}

#[test]
fn conversion_checks_the_target_and_keeps_each_session_bound_to_its_format() {
    let fixture = Fixture::new();
    let root = fixture.0.join("work");
    let mut mcp = Client::start(&["--root", root.to_str().unwrap(), "mcp"]);
    mcp.initialize("2025-11-25");
    let open = mcp.tool("project_open", json!({"path":"top.circ"}));
    let session = &open["result"]["structuredContent"]["data"]["session_id"];
    let mut args = json!({"session":session,"output":"board.lrs"});
    let created = mcp.tool("project_convert", args.clone());
    assert_eq!(created["result"]["isError"], false, "{created}");
    let sha = &created["result"]["structuredContent"]["data"]["sha256"];
    let original = fs::read(root.join("board.lrs")).unwrap();
    assert_eq!(
        tool_error(&mcp.tool("project_convert", args.clone()))["code"],
        "conflict"
    );
    args["expected_output_sha256"] = sha.clone();
    let lock = logisim_preferences::storage::lock_target(&root.join("board.lrs")).unwrap();
    assert_eq!(
        tool_error(&mcp.tool("project_convert", args.clone()))["code"],
        "conflict"
    );
    assert!(!root.join("board.lrs.bak").exists());
    lock.unlock().unwrap();
    drop(lock);
    let backup = root.join("board.lrs.bak");
    fs::create_dir(&backup).unwrap();
    let failure = tool_error(&mcp.tool("project_convert", args.clone()));
    assert_eq!(failure["code"], "backup_failed", "{failure}");
    assert_eq!(fs::read(root.join("board.lrs")).unwrap(), original);
    fs::remove_dir(&backup).unwrap();
    let changed = mcp.tool(
        "project_patch",
        json!({"session":session,"dry_run":false,"ops":[{"op":"add_circuit","name":"extra"}]}),
    );
    assert_eq!(changed["result"]["isError"], false, "{changed}");
    assert_eq!(
        mcp.tool("project_convert", args)["result"]["isError"],
        false
    );
    assert_eq!(fs::read(root.join("board.lrs.bak")).unwrap(), original);
    assert_eq!(fs::read_to_string(root.join("top.circ")).unwrap(), PROJECT);
    let snap = mcp.tool("project_snapshot", json!({"session":session}));
    assert_eq!(
        snap["result"]["structuredContent"]["data"]["path"],
        root.join("top.circ").to_str().unwrap()
    );
    let copy_before_save = fs::read(root.join("board.lrs")).unwrap();
    let saved = mcp.tool("project_save", json!({"session":session}));
    assert_eq!(saved["result"]["isError"], false, "{saved}");
    assert_eq!(
        fs::read_to_string(root.join("top.circ.bak")).unwrap(),
        PROJECT
    );
    let xml = fs::read_to_string(root.join("top.circ")).unwrap();
    assert!(
        logisim_file::parse(&xml)
            .unwrap()
            .document
            .circuits
            .iter()
            .any(|c| c.name == "extra")
    );
    assert_eq!(fs::read(root.join("board.lrs")).unwrap(), copy_before_save);
    let opened = mcp.tool("project_open", json!({"path":"board.lrs"}));
    let native = &opened["result"]["structuredContent"]["data"]["session_id"];
    assert_eq!(
        mcp.tool("project_save", json!({"session":native}))["result"]["isError"],
        false
    );
    assert!(logisim_file::parse_native(&fs::read(root.join("board.lrs")).unwrap()).is_ok());
    assert_eq!(
        tool_error(&mcp.tool(
            "project_convert",
            json!({"session":native,"output":"../outside.circ"})
        ))["code"],
        "path_outside_root"
    );
    mcp.finish();
}

#[test]
fn partial_import_and_unrepresentable_native_copy_preserve_both_files() {
    let fixture = Fixture::new();
    let root = fixture.0.join("work");
    let partial = PROJECT.replace("</project>", "<unknown/></project>");
    fs::write(root.join("partial.circ"), &partial).unwrap();
    let mut cli = Client::start(&["--root", root.to_str().unwrap(), "cli"]);
    cli.send(json!(["project", "open", "--path", "partial.circ"]));
    let s = cli.read()["data"]["session_id"].clone();
    cli.send(json!([
        "project",
        "convert",
        "--session",
        s,
        "--output",
        "blocked.lrs"
    ]));
    let e = cli.read();
    assert_eq!(e["error"]["code"], "partial_project");
    assert!(
        !e["error"]["details"]["warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!root.join("blocked.lrs").exists());
    assert_eq!(
        fs::read_to_string(root.join("partial.circ")).unwrap(),
        partial
    );
    let mut p = logisim_core::Project::default();
    let mut c =
        logisim_core::Component::new(logisim_core::Kind::Joystick, logisim_core::Point::ZERO);
    c.font.size = 24;
    p.main_circuit_mut().circuit.add_component(c);
    logisim_file::native::IdentityAssignments::default()
        .complete(&mut p)
        .unwrap();
    let bytes = logisim_file::serialize_native(&p).unwrap();
    fs::write(root.join("stick.lrs"), &bytes).unwrap();
    cli.send(json!(["project", "open", "--path", "stick.lrs"]));
    let s = cli.read()["data"]["session_id"].clone();
    cli.send(json!(["project", "open", "--path", "top.circ"]));
    let sha = cli.read()["data"]["source_sha256"].clone();
    cli.send(json!([
        "project",
        "convert",
        "--session",
        s,
        "--output",
        "top.circ",
        "--expected-output-sha256",
        sha
    ]));
    let e = cli.read();
    assert_eq!(e["error"]["code"], "invalid_project");
    assert!(
        e["error"]["message"]
            .as_str()
            .unwrap()
            .contains("component c-")
    );
    assert_eq!(fs::read(root.join("stick.lrs")).unwrap(), bytes);
    assert_eq!(fs::read_to_string(root.join("top.circ")).unwrap(), PROJECT);
    assert!(!root.join("top.circ.lock").exists() && !root.join("top.circ.bak").exists());
    cli.finish();
}

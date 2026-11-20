//! Exercise the installed binary's framing and process-local sessions through OS pipes.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdout, Command, Stdio},
};
const BIN: &str = env!("CARGO_BIN_EXE_logisim-agent");

struct Client {
    process: Child,
    output: BufReader<ChildStdout>,
}
impl Client {
    fn start(args: &[&str]) -> Self {
        let mut process = Command::new(BIN)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let output = BufReader::new(process.stdout.take().unwrap());
        Self { process, output }
    }
    fn send(&mut self, v: Value) {
        writeln!(self.process.stdin.as_mut().unwrap(), "{v}").unwrap();
    }
    fn read(&mut self) -> Value {
        let mut line = String::new();
        assert!(
            self.output.read_line(&mut line).unwrap() > 0,
            "unexpected EOF"
        );
        serde_json::from_str(&line).unwrap()
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":"test","method":method,"params":params}));
        let v = self.read();
        assert_eq!(v["id"], "test");
        v
    }
    fn tool(&mut self, name: &str, args: Value) -> Value {
        self.call("tools/call", json!({"name":name,"arguments":args}))
    }
    fn finish(mut self) {
        drop(self.process.stdin.take());
        assert!(self.process.wait().unwrap().success());
    }
}

#[test]
fn cli_one_shot_and_session_stream_report_only_protocol_json() {
    for (args, code) in [
        (vec!["settings", "catalog"], None),
        (vec!["settings", "get"], Some("invalid_arguments")),
        (
            vec!["simulation", "state", "--session", "closed"],
            Some("unknown_session"),
        ),
    ] {
        let out = Command::new(BIN).args(args).output().unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["protocol"], 1);
        assert_eq!(v["ok"], code.is_none());
        assert_eq!(out.status.success(), code.is_none());
        assert_eq!(v["error"]["code"].as_str(), code);
    }
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/course/single-cycle.circ");
    let mut c = Client::start(&["project", "open", "--path", path.to_str().unwrap()]);
    let opened = c.read();
    let session = &opened["data"]["session_id"];
    c.send(json!({"tool":"project_snapshot","arguments":{"session":session}}));
    assert_eq!(
        c.read()["data"]["source_sha256"],
        opened["data"]["source_sha256"]
    );
    c.send(json!({"tool":"project_close","arguments":{"session":session}}));
    assert_eq!(c.read()["data"]["closed"], true);
    c.finish();
}

#[test]
fn mcp_handshake_tools_errors_and_session_lifetime_match_cli() {
    let mut m = Client::start(&["mcp"]);
    writeln!(m.process.stdin.as_mut().unwrap(), "invalid").unwrap();
    assert_eq!(m.read()["error"]["code"], -32700);
    assert_eq!(
        m.call("tools/list", json!({}))["error"]["data"]["code"],
        "not_initialized"
    );
    let init=m.call("initialize",json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"test","version":"1"},"capabilities":{}}));
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    m.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let tools = m.call("tools/list", json!({}));
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 10);
    assert_eq!(
        m.tool("settings_patch", json!({"config":"whatever","ops":[]}))["error"]["code"],
        -32602
    );
    assert_eq!(
        m.tool("simulation_tick", json!({}))["error"]["data"]["code"],
        "unknown_tool"
    );
    for invalid in [
        json!(["path", false, null, []]),
        json!({"config":"path","dry_run":false,"ops":[["files.autosave",false]]}),
    ] {
        assert_eq!(m.tool("settings_patch", invalid)["error"]["code"], -32602);
    }
    let catalog = m.tool("settings_catalog", json!({}));
    let cli = Command::new(BIN)
        .args(["settings", "catalog"])
        .output()
        .unwrap();
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(catalog["result"]["structuredContent"], cli);
    assert_eq!(
        serde_json::from_str::<Value>(catalog["result"]["content"][0]["text"].as_str().unwrap())
            .unwrap(),
        cli
    );

    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/course/single-cycle.circ");
    let opened = m.tool("project_open", json!({"path":path}));
    let session = &opened["result"]["structuredContent"]["data"]["session_id"];
    let args = json!({"session":session});
    let snapshot = m.tool("project_snapshot", args.clone());
    let data = &snapshot["result"]["structuredContent"]["data"];
    assert_eq!(data["session_id"], *session);
    assert!(data["circuits"].as_array().unwrap().len() > 1);
    assert_eq!(
        m.tool("simulation_open", args.clone())["result"]["structuredContent"]["data"]["opened"],
        true
    );
    let sim = m.tool("simulation_state", args.clone());
    assert_eq!(sim["result"]["structuredContent"]["data"]["ticks"], 0);
    // An unacknowledged notification must never execute a tool.
    m.send(json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"project_close","arguments":args}}));
    assert_eq!(
        m.tool("project_close", args.clone())["result"]["structuredContent"]["data"]["closed"],
        true
    );
    assert_eq!(
        m.tool("simulation_state", args)["error"]["data"]["code"],
        "unknown_session"
    );
    m.finish();
}

#[test]
fn project_edits_have_the_same_outcome_over_cli_flags_and_mcp() {
    use std::fs;
    let dir = std::env::temp_dir().join(format!(
        "agent-transports-{:016x}",
        getrandom::u64().unwrap()
    ));
    fs::create_dir(&dir).unwrap();
    let source = r##"<project source="logisim-rs-plugin-v1" version="1.0"><main name="top"/><circuit name="top"/></project>"##;
    let ops = json!([{"op":"add_component","circuit":1,"kind":"Pin","position":[20,30],"properties":{"label":"port","bits":8}},{"op":"add_circuit","name":"child"}]);
    let edits = dir.join("ops.json");
    fs::write(&edits, json!({"ops":ops}).to_string()).unwrap();
    let a = dir.join("cli.circ");
    let b = dir.join("mcp.circ");
    fs::write(&a, source).unwrap();
    fs::write(&b, source).unwrap();
    let mut cli = Client::start(&["project", "open", "--path", a.to_str().unwrap()]);
    let ca = cli.read()["data"].clone();
    let mut m = Client::start(&["mcp"]);
    m.call("initialize",json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"parity","version":"1"},"capabilities":{}}));
    m.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let mb =
        m.tool("project_open", json!({"path":b}))["result"]["structuredContent"]["data"].clone();
    let tools = m.call("tools/list", json!({}));
    let patch = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "project_patch")
        .unwrap();
    assert_eq!(
        patch["inputSchema"]["properties"]["ops"]["items"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert!(
        patch["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("dry_run"))
    );

    cli.send(json!([
        "project",
        "patch",
        "--session",
        ca["session_id"],
        "--ops",
        edits
    ]));
    assert_eq!(cli.read()["error"]["code"], "invalid_arguments");
    assert_eq!(
        m.tool(
            "project_patch",
            json!({"session":mb["session_id"],"ops":ops})
        )["error"]["data"]["code"],
        "invalid_arguments"
    );
    for (mode, dry) in [("--dry-run", true), ("--apply", false)] {
        cli.send(json!([
            "project",
            "patch",
            "--session",
            ca["session_id"],
            "--ops",
            edits,
            mode
        ]));
        let a = cli.read();
        let b = m.tool(
            "project_patch",
            json!({"session":mb["session_id"],"ops":ops,"dry_run":dry}),
        );
        assert_eq!(a["ok"], true);
        let data = &b["result"]["structuredContent"]["data"];
        for field in ["source_sha256", "changes", "changed", "dry_run"] {
            assert_eq!(a["data"][field], data[field]);
        }
    }
    assert_eq!(fs::read_to_string(&a).unwrap(), source);
    assert_eq!(fs::read_to_string(&b).unwrap(), source);
    cli.send(json!([
        "project",
        "save",
        "--session",
        ca["session_id"],
        "--expected-sha256",
        ca["source_sha256"],
        "--no-backup"
    ]));
    let x = cli.read();
    let y = m.tool(
        "project_save",
        json!({"session":mb["session_id"],"expected_sha256":mb["source_sha256"],"backup":false}),
    );
    assert_eq!(x["data"]["saved"], true);
    assert_eq!(
        x["data"]["source_sha256"],
        y["result"]["structuredContent"]["data"]["source_sha256"]
    );
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
    assert!(!a.with_extension("circ.bak").exists());
    assert!(!b.with_extension("circ.bak").exists());
    cli.finish();
    m.finish();
    fs::remove_dir_all(dir).unwrap();
}

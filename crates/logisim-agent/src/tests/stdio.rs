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
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 8);
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

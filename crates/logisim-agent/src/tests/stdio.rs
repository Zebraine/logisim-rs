//! Exercise the installed binary's framing and process-local sessions through OS pipes.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdout, Command, Stdio},
};
const BIN: &str = env!("CARGO_BIN_EXE_logisim-agent");

#[path = "stdio/identity.rs"]
mod identity;
#[path = "stdio/rooted.rs"]
mod rooted;

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
    fn initialize(&mut self, version: &str) -> Value {
        let result = self.call("initialize", json!({"protocolVersion":version,"clientInfo":{"name":"test","version":"1"},"capabilities":{}}));
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        result["result"].clone()
    }
    fn finish(mut self) {
        drop(self.process.stdin.take());
        assert!(self.process.wait().unwrap().success());
    }
}

// Both MCP representations must carry the same CLI envelope.
fn tool_error(reply: &Value) -> Value {
    assert!(reply.get("error").is_none(), "{reply}");
    let result = &reply["result"];
    assert_eq!(result["isError"], true);
    let structured = &result["structuredContent"];
    assert_eq!(structured["ok"], false);
    assert_eq!(structured["protocol"], 1);
    assert_eq!(result["content"][0]["text"], structured.to_string());
    structured["error"].clone()
}

#[test]
fn mcp_negotiates_known_versions_and_reports_request_level_errors() {
    for (requested, expected) in [
        ("2024-11-05", "2024-11-05"),
        ("2025-03-26", "2025-03-26"),
        ("2025-06-18", "2025-06-18"),
        ("2025-11-25", "2025-11-25"),
        ("unknown", "2025-11-25"),
    ] {
        let mut c = Client::start(&["mcp"]);
        assert_eq!(
            c.tool("project_catalog", json!({}))["error"]["data"]["code"],
            "not_initialized"
        );
        let init = c.initialize(requested);
        assert_eq!(init["protocolVersion"], expected);
        assert!(
            init["capabilities"]["tools"].is_object() && init["serverInfo"]["name"].is_string()
        );
        assert_eq!(
            c.call("initialize", json!({}))["error"]["data"]["code"],
            "already_initialized"
        );
        assert_eq!(c.call("absent", json!({}))["error"]["code"], -32601);
        for params in [
            json!([]),
            json!({}),
            json!({"name":"project_catalog","arguments":[]}),
        ] {
            assert_eq!(c.call("tools/call", params)["error"]["code"], -32602);
        }
        c.send(json!({"jsonrpc":"1.0","id":12,"method":"tools/list"}));
        assert_eq!(c.read()["error"]["code"], -32600);
        c.finish();
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
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 18);
    assert_eq!(
        tool_error(&m.tool("settings_patch", json!({"config":"whatever","ops":[]})))["code"],
        "invalid_arguments"
    );
    assert_eq!(
        m.tool("simulation_run_forever", json!({}))["error"]["data"]["code"],
        "unknown_tool"
    );
    assert_eq!(m.tool("settings_patch", json!([]))["error"]["code"], -32602);
    assert_eq!(
        tool_error(&m.tool(
            "settings_patch",
            json!({"config":"path","dry_run":false,"ops":[["files.autosave",false]]})
        ))["code"],
        "invalid_arguments"
    );
    let catalog = m.tool("settings_catalog", json!({}));
    let cli = Command::new(BIN)
        .args(["settings", "catalog"])
        .output()
        .unwrap();
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(catalog["result"]["isError"], false);
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
        tool_error(&m.tool("simulation_state", args))["code"],
        "unknown_session"
    );
    m.finish();
}

#[test]
fn project_catalog_is_sessionless_and_byte_identical_over_cli_and_mcp() {
    let cli = Command::new(BIN)
        .args(["project", "catalog"])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let cli_text = std::str::from_utf8(&cli.stdout)
        .unwrap()
        .strip_suffix('\n')
        .unwrap();
    let envelope: Value = serde_json::from_str(cli_text).unwrap();
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["schema"], 1);
    let mut m = Client::start(&["mcp"]);
    m.call("initialize",json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"catalog","version":"1"},"capabilities":{}}));
    m.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let tools = m.call("tools/list", json!({}));
    let tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "project_catalog")
        .unwrap();
    assert_eq!(
        tool["inputSchema"],
        json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
    );
    let result = m.call("tools/call", json!({"name":"project_catalog"}));
    assert_eq!(result["result"]["structuredContent"], envelope);
    assert_eq!(
        result["result"]["content"][0]["text"].as_str(),
        Some(cli_text)
    );
    for arg in ["session", "path", "config"] {
        let cli = Command::new(BIN)
            .args(["project", "catalog", &format!("--{arg}"), "unused"])
            .output()
            .unwrap();
        assert!(!cli.status.success());
        let error: Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(error["error"]["code"], "invalid_arguments");
        assert_eq!(
            tool_error(&m.tool("project_catalog", json!({arg:"unused"})))["code"],
            "invalid_arguments"
        );
    }
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
    let ops = json!([
        {"op":"add_circuit","name":"child"},
        {"op":"add_component","circuit":2,"kind":"Pin","position":[20,30],"properties":{"label":"port","bits":8}},
        {"op":"add_component","circuit":1,"kind":"Constant","position":[20,30],"properties":{"bits":8,"value":200}},
        {"op":"add_component","circuit":1,"kind":"subcircuit","circuit_id":2,"position":[100,30],"properties":{"label":"A"}},
        {"op":"update_component","circuit":1,"id":1,"properties":{"value":42}},
        {"op":"add_component","circuit":1,"kind":"BitFinder","position":[100,100],"properties":{"bits":14,"search":"lowest_zero"}},
        {"op":"add_component","circuit":1,"kind":"Bit Selector","position":[200,100],"properties":{"group_bits":3}},
        {"op":"add_component","circuit":1,"kind":"Controlled Inverter","position":[300,100],"properties":{"size":"wide"}},
        {"op":"add_component","circuit":1,"kind":"SR Latch","position":[400,100],"properties":{"trigger":"latch"}},
        {"op":"add_component","circuit":1,"kind":"Counter","position":[500,100],"properties":{"bits":5,"counter_max":29}},
        {"op":"update_component","circuit":1,"id":7,"properties":{"trigger":"falling","counter_overflow":"stay"}},
        {"op":"add_component","circuit":1,"kind":"ROM","position":[100,300],"properties":{"address_bits":3,"bits":4,"contents":[[0,3],[3,0],[7,15]]}},
        {"op":"add_component","circuit":1,"kind":"RAM","position":[200,300],"properties":{"interface":"shared_async","simple":false}},
        {"op":"add_component","circuit":1,"kind":"PLA ROM","position":[300,300],"properties":{"terms":1,"inputs":2,"outputs":1,"input_plane":[["direct","inverted"]],"output_plane":[[true]]}},
        {"op":"add_component","circuit":1,"kind":"Slider","position":[400,300],"properties":{"bits":4,"slider_value":14}},
        {"op":"add_component","circuit":1,"kind":"Joystick","position":[500,300],"properties":{"bits":3,"facing":"West"}},
        {"op":"add_component","circuit":1,"kind":"Keyboard","position":[600,300],"properties":{"bits":16,"buffer_len":128}},
        {"op":"add_component","circuit":1,"kind":"TTY","position":[700,300],"properties":{"rows":29,"columns":70}}
    ]);
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
        12
    );
    let add = &patch["inputSchema"]["properties"]["ops"]["items"]["oneOf"][0];
    assert_eq!(add["properties"]["kind"]["type"], "string");
    assert!(add["properties"]["kind"].get("enum").is_none());
    assert_eq!(add["properties"]["circuit_id"]["type"], "integer");
    assert!(add["if"]["properties"]["kind"]["pattern"].is_string());
    let target_choices = json!([{"required":["circuit_id"]},{"required":["circuit_agent_id"]}]);
    assert_eq!(add["then"]["oneOf"], target_choices);
    assert_eq!(add["else"]["not"]["anyOf"], target_choices);
    assert_eq!(
        add["properties"]["circuit_agent_id"]["pattern"],
        "^k-[0-9a-f]{32}$"
    );
    for variant in patch["inputSchema"]["properties"]["ops"]["items"]["oneOf"]
        .as_array()
        .unwrap()
    {
        let name = variant["properties"]["op"]["const"].as_str().unwrap();
        if name.starts_with("add_") {
            assert!(variant["properties"].get("agent_id").is_none(), "{name}");
        } else {
            let prefix = if name.ends_with("_circuit") {
                "k"
            } else if name.ends_with("_wire") {
                "w"
            } else {
                "c"
            };
            assert_eq!(
                variant["properties"]["agent_id"]["pattern"],
                format!("^{prefix}-[0-9a-f]{{32}}$"),
                "{name}"
            );
            assert_eq!(
                variant["oneOf"],
                json!([{"required":["id"]},{"required":["agent_id"]}]),
                "{name}"
            );
            assert!(
                !variant["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("id")),
                "{name}"
            );
            assert_eq!(
                variant["properties"]["circuit"]["type"], "integer",
                "{name}"
            );
        }
    }
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
        tool_error(&m.tool(
            "project_patch",
            json!({"session":mb["session_id"],"ops":ops})
        ))["code"],
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
        identity::assert_equivalent_changes(&a["data"]["changes"], &data["changes"]);
        for field in ["source_sha256", "changed", "dry_run"] {
            assert_eq!(a["data"][field], data[field]);
        }
    }
    let bad = json!([{"op":"add_circuit","name":"discard"},
        {"op":"add_component","circuit":2,"kind":"subcircuit","circuit_id":1,"position":[0,0]}]);
    fs::write(&edits, json!({"ops":bad}).to_string()).unwrap();
    cli.send(json!([
        "project",
        "patch",
        "--session",
        ca["session_id"],
        "--ops",
        edits,
        "--apply"
    ]));
    let error = &cli.read()["error"];
    assert_eq!(error["details"]["op_index"], 1);
    assert_eq!(
        *error,
        tool_error(&m.tool(
            "project_patch",
            json!({"session":mb["session_id"],"dry_run":false,"ops":bad})
        ))
    );
    cli.send(json!([
        "project",
        "snapshot",
        "--session",
        ca["session_id"]
    ]));
    let circuits = &cli.read()["data"]["circuits"];
    let snapshot = m.tool("project_snapshot", json!({"session":mb["session_id"]}));
    identity::assert_equivalent_circuits(
        circuits,
        &snapshot["result"]["structuredContent"]["data"]["circuits"],
    );
    assert_eq!(circuits[0]["components"][0]["value"], 42);
    assert_eq!(
        circuits[0]["components"][1]["interface_ports"][0]["bits"],
        8
    );
    let counter = &circuits[0]["components"][6];
    assert_eq!(counter["trigger"], "falling");
    assert_eq!(counter["counter_overflow"], "stay");
    assert_eq!(counter["property_types"]["counter_max"], "integer");
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
    assert_eq!(x["data"]["source_sha256"], identity::disk_hash(&a));
    assert_eq!(
        y["result"]["structuredContent"]["data"]["source_sha256"],
        identity::disk_hash(&b)
    );
    identity::assert_equivalent_files(&a, &b);
    assert!(!a.with_extension("circ.bak").exists());
    assert!(!b.with_extension("circ.bak").exists());
    cli.finish();
    m.finish();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn simulation_controls_match_on_both_transports_and_invalid_requests_leave_state_intact() {
    let path = std::env::temp_dir().join(format!(
        "agent-controls-{:016x}.circ",
        getrandom::u64().unwrap()
    ));
    let xml = r##"<project source="logisim-rs-plugin-v1" version="1.0"><lib name="w" desc="#Wiring"/><main name="main"/><circuit name="main"><comp lib="w" name="Pin" loc="(10,20)"><a name="tristate" val="true"/></comp><comp lib="w" name="Clock" loc="(100,20)"/></circuit></project>"##;
    std::fs::write(&path, xml).unwrap();
    let mut cli = Client::start(&["project", "open", "--path", path.to_str().unwrap()]);
    let a = cli.read()["data"]["session_id"].clone();
    let mut mcp = Client::start(&["mcp"]);
    mcp.call("initialize",json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"control","version":"1"},"capabilities":{}}));
    mcp.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let b=mcp.tool("project_open",json!({"path":path}))["result"]["structuredContent"]["data"]["session_id"].clone();

    // Strip only per-process identity. All signals, counters, modes, action metadata and
    // error details must otherwise match, even after a rejected frame.
    fn local_ids(mut data: Value) -> Value {
        for key in ["/session_id", "/state/session_id"] {
            if let Some(id) = data.pointer_mut(key) {
                *id = "session".into();
            }
        }
        data
    }
    let commands = [
        (
            "open",
            "simulation_open",
            vec!["--mode", "event"],
            json!({"mode":"event"}),
        ),
        (
            "enable",
            "simulation_set_enabled",
            vec!["--enabled", "false"],
            json!({"enabled":false}),
        ),
        (
            "set-input",
            "simulation_set_input",
            vec!["--id", "1", "--value", "1"],
            json!({"id":1,"value":1}),
        ),
        ("tick", "simulation_tick", vec![], json!({})),
        ("step", "simulation_step", vec![], json!({})),
        ("settle", "simulation_settle", vec![], json!({})),
        (
            "enable",
            "simulation_set_enabled",
            vec!["--enabled", "true"],
            json!({"enabled":true}),
        ),
        (
            "set-input",
            "simulation_set_input",
            vec!["--id", "1", "--unknown"],
            json!({"id":1,"unknown":true}),
        ),
        ("tick", "simulation_tick", vec![], json!({})),
        ("reset", "simulation_reset", vec![], json!({})),
        ("state", "simulation_state", vec![], json!({})),
    ];
    let mut last = Value::Null;
    for (command, tool, flags, mut args) in commands {
        let mut argv = vec![
            json!("simulation"),
            json!(command),
            json!("--session"),
            a.clone(),
        ];
        argv.extend(flags.into_iter().map(Value::from));
        cli.send(argv.into());
        args["session"] = b.clone();
        let left = cli.read();
        let right = mcp.tool(tool, args);
        if command == "tick"
            && last["state"]["pending_count"]
                .as_u64()
                .is_some_and(|n| n > 0)
        {
            assert_eq!(left["error"]["code"], "not_settled");
            assert_eq!(left["error"], tool_error(&right));
        } else {
            assert_eq!(left["ok"], true, "{tool}: {left}");
            assert_eq!(
                local_ids(left["data"].clone()),
                local_ids(right["result"]["structuredContent"]["data"].clone()),
                "{tool}"
            );
            last = left["data"].clone();
        }
    }

    for args in [
        json!({"id":1}),
        json!({"value":1}),
        json!({"id":1,"value":1,"unknown":true}),
        json!({"id":1,"unknown":false}),
        json!({"id":1,"value":null}),
        json!({"id":1,"value":-1}),
        json!({"id":1,"value":4294967296u64}),
        json!({"id":1,"value":1,"path":[]}),
    ] {
        let mut args = args;
        args["session"] = b.clone();
        assert_eq!(
            tool_error(&mcp.tool("simulation_set_input", args))["code"],
            "invalid_arguments"
        );
    }
    for argv in [
        json!([
            "simulation",
            "enable",
            "--session",
            a,
            "--enabled",
            "true",
            "--enabled",
            "false"
        ]),
        json!(["simulation", "open", "--session", a, "--mode", "other"]),
        json!([
            "simulation",
            "set-input",
            "--session",
            a,
            "--id",
            "1",
            "--value",
            "0",
            "--unknown"
        ]),
        json!(["simulation", "step", "--session", a, "--id", "1"]),
        json!(["simulation", "tick", "--session"]),
    ] {
        cli.send(argv);
        assert_eq!(cli.read()["error"]["code"], "invalid_arguments");
    }
    let dup = format!(r#"{{"session":{b},"enabled":false,"enabled":true}}"#);
    writeln!(mcp.process.stdin.as_mut().unwrap(),r#"{{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{{"name":"simulation_set_enabled","arguments":{dup}}}}}"#).unwrap();
    assert_eq!(mcp.read()["error"]["data"]["code"], "invalid_arguments");
    writeln!(cli.process.stdin.as_mut().unwrap(),r#"{{"tool":"simulation_set_enabled","arguments":{{"session":{a},"enabled":false,"enabled":true}}}}"#).unwrap();
    assert_eq!(cli.read()["error"]["code"], "invalid_arguments");
    cli.send(json!(["simulation", "state", "--session", a]));
    assert_eq!(cli.read()["data"], last);
    let state = mcp.tool("simulation_state", json!({"session":b}));
    assert_eq!(
        local_ids(state["result"]["structuredContent"]["data"].clone()),
        local_ids(last)
    );
    cli.finish();
    mcp.finish();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), xml);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn annotations_describe_all_tools_including_in_memory_side_effects() {
    let mut c = Client::start(&["mcp"]);
    c.initialize("2025-11-25");
    let result = c.call("tools/list", json!({}));
    let tools = result["result"]["tools"].as_array().unwrap();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let (read, destructive, idempotent) = match name {
            "plugin_catalog" | "project_catalog" | "project_snapshot" | "settings_catalog"
            | "settings_get" | "simulation_state" => (true, false, true),
            "project_open" => (false, false, false),
            "simulation_open" => (false, false, true),
            "settings_patch" | "project_close" | "simulation_set_input" | "simulation_settle" => {
                (false, true, true)
            }
            "project_patch"
            | "project_save"
            | "simulation_step"
            | "simulation_tick"
            | "simulation_reset"
            | "simulation_set_enabled" => (false, true, false),
            _ => panic!("Missing annotations contract for {name}"),
        };
        assert_eq!(
            tool["annotations"],
            json!({"readOnlyHint":read,"destructiveHint":destructive,
            "idempotentHint":idempotent,"openWorldHint":false}),
            "{name}"
        );
    }
    c.finish();
}

#[test]
fn wasm_catalog_cli_and_mcp_return_matching_metadata_inside_root() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/input");
    let root = root.to_str().unwrap();
    let cli = Command::new(BIN)
        .args(["--root", root, "plugin", "catalog", "--wasm", "input.wasm"])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let envelope: Value = serde_json::from_slice(&cli.stdout).unwrap();
    let mut m = Client::start(&["--root", root, "mcp"]);
    m.initialize("2025-06-18");
    let reply = m.tool("plugin_catalog", json!({"path":"input.wasm"}));
    assert_eq!(reply["result"]["structuredContent"], envelope);
    assert_eq!(reply["result"]["isError"], false);
    let plugin = &envelope["data"];
    assert_eq!(plugin["id"], "org.logisim-rs.input");
    let tool = &plugin["tools"][0];
    assert_eq!(tool["id"], "keyboard");
    assert_eq!(tool["properties"]["capacity"]["maximum"], 64);
    assert_eq!(tool["property_schema"]["capacity"]["type"], "string");
    assert!(!tool["ports"].as_array().unwrap().is_empty());
    assert_eq!(tool["actions"][0]["id"], "key");
    assert!(tool.get("graphics").is_none());
    assert_eq!(
        tool_error(&m.tool("plugin_catalog", json!({"path":"../course/course.wasm"})))["code"],
        "path_outside_root"
    );
    m.finish();
}

#[path = "stdio/plugins.rs"]
mod plugins;

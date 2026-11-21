//! Pipelined calls over real pipes. A client correlates unrelated lanes by ID.
use super::{Client, tool_error};
use serde_json::{Value, json};
use std::path::Path;

fn send(c: &mut Client, id: u64, name: &str, arguments: Value) {
    c.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}}));
}

#[test]
fn pipelined_ticks_validation_close_and_eof_keep_each_sessions_order() {
    let mut c = Client::start(&["mcp"]);
    c.initialize("2025-11-25");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/agent/counter.circ");
    let sessions: Vec<_> = (0..2)
        .map(|_| {
            c.tool("project_open", json!({"path":path}))["result"]["structuredContent"]["data"]
                    ["session_id"]
                    .clone()
        })
        .collect();
    for (index, tool) in [
        "simulation_open",
        "simulation_tick",
        "project_patch",
        "simulation_state",
        "simulation_tick",
        "project_close",
        "simulation_state",
    ]
    .iter()
    .enumerate()
    {
        for (lane, session) in sessions.iter().enumerate() {
            send(
                &mut c,
                (2 * index + lane) as u64,
                tool,
                json!({"session":session}),
            );
        }
    }
    drop(c.process.stdin.take());
    let mut positions = [0usize; 2];
    for _ in 0..14 {
        let v = c.read();
        let id = v["id"].as_u64().unwrap() as usize;
        let lane = id % 2;
        let index = id / 2;
        assert_eq!(positions[lane], index);
        positions[lane] += 1;
        let data = &v["result"]["structuredContent"]["data"];
        match index {
            2 => assert_eq!(tool_error(&v)["code"], "invalid_arguments"),
            6 => assert_eq!(tool_error(&v)["code"], "unknown_session"),
            _ => {
                assert_eq!(data["session_id"], sessions[lane]);
                match index {
                    0 => assert_eq!(data["wasm_engine"], "wasmi"),
                    1 | 4 => assert_eq!(data["state"]["ticks"], if index == 1 { 1 } else { 2 }),
                    3 => assert_eq!(data["ticks"], 1),
                    5 => assert_eq!(data["closed"], true),
                    _ => unreachable!(),
                }
            }
        }
    }
    assert_eq!(positions, [7, 7]);
    c.finish();
}

#[test]
fn failed_open_releases_reservation_and_close_frees_the_sixty_fourth_slot() {
    let mut c = Client::start(&["mcp"]);
    c.initialize("2025-11-25");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/agent/counter.circ");
    assert_eq!(
        tool_error(&c.tool(
            "project_open",
            json!({"path":path.with_extension("missing")})
        ))["code"],
        "not_found"
    );
    for id in 0..65 {
        send(&mut c, id, "project_open", json!({"path":path}));
    }
    let mut sessions = std::collections::BTreeSet::new();
    let mut rejected = 0;
    for _ in 0..65 {
        let v = c.read();
        if v["result"]["isError"] == true {
            assert_eq!(tool_error(&v)["code"], "session_limit");
            rejected += 1;
        } else {
            assert!(
                sessions.insert(
                    v["result"]["structuredContent"]["data"]["session_id"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                )
            );
        }
    }
    assert_eq!((sessions.len(), rejected), (64, 1));
    assert_eq!(
        c.tool(
            "project_close",
            json!({"session":sessions.first().unwrap()})
        )["result"]["isError"],
        false
    );
    assert_eq!(
        c.tool("project_open", json!({"path":path}))["result"]["isError"],
        false
    );
    c.finish();
}

#[test]
fn idless_cli_pipeline_replies_in_command_order() {
    let mut c = Client::start(&["cli"]);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/agent/counter.circ");
    c.send(json!({"tool":"project_open","arguments":{"path":path}}));
    let session = c.read()["data"]["session_id"].clone();
    for tool in [
        "simulation_open",
        "simulation_tick",
        "simulation_state",
        "simulation_reset",
        "simulation_state",
        "project_close",
    ] {
        c.send(json!({"tool":tool,"arguments":{"session":session}}));
    }
    assert_eq!(c.read()["data"]["opened"], true);
    assert_eq!(c.read()["data"]["action"], "tick");
    assert_eq!(c.read()["data"]["ticks"], 1);
    assert_eq!(c.read()["data"]["action"], "reset");
    assert_eq!(c.read()["data"]["ticks"], 0);
    assert_eq!(c.read()["data"]["closed"], true);
    c.finish();
}

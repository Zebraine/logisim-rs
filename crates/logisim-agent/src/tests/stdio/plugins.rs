use super::{Client, tool_error};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[test]
fn plugin_instance_lifecycle_matches_between_cli_flags_and_mcp() {
    let dir = std::env::temp_dir().join(format!(
        "plugin-transports-{:016x}",
        getrandom::u64().unwrap()
    ));
    fs::create_dir(&dir).unwrap();
    let root = dir.to_str().unwrap();
    let source = r#"<project source="logisim-rs-plugin-v1" version="1.0"><main name="top"/><circuit name="top"/></project>"#;
    let setup = json!({"plugin":"org.logisim-rs.input","version":"1.0.0","path":"input.wasm","fallback":""});
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/input/input.wasm"),
        dir.join("input.wasm"),
    )
    .unwrap();
    fs::write(dir.join("cli.circ"), source).unwrap();
    fs::write(dir.join("mcp.circ"), source).unwrap();
    let mut cli = Client::start(&["--root", root, "cli"]);
    let mut mcp = Client::start(&["--root", root, "mcp"]);
    mcp.initialize("2025-11-25");
    let schema = mcp.call("tools/list", json!({}));
    let patch = schema["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "project_patch")
        .unwrap();
    let variants = patch["inputSchema"]["properties"]["ops"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    for kind in ["add_plugin", "update_plugin"] {
        let op = variants
            .iter()
            .find(|v| v["properties"]["op"]["const"] == kind)
            .unwrap();
        assert_eq!(
            op["properties"]["properties"]["additionalProperties"]["type"],
            "string"
        );
        assert_eq!(op["properties"]["font"]["additionalProperties"], false);
        assert_eq!(op["additionalProperties"], false);
        if kind == "add_plugin" {
            assert_eq!(
                op["anyOf"],
                json!([{"required":["source"]},{"required":["plugin","version"]}])
            );
            assert_eq!(
                op["properties"]["source"]["required"],
                json!(["plugin", "version", "path"])
            );
        }
    }
    let open = |cli: &mut Client, mcp: &mut Client| {
        cli.send(json!(["project", "open", "--path", "cli.circ"]));
        let a = cli.read();
        let b = mcp.tool("project_open", json!({"path":"mcp.circ"}));
        assert_eq!(
            a["data"]["source_sha256"],
            b["result"]["structuredContent"]["data"]["source_sha256"]
        );
        (
            a["data"]["session_id"].clone(),
            b["result"]["structuredContent"]["data"]["session_id"].clone(),
        )
    };
    let (mut ca, mut mb) = open(&mut cli, &mut mcp);
    let patch =
        |cli: &mut Client, mcp: &mut Client, ca: &Value, mb: &Value, ops: Value, ok: bool| {
            fs::write(dir.join("edits.json"), json!({"ops":ops}).to_string()).unwrap();
            cli.send(json!([
                "project",
                "patch",
                "--session",
                ca,
                "--ops",
                "edits.json",
                "--apply"
            ]));
            let a = cli.read();
            let b = mcp.tool(
                "project_patch",
                json!({"session":mb,"ops":ops,"dry_run":false}),
            );
            assert_eq!(a["ok"], ok, "{a}");
            assert_eq!(b["result"]["isError"], !ok, "{b}");
            if ok {
                assert_eq!(
                    a["data"]["changes"],
                    b["result"]["structuredContent"]["data"]["changes"]
                );
            } else {
                assert_eq!(a["error"], tool_error(&b));
            }
            a
        };
    let snapshot = |cli: &mut Client, mcp: &mut Client, ca: &Value, mb: &Value| {
        cli.send(json!(["project", "snapshot", "--session", ca]));
        let a = cli.read();
        let b = mcp.tool("project_snapshot", json!({"session":mb}));
        for k in ["circuits", "plugin_sources", "source_sha256"] {
            assert_eq!(a["data"][k], b["result"]["structuredContent"]["data"][k]);
        }
        a["data"].clone()
    };
    let save = |cli: &mut Client, mcp: &mut Client, ca: &Value, mb: &Value, sha: &Value| {
        cli.send(json!([
            "project",
            "save",
            "--session",
            ca,
            "--expected-sha256",
            sha
        ]));
        let a = cli.read();
        let b = mcp.tool("project_save", json!({"session":mb,"expected_sha256":sha}));
        assert_eq!(a["data"]["saved"], true);
        assert_eq!(b["result"]["isError"], false);
        assert_eq!(
            a["data"]["source_sha256"],
            b["result"]["structuredContent"]["data"]["source_sha256"]
        );
        assert_eq!(
            fs::read(dir.join("cli.circ")).unwrap(),
            fs::read(dir.join("mcp.circ")).unwrap()
        );
        a["data"]["source_sha256"].clone()
    };
    let added = patch(
        &mut cli,
        &mut mcp,
        &ca,
        &mb,
        json!([{"op":"add_plugin","source":setup,"position":[40,50],"circuit":1,"tool":"keyboard"}]),
        true,
    );
    let id = &added["data"]["changes"][0]["id"];
    let err = patch(
        &mut cli,
        &mut mcp,
        &ca,
        &mb,
        json!([{"op":"update_plugin","id":id,"properties":{"capacity":"0"}}]),
        false,
    );
    assert_eq!(err["error"]["code"], "invalid_property");
    patch(
        &mut cli,
        &mut mcp,
        &ca,
        &mb,
        json!([{"op":"update_plugin","id":id,"properties":{"capacity":"8"},"label":"Keyboard A"},
        {"op":"move_component","id":id,"position":[150,200]}]),
        true,
    );
    let pre = snapshot(&mut cli, &mut mcp, &ca, &mb);
    let sha = save(&mut cli, &mut mcp, &ca, &mb, &pre["source_sha256"]);
    for name in ["cli.circ.bak", "mcp.circ.bak"] {
        assert_eq!(fs::read(dir.join(name)).unwrap(), source.as_bytes());
    }
    cli.send(json!(["project", "close", "--session", ca]));
    assert_eq!(cli.read()["data"]["closed"], true);
    assert_eq!(
        mcp.tool("project_close", json!({"session":mb}))["result"]["isError"],
        false
    );
    (ca, mb) = open(&mut cli, &mut mcp);
    let post = snapshot(&mut cli, &mut mcp, &ca, &mb);
    assert_eq!(post["source_sha256"], sha);
    assert_eq!(post["plugin_sources"], json!([setup]));
    let c = &post["circuits"][0]["components"][0];
    assert_eq!(c["position"], json!([150, 200]));
    assert_eq!(c["label"], "Keyboard A");
    assert_eq!(c["plugin"]["properties"]["capacity"], "8");
    assert_eq!(c["tool"]["id"], "keyboard");
    let saved_bytes = fs::read(dir.join("cli.circ")).unwrap();
    patch(
        &mut cli,
        &mut mcp,
        &ca,
        &mb,
        json!([{"op":"remove_component","id":c["id"],"circuit":1}]),
        true,
    );
    save(&mut cli, &mut mcp, &ca, &mb, &sha);
    let removed = snapshot(&mut cli, &mut mcp, &ca, &mb);
    assert_eq!(removed["circuits"][0]["components"], json!([]));
    assert_eq!(removed["plugin_sources"], json!([setup]));
    for name in ["cli.circ.bak", "mcp.circ.bak"] {
        assert_eq!(fs::read(dir.join(name)).unwrap(), saved_bytes);
    }
    cli.finish();
    mcp.finish();
    fs::remove_dir_all(dir).unwrap();
}

//! Independent opens of an untagged fixture assign independent random metadata.
use logisim_core::AgentIdKind;
use serde_json::Value;
use std::{collections::HashSet, fs, path::Path};

pub(super) fn assert_equivalent_circuits(left: &Value, right: &Value) {
    let design = |mut circuits: Value| {
        let mut unique = HashSet::new();
        let mut take_id = |item: &mut Value, kind: AgentIdKind| {
            let id = item.as_object_mut().unwrap().remove("agent_id").unwrap();
            let raw = kind
                .decode(id.as_str().unwrap())
                .expect("canonical typed ID");
            assert!(unique.insert((kind, raw)), "duplicate {id}");
        };
        for circuit in circuits.as_array_mut().unwrap() {
            take_id(circuit, AgentIdKind::Circuit);
            for c in circuit["components"].as_array_mut().unwrap() {
                take_id(c, AgentIdKind::Component);
            }
            for w in circuit["wires"].as_array_mut().unwrap() {
                take_id(w, AgentIdKind::Wire);
            }
        }
        circuits
    };
    assert_eq!(design(left.clone()), design(right.clone()));
}

/// Independent sessions propose independent tags. Compare both transports' complete
/// change lists after checking and replacing only addition tags with their numeric references.
pub(super) fn assert_equivalent_changes(left: &Value, right: &Value) {
    let normalized = |mut changes: Value| {
        for c in changes.as_array_mut().unwrap() {
            let kind = match c["op"].as_str().unwrap() {
                "add_component" | "add_plugin" => AgentIdKind::Component,
                "add_wire" => AgentIdKind::Wire,
                "add_circuit" => AgentIdKind::Circuit,
                _ => continue,
            };
            kind.decode(c["agent_id"].as_str().unwrap()).unwrap();
            if kind == AgentIdKind::Wire {
                let tags = c["wire_agent_ids"].as_array().unwrap();
                let ids = c["wire_ids"].as_array().unwrap();
                assert_eq!(tags.len(), ids.len());
                assert!(
                    tags.iter()
                        .all(|tag| kind.decode(tag.as_str().unwrap()).is_some())
                );
                assert_eq!(tags.iter().collect::<HashSet<_>>().len(), ids.len());
                if !ids.is_empty() {
                    assert_eq!(tags[0], c["agent_id"]);
                    assert_eq!(ids[0], c["id"]);
                }
                c.as_object_mut().unwrap().remove("wire_agent_ids");
            }
            c.as_object_mut().unwrap().remove("agent_id");
        }
        changes
    };
    assert_eq!(normalized(left.clone()), normalized(right.clone()));
}

pub(super) fn disk_hash(path: &Path) -> Value {
    logisim_agent::Service::default()
        .call(logisim_agent::Request::ProjectOpen {
            path: path.to_owned(),
        })
        .unwrap()["source_sha256"]
        .clone()
}

pub(super) fn assert_equivalent_files(left: &Path, right: &Path) {
    let design = |path: &Path| {
        let xml = fs::read_to_string(path).unwrap();
        let import = logisim_file::parse(&xml).unwrap();
        assert!(import.warnings.is_empty());
        let mut project = import.document;
        // Verify the exact output bytes, then compare the ordinary XML independently of the random tags.
        assert_eq!(logisim_file::serialize(&project).unwrap(), xml);
        for d in &mut project.circuits {
            d.circuit.agent_ids = Default::default();
        }
        logisim_file::serialize(&project).unwrap()
    };
    assert_eq!(design(left), design(right));
}

#[test]
fn cli_save_and_mcp_open_copy_share_persistent_ids_even_after_numeric_reassignment() {
    use super::Client;
    use serde_json::json;
    let tmp =
        std::env::temp_dir().join(format!("agent-persist-{:016x}", getrandom::u64().unwrap()));
    fs::create_dir(&tmp).unwrap();
    let small = tmp.join("small.circ");
    let copy = tmp.join("copy.circ");
    let xml = r##"<project source="2.16.2.2.jar" version="1.0"><lib name="w" desc="#Wiring"/><main name="top"/><circuit name="top">
    <comp lib="w" name="Pin" loc="(10,10)"/><comp name="child" loc="(90,90)"/><wire from="(10,10)" to="(40,10)"/></circuit>
    <circuit name="child"><comp lib="w" name="Pin" loc="(0,0)"/></circuit></project>"##;
    fs::write(&small, xml).unwrap();
    let root = tmp.to_str().unwrap();
    let mut first = Client::start(&["--root", root, "project", "open", "--path", "small.circ"]);
    let opened = first.read()["data"].clone();
    let id = &opened["session_id"];
    let snapshot = |cli: &mut Client| {
        cli.send(json!(["project", "snapshot", "--session", id]));
        cli.read()["data"]["circuits"].clone()
    };
    let before = snapshot(&mut first);
    assert_eq!(fs::read(&small).unwrap(), xml.as_bytes());
    first.send(json!([
        "project",
        "save",
        "--session",
        id,
        "--expected-sha256",
        opened["source_sha256"]
    ]));
    let saved = first.read()["data"].clone();
    assert_eq!(saved["saved"], true);
    assert_eq!(snapshot(&mut first), before);
    first.finish();
    assert_eq!(
        fs::read(small.with_extension("circ.bak")).unwrap(),
        xml.as_bytes()
    );
    fs::copy(&small, &copy).unwrap();

    let mut mcp = Client::start(&["--root", root, "mcp"]);
    mcp.initialize("2025-11-25");
    for file in ["small.circ", "copy.circ"] {
        let opened = mcp.tool("project_open", json!({"path":file}));
        let data = &opened["result"]["structuredContent"]["data"];
        let args = json!({"session":data["session_id"]});
        assert_eq!(data["source_sha256"], saved["source_sha256"]);
        let response = mcp.tool("project_snapshot", args.clone());
        assert_eq!(response["result"]["isError"], false);
        let after = &response["result"]["structuredContent"]["data"]["circuits"];
        for (a, b) in before
            .as_array()
            .unwrap()
            .iter()
            .zip(after.as_array().unwrap())
        {
            assert_eq!(a["agent_id"], b["agent_id"]);
            for group in ["components", "wires"] {
                let ids = |v: &Value| {
                    v[group]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|obj| obj["agent_id"].clone())
                        .collect::<Vec<_>>()
                };
                assert_eq!(ids(a), ids(b));
            }
        }
        // The writer emits wires first. The original Pin was the first item on disk.
        assert_ne!(
            before[0]["components"][0]["id"],
            after[0]["components"][0]["id"]
        );
        let again = mcp.tool("project_save", args);
        assert_eq!(
            again["result"]["structuredContent"]["data"]["source_sha256"],
            saved["source_sha256"]
        );
    }
    mcp.finish();
    assert_eq!(fs::read(&copy).unwrap(), fs::read(&small).unwrap());
    assert_eq!(
        fs::read(copy.with_extension("circ.bak")).unwrap(),
        fs::read(&small).unwrap()
    );
    fs::remove_dir_all(tmp).unwrap();
}

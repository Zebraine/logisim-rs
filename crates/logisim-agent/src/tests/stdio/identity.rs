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

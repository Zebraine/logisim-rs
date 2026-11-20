use super::*;
#[test]
fn source_hash_handles_single_and_multiple_padding_blocks() {
    for (input, expected) in [
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ] {
        assert_eq!(digest::sha256(input.as_bytes()), expected);
    }
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("logisim-agent-{:016x}", getrandom::u64().unwrap()));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn settings_patch_validates_the_entire_transaction_before_writing() {
    use std::fs;
    let temp = Temp::new();
    let path = temp.0.join("prefs.json");
    let bytes=serde_json::to_vec(&json!({"version":16,"recent":[temp.0.join("local.circ")],"plugins":{"fallback_directory":temp.0}})).unwrap();
    fs::write(&path, &bytes).unwrap();
    let hash = digest::sha256(&bytes);
    let edit = |key: &str, value: Value| Operation {
        key: key.into(),
        value,
    };
    let dark = || edit("appearance.theme", json!("dark"));
    let preview = settings::patch(&path, Some(&hash), vec![dark()], true).unwrap();
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    assert_eq!(preview["source_sha256"], hash);
    for (op, code) in [
        (edit("recent", json!([])), "invalid_arguments"),
        (
            edit("plugins.fallback_directory", json!("/elsewhere")),
            "unknown_setting",
        ),
        (edit("files.autosave", json!(1)), "invalid_settings"),
        (
            edit("editor.default_gate_inputs", json!(33)),
            "invalid_settings",
        ),
        (
            edit("shortcuts.file.save", json!("primary+o")),
            "invalid_settings",
        ),
        (dark(), "invalid_arguments"),
    ] {
        assert_eq!(
            settings::patch(&path, None, vec![dark(), op], false)
                .unwrap_err()
                .code,
            code
        );
    }
    assert_eq!(
        settings::patch(&path, Some(&"0".repeat(64)), vec![dark()], false)
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let applied = settings::patch(&path, Some(&hash), vec![dark()], false).unwrap();
    assert_eq!(applied["source_sha256"], preview["result_sha256"]);
    assert_eq!(
        applied["source_sha256"],
        digest::sha256(&fs::read(&path).unwrap())
    );
    fs::write(&path, b"broken").unwrap();
    assert_eq!(
        settings::patch(&path, None, vec![dark()], false)
            .unwrap_err()
            .code,
        "invalid_settings"
    );
    assert_eq!(fs::read(&path).unwrap(), b"broken");
}

#[test]
fn competing_settings_writers_cannot_both_commit_the_same_revision() {
    use std::{fs, sync::Barrier, thread};
    let temp = Temp::new();
    let path = temp.0.join("prefs.json");
    let bytes = br#"{"version":16}"#;
    fs::write(&path, bytes).unwrap();
    let hash = digest::sha256(bytes);
    let barrier = Barrier::new(2);
    thread::scope(|scope| {
        let writers: Vec<_> = ["files.autosave", "appearance.show_telemetry"]
            .map(|key| {
                scope.spawn({
                    let (barrier, path, hash) = (&barrier, &path, &hash);
                    move || {
                        barrier.wait();
                        settings::patch(
                            path,
                            Some(hash),
                            vec![Operation {
                                key: key.into(),
                                value: false.into(),
                            }],
                            false,
                        )
                    }
                })
            })
            .into();
        let outcomes: Vec<_> = writers.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            outcomes.into_iter().find_map(Result::err).unwrap().code,
            "conflict"
        );
    });
}

fn open(service: &mut Service, path: &std::path::Path) -> String {
    service
        .call(Request::ProjectOpen { path: path.into() })
        .unwrap()["session_id"]
        .as_str()
        .unwrap()
        .into()
}

#[test]
fn snapshots_and_settled_signals_share_ids_and_never_write_the_source() {
    use std::fs;
    let temp = Temp::new();
    let path = temp.0.join("demo.circ");
    let xml = r##"<project version="1.0" source="logisim-rs-plugin-v1"><lib name="w" desc="#Wiring"/><main name="main"/><circuit name="main">
    <comp lib="w" name="Constant" loc="(0,0)"><a name="value" val="1"/></comp>
    <comp lib="w" name="Probe" loc="(60,0)"><a name="label" val="sense"/></comp>
    <wire from="(0,0)" to="(60,0)"/><comp lib="w" name="Pin" loc="(0,40)"><a name="label" val="input"/></comp>
    </circuit><circuit name="child"/></project>"##;
    fs::write(&path, xml).unwrap();
    let mut s = Service::default();
    let session = open(&mut s, &path);
    let state = || Request::SimulationState {
        session: session.clone(),
    };
    let snap = || Request::ProjectSnapshot {
        session: session.clone(),
    };
    assert_eq!(s.call(state()).unwrap_err().code, "simulation_not_open");
    let snapshot = s.call(snap()).unwrap();
    assert_eq!(snapshot, s.call(snap()).unwrap());
    assert_eq!(snapshot["source_sha256"], digest::sha256(xml.as_bytes()));
    assert_eq!(snapshot["circuits"].as_array().unwrap().len(), 2);
    assert_eq!(
        snapshot["circuits"][0]["interface_ports"][0]["label"],
        "input"
    );
    s.call(Request::SimulationOpen {
        mode: Default::default(),
        session: session.clone(),
    })
    .unwrap();
    let data = s.call(state()).unwrap();
    assert_eq!(data["error"], Value::Null);
    assert_eq!(data["ticks"], 0);
    assert_eq!(data["diagnostics"], json!([]));
    assert_eq!(data["wires"][0]["tone"], "high");
    assert_eq!(
        data["wires"][0]["id"],
        snapshot["circuits"][0]["wires"][0]["id"]
    );
    assert_eq!(data["probes"][0]["value"]["number"], 1);
    assert_eq!(data["pins"][0]["id"], data["inputs"][0]["id"]);
    assert_eq!(s.call(state()).unwrap(), data);
    assert_eq!(s.call(snap()).unwrap(), snapshot);
    s.call(Request::ProjectClose {
        session: session.clone(),
    })
    .unwrap();
    assert_eq!(s.call(state()).unwrap_err().code, "unknown_session");
    assert_eq!(s.call(snap()).unwrap_err().code, "unknown_session");
    assert_ne!(session, open(&mut s, &path));
    assert_eq!(fs::read_to_string(&path).unwrap(), xml);
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
}

#[test]
fn course_modules_are_loaded_only_from_recorded_and_matching_paths() {
    use std::{fs, path::Path};
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let real = repo.join("examples/course/single-cycle.circ");
    let file = fs::read_to_string(&real).unwrap();
    let temp = Temp::new();
    let altered = temp.0.join("copy.circ");
    fs::copy(
        repo.join("plugins/display/display.wasm"),
        temp.0.join("wrong.wasm"),
    )
    .unwrap();
    for (path, substitute, expected) in [
        (&real, None, "loaded"),
        (&altered, Some("missing.wasm"), "missing"),
        (&altered, Some("wrong.wasm"), "missing"),
    ] {
        if let Some(substitute) = substitute {
            fs::write(
                path,
                file.replace("../../plugins/course/course.wasm", substitute),
            )
            .unwrap();
        }
        let mut service = Service::default();
        let session = open(&mut service, path);
        let initial = service
            .call(Request::SimulationOpen {
                mode: Default::default(),
                session: session.clone(),
            })
            .unwrap();
        assert_eq!(initial["plugins"][0]["status"], expected);
        let data = service.call(Request::SimulationState { session }).unwrap();
        if expected == "missing" {
            assert!(!data["plugins"][0]["error"].as_str().unwrap().is_empty());
            assert!(
                data["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["problem"] == "plugin_error")
            );
        }
        assert_eq!(data["ticks"], 0);
    }
}

#[test]
fn parse_problems_are_reported_and_sessions_are_process_local() {
    let temp = Temp::new();
    let path = temp.0.join("problem.circ");
    let mut s = Service::default();
    assert_eq!(
        s.call(Request::ProjectOpen { path: path.clone() })
            .unwrap_err()
            .code,
        "not_found"
    );
    std::fs::write(&path, "<invalid/>").unwrap();
    assert_eq!(
        s.call(Request::ProjectOpen { path: path.clone() })
            .unwrap_err()
            .code,
        "invalid_project"
    );
    std::fs::write(
        &path,
        r##"<project version="1.0"><main name="main"/><circuit name="main"/></project>"##,
    )
    .unwrap();
    let session = open(&mut s, &path);
    assert_eq!(
        s.call(Request::ProjectSnapshot {
            session: session.clone()
        })
        .unwrap()["partial"],
        true
    );
    assert_eq!(
        Service::default()
            .call(Request::ProjectSnapshot { session })
            .unwrap_err()
            .code,
        "unknown_session"
    );
}

#[test]
fn positional_json_sections_cannot_be_overwritten_as_settings_objects() {
    let temp = Temp::new();
    let path = temp.0.join("prefs.json");
    for bytes in [r#"[16]"#, r#"{"version":16,"editor":[]}"#] {
        std::fs::write(&path, bytes).unwrap();
        let ops = vec![Operation {
            key: "editor.default_font_size".into(),
            value: 24.into(),
        }];
        assert_eq!(
            settings::patch(&path, None, ops, false).unwrap_err().code,
            "invalid_settings"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
    }
}

mod project_patch;

#[test]
fn repeat_open_and_save_retain_the_initialized_engine_and_observations() {
    let temp = Temp::new();
    let path = temp.0.join("clock.circ");
    std::fs::write(&path, r##"<project source="logisim-rs-plugin-v1" version="1.0"><lib name="w" desc="#Wiring"/><main name="top"/><circuit name="top"><comp lib="w" name="Clock" loc="(20,20)"/></circuit></project>"##).unwrap();
    let mut service = Service::default();
    let session = open(&mut service, &path);
    let mut call = |name: &str, mut args: Value| {
        args["session"] = json!(session);
        service
            .call(Request::from_json(json!({"tool":name,"arguments":args})).unwrap())
            .unwrap()
    };
    let initial = call("simulation_open", json!({"mode":"event"}));
    assert_eq!(initial["created"], true);
    assert_eq!(initial["engine"], "event");
    assert_eq!(initial["pending_count"], 0);
    let observed = call("simulation_state", json!({}));
    assert!(observed.get("changed").is_none());
    call("project_save", json!({"backup":false}));
    let reopened = call("simulation_open", json!({"mode":"stable"}));
    assert_eq!(reopened["created"], false);
    assert_eq!(reopened["mode"], "event");
    assert_eq!(call("simulation_state", json!({})), observed);
}

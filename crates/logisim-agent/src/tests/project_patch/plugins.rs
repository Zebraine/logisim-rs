use super::{Fixture, SOURCE};
use crate::{Request, Service};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn module() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/input/input.wasm")
}
fn source() -> Value {
    json!({"plugin":"org.logisim-rs.input","version":"1.0.0","path":"input.wasm","fallback":""})
}
fn add() -> Value {
    json!({"op":"add_plugin","circuit":1,"tool":"keyboard","source":source(),"position":[200,300]})
}

#[test]
fn plugin_document_properties_defaults_identity_and_source_round_trip() {
    let mut f = Fixture::new(SOURCE);
    let wasm = f.path.with_file_name("input.wasm");
    fs::copy(module(), &wasm).unwrap();
    let catalog = f.cli.call(Request::PluginCatalog { path: wasm }).unwrap();
    let mut op = add();
    op["properties"] = json!({"capacity":"5"});
    op["facing"] = json!("west");
    op["color"] = json!([10, 20, 30, 255]);
    op["font"] = json!({"bold":true,"size":22});
    let reuse = json!({"op":"add_plugin","circuit":1,"position":[700,0],"plugin":"org.logisim-rs.input","version":"1.0.0","tool":"keyboard"});
    let ops = json!([op, reuse]);
    let before = f.snapshot();
    let dry = f.patch(ops.clone(), true).unwrap();
    assert_eq!(before, f.snapshot());
    let result = f.patch(ops, false).unwrap();
    assert_eq!(result["changes"], dry["changes"]);
    let id = &result["changes"][0]["id"];
    let snapshot = f.snapshot();
    assert_eq!(snapshot["plugin_sources"], json!([source()]));
    let first = &snapshot["circuits"][0]["components"][2];
    let second = &snapshot["circuits"][0]["components"][3];
    assert_eq!(first["id"], *id);
    assert_eq!(first["agent_id"], result["changes"][0]["agent_id"]);
    assert_eq!(first["plugin"]["properties"]["capacity"], "5");
    assert_eq!(second["plugin"]["properties"]["capacity"], "16");
    assert_eq!(
        first["property_schema"],
        catalog["tools"][0]["property_schema"]
    );
    assert_eq!(first["source"], source());
    assert_eq!(first["tool"]["ports"], catalog["tools"][0]["ports"]);
    assert_eq!(first["property_types"]["trigger"], "string");
    assert_eq!(first["facing"], "West");
    f.patch(json!([{"op":"update_plugin","agent_id":first["agent_id"],"properties":{"trigger":"falling"},"label":"Input board","font":{"italic":true}}]),false).unwrap();
    let snapshot = f.snapshot();
    let updated = &snapshot["circuits"][0]["components"][2];
    for k in [
        "id", "position", "ports", "facing", "color", "tool", "source",
    ] {
        assert_eq!(updated[k], first[k], "{k}");
    }
    assert_eq!(updated["label"], "Input board");
    assert_eq!(updated["font"]["size"], 22);
    assert_eq!(updated["font"]["bold"], true);
    assert_eq!(updated["font"]["italic"], true);
    assert_eq!(updated["plugin"]["properties"]["capacity"], "5");
    assert_eq!(updated["plugin"]["properties"]["trigger"], "falling");
    assert_eq!(snapshot["circuits"][0]["components"][3], *second);
    let saved = f.call("project_save", json!({})).unwrap();
    assert_eq!(fs::read(f.backup()).unwrap(), SOURCE.as_bytes());
    f.id = crate::tests::open(&mut f.cli, &f.path);
    let reopened = f.snapshot();
    assert_eq!(reopened["source_sha256"], saved["source_sha256"]);
    let loaded = &reopened["circuits"][0]["components"][2];
    assert_eq!(loaded["agent_id"], first["agent_id"]);
    assert_eq!(
        reopened["circuits"][0]["agent_id"],
        snapshot["circuits"][0]["agent_id"]
    );
    for k in [
        "plugin",
        "tool",
        "position",
        "color",
        "font",
        "label",
        "facing",
        "source",
        "property_schema",
    ] {
        assert_eq!(loaded[k], updated[k], "{k}");
    }
    assert_eq!(
        f.call("simulation_open", json!({})).unwrap()["plugins"][0]["status"],
        "loaded"
    );
}

#[test]
fn plugin_failures_rollback_and_missing_module_does_not_advertise_schema() {
    let mut f = Fixture::new(SOURCE);
    let path = f.path.with_file_name("input.wasm");
    fs::copy(module(), &path).unwrap();
    let before = f.snapshot();
    f.call("simulation_open", json!({})).unwrap();
    for (key, value, code) in [
        (
            "source",
            json!({"plugin":"wrong","version":"1.0.0","path":"input.wasm"}),
            "plugin_unavailable",
        ),
        (
            "source",
            json!({"plugin":"org.logisim-rs.input","version":"2.0","path":"input.wasm"}),
            "plugin_unavailable",
        ),
        (
            "source",
            json!({"plugin":"org.logisim-rs.input","version":"1.0.0","path":"missing.wasm"}),
            "plugin_unavailable",
        ),
        ("tool", json!("absent"), "plugin_unavailable"),
        ("properties", json!({"capacity":"65"}), "invalid_property"),
        ("properties", json!({"capacity":5}), "invalid_arguments"),
        ("properties", json!({"action":"key"}), "invalid_property"),
        ("properties", json!({"trigger":"other"}), "invalid_property"),
        ("font", json!({"size":0}), "invalid_property"),
        ("color", json!([0, 0, 0, 10]), "invalid_property"),
        ("unexpected", json!(false), "invalid_arguments"),
    ] {
        let mut bad = add();
        bad[key] = value;
        let err = f
            .patch(json!([{"op":"add_circuit","name":"discard"},bad]), false)
            .unwrap_err();
        assert_eq!(err.code, code, "{bad}");
        assert_eq!(err.details["op_index"], 1, "{err:?}");
        assert_eq!(f.snapshot(), before);
    }
    let no_source = json!({"op":"add_plugin","circuit":1,"position":[0,0],"tool":"keyboard","plugin":"org.logisim-rs.input","version":"1.0.0"});
    assert_eq!(
        f.patch(json!([no_source]), false).unwrap_err().code,
        "plugin_unavailable"
    );
    assert!(f.call("simulation_state", json!({})).is_ok());
    let result = f.patch(json!([add()]), false).unwrap();
    let id = &result["changes"][0]["id"];
    fs::remove_file(path).unwrap();
    let missing = f.snapshot();
    let c = &missing["circuits"][0]["components"][2];
    assert_eq!(c["plugin_error"]["code"], "plugin_unavailable");
    assert_eq!(c["patchable_properties"], json!([]));
    assert_eq!(c["property_schema"], json!({}));
    assert!(c.get("tool").is_none());
    assert_eq!(c["plugin"]["properties"]["capacity"], "16");
    assert_eq!(
        f.patch(
            json!([{"op":"update_plugin","id":id,"properties":{"capacity":"6"}}]),
            false
        )
        .unwrap_err()
        .code,
        "plugin_unavailable"
    );
    assert_eq!(f.snapshot(), missing);
    assert_eq!(fs::read(&f.path).unwrap(), SOURCE.as_bytes());
}

#[test]
fn root_checks_both_explicit_source_paths_but_reuses_recorded_dependencies() {
    let mut f = Fixture::new(SOURCE);
    let dir = f.path.parent().unwrap().to_owned();
    let project = dir.join("root/nested");
    fs::create_dir_all(&project).unwrap();
    fs::copy(module(), dir.join("outside.wasm")).unwrap();
    fs::copy(module(), project.join("input.wasm")).unwrap();
    f.path = project.join("test.circ");
    fs::write(&f.path, SOURCE).unwrap();
    f.cli = Service::with_root(&dir.join("root")).unwrap();
    f.id = crate::tests::open(&mut f.cli, &f.path);
    let before = f.snapshot();
    for (field, path) in [
        ("path", "../../outside.wasm"),
        ("fallback", "missing/../../../outside.wasm"),
    ] {
        let mut op = add();
        op["source"][field] = json!(path);
        let e = f.patch(json!([op]), false).unwrap_err();
        assert_eq!(e.code, "path_outside_root");
        assert_eq!(f.snapshot(), before);
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.join("outside.wasm"), project.join("link.wasm")).unwrap();
        let mut op = add();
        op["source"]["fallback"] = json!("link.wasm");
        assert_eq!(
            f.patch(json!([op]), false).unwrap_err().code,
            "path_outside_root"
        );
    }
    let mut op = add();
    op["source"]["path"] = json!("missing/subdir/absent.wasm");
    op["source"]["fallback"] = json!("input.wasm");
    f.patch(json!([op]), false).unwrap();
    f.call("project_save", json!({})).unwrap();
    let xml = fs::read_to_string(&f.path)
        .unwrap()
        .replace("input.wasm", "../../outside.wasm");
    fs::write(&f.path, xml).unwrap();
    f.id = crate::tests::open(&mut f.cli, &f.path);
    let op = json!({"op":"add_plugin","circuit":1,"position":[0,0],"tool":"keyboard","plugin":"org.logisim-rs.input","version":"1.0.0"});
    assert!(f.patch(json!([op]), false).is_ok());
    assert_eq!(f.snapshot()["plugin_sources"].as_array().unwrap().len(), 1);
}

#[test]
fn alternate_location_keeps_recorded_source_and_mismatched_saved_tool_is_readable() {
    let mut f = Fixture::new(SOURCE);
    for name in ["input.wasm", "another.wasm"] {
        fs::copy(module(), f.path.with_file_name(name)).unwrap();
    }
    let mut alternate = add();
    alternate["source"]["path"] = json!("another.wasm");
    f.patch(json!([add(), alternate]), false).unwrap();
    assert_eq!(f.snapshot()["plugin_sources"], json!([source()]));
    f.call("project_save", json!({})).unwrap();
    let xml = fs::read_to_string(&f.path).unwrap();
    assert!(xml.contains("Key controller"));
    fs::write(&f.path, xml.replace("Key controller", "Old key controller")).unwrap();
    f.id = crate::tests::open(&mut f.cli, &f.path);
    let before = f.snapshot();
    let old = &before["circuits"][0]["components"][2];
    assert_eq!(old["plugin_error"]["code"], "plugin_unavailable");
    assert_eq!(old["plugin"]["name"], "Old key controller");
    assert_eq!(old["property_schema"], json!({}));
    let e = f
        .patch(
            json!([{"op":"update_plugin","id":old["id"],"label":"discard"}]),
            false,
        )
        .unwrap_err();
    assert_eq!(e.code, "plugin_unavailable");
    assert_eq!(f.snapshot(), before);
}

#[test]
fn moving_a_plugin_reroutes_attached_wire_and_removing_last_keeps_source() {
    let mut f = Fixture::new(SOURCE);
    fs::copy(module(), f.path.with_file_name("input.wasm")).unwrap();
    let added = f
        .patch(
            json!([add(), {"op":"add_wire","circuit":1,"from":[100,300],"to":[200,300]}]),
            false,
        )
        .unwrap();
    let id = &added["changes"][0]["id"];
    f.patch(
        json!([{"op":"move_component","circuit":1,"id":id,"position":[250,300]}]),
        false,
    )
    .unwrap();
    let snap = f.snapshot();
    let circuit = &snap["circuits"][0];
    assert_eq!(circuit["components"][2]["id"], *id);
    assert_eq!(
        circuit["components"][2]["ports"][0]["position"],
        json!([250, 300])
    );
    assert!(
        circuit["wires"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["from"] == json!([250, 300]) || w["to"] == json!([250, 300]))
    );
    f.patch(
        json!([{"op":"remove_component","circuit":1,"id":id}]),
        false,
    )
    .unwrap();
    f.call("project_save", json!({})).unwrap();
    f.id = crate::tests::open(&mut f.cli, &f.path);
    let reopened = f.snapshot();
    assert_eq!(reopened["plugin_sources"], json!([source()]));
    assert_eq!(
        reopened["circuits"][0]["components"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

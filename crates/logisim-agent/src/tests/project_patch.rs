use super::{Temp, open};
use crate::{Request, Result, Service, digest};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

const SOURCE: &str = r##"<project version="1.0" source="logisim-rs-plugin-v1"><lib name="w" desc="#Wiring"/><main name="top"/><circuit name="top"><comp lib="w" name="Pin" loc="(10,10)"/><comp lib="w" name="Probe" loc="(70,10)"/><wire from="(10,10)" to="(70,10)"/></circuit></project>"##;
struct Fixture {
    _temp: Temp,
    path: PathBuf,
    cli: Service,
    id: String,
}
impl Fixture {
    fn new(xml: &str) -> Self {
        let temp = Temp::new();
        let path = temp.0.join("project.circ");
        fs::write(&path, xml).unwrap();
        let mut cli = Service::default();
        let id = open(&mut cli, &path);
        Self {
            _temp: temp,
            path,
            cli,
            id,
        }
    }
    fn call(&mut self, name: &str, mut args: Value) -> Result<Value> {
        args["session"] = self.id.clone().into();
        self.cli
            .call(Request::from_json(json!({"tool":name,"arguments":args}))?)
    }
    fn snapshot(&mut self) -> Value {
        self.call("project_snapshot", json!({})).unwrap()
    }
    fn patch(&mut self, ops: Value, dry: bool) -> Result<Value> {
        self.call("project_patch", json!({"ops":ops,"dry_run":dry}))
    }
    fn backup(&self) -> PathBuf {
        self.path.with_extension("circ.bak")
    }
}

#[test]
fn all_ten_operations_preview_commit_save_and_reopen() {
    let mut f = Fixture::new(SOURCE);
    let first = f.snapshot();
    f.call("simulation_open", json!({})).unwrap();
    let probe = first["circuits"][0]["components"][1]["id"].clone();
    let wire = first["circuits"][0]["wires"][0]["id"].clone();
    let ops = json!([
        {"op":"move_component","id":1,"position":[20,10]},
        {"op":"update_component","id":1,"properties":{"bits":4,"output":true,"three_state":true,"pull":"down","label_facing":"west","label":"A & B","color":[1,2,3,255],"font":{"size":22,"italic":true}}},
        {"op":"add_circuit","name":"unit"},{"op":"rename_circuit","id":2,"name":"alu"},{"op":"set_main_circuit","id":2},
        {"op":"add_component","circuit":2,"kind":"Tunnel","position":[100,50],"properties":{"bits":8,"label":"BUS"}},
        {"op":"add_wire","circuit":2,"from":[100,50],"to":[150,50]},
        {"op":"add_circuit","name":"unused"},{"op":"remove_circuit","id":3}]);
    let dry = f.patch(ops.clone(), true).unwrap();
    assert_eq!(first, f.snapshot());
    assert!(f.call("simulation_state", json!({})).is_ok());
    let apply = f.patch(ops, false).unwrap();
    assert_eq!(dry["changes"], apply["changes"]);
    assert_eq!(
        f.call("simulation_state", json!({})).unwrap_err().code,
        "simulation_not_open"
    );
    let second = f.snapshot();
    assert_eq!(second["main"], 2);
    let p = &second["circuits"][0]["components"][0];
    assert_eq!(p["position"], json!([20, 10]));
    assert_eq!(p["label"], "A & B");
    assert_eq!(p["font"]["size"], 22);
    assert_eq!(second["circuits"][0]["components"][1]["id"], probe);
    assert_eq!(second["circuits"][0]["wires"][0]["id"], wire);
    let w = &apply["changes"][6]["id"];
    assert_eq!(second["circuits"][1]["wires"][0]["id"], *w);
    assert_eq!(
        second["circuits"][1]["components"][0]["id"],
        apply["changes"][5]["id"]
    );
    f.patch(json!([{"op":"remove_component","circuit":1,"id":probe},{"op":"remove_wire","circuit":2,"id":w}]),false).unwrap();
    assert_eq!(fs::read(&f.path).unwrap(), SOURCE.as_bytes());
    assert!(!f.backup().exists());
    let old = first["source_sha256"].clone();
    let saved = f
        .call("project_save", json!({"expected_sha256":old,"backup":true}))
        .unwrap();
    let bytes = fs::read(&f.path).unwrap();
    assert_eq!(fs::read(f.backup()).unwrap(), SOURCE.as_bytes());
    assert_eq!(saved["source_sha256"], digest::sha256(&bytes));
    assert_ne!(saved["source_sha256"], old);
    assert_eq!(f.snapshot()["circuits"][0]["components"][0]["id"], 1);
    f.patch(
        json!([{"op":"rename_circuit","id":2,"name":"new main"}]),
        false,
    )
    .unwrap();
    f.call("project_save", json!({})).unwrap();
    assert_eq!(fs::read(f.backup()).unwrap(), bytes);
    f.id = open(&mut f.cli, &f.path);
    let reopened = f.snapshot();
    assert_eq!(reopened["partial"], false);
    assert_eq!(reopened["circuits"][1]["name"], "new main");
    assert_eq!(reopened["circuits"][0]["components"][0]["label"], "A & B");
    assert_eq!(reopened["circuits"][0]["components"][0]["pull"], "down");
    assert!(
        reopened["circuits"][1]["wires"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn failure_of_each_operation_rolls_back_allocations_and_simulation() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    f.call("simulation_open", json!({})).unwrap();
    for (op, code) in [
        (
            json!({"op":"add_component","circuit":1,"kind":"RAM","position":[0,0]}),
            "unsupported_component",
        ),
        (
            json!({"op":"move_component","id":1,"position":[1000001,0]}),
            "invalid_edit",
        ),
        (
            json!({"op":"update_component","id":1,"properties":{"font":{"fontname":"X"}}}),
            "invalid_property",
        ),
        (json!({"op":"remove_component","id":99}), "invalid_edit"),
        (
            json!({"op":"add_wire","circuit":1,"from":[0,0],"to":[10,10]}),
            "invalid_edit",
        ),
        (
            json!({"op":"add_wire","circuit":1,"from":[0,0],"to":[0,0]}),
            "invalid_edit",
        ),
        (json!({"op":"remove_wire","id":1}), "invalid_edit"),
        (json!({"op":"add_circuit","name":"top"}), "invalid_edit"),
        (
            json!({"op":"rename_circuit","id":1,"name":""}),
            "invalid_edit",
        ),
        (json!({"op":"remove_circuit","id":88}), "invalid_edit"),
        (json!({"op":"set_main_circuit","id":99}), "invalid_edit"),
        (
            json!({"op":"update_component","id":1,"properties":{"label":"bad\u{0000}"}}),
            "invalid_edit",
        ),
        (
            json!({"op":"move_component","id":1,"position":["bad",5]}),
            "invalid_arguments",
        ),
    ] {
        let error = f
            .patch(json!([{"op":"add_circuit","name":"provisional"},op]), false)
            .unwrap_err();
        assert_eq!(error.code, code, "{op}");
        assert_eq!(error.details["op_index"], 1, "{op}");
        assert_eq!(f.snapshot(), before);
    }
    assert!(f.call("simulation_state", json!({})).is_ok());
    assert_eq!(
        f.patch(json!([{"op":"add_circuit","name":"next"}]), false)
            .unwrap()["changes"][0]["id"],
        2
    );
}

#[test]
fn partial_stale_source_and_failed_backup_protect_bytes_and_session() {
    let mut partial = Fixture::new(&SOURCE.replace("</project>", "<unknown/></project>"));
    for (name, args) in [
        ("project_patch", json!({"ops":[],"dry_run":true})),
        ("project_patch", json!({"ops":[],"dry_run":false})),
        ("project_save", json!({})),
    ] {
        assert_eq!(
            partial.call(name, args).unwrap_err().code,
            "partial_project"
        );
    }
    assert!(!partial.backup().exists());
    let mut f = Fixture::new(SOURCE);
    let old = f.snapshot()["source_sha256"].clone();
    let ops = json!([{"op":"add_circuit","name":"local"}]);
    let wrong = "0".repeat(64);
    assert_eq!(
        f.call(
            "project_patch",
            json!({"ops":ops,"dry_run":false,"expected_sha256":wrong})
        )
        .unwrap_err()
        .code,
        "conflict"
    );
    f.patch(ops, false).unwrap();
    let edited = f.snapshot();
    assert_eq!(
        f.call("project_save", json!({"expected_sha256":wrong}))
            .unwrap_err()
            .code,
        "conflict"
    );
    fs::write(&f.path, "external change").unwrap();
    assert_eq!(
        f.call("project_save", json!({"expected_sha256":old}))
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(fs::read(&f.path).unwrap(), b"external change");
    assert!(!f.backup().exists());
    fs::write(&f.path, SOURCE).unwrap();
    fs::create_dir(f.backup()).unwrap();
    assert_eq!(
        f.call("project_save", json!({})).unwrap_err().code,
        "backup_failed"
    );
    assert_eq!(fs::read(&f.path).unwrap(), SOURCE.as_bytes());
    assert_eq!(f.snapshot(), edited);
    f.call("project_save", json!({"backup":false})).unwrap();
    assert!(f.backup().is_dir());
}

#[test]
fn referenced_circuits_and_custom_port_bindings_survive_failed_transactions() {
    let xml = r##"<project version="1.0" source="logisim-rs-plugin-v1">
      <lib name="w" desc="#Wiring"/><main name="top"/>
      <circuit name="top"><comp name="child" loc="(100,100)"/></circuit>
      <circuit name="child"><appear>
        <circ-port x="46" y="56" width="8" height="8" pin="10,10"/>
        <circ-port x="76" y="56" width="8" height="8" pin="50,10"/>
        <circ-anchor x="77" y="57" width="6" height="6" facing="east"/>
      </appear><comp lib="w" name="Pin" loc="(10,10)"/>
      <comp lib="w" name="Pin" loc="(50,10)"/></circuit></project>"##;
    let mut f = Fixture::new(xml);
    let before = f.snapshot();
    assert_eq!(before["partial"], false);
    for (op, code) in [
        (json!({"op":"remove_circuit","id":2}), "invalid_edit"),
        (
            json!({"op":"remove_component","circuit":1,"id":1}),
            "unsupported_component",
        ),
        (
            json!({"op":"move_component","circuit":2,"id":2,"position":[10,10]}),
            "invalid_project",
        ),
    ] {
        let result = f.patch(
            json!([{"op":"rename_circuit","id":1,"name":"top2"},op]),
            false,
        );
        let error = result.unwrap_err();
        assert_eq!(error.code, code, "{op}");
        assert_eq!(error.details["op_index"], 1);
        assert_eq!(f.snapshot(), before);
    }
    assert_eq!(fs::read_to_string(&f.path).unwrap(), xml);
    let mut single = Fixture::new(SOURCE);
    assert_eq!(
        single
            .patch(json!([{"op":"remove_circuit","id":1}]), false)
            .unwrap_err()
            .code,
        "invalid_edit"
    );
}

#[cfg(unix)]
#[test]
fn replacement_symlinks_are_preserved_on_save_conflict() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new(SOURCE);
    let other = f.path.with_extension("other");
    fs::rename(&f.path, &other).unwrap();
    symlink(&other, &f.path).unwrap();
    assert_eq!(
        f.call("project_save", json!({})).unwrap_err().code,
        "conflict"
    );
    assert_eq!(fs::read_link(&f.path).unwrap(), other);
    assert_eq!(fs::read_to_string(&other).unwrap(), SOURCE);
    assert!(!f.backup().exists());
    // A dangling directory entry is an existing destination for atomic creation.
    let missing = f.path.with_extension("missing");
    symlink(&missing, f.backup()).unwrap();
    let result = logisim_preferences::storage::replace_checked(&f.backup(), b"new", None, None);
    assert!(matches!(
        result,
        Err(logisim_preferences::storage::WriteError::Conflict)
    ));
    assert_eq!(fs::read_link(f.backup()).unwrap(), missing);
}

#[test]
fn only_unique_item_ids_work_without_an_owner_and_complex_components_are_read_only() {
    let xml = SOURCE.replace(
        "</project>",
        r#"<circuit name="other"><comp lib="w" name="Transistor" loc="(0,0)"/></circuit></project>"#,
    );
    let mut f = Fixture::new(&xml);
    let orig = f.snapshot();
    for (op, code) in [
        (json!({"op":"remove_component","id":1}), "ambiguous_id"),
        (
            json!({"op":"remove_component","circuit":2,"id":1}),
            "unsupported_component",
        ),
        (
            json!({"op":"update_component","circuit":2,"id":1,"properties":{"label":"new"}}),
            "unsupported_component",
        ),
        (
            json!({"op":"move_component","circuit":2,"id":1,"position":[20,20]}),
            "unsupported_component",
        ),
    ] {
        assert_eq!(f.patch(json!([op]), false).unwrap_err().code, code);
    }
    assert_eq!(f.snapshot(), orig);
    f.patch(json!([{"op":"remove_component","circuit":1,"id":1}]), false)
        .unwrap();
    assert_eq!(
        f.snapshot()["circuits"][1]["components"],
        orig["circuits"][1]["components"]
    );
}

#[test]
fn supported_families_round_trip_and_reject_attributes_the_codec_cannot_persist() {
    let mut f = Fixture::new(SOURCE);
    let settings = [
        (
            "XOR Gate",
            json!({"inputs":4,"size":"Wide","negation_mask":9,"xor_odd":true,"bits":8}),
        ),
        (
            "Even Parity",
            json!({"inputs":32,"size":"Medium","negation_mask":2147483648u32,"output":"floating/1"}),
        ),
        ("Probe", json!({"radix":"hex","label_facing":"West"})),
        ("Tunnel", json!({"bits":32,"label":"data"})),
        (
            "Splitter",
            json!({"fanout":3,"input_bits":4,"mapping":[2,null,1,0]}),
        ),
        ("NOT Gate", json!({"size":"Wide"})),
    ];
    let ops:Vec<_>=settings.iter().enumerate().map(|(i,(kind,properties))|json!({"op":"add_component","kind":kind,"circuit":1,"position":[100*i,100],"properties":properties})).collect();
    f.patch(json!(ops), false).unwrap();
    f.call("project_save", json!({"backup":false})).unwrap();
    f.id = open(&mut f.cli, &f.path);
    let snap = f.snapshot();
    assert_eq!(snap["partial"], false);
    for (i, (_, properties)) in settings.iter().enumerate() {
        let c = &snap["circuits"][0]["components"][i + 2];
        for (key, v) in properties.as_object().unwrap() {
            assert_eq!(&c[key], v, "{key}");
        }
    }
    for (kind, properties) in [
        ("Buffer", json!({"size":"Wide"})),
        ("NOT Gate", json!({"size":"Medium"})),
        ("AND Gate", json!({"inputs":9})),
        ("XOR Gate", json!({"inputs":3,"negation_mask":8})),
        ("Odd Parity", json!({"output":"bad"})),
        ("Splitter", json!({"fanout":2,"mapping":[2,1,0,1]})),
        ("Probe", json!({"bits":8})),
        ("Tunnel", json!({"color":[0,0,0,255]})),
    ] {
        let err=f.patch(json!([{"op":"add_component","kind":kind,"circuit":1,"position":[0,0],"properties":properties}]),false).unwrap_err();
        assert_eq!(err.code, "invalid_property", "{kind}");
        assert_eq!(err.details["op_index"], 0);
    }
}

#[test]
fn wire_normalization_returns_surviving_ids_and_preserves_other_geometry() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    let result = f
        .patch(
            json!([
                {"op":"add_component","circuit":1,"kind":"Probe","position":[40,80]},
                {"op":"add_wire","circuit":1,"from":[10,80],"to":[90,80]}
            ]),
            false,
        )
        .unwrap();
    let added = result["changes"][1]["wire_ids"].as_array().unwrap();
    assert_eq!(added.len(), 2);
    assert!(added.contains(&result["changes"][1]["id"]));
    let after = f.snapshot();
    let wires = after["circuits"][0]["wires"].as_array().unwrap();
    assert!(wires.contains(&before["circuits"][0]["wires"][0]));
    for id in added {
        assert!(wires.iter().any(|w| w["id"] == *id));
    }
}

#[test]
fn concurrent_sessions_cannot_overwrite_a_new_source_or_its_backup() {
    use std::{sync::Barrier, thread};
    let f = Fixture::new(SOURCE);
    let barrier = Barrier::new(2);
    thread::scope(|scope| {
        let writers=["left","right"].map(|name| {
            let (path,barrier)=(&f.path,&barrier);
            scope.spawn(move||{
                let mut cli=Service::default();let session=open(&mut cli,path);
                let req=Request::from_json(json!({"tool":"project_patch","arguments":{"session":session,"dry_run":false,"ops":[{"op":"add_circuit","name":name}]}})).unwrap();
                cli.call(req).unwrap();barrier.wait();
                cli.call(Request::ProjectSave{session,expected_sha256:None,backup:true})
            })
        });
        let results = writers.map(|w| w.join().unwrap());
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results.into_iter().find_map(Result::err).unwrap().code,
            "conflict"
        );
    });
    assert_eq!(fs::read(f.backup()).unwrap(), SOURCE.as_bytes());
    assert_eq!(
        logisim_file::parse(&fs::read_to_string(&f.path).unwrap())
            .unwrap()
            .document
            .circuits
            .len(),
        2
    );
}

mod builtins;

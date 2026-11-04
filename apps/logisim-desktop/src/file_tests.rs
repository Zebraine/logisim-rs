use crate::{
    files::Files,
    storage::{self, SaveMode, Storage},
};
use logisim_core::{Circuit, Component, Gate, Kind, Point};
use logisim_file::Document;
use std::{fs, path::PathBuf};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "logisim-rs-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn circuit() -> Circuit {
    let mut c = Circuit::default();
    c.add_component(Component::new(Kind::Gate(Gate::And), Point::new(200, 100)));
    c
}
#[test]
fn disk_roundtrip_updates_baseline_only_after_success() {
    let dir = Temp::new();
    let path = dir.path("test.circ");
    let mut storage = Storage::new();
    let mut circuit = circuit();
    assert!(storage.dirty(&circuit));
    storage.save(&circuit, &path, SaveMode::Current).unwrap();
    assert!(!storage.dirty(&circuit));
    circuit.components[0].label = "saved again".into();
    storage.save(&circuit, &path, SaveMode::Current).unwrap();
    let mut loaded = Storage::new();
    let (result, warnings) = loaded.load(&path).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(result, circuit);
    let baseline = fs::read(&path).unwrap();
    circuit.components[0].label = "pending".into();
    assert!(
        storage
            .save(&circuit, &dir.path("absent/failed.circ"), SaveMode::Current)
            .is_err()
    );
    assert!(storage.dirty(&circuit));
    assert_eq!(storage.path.as_ref(), Some(&path));
    assert_eq!(fs::read(&path).unwrap(), baseline);
    fs::write(dir.path("invalid.circ"), "not XML").unwrap();
    assert!(storage.load(&dir.path("invalid.circ")).is_err());
    assert_eq!(storage.path.as_ref(), Some(&path));
    assert_eq!(storage.name, "main");
    assert_eq!(
        fs::read_dir(&dir.0).unwrap().count(),
        2,
        "No temporary files remain"
    );
}
#[test]
fn external_changes_and_unconfirmed_overwrites_preserve_both_documents() {
    let dir = Temp::new();
    let path = dir.path("one.circ");
    let other = dir.path("two.circ");
    let mut storage = Storage::new();
    let circuit = circuit();
    storage.save(&circuit, &path, SaveMode::Current).unwrap();
    fs::write(&path, "external edit").unwrap();
    assert!(storage.save(&circuit, &path, SaveMode::Current).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    fs::write(&other, "existing file").unwrap();
    assert!(storage.save(&circuit, &other, SaveMode::Current).is_err());
    assert_eq!(fs::read_to_string(&other).unwrap(), "existing file");
    let confirmed = storage::read(&other).unwrap();
    fs::write(&other, "changed again").unwrap();
    assert!(
        storage
            .save(&circuit, &other, SaveMode::Selected(Some(&confirmed)))
            .is_err()
    );
    let confirmed = storage::read(&other).unwrap();
    storage
        .save(&circuit, &other, SaveMode::Selected(Some(&confirmed)))
        .unwrap();
    assert_eq!(storage.path.as_ref(), Some(&other));
    assert!(logisim_file::parse(&fs::read_to_string(&other).unwrap()).is_ok());
}
#[test]
fn partial_import_requires_a_different_destination() {
    let dir = Temp::new();
    let path = dir.path("mixed.circ");
    let xml = logisim_file::serialize(&Document {
        circuit: circuit(),
        ..Document::default()
    })
    .unwrap()
    .replace(
        "</circuit>",
        "<comp lib=\"0\" loc=\"(0,0)\" name=\"Clock\"/></circuit>",
    );
    fs::write(&path, &xml).unwrap();
    let mut storage = Storage::new();
    let (circuit, warnings) = storage.load(&path).unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(circuit.components.len(), 1);
    assert!(storage.save(&circuit, &path, SaveMode::Current).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), xml);
    #[cfg(unix)]
    {
        let link = dir.path("alias.circ");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(storage.save(&circuit, &link, SaveMode::Current).is_err());
    }
    let copy = dir.path("subset.circ");
    storage.save(&circuit, &copy, SaveMode::Current).unwrap();
    assert!(storage.protected.is_none());
    assert!(!fs::read_to_string(copy).unwrap().contains("Clock"));
    assert_eq!(fs::read_to_string(path).unwrap(), xml);
}
#[test]
fn imported_runtime_wires_retain_gate_connectivity_and_simulation() {
    let xml = r##"<project source="2.16.2.2.jar" version="1.0"><lib name="0" desc="#Wiring"/><lib name="1" desc="#Gates"/><main name="main"/><circuit name="main"><wire from="(200,150)" to="(270,150)"/><wire from="(270,150)" to="(270,180)"/><wire from="(270,180)" to="(310,180)"/><wire from="(200,230)" to="(270,230)"/><wire from="(270,200)" to="(270,230)"/><wire from="(270,200)" to="(310,200)"/><wire from="(340,190)" to="(440,190)"/><comp lib="0" loc="(200,150)" name="Pin"><a name="tristate" val="false"/></comp><comp lib="0" loc="(200,230)" name="Pin"><a name="tristate" val="false"/></comp><comp lib="1" loc="(340,190)" name="AND Gate"/><comp lib="0" loc="(440,190)" name="Pin"><a name="output" val="true"/></comp></circuit></project>"##;
    let imported = logisim_file::parse(xml).unwrap();
    assert!(imported.warnings.is_empty());
    let c = &imported.document.circuit;
    let mut sim = logisim_sim::Simulator::default();
    sim.synchronize(c);
    sim.advance(64);
    for (a, b, expected) in [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 1)] {
        sim.set_input(c.components[0].id, a);
        sim.set_input(c.components[1].id, b);
        sim.advance(64);
        assert_eq!(
            sim.port_value(c.components[3].id, 0).unwrap().number(),
            Some(expected)
        );
    }
    let saved = logisim_file::serialize(&imported.document).unwrap();
    assert!(logisim_file::parse(&saved).unwrap().warnings.is_empty());
}
#[test]
fn replacing_document_resets_simulation_and_unsaved_actions_wait() {
    let dir = Temp::new();
    let path = dir.path("new.circ");
    let mut c = Circuit::default();
    c.add_component(Component::new(Kind::Pin, Point::ZERO));
    fs::write(
        &path,
        logisim_file::serialize(&Document {
            circuit: c.clone(),
            ..Document::default()
        })
        .unwrap(),
    )
    .unwrap();
    let mut files = Files::default();
    let mut ws = logisim_ui::Workspace::default();
    let mut sim = logisim_sim::Simulator::default();
    sim.synchronize(&c);
    sim.set_input(c.components[0].id, 1);
    files.load(&path, &mut ws, &mut sim);
    sim.synchronize(ws.circuit());
    sim.advance(64);
    assert_eq!(
        sim.input(ws.circuit().components[0].id).unwrap().number(),
        Some(0)
    );
    ws.replace_circuit(circuit());
    let before = ws.circuit().clone();
    let ctx = eframe::egui::Context::default();
    files.request(logisim_ui::FileAction::New, &ctx, &mut ws, &mut sim);
    assert!(files.busy());
    assert_eq!(ws.circuit(), &before);
    assert!(!files.allow_close);
}

#[test]
fn save_as_can_confirm_an_externally_modified_or_deleted_current_path() {
    let dir = Temp::new();
    let path = dir.path("document.circ");
    let mut storage = Storage::new();
    let c = circuit();
    storage.save(&c, &path, SaveMode::Current).unwrap();
    fs::write(&path, "external change").unwrap();
    assert!(storage.save(&c, &path, SaveMode::Current).is_err());
    let confirmed = storage::read(&path).unwrap();
    storage
        .save(&c, &path, SaveMode::Selected(Some(&confirmed)))
        .unwrap();
    fs::remove_file(&path).unwrap();
    assert!(storage.save(&c, &path, SaveMode::Current).is_err());
    storage.save(&c, &path, SaveMode::Selected(None)).unwrap();
    assert!(!storage.dirty(&c));
}

use super::support::Temp;
use crate::{
    files::Files,
    storage::{self, SaveMode, Storage},
};
use logisim_core::{Circuit, Component, Gate, Kind, Point, Project};
use logisim_file::Document;
use std::fs;
fn circuit() -> Project {
    let mut c = Circuit::default();
    c.add_component(Component::new(Kind::Gate(Gate::And), Point::new(200, 100)));
    Project::single("main", c)
}

#[test]
fn backup_rotates_exactly_one_generation_of_disk_bytes() {
    let dir = Temp::new();
    let path = dir.path("layout.circ.bak");
    let backup = dir.path("layout.circ.bak.bak");
    let mut disk = Storage::new();
    let mut project = circuit();
    disk.open_legacy_fixture(&project, &path);
    assert!(!backup.exists());
    let mut original = fs::read(&path).unwrap();
    original.extend_from_slice(b"\r\n<!-- user formatting -->\r\n");
    fs::write(&path, &original).unwrap();
    disk.load(&path).unwrap();
    project.circuits[0].name = "first".into();
    disk.save(&project, &path, SaveMode::Current, true).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), original);
    let first = fs::read(&path).unwrap();
    project.circuits[0].name = "second".into();
    disk.save(&project, &path, SaveMode::Current, true).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), first);
    assert_eq!(
        logisim_file::parse(&fs::read_to_string(&path).unwrap())
            .unwrap()
            .document,
        project
    );
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 3);
    fs::write(&path, b"external edit").unwrap();
    assert!(disk.save(&project, &path, SaveMode::Current, true).is_err());
    assert_eq!(fs::read(&backup).unwrap(), first);
    assert_eq!(fs::read(&path).unwrap(), b"external edit");
}

#[cfg(unix)]
#[test]
fn new_backup_inherits_private_mode_and_never_follows_sidecar_links() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = Temp::new();
    let path = dir.path("private.circ");
    let backup = dir.path("private.circ.bak");
    let mut disk = Storage::new();
    let project = circuit();
    disk.open_legacy_fixture(&project, &path);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    disk.save(&project, &path, SaveMode::Current, true).unwrap();
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::remove_file(&backup).unwrap();
    symlink(&path, &backup).unwrap();
    assert!(
        disk.save(&Project::default(), &path, SaveMode::Current, true)
            .is_err()
    );
    assert!(backup.is_symlink());
    assert_eq!(
        fs::read(&path).unwrap(),
        logisim_file::serialize(&project).unwrap().as_bytes()
    );
}
#[test]
fn disk_roundtrip_updates_baseline_only_after_success() {
    let dir = Temp::new();
    let path = dir.path("test.circ");
    let mut storage = Storage::new();
    let mut circuit = circuit();
    assert!(storage.dirty(&circuit));
    storage.open_legacy_fixture(&circuit, &path);
    assert!(!storage.dirty(&circuit));
    circuit.main_circuit_mut().circuit.components[0].label = "saved again".into();
    storage
        .save(&circuit, &path, SaveMode::Current, false)
        .unwrap();
    let mut loaded = Storage::new();
    assert!(!dir.path("test.circ.bak").exists());
    let (result, warnings) = loaded.load(&path).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(result, circuit);
    let baseline = fs::read(&path).unwrap();
    circuit.main_circuit_mut().circuit.components[0].label = "pending".into();
    assert!(
        storage
            .save(
                &circuit,
                &dir.path("absent/failed.circ"),
                SaveMode::Current,
                false
            )
            .is_err()
    );
    assert!(storage.dirty(&circuit));
    assert_eq!(storage.path.as_ref(), Some(&path));
    assert_eq!(fs::read(&path).unwrap(), baseline);
    fs::write(dir.path("invalid.circ"), "not XML").unwrap();
    assert!(storage.load(&dir.path("invalid.circ")).is_err());
    assert_eq!(storage.path.as_ref(), Some(&path));
    assert!(storage.dirty(&circuit));
    assert_eq!(
        fs::read_dir(&dir.0).unwrap().count(),
        3,
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
    storage.open_legacy_fixture(&circuit, &path);
    fs::write(&path, "external edit").unwrap();
    assert!(
        storage
            .save(&circuit, &path, SaveMode::Current, false)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    fs::write(&other, "existing file").unwrap();
    assert!(
        storage
            .save(&circuit, &other, SaveMode::Current, false)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&other).unwrap(), "existing file");
    let confirmed = storage::read(&other).unwrap();
    fs::write(&other, "changed again").unwrap();
    assert!(
        storage
            .save(
                &circuit,
                &other,
                SaveMode::Selected(logisim_file::ProjectFormat::Circ, Some(&confirmed)),
                false
            )
            .is_err()
    );
    let confirmed = storage::read(&other).unwrap();
    storage
        .save(
            &circuit,
            &other,
            SaveMode::Selected(logisim_file::ProjectFormat::Circ, Some(&confirmed)),
            false,
        )
        .unwrap();
    assert_eq!(storage.path.as_ref(), Some(&other));
    assert!(logisim_file::parse(&fs::read_to_string(&other).unwrap()).is_ok());
}
#[test]
fn partial_import_requires_a_different_destination() {
    let dir = Temp::new();
    let path = dir.path("mixed.circ");
    let xml = logisim_file::serialize(&circuit()).unwrap().replace(
        "</circuit>",
        "<comp lib=\"0\" loc=\"(0,0)\" name=\"Unknown Wiring Component\"/></circuit>",
    );
    fs::write(&path, &xml).unwrap();
    let mut storage = Storage::new();
    let (circuit, warnings) = storage.load(&path).unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(circuit.main_circuit().circuit.components.len(), 1);
    assert!(
        storage
            .save(&circuit, &path, SaveMode::Current, true)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), xml);
    assert!(!dir.path("mixed.circ.bak").exists());
    let protected_backup = dir.path("copy.circ.bak");
    fs::write(&protected_backup, &xml).unwrap();
    let mut from_backup = Storage::new();
    from_backup.load(&protected_backup).unwrap();
    let target = dir.path("copy.circ");
    fs::write(&target, b"previous").unwrap();
    assert!(
        from_backup
            .save(
                &circuit,
                &target,
                SaveMode::Selected(logisim_file::ProjectFormat::Circ, Some(b"previous")),
                true
            )
            .is_err()
    );
    assert_eq!(fs::read(target).unwrap(), b"previous");
    assert_eq!(fs::read_to_string(protected_backup).unwrap(), xml);
    #[cfg(unix)]
    {
        let link = dir.path("alias.circ");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(
            storage
                .save(&circuit, &link, SaveMode::Current, true)
                .is_err()
        );
    }
    let copy = dir.path("subset.circ");
    storage
        .save(
            &circuit,
            &copy,
            SaveMode::Selected(logisim_file::ProjectFormat::Circ, None),
            true,
        )
        .unwrap();
    assert!(storage.protected.is_none());
    assert!(
        !fs::read_to_string(copy)
            .unwrap()
            .contains("Unknown Wiring Component")
    );
    assert_eq!(fs::read_to_string(path).unwrap(), xml);
}
#[test]
fn imported_runtime_wires_retain_gate_connectivity_and_simulation() {
    let xml = r##"<project source="2.16.2.2.jar" version="1.0"><lib name="0" desc="#Wiring"/><lib name="1" desc="#Gates"/><main name="main"/><circuit name="main"><wire from="(200,150)" to="(270,150)"/><wire from="(270,150)" to="(270,180)"/><wire from="(270,180)" to="(310,180)"/><wire from="(200,230)" to="(270,230)"/><wire from="(270,200)" to="(270,230)"/><wire from="(270,200)" to="(310,200)"/><wire from="(340,190)" to="(440,190)"/><comp lib="0" loc="(200,150)" name="Pin"><a name="tristate" val="false"/></comp><comp lib="0" loc="(200,230)" name="Pin"><a name="tristate" val="false"/></comp><comp lib="1" loc="(340,190)" name="AND Gate"/><comp lib="0" loc="(440,190)" name="Pin"><a name="output" val="true"/></comp></circuit></project>"##;
    let imported = logisim_file::parse(xml).unwrap();
    assert!(imported.warnings.is_empty());
    let c = &imported.document.main_circuit().circuit;
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
        logisim_file::serialize(&Document::single("main", c.clone())).unwrap(),
    )
    .unwrap();
    let mut files = Files::default();
    let mut ws = logisim_ui::Workspace::default();
    let mut sim = logisim_sim::ProjectSimulator::default();
    sim.synchronize(&Project::single("main", c.clone()), 1);
    sim.set_input(c.components[0].id, 1);
    files.load(&path, &mut ws, &mut sim);
    sim.synchronize(ws.project(), ws.simulation_root());
    sim.advance(64);
    assert_eq!(
        sim.input(ws.circuit().components[0].id).unwrap().number(),
        Some(0)
    );
    ws.replace_project(circuit());
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
    storage.open_legacy_fixture(&c, &path);
    fs::write(&path, "external change").unwrap();
    assert!(storage.save(&c, &path, SaveMode::Current, false).is_err());
    let confirmed = storage::read(&path).unwrap();
    storage
        .save(
            &c,
            &path,
            SaveMode::Selected(logisim_file::ProjectFormat::Circ, Some(&confirmed)),
            false,
        )
        .unwrap();
    fs::remove_file(&path).unwrap();
    assert!(storage.save(&c, &path, SaveMode::Current, false).is_err());
    storage
        .save(
            &c,
            &path,
            SaveMode::Selected(logisim_file::ProjectFormat::Circ, None),
            false,
        )
        .unwrap();
    assert!(!storage.dirty(&c));
}

#[test]
fn selected_codec_mismatch_partial_conversion_and_loss_leave_the_target_untouched() {
    use logisim_file::ProjectFormat::{Circ, Lrs};
    let dir = Temp::new();
    let native = dir.path("protected.lrs");
    let legacy = dir.path("compatible.circ");
    let mut disk = Storage::new();
    let mut p = circuit();
    fs::write(&legacy, b"previous target").unwrap();
    assert!(
        disk.save(
            &p,
            &legacy,
            SaveMode::Selected(Lrs, Some(b"previous target")),
            true
        )
        .is_err()
    );
    let mut joystick = Component::new(Kind::Joystick, Point::ZERO);
    joystick.font.size = 24;
    p.main_circuit_mut().circuit.add_component(joystick);
    let err = disk
        .save(
            &p,
            &legacy,
            SaveMode::Selected(Circ, Some(b"previous target")),
            true,
        )
        .unwrap_err();
    assert!(err.contains("component c-"), "{err}");
    assert!(disk.path.is_none());
    assert_eq!(fs::read(&legacy).unwrap(), b"previous target");
    assert!(
        !dir.path("compatible.circ.bak").exists() && !dir.path("compatible.circ.lock").exists()
    );

    let xml = logisim_file::serialize(&circuit())
        .unwrap()
        .replace("</circuit>", "<unknown/></circuit>");
    fs::write(&legacy, &xml).unwrap();
    let (subset, warnings) = disk.load(&legacy).unwrap();
    assert!(!warnings.is_empty());
    assert!(
        disk.save(&subset, &native, SaveMode::Selected(Lrs, None), false)
            .unwrap_err()
            .contains(&warnings[0])
    );
    assert_eq!(disk.protected.as_ref(), Some(&legacy));
    assert_eq!(fs::read(&legacy).unwrap(), xml.as_bytes());
    assert!(!native.exists());
}

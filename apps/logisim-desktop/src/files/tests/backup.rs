use super::super::*;
use crate::tests::support::Temp;
use logisim_core::{Circuit, Project};
use logisim_sim::ProjectSimulator;
use std::fs;

#[test]
fn save_as_backs_up_only_the_confirmed_destination_and_records_only_the_project() {
    let temp = Temp::new();
    let fresh = temp.path("new.lrs");
    let existing = temp.path("existing.lrs");
    let backup = temp.path("existing.lrs.bak");
    let mut h = super::Harness::default();
    let mut settings = h.ws.preferences();
    settings.files.backup_before_save = true;
    h.ws.apply_settings(settings);
    h.ws.replace_project(Project::single("new project", Circuit::default()));
    let choose = |h: &mut super::Harness, target: &Path| {
        h.files
            .request(FileAction::SaveAs, &h.ctx, &mut h.ws, &mut h.sim);
        let picker = h.files.dialog.take().unwrap();
        h.files.respond(
            picker,
            Response::Pick(target.into()),
            &h.ctx,
            &mut h.ws,
            &mut h.sim,
        );
    };
    choose(&mut h, &fresh);
    assert!(!h.files.busy() && !temp.path("new.lrs.bak").exists());
    let old = b"confirmed\0raw\xff\r\n";
    fs::write(&existing, old).unwrap();
    choose(&mut h, &existing);
    assert!(matches!(&h.files.dialog, Some(Dialog::Replace { bytes, .. }) if bytes == old));
    assert!(!backup.exists());
    let prompt = h.files.dialog.take().unwrap();
    h.files
        .respond(prompt, Response::Cancel, &h.ctx, &mut h.ws, &mut h.sim);
    assert!(!backup.exists());
    choose(&mut h, &existing);
    let prompt = h.files.dialog.take().unwrap();
    h.files
        .respond(prompt, Response::Replace, &h.ctx, &mut h.ws, &mut h.sim);
    assert_eq!(fs::read(&backup).unwrap(), old);
    assert_eq!(fs::read(&existing).unwrap(), fs::read(&fresh).unwrap());
    assert_eq!(
        h.ws.recent(),
        &[existing.to_str().unwrap(), fresh.to_str().unwrap()]
    );
}

#[test]
fn blocked_backup_reports_failure_and_keeps_target_baseline_and_pending_edits() {
    let temp = Temp::new();
    let path = temp.path("layout.circ");
    let backup = temp.path("layout.circ.bak");
    let mut files = Files::default();
    let mut ws = Workspace::default();
    let ctx = egui::Context::default();
    let mut sim = ProjectSimulator::default();
    fs::write(&path, logisim_file::serialize(ws.project()).unwrap()).unwrap();
    assert!(files.load(&path, &mut ws, &mut sim));
    let original = fs::read(&path).unwrap();
    let mut options = ws.preferences();
    options.files.backup_before_save = true;
    ws.apply_settings(options);
    ws.replace_project(Project::single("unsaved", Circuit::default()));
    fs::create_dir(&backup).unwrap();
    for directory in [true, false] {
        files.request(FileAction::Save, &ctx, &mut ws, &mut sim);
        let Some(Dialog::Message { body, .. }) = files.dialog.take() else {
            panic!("expected a visible save error")
        };
        assert!(body.contains("layout.circ.bak") && body.contains("Save stopped"));
        assert!(files.present(&ws).dirty);
        assert_eq!(fs::read(&path).unwrap(), original);
        if directory {
            fs::remove_dir(&backup).unwrap();
            fs::write(&backup, "earlier generation").unwrap();
            let mut perms = fs::metadata(&backup).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(&backup, perms).unwrap();
        } else {
            assert_eq!(fs::read(&backup).unwrap(), b"earlier generation");
        }
    }
    fs::set_permissions(&backup, fs::metadata(&path).unwrap().permissions()).unwrap();
    files.request(FileAction::Save, &ctx, &mut ws, &mut sim);
    assert!(!files.busy() && !files.present(&ws).dirty);
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert_ne!(fs::read(&path).unwrap(), original);
}

#[test]
fn project_copy_cancel_conflict_and_open_native_preserve_binding_and_recent() {
    let dir = Temp::new();
    let circ = dir.path("original.circ");
    let lrs = dir.path("converted.lrs");
    let xml = logisim_file::serialize(&Project::default()).unwrap();
    fs::write(&circ, &xml).unwrap();
    fs::write(&lrs, b"previous").unwrap();
    let mut h = super::Harness::default();
    let mut prefs = h.ws.preferences();
    prefs.files.backup_before_save = true;
    h.ws.apply_settings(prefs);
    assert!(h.files.load(&circ, &mut h.ws, &mut h.sim));
    fn respond(h: &mut super::Harness, answer: Response) {
        let dialog = h.files.dialog.take().unwrap();
        h.files
            .respond(dialog, answer, &h.ctx, &mut h.ws, &mut h.sim);
    }
    let choose = |h: &mut super::Harness| {
        h.files
            .request(FileAction::ConvertNative, &h.ctx, &mut h.ws, &mut h.sim);
        respond(h, Response::Pick(lrs.clone()));
        assert!(matches!(h.files.dialog, Some(Dialog::ReplaceCopy { .. })));
    };
    choose(&mut h);
    respond(&mut h, Response::Cancel);
    assert_eq!(fs::read(&lrs).unwrap(), b"previous");
    choose(&mut h);
    fs::write(&lrs, b"external").unwrap();
    respond(&mut h, Response::Replace);
    assert_eq!(fs::read(&lrs).unwrap(), b"external");
    respond(&mut h, Response::Cancel);
    choose(&mut h);
    respond(&mut h, Response::Replace);
    assert_eq!(
        fs::read(dir.path("converted.lrs.bak")).unwrap(),
        b"external"
    );
    let bytes = fs::read(&lrs).unwrap();
    let native = logisim_file::parse_native(&bytes).unwrap();
    assert_eq!(h.files.storage.path.as_ref(), Some(&circ));
    assert_eq!(h.ws.recent(), &[circ.to_str().unwrap()]);
    assert!(!h.files.present(&h.ws).dirty);
    assert_eq!(fs::read(&circ).unwrap(), xml.as_bytes());
    respond(&mut h, Response::Cancel);
    h.files
        .request(FileAction::OpenNative, &h.ctx, &mut h.ws, &mut h.sim);
    respond(&mut h, Response::Pick(lrs.clone()));
    assert_eq!(h.ws.project(), &native);
    assert_eq!(h.ws.recent()[0], lrs.to_str().unwrap());
    let mut edit = native;
    edit.main_circuit_mut().circuit.label.text = "native edit".into();
    h.ws.replace_project(edit);
    let lock = logisim_preferences::storage::lock_target(&lrs).unwrap();
    h.files
        .request(FileAction::Save, &h.ctx, &mut h.ws, &mut h.sim);
    assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
    assert!(h.files.present(&h.ws).dirty);
    assert_eq!(fs::read(&lrs).unwrap(), bytes);
    lock.unlock().unwrap();
    respond(&mut h, Response::Cancel);
    h.files
        .request(FileAction::Save, &h.ctx, &mut h.ws, &mut h.sim);
    assert_eq!(
        logisim_file::parse_native(&fs::read(&lrs).unwrap()).unwrap(),
        *h.ws.project()
    );
    assert_eq!(fs::read(dir.path("converted.lrs.bak")).unwrap(), bytes);
    assert_eq!(fs::read(circ).unwrap(), xml.as_bytes());
}

#[test]
fn first_save_binds_native_and_legacy_save_as_requires_a_type_and_confirmation() {
    let mut h = super::Harness::default();
    let dir = Temp::new();
    h.files.directory.clone_from(&dir.0);
    h.files
        .request(FileAction::New, &h.ctx, &mut h.ws, &mut h.sim);
    h.files
        .request(FileAction::Save, &h.ctx, &mut h.ws, &mut h.sim);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::Picker(Picker {
            mode: Mode::Save(ProjectFormat::Lrs),
            ..
        }))
    ));
    h.click("Save");
    let native = dir.path("Untitled.lrs");
    assert!(logisim_file::parse_native(&fs::read(&native).unwrap()).is_ok());
    assert_eq!(h.files.storage.path.as_ref(), Some(&native));
    let legacy = dir.path("Untitled.circ");
    h.files
        .request(FileAction::SaveAs, &h.ctx, &mut h.ws, &mut h.sim);
    h.click("Logisim-rs Project (*.lrs)");
    h.click("Legacy Logisim (*.circ)");
    h.click("Save");
    assert!(matches!(h.files.dialog, Some(Dialog::ConfirmLegacy(_))));
    h.click("Cancel");
    assert!(!legacy.exists());
    assert_eq!(h.files.storage.path.as_ref(), Some(&native));
    h.files
        .request(FileAction::SaveAs, &h.ctx, &mut h.ws, &mut h.sim);
    h.click("Logisim-rs Project (*.lrs)");
    h.click("Legacy Logisim (*.circ)");
    h.click("Save");
    h.click("Save");
    let first = fs::read(&legacy).unwrap();
    let xml = |bytes: &[u8]| ProjectFormat::Circ.parse(bytes).unwrap().document;
    let tag = xml(&first)
        .main_circuit()
        .circuit
        .agent_ids
        .circuit
        .unwrap();
    assert_eq!(
        h.ws.recent(),
        &[legacy.to_str().unwrap(), native.to_str().unwrap()]
    );
    h.files
        .request(FileAction::Save, &h.ctx, &mut h.ws, &mut h.sim);
    assert_eq!(
        xml(&fs::read(&legacy).unwrap())
            .main_circuit()
            .circuit
            .agent_ids
            .circuit,
        Some(tag)
    );
    h.files
        .request(FileAction::SaveAs, &h.ctx, &mut h.ws, &mut h.sim);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::Picker(Picker {
            mode: Mode::Save(ProjectFormat::Lrs),
            ..
        }))
    ));
}

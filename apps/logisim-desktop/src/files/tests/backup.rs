use super::super::*;
use crate::tests::support::Temp;
use logisim_core::{Circuit, Project};
use logisim_sim::ProjectSimulator;
use std::fs;

#[test]
fn save_as_backs_up_only_the_confirmed_destination_and_records_only_the_project() {
    let temp = Temp::new();
    let fresh = temp.path("new.circ");
    let existing = temp.path("existing.circ");
    let backup = temp.path("existing.circ.bak");
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
    assert!(!h.files.busy() && !temp.path("new.circ.bak").exists());
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

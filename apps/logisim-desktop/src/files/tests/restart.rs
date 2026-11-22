use super::super::*;
use super::{
    Harness,
    recovery::{checkpoint, dirty_project},
};
use std::fs;

fn request_restart(h: &mut Harness) {
    h.files
        .request(FileAction::Restart, &h.ctx, &mut h.ws, &mut h.sim);
}

fn answer(h: &mut Harness, response: Response) {
    let dialog = h.files.dialog.take().unwrap();
    h.files
        .respond(dialog, response, &h.ctx, &mut h.ws, &mut h.sim);
}

#[test]
fn clean_restart_flushes_the_latest_preferences_before_launching_once() {
    let mut h = Harness::default();
    let config = h.config_dir.path("preferences.json");
    let mut settings = h.ws.preferences();
    settings.wasm_engine = logisim_ui::WasmEnginePreference::Wasmi;
    h.ws.apply_settings(settings);
    request_restart(&mut h);
    assert!(h.files.restart_pending && !h.files.allow_close && h.files.dialog.is_none());
    let launched = h
        .files
        .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || {
            let prefs: serde_json::Value =
                serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
            assert_eq!(prefs["plugins"]["wasm_engine"], "wasmi");
            Ok(())
        });
    assert_eq!(launched, Some(()));
    assert!(h.files.allow_close);
    assert_eq!(
        h.files
            .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || panic!(
                "duplicate launch"
            )),
        None::<()>
    );
}

#[test]
fn restart_waits_for_dirty_navigation_and_successful_save() {
    let mut h = Harness::default();
    let edited = dirty_project();
    h.ws.replace_project(edited.clone());
    request_restart(&mut h);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::Unsaved(NextDocument::Restart))
    ));
    answer(&mut h, Response::Cancel);
    let no_launch = |h: &mut Harness| {
        assert_eq!(
            h.files
                .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || panic!(
                    "unapproved restart"
                )),
            None::<()>
        );
        assert!(!h.files.allow_close && h.files.after_save.is_none());
        assert!(h.files.storage.dirty(h.ws.project()));
    };
    no_launch(&mut h);

    request_restart(&mut h);
    answer(&mut h, Response::Save);
    answer(&mut h, Response::Cancel);
    no_launch(&mut h);

    let target = h.config_dir.path("project.circ");
    h.files
        .storage
        .open_legacy_fixture(&logisim_core::Project::default(), &target);
    let previous = fs::read(&target).unwrap();
    fs::write(&target, b"external edit").unwrap();
    request_restart(&mut h);
    answer(&mut h, Response::Save);
    assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
    assert_eq!(fs::read(&target).unwrap(), b"external edit");
    no_launch(&mut h);
    answer(&mut h, Response::Cancel);

    fs::write(&target, previous).unwrap();
    request_restart(&mut h);
    answer(&mut h, Response::Save);
    assert!(h.files.restart_pending && !h.files.storage.dirty(h.ws.project()));
    let result = h
        .files
        .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || {
            let saved = ProjectFormat::Circ
                .parse(&fs::read(target).unwrap())
                .unwrap();
            assert_eq!(saved.document.main_circuit().circuit.components.len(), 1);
            Ok(())
        });
    assert_eq!(result, Some(()));
    assert!(h.files.allow_close);
}

#[test]
fn restart_after_discard_preserves_recovery_until_the_child_has_started() {
    let mut h = Harness::default();
    let dir = h.config_dir.path("recovery");
    h.files = Files::with_recovery(Some(dir.clone()));
    let source = h.config_dir.path("original.circ");
    h.files.storage.open_legacy_fixture(h.ws.project(), &source);
    let original = fs::read(&source).unwrap();
    let edited = dirty_project();
    h.ws.replace_project(edited.clone());
    checkpoint(&mut h.files, &h.ws);
    let data = dir.join("current.circ");
    let snapshot = fs::read(&data).unwrap();
    request_restart(&mut h);
    answer(&mut h, Response::Discard);
    let result = h
        .files
        .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || {
            Err::<(), _>(std::io::Error::other("test spawn failure"))
        });
    assert!(result.is_none() && !h.files.allow_close && !h.files.restart_pending);
    assert!(
        matches!(&h.files.dialog, Some(Dialog::Message { title, body }) if title == "Restart failed" && body.contains("test spawn failure"))
    );
    assert!(h.files.autosave.view().owned);
    assert_eq!(h.ws.project(), &edited);
    assert_eq!(fs::read(&data).unwrap(), snapshot);
    answer(&mut h, Response::Cancel);

    request_restart(&mut h);
    answer(&mut h, Response::Discard);
    let result = h
        .files
        .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || {
            assert_eq!(fs::read(&data).unwrap(), snapshot);
            Ok(())
        });
    assert_eq!(result, Some(()));
    assert!(h.files.allow_close && !data.exists() && !dir.join("current.json").exists());
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn a_preferences_conflict_blocks_restart_and_preserves_external_changes() {
    let mut h = Harness::default();
    let mut settings = h.ws.preferences();
    settings.wasm_engine = logisim_ui::WasmEnginePreference::Wasmi;
    h.ws.apply_settings(settings);
    h.preferences.synchronize(&mut h.ws);
    let path = h.config_dir.path("preferences.json");
    fs::write(&path, b"changed after engine selection").unwrap();
    request_restart(&mut h);
    let result = h
        .files
        .restart_if_pending(&h.ctx, &mut h.ws, &mut h.preferences, || {
            panic!("preferences unavailable")
        });
    assert_eq!(result, None::<()>);
    assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
    assert!(!h.files.allow_close && !h.files.restart_pending);
    assert_eq!(fs::read(&path).unwrap(), b"changed after engine selection");
}

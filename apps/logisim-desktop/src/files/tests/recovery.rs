use super::super::*;
use crate::tests::support::Temp;
use logisim_core::{Component, Kind, Point};
use logisim_sim::ProjectSimulator as Simulator;
use std::{
    fs,
    time::{Duration, Instant},
};

fn checkpoint(files: &mut Files, w: &Workspace) {
    let options = w.preferences().files;
    let now = Instant::now();
    for at in [
        now,
        now + Duration::from_secs(u64::from(options.autosave_minutes) * 60),
    ] {
        files.autosave.update(
            at,
            options,
            files.storage.dirty(w.project()),
            &files.storage,
            w.project(),
        );
    }
}

#[test]
fn only_completed_file_actions_clear_the_owned_snapshot() {
    for action in [
        FileAction::Save,
        FileAction::New,
        FileAction::Open,
        FileAction::Close,
    ] {
        let tmp = Temp::new();
        let dir = tmp.path("recovery");
        let source = tmp.path("original.circ");
        let mut files = Files::with_recovery(Some(dir.clone()));
        let mut w = Workspace::default();
        let mut sim = Simulator::default();
        let ctx = egui::Context::default();
        files.storage.open_legacy_fixture(w.project(), &source);
        let original = fs::read(&source).unwrap();
        let mut edited = w.project().clone();
        edited
            .main_circuit_mut()
            .circuit
            .add_component(Component::new(Kind::Pin, Point::ZERO));
        w.replace_project(edited.clone());
        checkpoint(&mut files, &w);
        let exists = || dir.join("current.circ").is_file() && dir.join("current.json").is_file();
        assert!(exists());
        files.request(FileAction::Close, &ctx, &mut w, &mut sim);
        let confirm = files.dialog.take().unwrap();
        files.respond(confirm, Response::Cancel, &ctx, &mut w, &mut sim);
        assert!(exists() && !files.allow_close);
        files.request(FileAction::Open, &ctx, &mut w, &mut sim);
        let confirm = files.dialog.take().unwrap();
        files.respond(confirm, Response::Discard, &ctx, &mut w, &mut sim);
        let picker = files.dialog.take().unwrap();
        files.respond(picker, Response::Cancel, &ctx, &mut w, &mut sim);
        assert!(exists());
        assert_eq!(w.project(), &edited);
        assert_eq!(fs::read(&source).unwrap(), original);
        files.request(action, &ctx, &mut w, &mut sim);
        if action != FileAction::Save {
            let confirm = files.dialog.take().unwrap();
            files.respond(confirm, Response::Discard, &ctx, &mut w, &mut sim);
        }
        if action == FileAction::Open {
            assert!(exists());
            let picker = files.dialog.take().unwrap();
            files.respond(
                picker,
                Response::Pick(source.clone()),
                &ctx,
                &mut w,
                &mut sim,
            );
        }
        assert!(!dir.join("current.circ").exists() && !dir.join("current.json").exists());
        assert_eq!(
            fs::read(&source).unwrap() == original,
            action != FileAction::Save
        );
    }
}

fn dirty_project() -> logisim_core::Project {
    let mut p = logisim_core::Project::default();
    let c = &mut p.main_circuit_mut().circuit;
    let pin = c.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    c.agent_ids.circuit = Some(1);
    c.agent_ids.items.insert(pin, 2);
    p
}

#[test]
fn startup_gives_recovery_priority_even_when_disabled_and_save_keeps_conflict_checks() {
    let temp = Temp::new();
    let dir = temp.path("recovery");
    let source = temp.path("source.circ");
    let cli = temp.path("cli.circ");
    let bytes = logisim_file::serialize(&logisim_core::Project::default()).unwrap();
    let mut files = Files::with_recovery(Some(dir.clone()));
    let mut w = Workspace::default();
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    fs::write(&cli, &bytes).unwrap();
    fs::write(&source, &bytes).unwrap();
    assert!(files.load(&source, &mut w, &mut sim));
    let edited = dirty_project();
    w.replace_project(edited.clone());
    checkpoint(&mut files, &w);
    let different = format!("{bytes}\n<!-- independent edit before restart -->");
    fs::write(&source, &different).unwrap();
    drop(files); // Simulate the original process exiting.
    let mut recovered = Workspace::default();
    let mut options = recovered.preferences();
    options.files.autosave = false;
    recovered.apply_settings(options);
    let mut files = Files::with_recovery(Some(dir.clone()));
    files.startup(Some(cli), &mut recovered, &mut sim);
    assert!(files.storage.path.is_none() && !files.present(&recovered).dirty);
    let prompt = files.dialog.take().unwrap();
    assert!(matches!(prompt, Dialog::Recovery { .. }));
    files.respond(prompt, Response::Cancel, &ctx, &mut recovered, &mut sim);
    let prompt = files.dialog.take().unwrap();
    files.respond(prompt, Response::Recover, &ctx, &mut recovered, &mut sim);
    assert_eq!(recovered.project(), &edited);
    assert_eq!(files.storage.path.as_deref(), Some(source.as_path()));
    assert!(files.present(&recovered).dirty && recovered.recent().is_empty() && !files.busy());
    files.request(FileAction::Save, &ctx, &mut recovered, &mut sim);
    assert!(matches!(files.dialog, Some(Dialog::Message { .. })));
    assert_eq!(fs::read_to_string(&source).unwrap(), different);
    assert!(dir.join("current.circ").exists());
    let copy = temp.path("copy.lrs");
    files.dialog = None;
    files.request(FileAction::SaveAs, &ctx, &mut recovered, &mut sim);
    let picker = files.dialog.take().unwrap();
    files.respond(
        picker,
        Response::Pick(copy.clone()),
        &ctx,
        &mut recovered,
        &mut sim,
    );
    assert!(!files.present(&recovered).dirty && !dir.join("current.json").exists());
    assert_eq!(
        logisim_file::parse_native(&fs::read(copy).unwrap()).unwrap(),
        edited
    );
}

#[test]
fn missing_and_untitled_sources_recover_without_a_destination_or_history_entry() {
    for named in [false, true] {
        let temp = Temp::new();
        let dir = temp.path("recovery");
        let mut files = Files::with_recovery(Some(dir.clone()));
        let mut w = Workspace::default();
        let source = temp.path("gone.circ");
        if named {
            files.storage.open_legacy_fixture(w.project(), &source);
        }
        w.replace_project(dirty_project());
        checkpoint(&mut files, &w);
        if named {
            fs::remove_file(&source).unwrap();
        }
        drop(files); // The crashed process no longer holds the writer lease.
        let mut w = Workspace::default();
        let mut files = Files::with_recovery(Some(dir.clone()));
        let ctx = egui::Context::default();
        let mut sim = Simulator::default();
        files.startup(None, &mut w, &mut sim);
        let prompt = files.dialog.take().unwrap();
        files.respond(prompt, Response::Recover, &ctx, &mut w, &mut sim);
        assert!(files.present(&w).dirty && files.present(&w).untitled);
        assert!(w.recent().is_empty());
        assert_eq!(w.project(), &dirty_project());
        files.request(FileAction::Save, &ctx, &mut w, &mut sim);
        assert!(matches!(
            files.dialog,
            Some(Dialog::Picker(Picker {
                mode: Mode::Save(ProjectFormat::Lrs),
                ..
            }))
        ));
        assert!(dir.join("current.json").is_file() && !source.exists());
    }
}

#[test]
fn damaged_or_incomplete_slot_waits_for_explicit_discard_before_cli_or_reopen() {
    for cli in [true, false] {
        let tmp = Temp::new();
        let dir = tmp.path("recovery");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("current.circ"), b"broken XML").unwrap();
        if cli {
            fs::write(dir.join("current.json"), b"{").unwrap();
        }
        let path = tmp.path("startup.circ");
        fs::write(&path, logisim_file::serialize(&dirty_project()).unwrap()).unwrap();
        let mut files = Files::with_recovery(Some(dir.clone()));
        let mut w = Workspace::default();
        let mut sim = Simulator::default();
        let ctx = egui::Context::default();
        let mut opts = w.preferences();
        opts.workspace.reopen_last = true;
        w.apply_settings(opts);
        w.set_recent(vec![path.display().to_string()]);
        files.startup(cli.then(|| path.clone()), &mut w, &mut sim);
        assert!(matches!(
            files.autosave.view().state,
            logisim_ui::SnapshotState::Damaged
        ));
        let prompt = files.dialog.take().unwrap();
        files.respond(prompt, Response::Recover, &ctx, &mut w, &mut sim);
        assert!(
            files.storage.path.is_none()
                && fs::read(dir.join("current.circ")).unwrap() == b"broken XML"
        );
        let prompt = files.dialog.take().unwrap();
        files.respond(prompt, Response::Discard, &ctx, &mut w, &mut sim);
        assert_eq!(files.storage.path, Some(path));
        assert!(!files.present(&w).dirty && !dir.join("current.circ").exists());
    }
}

#[test]
fn restored_project_resolves_external_plugins_from_the_snapshot_and_keeps_source_bytes() {
    let tmp = Temp::new();
    let dir = tmp.path("private/recovery");
    let path = tmp.path("board.circ");
    let (mut project, wasm) = crate::tests::support::plugins::fixture("test.recovery", 1);
    project.plugin_sources[0].path = "module.wasm".into();
    fs::write(tmp.path("module.wasm"), wasm).unwrap();
    let original = logisim_file::serialize(&project).unwrap();
    fs::write(&path, &original).unwrap();
    let mut files = Files::with_recovery(Some(dir.clone()));
    let mut w = Workspace::default();
    let mut sim = Simulator::default();
    files.load(&path, &mut w, &mut sim);
    let mut edited = w.project().clone();
    edited
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    w.replace_project(edited);
    checkpoint(&mut files, &w);
    drop(files);
    let mut files = Files::with_recovery(Some(dir));
    let mut fresh = Workspace::default();
    files.startup(None, &mut fresh, &mut sim);
    let prompt = files.dialog.take().unwrap();
    files.respond(
        prompt,
        Response::Recover,
        &egui::Context::default(),
        &mut fresh,
        &mut sim,
    );
    assert!(files.present(&fresh).dirty && !files.busy());
    assert_eq!(
        files.plugins.manifests().next().unwrap().id,
        "test.recovery"
    );
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn discard_conflict_preserves_concurrent_checkpoint_and_allows_startup_to_continue() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut owner = Files::with_recovery(Some(dir.clone()));
    let mut w = Workspace::default();
    w.replace_project(dirty_project());
    checkpoint(&mut owner, &w);
    let cli = tmp.path("cli.circ");
    fs::write(&cli, logisim_file::serialize(w.project()).unwrap()).unwrap();
    let mut second = Files::with_recovery(Some(dir.clone()));
    let mut fresh = Workspace::default();
    let mut sim = Simulator::default();
    second.startup(Some(cli.clone()), &mut fresh, &mut sim);
    let record = dir.join("current.json");
    let mut changed = fs::read(&record).unwrap();
    changed.push(b'\n');
    fs::write(&record, &changed).unwrap();
    let prompt = second.dialog.take().unwrap();
    second.respond(
        prompt,
        Response::Discard,
        &egui::Context::default(),
        &mut fresh,
        &mut sim,
    );
    assert_eq!(fs::read(record).unwrap(), changed);
    assert_eq!(second.storage.path, Some(cli));
    assert!(!second.busy() && second.autosave.view().error.is_some());
    assert!(matches!(
        second.autosave.view().state,
        logisim_ui::SnapshotState::Available { .. }
    ));
}

#[test]
fn clear_only_removes_snapshot_files_and_preserves_the_working_document() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut files = Files::with_recovery(Some(dir.clone()));
    let mut w = Workspace::default();
    let mut sim = Simulator::default();
    let path = tmp.path("source.circ");
    files.storage.open_legacy_fixture(w.project(), &path);
    let disk = fs::read(&path).unwrap();
    w.replace_project(dirty_project());
    w.simulation_mut().ticks = 37;
    checkpoint(&mut files, &w);
    let mut settings = w.preferences();
    settings.files.autosave = false;
    w.apply_settings(settings);
    files.request(
        FileAction::ClearRecovery {
            disabled_autosave: true,
        },
        &egui::Context::default(),
        &mut w,
        &mut sim,
    );
    assert!(files.present(&w).dirty && !files.busy());
    assert_eq!(w.project(), &dirty_project());
    assert_eq!(w.simulation_mut().ticks, 37);
    assert_eq!(fs::read(path).unwrap(), disk);
    checkpoint(&mut files, &w);
    assert!(!w.preferences().files.autosave);
    assert!(fs::read_dir(dir).unwrap().next().is_none());
}

#[test]
fn closing_after_returning_to_the_clean_baseline_removes_the_old_checkpoint() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut files = Files::with_recovery(Some(dir.clone()));
    let mut w = Workspace::default();
    let baseline = w.project().clone();
    w.replace_project(dirty_project());
    checkpoint(&mut files, &w);
    assert!(dir.join("current.circ").is_file());
    w.replace_project(baseline);
    assert!(!files.present(&w).dirty);
    files.request(
        FileAction::Close,
        &egui::Context::default(),
        &mut w,
        &mut Simulator::default(),
    );
    assert!(files.allow_close && !files.busy());
    assert!(fs::read_dir(dir).unwrap().next().is_none());
}

#[test]
fn clear_failure_rolls_back_only_the_disclosed_setting_and_allows_retry() {
    for previously_on in [true, false] {
        let tmp = Temp::new();
        let dir = tmp.path("recovery");
        let mut files = Files::with_recovery(Some(dir.clone()));
        let mut w = Workspace::default();
        w.replace_project(dirty_project());
        checkpoint(&mut files, &w);
        let mut values = w.preferences();
        values.files.autosave = false;
        w.apply_settings(values);
        let lock = fs::File::open(tmp.path(".recovery.lock")).unwrap();
        lock.try_lock().unwrap();
        let ctx = egui::Context::default();
        let mut sim = Simulator::default();
        let action = FileAction::ClearRecovery {
            disabled_autosave: previously_on,
        };
        files.request(action, &ctx, &mut w, &mut sim);
        let view = files.autosave.view();
        assert!(view.owned && view.error.as_deref() == Some("Recovery storage is busy."));
        assert_eq!(view.directory.unwrap(), dir.to_str().unwrap());
        assert!(dir.join("current.circ").is_file() && dir.join("current.json").is_file());
        assert_eq!(w.preferences().files.autosave, previously_on);
        assert!(!files.busy() && files.present(&w).dirty);
        drop(lock);
        w.apply_settings(values);
        files.request(action, &ctx, &mut w, &mut sim);
        assert_eq!(
            files.autosave.view().state,
            logisim_ui::SnapshotState::Empty
        );
        assert!(!w.preferences().files.autosave);
        assert_eq!(w.project(), &dirty_project());
    }
}

#[test]
fn foreign_recover_keeps_the_same_dialog_document_and_paused_simulator() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut owner = Files::with_recovery(Some(dir.clone()));
    let mut source = Workspace::default();
    source.replace_project(dirty_project());
    checkpoint(&mut owner, &source);
    let mut files = Files::with_recovery(Some(dir));
    let mut w = Workspace::default();
    let project = w.project().clone();
    let prefs = w.preferences();
    let cli = tmp.path("requested.circ");
    let mut sim = Simulator::default();
    sim.set_enabled(false);
    let ctx = egui::Context::default();
    files.startup(Some(cli.clone()), &mut w, &mut sim);
    let prompt = files.dialog.take().unwrap();
    files.respond(prompt, Response::Recover, &ctx, &mut w, &mut sim);
    let prompt = files.dialog.take().unwrap();
    let Dialog::Recovery { cli: pending, view } = &prompt else {
        panic!("Expected the recovery prompt")
    };
    assert_eq!(pending.as_ref(), Some(&cli));
    assert_eq!(
        view.error.as_deref(),
        Some("Recovery copy is owned by another window.")
    );
    assert!(!view.owned && !sim.enabled());
    assert_eq!(w.project(), &project);
    assert_eq!(w.preferences(), prefs);
    assert!(!files.present(&w).dirty);
    drop(owner);
    files.respond(prompt, Response::Recover, &ctx, &mut w, &mut sim);
    assert_eq!(w.project(), source.project());
    assert!(files.autosave.view().owned && files.present(&w).dirty && !files.busy());
}

#[test]
fn retry_after_a_changed_snapshot_recovers_the_latest_confirmed_project() {
    let tmp = Temp::new();
    let dir = tmp.path("recovery");
    let mut owner = Files::with_recovery(Some(dir.clone()));
    let mut source = Workspace::default();
    source.replace_project(dirty_project());
    checkpoint(&mut owner, &source);
    let mut files = Files::with_recovery(Some(dir));
    let mut w = Workspace::default();
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    files.startup(None, &mut w, &mut sim);
    let mut edited = source.project().clone();
    edited.main_circuit_mut().name = "next checkpoint".into();
    owner.autosave.update(
        Instant::now() + Duration::from_secs(900),
        source.preferences().files,
        true,
        &owner.storage,
        &edited,
    );
    drop(owner);
    let before = w.project().clone();
    let prompt = files.dialog.take().unwrap();
    files.respond(prompt, Response::Recover, &ctx, &mut w, &mut sim);
    let prompt = files.dialog.take().unwrap();
    assert!(
        matches!(&prompt, Dialog::Recovery { view, .. } if view.error.as_deref() == Some("The recovery snapshot changed in another window."))
    );
    assert_eq!(w.project(), &before);
    files.respond(prompt, Response::Recover, &ctx, &mut w, &mut sim);
    assert_eq!(w.project(), &edited);
    assert!(!files.busy() && files.autosave.view().owned);
}

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
        files
            .storage
            .save(w.project(), &source, SaveMode::Current)
            .unwrap();
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

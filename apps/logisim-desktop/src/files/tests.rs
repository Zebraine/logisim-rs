use super::*;
use logisim_core::{Circuit, Component, Kind, Point, Project};

#[test]
fn library_picker_is_additive_and_failure_or_cancel_keeps_document_baseline() {
    let temp = crate::file_tests::Temp::new();
    let current = temp.path("current.circ");
    let library = temp.path("File.circ");
    let source = logisim_file::serialize(&Project::default()).unwrap();
    std::fs::write(&library, &source).unwrap();
    let mut files = Files::default();
    let mut workspace = Workspace::default();
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    files
        .storage
        .save(workspace.project(), &current, SaveMode::Current)
        .unwrap();
    let bytes = std::fs::read(&current).unwrap();
    for cancel in [true, false] {
        files.request(FileAction::ImportLibrary, &ctx, &mut workspace, &mut sim);
        let dialog = files.dialog.take().unwrap();
        assert!(matches!(&dialog, Dialog::Picker(p) if p.mode == Mode::Library));
        let response = if cancel {
            Response::Cancel
        } else {
            Response::Pick(temp.path("missing.circ"))
        };
        files.respond(dialog, response, &ctx, &mut workspace, &mut sim);
        assert!(!files.storage.dirty(workspace.project()));
        files.dialog = None;
    }
    files.request(FileAction::ImportLibrary, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    files.respond(
        dialog,
        Response::Pick(library.clone()),
        &ctx,
        &mut workspace,
        &mut sim,
    );
    assert!(!files.busy());
    assert!(files.storage.dirty(workspace.project()));
    assert_eq!(files.storage.path, Some(current.clone()));
    assert_eq!(workspace.project().circuits.len(), 2);
    assert_eq!(std::fs::read(&current).unwrap(), bytes);
    assert_eq!(std::fs::read_to_string(library).unwrap(), source);
}

#[test]
fn cancelling_save_before_new_preserves_document_and_clears_deferred_action() {
    let mut files = Files::default();
    let mut workspace = Workspace::default();
    let mut circuit = Circuit::default();
    circuit.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    let project = Project::single("unsaved", circuit);
    workspace.replace_project(project.clone());
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    files.request(FileAction::New, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    assert!(matches!(dialog, Dialog::Unsaved(FileAction::New)));
    files.respond(dialog, Response::Save, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    assert!(matches!(dialog, Dialog::Picker(_)));
    assert!(matches!(files.after_save, Some(FileAction::New)));
    files.respond(dialog, Response::Cancel, &ctx, &mut workspace, &mut sim);
    assert!(!files.busy());
    assert!(files.after_save.is_none());
    assert_eq!(workspace.project(), &project);
    files.request(FileAction::New, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    files.respond(dialog, Response::Discard, &ctx, &mut workspace, &mut sim);
    assert_eq!(workspace.project(), &Project::default());
    assert!(!files.storage.dirty(workspace.project()));
}

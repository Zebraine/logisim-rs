use super::*;
use logisim_core::{Circuit, Component, Kind, Point, Project};

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

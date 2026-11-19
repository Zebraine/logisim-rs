use super::*;
use crate::{Workspace, editor::Tool, project::CircuitCommand};
use logisim_core::Point;

#[test]
fn cached_geometry_survives_rename_and_expires_on_edit_undo_redo_and_load() {
    let mut w = Workspace::default();
    let id = w.active_circuit();
    let cache_empty = |w: &Workspace| w.editor.thumbnails.entries.is_empty();
    fn warm(w: &mut Workspace, id: Id) {
        let preview = Preview::new(&w.project().circuit(id).unwrap().circuit);
        w.editor.thumbnails.entries.insert(id, preview);
    }
    warm(&mut w, id);
    w.editor
        .edit_project(CircuitCommand::Rename, "CPU".into())
        .unwrap();
    assert!(!cache_empty(&w));
    w.editor.set_tool(Tool::Place(Kind::Pin));
    w.editor.place(Point::new(100, 40));
    assert!(cache_empty(&w));
    warm(&mut w, id);
    assert!(!w.editor.thumbnails.entries[&id].segments.is_empty());
    w.editor.undo();
    assert!(cache_empty(&w));
    warm(&mut w, id);
    w.editor.redo();
    assert!(cache_empty(&w));
    warm(&mut w, id);
    w.replace_project(Project::default());
    assert!(cache_empty(&w));
}

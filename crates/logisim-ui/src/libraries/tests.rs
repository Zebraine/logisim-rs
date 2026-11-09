use super::*;
use logisim_core::{Point, Project};
#[test]
fn one_undo_removes_import_and_redo_restores_placeable_definitions() {
    let mut workspace = Workspace::default();
    let mut source = Project::default();
    source.add("child".into()).unwrap();
    let import = workspace.project().with_library(&source, "File").unwrap();
    let id = import.circuits[0];
    workspace.apply_library_import(import);
    workspace.editor.set_tool(Tool::Place(Kind::Subcircuit(id)));
    workspace.editor.undo();
    assert_eq!(workspace.project(), &Project::default());
    assert_eq!(workspace.editor.tool, Tool::Select);
    workspace.editor.redo();
    workspace.editor.set_tool(Tool::Place(Kind::Subcircuit(id)));
    workspace.editor.place(Point::new(200, 100));
    workspace.finish_edit();
    assert_eq!(
        workspace.project().main_circuit().circuit.components[0].kind,
        Kind::Subcircuit(id)
    );
    workspace.editor.undo();
    workspace.editor.undo();
    let next = workspace.project().with_library(&source, "File").unwrap();
    assert!(!next.circuits.contains(&id));
    workspace.replace_project(Project::default());
    assert!(workspace.libraries.is_empty());
}

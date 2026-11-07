//! Session palette groups for imported, editable project snapshots.
use crate::{Workspace, editor::Tool, i18n::tr};
use logisim_core::{Id, Kind, LibraryImport};

pub(crate) struct LibraryGroup {
    name: String,
    circuits: Vec<Id>,
}

impl Workspace {
    /// Commit an import prepared from `project()` while file editing is modal.
    /// The host checks file-format limits before handing the result to the editor.
    pub fn apply_library_import(&mut self, import: LibraryImport) {
        self.finish_edit();
        let before = self.editor.snapshot();
        self.editor.project = import.project;
        self.editor.circuit = self
            .editor
            .project
            .circuit(self.editor.active)
            .unwrap()
            .circuit
            .clone();
        self.editor.record(before);
        self.libraries.push(LibraryGroup {
            name: import.name,
            circuits: import.circuits,
        });
    }

    pub(crate) fn library_groups(&mut self, ui: &mut egui::Ui) {
        for group in &self.libraries {
            let definitions: Vec<_> = group
                .circuits
                .iter()
                .filter_map(|id| self.editor.project.circuit(*id))
                .map(|d| (d.id, d.name.clone()))
                .collect();
            if definitions.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(&group.name)
                .id_salt(("imported-library", group.circuits[0]))
                .default_open(true)
                .show(ui, |ui| {
                    ui.weak(tr("Embedded library snapshot"));
                    for (id, name) in definitions {
                        let tool = Tool::Place(Kind::Subcircuit(id));
                        let allowed = self.editor.project.can_instantiate(self.editor.active, id);
                        let response = ui.add_enabled(allowed, egui::Button::selectable(self.editor.tool == tool, name))
                            .on_hover_text(tr("Double-click to edit; single-click another circuit to place an instance."));
                        if response.double_clicked() {
                            self.editor.switch_circuit(id);
                        } else if response.clicked() {
                            self.editor.appearance_mode = false;
                            self.editor.set_tool(tool);
                        }
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
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
}

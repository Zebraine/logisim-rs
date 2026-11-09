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
mod tests;

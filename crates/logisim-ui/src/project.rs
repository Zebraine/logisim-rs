//! Project editing shares the same undo timeline as geometry edits.
use crate::{
    Workspace,
    editor::{Editor, Tool},
};
use crate::{i18n::tr, tf};
use logisim_core::{Id, Kind, Project};

#[derive(Clone, Copy)]
pub(crate) enum CircuitCommand {
    Add,
    Rename,
    Remove,
    Main,
    Up,
    Down,
}
pub(crate) struct NameDialog {
    pub rename: bool,
    pub name: String,
    pub error: String,
}
impl Editor {
    pub fn sync_project(&mut self) {
        self.session.synchronize();
        if let Kind::Subcircuit(id) = self.template.kind
            && let Some(definition) = self.session.project.circuit(id)
        {
            self.template.interface = definition.interface();
        }
    }
    pub fn context_label(&self) -> String {
        self.session.context_label()
    }
    fn prepare_navigation(&mut self) {
        self.session.context.appearance_mode = false;
        self.cancel_gesture();
        self.commit_properties();
        self.sync_project();
    }
    fn finish_navigation(&mut self) {
        self.set_tool(Tool::Select);
        self.simulation.view = Default::default();
        self.simulation.plugin_arguments.clear();
    }
    pub fn switch_circuit(&mut self, id: Id) {
        self.prepare_navigation();
        if self.session.switch_circuit(id) {
            self.finish_navigation();
        }
    }
    pub fn enter_instance(&mut self, id: Id) {
        self.prepare_navigation();
        if self.session.enter_instance(id) {
            self.finish_navigation();
        }
    }
    pub fn leave_instance(&mut self) {
        self.prepare_navigation();
        if self.session.leave_instance() {
            self.finish_navigation();
        }
    }
    pub fn edit_project(&mut self, command: CircuitCommand, name: String) -> Result<(), String> {
        self.cancel_gesture();
        self.commit_properties();
        self.sync_project();
        let before = self.snapshot();
        match command {
            CircuitCommand::Add => {
                let id = self.session.project.add(name)?;
                self.switch_circuit(id);
            }
            CircuitCommand::Rename => self
                .session
                .project
                .rename(self.session.context.active, name)?,
            CircuitCommand::Remove => {
                self.session.project.remove(self.session.context.active)?;
                let id = self.session.project.main;
                self.session.context.active = id;
                self.session.context.root = id;
                self.session.context.path.clear();
                self.session.selection.clear();
            }
            CircuitCommand::Main => self.session.project.main = self.session.context.active,
            CircuitCommand::Up | CircuitCommand::Down => {
                let index = self
                    .session
                    .project
                    .circuits
                    .iter()
                    .position(|c| c.id == self.session.context.active)
                    .unwrap();
                let other = if matches!(command, CircuitCommand::Up) {
                    index.checked_sub(1)
                } else {
                    Some(index + 1).filter(|i| *i < self.session.project.circuits.len())
                };
                if let Some(other) = other {
                    self.session.project.circuits.swap(index, other);
                }
            }
        }
        self.session.reload();
        self.record(before);
        Ok(())
    }
}
impl Workspace {
    pub fn project(&self) -> &Project {
        &self.editor.session.project
    }
    pub fn active_circuit(&self) -> Id {
        self.editor.session.context.active
    }
    pub fn circuit_name(&self) -> &str {
        &self
            .editor
            .session
            .project
            .circuit(self.editor.session.context.active)
            .unwrap()
            .name
    }
    pub fn simulation_root(&self) -> Id {
        self.editor.session.context.root
    }
    pub fn simulation_path(&self) -> &[Id] {
        &self.editor.session.context.path
    }
    pub fn replace_project(&mut self, mut project: Project) {
        self.analysis.cancel();
        self.generator.cancel_computation();
        let serial = self.generator.serial;
        self.generator = Default::default();
        self.generator.serial = serial;
        self.generated_expressions = Default::default();
        self.libraries.clear();
        project.refresh_interfaces(false);
        self.editor = Editor::new(project);
        self.pla_editor = crate::pla::PlaEditor::default();
        self.appearance.reset_context();
        self.name_dialog = None;
        self.remove_dialog = false;
        self.canvas.reset();
        self.fit_pending = true;
    }
    pub(crate) fn circuit_command(&mut self, command: CircuitCommand) {
        self.finish_edit();
        match command {
            CircuitCommand::Add | CircuitCommand::Rename => {
                let rename = matches!(command, CircuitCommand::Rename);
                self.name_dialog = Some(NameDialog {
                    rename,
                    name: if rename {
                        self.circuit_name().into()
                    } else {
                        String::new()
                    },
                    error: String::new(),
                });
            }
            CircuitCommand::Remove => self.remove_dialog = true,
            _ => {
                if let Err(e) = self.editor.edit_project(command, String::new()) {
                    self.editor.project_error = Some(e);
                }
            }
        }
    }
    pub(crate) fn project_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.name_dialog.take() {
            let mut apply = false;
            let mut cancel = false;
            let response = egui::Modal::new(egui::Id::unique("circuit-name")).show(ctx, |ui| {
                ui.heading(if dialog.rename {
                    tr("Rename circuit")
                } else {
                    tr("Add circuit")
                });
                ui.label(tr("Circuit name"));
                let response = ui.text_edit_singleline(&mut dialog.name);
                if !ctx.memory(|m| m.has_focus(response.id)) && dialog.name.is_empty() {
                    response.request_focus();
                }
                if !dialog.error.is_empty() {
                    ui.colored_label(egui::Color32::DARK_RED, tr(&dialog.error));
                }
                ui.horizontal(|ui| {
                    apply = ui
                        .button(if dialog.rename {
                            tr("Rename")
                        } else {
                            tr("Add")
                        })
                        .clicked();
                    cancel = ui.button(tr("Cancel")).clicked();
                });
                apply |= ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
            if apply {
                match self.editor.edit_project(
                    if dialog.rename {
                        CircuitCommand::Rename
                    } else {
                        CircuitCommand::Add
                    },
                    dialog.name.trim().into(),
                ) {
                    Ok(()) => {
                        self.canvas.reset();
                        self.fit_pending = true;
                    }
                    Err(e) => {
                        dialog.error = e;
                        self.name_dialog = Some(dialog);
                    }
                }
            } else if !cancel && !response.should_close() {
                self.name_dialog = Some(dialog);
            }
        }
        if self.remove_dialog {
            let mut remove = false;
            let mut cancel = false;
            let response = egui::Modal::new(egui::Id::unique("remove-circuit")).show(ctx, |ui| {
                ui.heading(tf!("Remove '{}' ?", self.circuit_name()));
                ui.label(tr(
                    "This removes the circuit and its contents. You can undo this change.",
                ));
                ui.horizontal(|ui| {
                    remove = ui.button(tr("Remove")).clicked();
                    cancel = ui.button(tr("Cancel")).clicked();
                });
            });
            if remove {
                self.remove_dialog = false;
                if let Err(e) = self
                    .editor
                    .edit_project(CircuitCommand::Remove, String::new())
                {
                    self.editor.project_error = Some(e);
                }
                self.canvas.reset();
                self.fit_pending = true;
            } else if cancel || response.should_close() {
                self.remove_dialog = false;
            }
        }
        if let Some(message) = self.editor.project_error.clone() {
            let mut close = false;
            let response = egui::Modal::new(egui::Id::unique("project-error")).show(ctx, |ui| {
                ui.heading(tr("Circuit operation unavailable"));
                ui.label(tr(&message));
                close = ui.button(tr("OK")).clicked();
            });
            if close || response.should_close() {
                self.editor.project_error = None;
            }
        }
    }
}

#[cfg(test)]
mod tests;

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
        let changed = self
            .project
            .circuit(self.active)
            .is_some_and(|d| d.circuit != self.circuit);
        if changed {
            self.project.circuit_mut(self.active).unwrap().circuit = self.circuit.clone();
            self.project.refresh_interfaces(true);
            self.circuit = self.project.circuit(self.active).unwrap().circuit.clone();
        }
        if let Kind::Subcircuit(id) = self.template.kind
            && let Some(definition) = self.project.circuit(id)
        {
            self.template.interface = definition.interface();
        }
    }
    pub fn context_label(&self) -> String {
        let mut definition = self.root;
        let mut names = vec![
            self.project
                .circuit(definition)
                .map(|d| d.name.clone())
                .unwrap_or_default(),
        ];
        for id in &self.path {
            if let Some(c) = self
                .project
                .circuit(definition)
                .and_then(|d| d.circuit.component(*id))
                && let Kind::Subcircuit(target) = c.kind
            {
                names.push(if c.label.is_empty() {
                    format!("{} #{}", c.interface.name, c.id)
                } else {
                    c.label.clone()
                });
                definition = target;
            }
        }
        names.join(" / ")
    }
    pub fn switch_circuit(&mut self, id: Id) {
        self.appearance_mode = false;
        self.cancel_gesture();
        self.commit_properties();
        self.sync_project();
        if let Some(definition) = self.project.circuit(id) {
            self.active = id;
            self.root = id;
            self.path.clear();
            self.circuit = definition.circuit.clone();
            self.selection.clear();
            self.set_tool(Tool::Select);
            self.simulation.view = Default::default();
            self.simulation.plugin_arguments.clear();
        }
    }
    pub fn enter_instance(&mut self, id: Id) {
        self.appearance_mode = false;
        self.cancel_gesture();
        self.commit_properties();
        self.sync_project();
        if let Some(c) = self.circuit.component(id)
            && let Kind::Subcircuit(target) = c.kind
            && let Some(definition) = self.project.circuit(target)
        {
            self.path.push(id);
            self.active = target;
            self.circuit = definition.circuit.clone();
            self.selection.clear();
            self.set_tool(Tool::Select);
            self.simulation.view = Default::default();
            self.simulation.plugin_arguments.clear();
        }
    }
    pub fn leave_instance(&mut self) {
        self.appearance_mode = false;
        self.cancel_gesture();
        self.commit_properties();
        self.sync_project();
        if self.path.pop().is_none() {
            return;
        }
        let mut id = self.root;
        for instance in &self.path {
            if let Some(c) = self
                .project
                .circuit(id)
                .and_then(|d| d.circuit.component(*instance))
                && let Kind::Subcircuit(target) = c.kind
            {
                id = target;
            }
        }
        self.active = id;
        self.circuit = self.project.circuit(id).unwrap().circuit.clone();
        self.selection.clear();
        self.set_tool(Tool::Select);
        self.simulation.view = Default::default();
        self.simulation.plugin_arguments.clear();
    }
    pub fn edit_project(&mut self, command: CircuitCommand, name: String) -> Result<(), String> {
        self.cancel_gesture();
        self.commit_properties();
        self.sync_project();
        let before = self.snapshot();
        match command {
            CircuitCommand::Add => {
                let id = self.project.add(name)?;
                self.switch_circuit(id);
            }
            CircuitCommand::Rename => self.project.rename(self.active, name)?,
            CircuitCommand::Remove => {
                self.project.remove(self.active)?;
                let id = self.project.main;
                self.active = id;
                self.root = id;
                self.path.clear();
                self.selection.clear();
            }
            CircuitCommand::Main => self.project.main = self.active,
            CircuitCommand::Up | CircuitCommand::Down => {
                let index = self
                    .project
                    .circuits
                    .iter()
                    .position(|c| c.id == self.active)
                    .unwrap();
                let other = if matches!(command, CircuitCommand::Up) {
                    index.checked_sub(1)
                } else {
                    Some(index + 1).filter(|i| *i < self.project.circuits.len())
                };
                if let Some(other) = other {
                    self.project.circuits.swap(index, other);
                }
            }
        }
        self.circuit = self.project.circuit(self.active).unwrap().circuit.clone();
        self.record(before);
        Ok(())
    }
}
impl Workspace {
    pub fn project(&self) -> &Project {
        &self.editor.project
    }
    pub fn active_circuit(&self) -> Id {
        self.editor.active
    }
    pub fn circuit_name(&self) -> &str {
        &self
            .editor
            .project
            .circuit(self.editor.active)
            .unwrap()
            .name
    }
    pub fn simulation_root(&self) -> Id {
        self.editor.root
    }
    pub fn simulation_path(&self) -> &[Id] {
        &self.editor.path
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
        let id = project.main;
        self.editor = Editor::default();
        self.pla_editor = crate::pla::PlaEditor::default();
        self.editor.active = id;
        self.editor.root = id;
        self.editor.circuit = project.main_circuit().circuit.clone();
        self.editor.project = project;
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

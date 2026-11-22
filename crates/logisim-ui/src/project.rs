//! Project editing shares the same undo timeline as geometry edits.
mod options;

use crate::{
    Workspace,
    editor::{Editor, Tool},
};
use crate::{
    dialog::{Dialog, Dismiss},
    i18n::tr,
    tf,
};
use logisim_core::{Id, Kind, Project};

#[derive(Clone, Copy)]
pub(crate) enum CircuitCommand {
    Add,
    Rename,
    Remove,
    Up,
    Down,
}
impl CircuitCommand {
    pub(crate) fn enabled(self, workspace: &Workspace) -> bool {
        let project = workspace.project();
        let active = workspace.active_circuit();
        match self {
            Self::Up => project.circuits.first().is_some_and(|c| c.id != active),
            Self::Down => project.circuits.last().is_some_and(|c| c.id != active),
            Self::Remove => project.can_remove(active),
            Self::Add | Self::Rename => true,
        }
    }
}
pub(crate) struct NameDialog {
    pub rename: bool,
    focus: bool,
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
    pub(crate) fn set_main_circuit(&mut self, id: Id) {
        self.finish_edit();
        self.editor.set_main_circuit(id);
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
        self.fallback_plugin_sources.clear();
        self.analysis.cancel();
        self.generator.cancel_computation();
        let serial = self.generator.serial;
        self.generator = Default::default();
        self.generator.serial = serial;
        self.generated_expressions = Default::default();
        self.libraries.clear();
        project.refresh_interfaces(false);
        let pending_rate = self
            .editor
            .simulation
            .actions
            .iter()
            .any(|a| matches!(a, crate::SimulationAction::SetTickRate(_)));
        self.editor.replace_document(project, self.preferences);
        if pending_rate {
            self.editor
                .simulation
                .actions
                .push(crate::SimulationAction::SetTickRate(
                    self.preferences.tick_hz.get(),
                ));
        }
        self.pla_editor = crate::pla::PlaEditor::default();
        self.appearance.reset_context();
        self.name_dialog = None;
        self.project_options_open = false;
        self.remove_dialog = false;
        self.reset_canvas_for_navigation();
    }
    pub(crate) fn circuit_command(&mut self, command: CircuitCommand) {
        self.finish_edit();
        match command {
            CircuitCommand::Add | CircuitCommand::Rename => {
                let rename = matches!(command, CircuitCommand::Rename);
                self.name_dialog = Some(NameDialog {
                    rename,
                    focus: true,
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
        if self.project_options_open {
            self.project_options(ctx);
        }
        if let Some(mut dialog) = self.name_dialog.take() {
            let mut apply = false;
            let title = tr(if dialog.rename {
                "Rename circuit"
            } else {
                "Add circuit"
            });
            let dismiss = Dialog::prompt("circuit-name").show(ctx, title, |window| {
                window.body(|ui| {
                    ui.label(tr("Circuit name"));
                    let response = ui.text_edit_singleline(&mut dialog.name);
                    if std::mem::take(&mut dialog.focus) {
                        response.request_focus();
                    }
                    if !dialog.error.is_empty() {
                        ui.colored_label(crate::theme::palette(ui.ctx()).danger, tr(&dialog.error));
                    }
                    apply = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                });
                window.footer("Cancel", |ui| {
                    apply |= ui
                        .button(tr(if dialog.rename { "Rename" } else { "Add" }))
                        .clicked();
                });
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
            } else if dismiss == Dismiss::Open {
                self.name_dialog = Some(dialog);
            }
        }
        if self.remove_dialog {
            let dismiss = Dialog::prompt("remove-circuit").confirm(
                ctx,
                &tf!("Remove '{}' ?", self.circuit_name()),
                tr("This removes the circuit and its contents. You can undo this change."),
                tr("Remove"),
                || {
                    if let Err(e) = self
                        .editor
                        .edit_project(CircuitCommand::Remove, String::new())
                    {
                        self.editor.project_error = Some(e);
                    }
                    self.canvas.reset();
                    self.fit_pending = true;
                },
            );
            self.remove_dialog = dismiss == Dismiss::Open;
        }
        if let Some(message) = &self.editor.project_error
            && Dialog::prompt("project-error").message(
                ctx,
                tr("Circuit operation unavailable"),
                tr(message),
            ) != Dismiss::Open
        {
            self.editor.project_error = None;
        }
    }
}

#[cfg(test)]
mod tests;

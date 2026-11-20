//! Workspace menus; document and simulation commands stay in their existing layers.
use crate::shortcuts::Command;
use crate::{FileAction, Workspace, document::recent_button, i18n::tr, project::CircuitCommand};
use egui::Ui;

impl Workspace {
    fn command_button(&mut self, ui: &mut Ui, c: Command) {
        let label = self.preferences.shortcuts.caption(c, ui);
        if ui
            .add_enabled(self.command_enabled(c), egui::Button::new(label))
            .clicked()
        {
            self.run_command(c);
            ui.close();
        }
    }
    pub(super) fn menus(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr("File"), |ui| {
                for c in [Command::New, Command::Open, Command::Save, Command::SaveAs] {
                    self.command_button(ui, c);
                }
                ui.menu_button(tr("Recent Projects"), |ui| {
                    let recent = &self.recent;
                    if recent.is_empty() {
                        ui.add_enabled(false, egui::Button::new(tr("No recent projects")));
                    }
                    for (index, path) in recent.iter().enumerate() {
                        if recent_button(ui, path) {
                            self.file_actions.push(FileAction::OpenRecent(index));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(!recent.is_empty(), egui::Button::new(tr("Clear recent")))
                        .clicked()
                    {
                        self.file_actions.push(FileAction::ClearRecent);
                        ui.close();
                    }
                });
                ui.separator();
                disabled_actions(ui, &["Export Image…", "Print…"]);
                ui.separator();
                if ui.button(tr("Settings")).clicked() {
                    self.finish_edit();
                    self.settings_open = true;
                    ui.close();
                }
                self.command_button(ui, Command::Exit);
            });
            ui.menu_button(tr("Edit"), |ui| {
                if self.editor.session.context.appearance_mode {
                    self.appearance.edit_menu(ui, &mut self.editor);
                    return;
                }
                self.command_button(ui, Command::Undo);
                self.command_button(ui, Command::Redo);
                ui.separator();
                for c in [
                    Command::Cut,
                    Command::Copy,
                    Command::Paste,
                    Command::Duplicate,
                ] {
                    self.command_button(ui, c);
                }
                let selected = !self.editor.session.selection.is_empty();
                if ui
                    .add_enabled(selected, egui::Button::new(tr("Delete")))
                    .clicked()
                {
                    self.editor.delete();
                    ui.close();
                }
                self.command_button(ui, Command::SelectAll);
            });
            ui.menu_button(tr("Project"), |ui| {
                if ui.button(tr("Analyze Circuit…")).clicked() {
                    self.request_analysis();
                    ui.close();
                }
                ui.separator();
                if ui.button(tr("Load Wasm Plugin…")).clicked() {
                    self.file_actions.push(FileAction::LoadPlugin);
                    ui.close();
                }
                if ui.button(tr("Load .circ Library…")).clicked() {
                    self.file_actions.push(FileAction::ImportLibrary);
                    ui.close();
                }
                ui.separator();
                self.command_button(ui, Command::AddCircuit);
                for (label, command, enabled) in [
                    ("Rename Circuit…", CircuitCommand::Rename, true),
                    (
                        "Set as Main Circuit",
                        CircuitCommand::Main,
                        self.editor.session.context.active != self.editor.session.project.main,
                    ),
                    ("Move Circuit Up", CircuitCommand::Up, true),
                    ("Move Circuit Down", CircuitCommand::Down, true),
                    (
                        "Remove Circuit…",
                        CircuitCommand::Remove,
                        self.editor
                            .session
                            .project
                            .can_remove(self.editor.session.context.active),
                    ),
                ] {
                    if ui
                        .add_enabled(enabled, egui::Button::new(tr(label)))
                        .clicked()
                    {
                        self.circuit_command(command);
                        ui.close();
                    }
                }
                ui.separator();
                self.command_button(ui, Command::ReturnParent);
                ui.separator();
                if ui
                    .selectable_label(
                        !self.editor.session.context.appearance_mode,
                        tr("Edit Circuit Layout"),
                    )
                    .clicked()
                {
                    self.finish_edit();
                    self.editor.session.context.appearance_mode = false;
                    ui.close();
                }
                if ui
                    .selectable_label(
                        self.editor.session.context.appearance_mode,
                        self.preferences
                            .shortcuts
                            .caption(Command::ToggleAppearance, ui),
                    )
                    .clicked()
                {
                    self.finish_edit();
                    self.editor.session.context.appearance_mode = true;
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor.session.circuit.appearance.is_some(),
                        egui::Button::new(tr("Revert to Default Appearance")),
                    )
                    .clicked()
                {
                    self.appearance.revert(&mut self.editor);
                    ui.close();
                }
                disabled_actions(ui, &["Project Options…"]);
            });
            ui.menu_button(tr("Simulate"), |ui| {
                self.editor.simulation.menu(
                    ui,
                    &mut self.preferences.tick_hz,
                    &self.preferences.shortcuts,
                );
            });
            ui.menu_button(tr("Window"), |ui| {
                if ui.button(tr("Minimize")).clicked() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    ui.close();
                }
                if ui.button(tr("Maximize / Restore")).clicked() {
                    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    ui.close();
                }
                if ui.button(tr("Close")).clicked() {
                    self.file_actions.push(FileAction::Close);
                    ui.close();
                }
                ui.separator();
                ui.add_enabled(
                    false,
                    egui::Button::new(self.document.label()).selected(true),
                );
            });
            ui.menu_button(tr("Help"), |ui| {
                if ui.button(tr("About logisim-rs")).clicked() {
                    self.about_open = true;
                    ui.close();
                }
            });
        });
    }
}

fn disabled_actions(ui: &mut Ui, labels: &[&str]) {
    for label in labels {
        ui.add_enabled(false, egui::Button::new(tr(label)));
    }
}

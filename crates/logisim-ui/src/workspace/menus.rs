//! Workspace menus; document and simulation commands stay in their existing layers.
use crate::{FileAction, Workspace, i18n::tr, project::CircuitCommand};
use egui::Ui;

impl Workspace {
    pub(super) fn menus(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr("File"), |ui| {
                for (label, action) in [
                    ("New · Ctrl+N", FileAction::New),
                    ("Open… · Ctrl+O", FileAction::Open),
                    ("Save · Ctrl+S", FileAction::Save),
                    ("Save As… · Ctrl+Shift+S", FileAction::SaveAs),
                ] {
                    if ui.button(tr(label)).clicked() {
                        self.file_actions.push(action);
                        ui.close();
                    }
                }
                ui.separator();
                disabled_actions(ui, &["Export Image…", "Print…"]);
                ui.separator();
                if ui.button(tr("Settings")).clicked() {
                    self.finish_edit();
                    self.settings_open = true;
                    ui.close();
                }
                if ui.button(tr("Exit")).clicked() {
                    self.file_actions.push(FileAction::Close);
                    ui.close();
                }
            });
            ui.menu_button(tr("Edit"), |ui| {
                if self.editor.appearance_mode {
                    self.appearance.edit_menu(ui, &mut self.editor);
                    return;
                }
                if ui
                    .add_enabled(
                        self.editor.can_undo(),
                        egui::Button::new(tr("Undo · Ctrl+Z")),
                    )
                    .clicked()
                {
                    self.editor.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor.can_redo(),
                        egui::Button::new(tr("Redo · Ctrl+Shift+Z")),
                    )
                    .clicked()
                {
                    self.editor.redo();
                    ui.close();
                }
                ui.separator();
                let selected = !self.editor.selection.is_empty();
                if ui
                    .add_enabled(selected, egui::Button::new(tr("Cut · Ctrl+X")))
                    .clicked()
                {
                    self.editor.cut();
                    ui.close();
                }
                if ui
                    .add_enabled(selected, egui::Button::new(tr("Copy · Ctrl+C")))
                    .clicked()
                {
                    self.editor.copy();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor.can_paste(),
                        egui::Button::new(tr("Paste · Ctrl+V")),
                    )
                    .clicked()
                {
                    self.editor.paste();
                    ui.close();
                }
                if ui
                    .add_enabled(selected, egui::Button::new(tr("Duplicate · Ctrl+D")))
                    .clicked()
                {
                    self.editor.duplicate();
                    ui.close();
                }
                if ui
                    .add_enabled(selected, egui::Button::new(tr("Delete")))
                    .clicked()
                {
                    self.editor.delete();
                    ui.close();
                }
                if ui.button(tr("Select All · Ctrl+A")).clicked() {
                    self.editor.select_all();
                    ui.close();
                }
            });
            ui.menu_button(tr("Project"), |ui| {
                for (label, command, enabled) in [
                    ("Add Circuit… · Ctrl+Shift+N", CircuitCommand::Add, true),
                    ("Rename Circuit…", CircuitCommand::Rename, true),
                    (
                        "Set as Main Circuit",
                        CircuitCommand::Main,
                        self.editor.active != self.editor.project.main,
                    ),
                    ("Move Circuit Up", CircuitCommand::Up, true),
                    ("Move Circuit Down", CircuitCommand::Down, true),
                    (
                        "Remove Circuit…",
                        CircuitCommand::Remove,
                        self.editor.project.can_remove(self.editor.active),
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
                if ui
                    .add_enabled(
                        !self.editor.path.is_empty(),
                        egui::Button::new(tr("Return to Parent · Alt+Left")),
                    )
                    .clicked()
                {
                    self.editor.leave_instance();
                    ui.close();
                }
                ui.separator();
                if ui
                    .selectable_label(!self.editor.appearance_mode, tr("Edit Circuit Layout"))
                    .clicked()
                {
                    self.finish_edit();
                    self.editor.appearance_mode = false;
                    ui.close();
                }
                if ui
                    .selectable_label(
                        self.editor.appearance_mode,
                        tr("Edit Circuit Appearance · Ctrl+Shift+E"),
                    )
                    .clicked()
                {
                    self.finish_edit();
                    self.editor.appearance_mode = true;
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.editor.circuit.appearance.is_some(),
                        egui::Button::new(tr("Revert to Default Appearance")),
                    )
                    .clicked()
                {
                    self.appearance.revert(&mut self.editor);
                    ui.close();
                }
                disabled_actions(ui, &["Load Library…", "Project Options…"]);
            });
            ui.menu_button(tr("Simulate"), |ui| {
                self.editor.simulation.menu(ui);
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

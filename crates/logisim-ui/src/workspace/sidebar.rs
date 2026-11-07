//! Workspace sidebar; document and simulation commands stay in their existing layers.
use crate::{
    CHROME, Workspace,
    editor::Tool,
    i18n::tr,
    icons::{self, Icon},
    project::CircuitCommand,
    properties,
};
use egui::{Color32, Frame, Margin, Panel, RichText, ScrollArea, Ui};
use logisim_core::Kind;

impl Workspace {
    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        Panel::bottom("zoom")
            .exact_size(37.0)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 3)),
            )
            .show(ui, |ui| self.zoom_controls(ui));

        let height = ui.available_height();
        Panel::top("project")
            .resizable(true)
            .default_size(height * 0.5)
            .size_range(125.0..=(height - 125.0).max(125.0))
            .frame(Frame::NONE.fill(Color32::WHITE))
            .show(ui, |ui| {
                Panel::top("project_actions")
                    .exact_size(36.0)
                    .frame(
                        Frame::NONE
                            .fill(CHROME)
                            .inner_margin(Margin::symmetric(8, 3)),
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (icon, name, command) in [
                                (Icon::Plus, "Add circuit", CircuitCommand::Add),
                                (Icon::Up, "Move up", CircuitCommand::Up),
                                (Icon::Down, "Move down", CircuitCommand::Down),
                                (Icon::Trash, "Remove circuit", CircuitCommand::Remove),
                            ] {
                                let enabled = !matches!(command,CircuitCommand::Remove) || self.editor.project.can_remove(self.editor.active);
                                if icons::button(ui, icon, false, enabled, name).clicked() { self.circuit_command(command); }
                            }
                            if !self.editor.path.is_empty() && ui.small_button(tr("Back")).on_hover_text(tr("Return to parent instance · Alt+Left")).clicked() { self.editor.leave_instance(); }

                        });
                    });
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        egui::CollapsingHeader::new(RichText::new(self.document.label()).strong())
                            .id_salt("document-tree")
                            .default_open(true)
                            .show(ui, |ui| {
                                let definitions: Vec<_> = self.editor.project.circuits.iter().map(|c|(c.id,c.name.clone())).collect();
                                for (id,name) in definitions {
                                    let allowed = self.editor.project.can_instantiate(self.editor.active,id);
                                    let label = format!("{}{}",name,if id == self.editor.project.main { tr("  (main)") } else { "" });
                                    let response = ui.selectable_label(self.editor.active == id, label)
                                        .on_hover_text(tr("Double-click to edit; single-click another circuit to place an instance."));
                                    if response.double_clicked() { self.editor.switch_circuit(id); }
                                    else if response.clicked() && allowed { self.editor.appearance_mode=false; self.editor.set_tool(Tool::Place(Kind::Subcircuit(id))); }
                                    response.context_menu(|ui| {
                                        if ui.button(tr("Open circuit")).clicked() { self.editor.switch_circuit(id); ui.close(); }
                                        if ui.add_enabled(allowed,egui::Button::new(tr("Place instance"))).clicked() { self.editor.set_tool(Tool::Place(Kind::Subcircuit(id))); ui.close(); }
                                        if ui.button(tr("Edit appearance")).clicked(){self.finish_edit();self.editor.switch_circuit(id);self.editor.appearance_mode=true;ui.close();}
                                        if ui.button(tr("Rename…")).clicked() { self.editor.switch_circuit(id); self.circuit_command(CircuitCommand::Rename); ui.close(); }
                                    });
                                }
                                if !self.editor.path.is_empty() {
                                    ui.weak(self.editor.context_label()).on_hover_text(tr("Viewing this instance. Edits change the shared circuit definition."));
                                }
                                self.library_groups(ui);
                                self.plugin_groups(ui);
                                for group in super::library::groups() {
                                    egui::CollapsingHeader::new(tr(group.key))
                                        .id_salt(group.key)
                                        .show(ui, |ui| {
                                            for &kind in &group.tools {
                                                self.library_tool(ui, kind);
                                            }
                                        });
                                }
                            });
                    });
            });

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(CHROME))
            .show(ui, |ui| {
                let name = self.circuit_name().to_owned();
                if self.editor.appearance_mode {
                    self.appearance.properties(ui, &mut self.editor);
                } else {
                    properties::show(ui, &mut self.editor, &name);
                }
            });
    }

    fn library_tool(&mut self, ui: &mut Ui, kind: Kind) {
        let tool = Tool::Place(kind);
        let response = ui.selectable_label(self.editor.tool == tool, tr(kind.name()));
        let response = if let Kind::Ttl(t) = kind {
            response.on_hover_text(tr(t.description()))
        } else {
            response
        };
        if response.clicked() {
            self.editor.appearance_mode = false;
            self.editor.set_tool(tool);
        }
    }
}

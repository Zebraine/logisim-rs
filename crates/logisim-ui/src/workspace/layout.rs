//! Workspace layout; document and simulation commands stay in their existing layers.
use crate::{
    CHROME, FileAction, Workspace, appearance,
    canvas::{MAX_ZOOM, MIN_ZOOM},
    drawing,
    editor::Tool,
    i18n::{self, tr},
    icons::{self, Icon},
    project::CircuitCommand,
};
use egui::{Frame, Margin, Panel, Ui, vec2};

impl Workspace {
    pub fn show(&mut self, ui: &mut Ui) {
        i18n::with_language(self.language, || self.show_localized(ui));
    }
    fn show_localized(&mut self, ui: &mut Ui) {
        let before = (
            self.editor.active,
            self.editor.path.clone(),
            self.editor.appearance_mode,
        );
        let modal = self.editor.memory_target.is_some()
            || self.analysis.open()
            || self.generator.open
            || self.editor.simulation.tick_rate_draft.is_some()
            || self.editor.pla_target.is_some()
            || self.settings_open
            || self.name_dialog.is_some()
            || self.remove_dialog
            || self.editor.project_error.is_some();
        if modal || !ui.is_enabled() || ui.input(|i| !i.focused) {
            self.editor.cancel_slider();
            self.editor.simulation.keyboard_focus = None;
        }
        if modal
            || self.editor.appearance_mode
            || !ui.is_enabled()
            || ui.input(|i| !i.focused || !i.pointer.primary_down())
        {
            self.editor.simulation.release_momentary();
        }
        ui.add_enabled_ui(!modal, |ui| self.show_inner(ui));
        if ui.is_enabled() {
            self.project_dialogs(ui.ctx());
            self.analysis_dialog(ui.ctx());
            self.settings(ui.ctx());
            self.editor.simulation.frequency_dialog(ui.ctx());
            self.memory_editor
                .show(ui.ctx(), &mut self.editor, &mut self.file_actions);
            self.pla_editor.show(ui.ctx(), &mut self.editor);
            self.generator.show(ui.ctx(), self.truth_table_generation);
        }
        if let Some(id) = self.editor.open_instance.take() {
            self.editor.enter_instance(id);
        }
        if let Some(id) = self.editor.analyze_circuit.take() {
            self.request_circuit_analysis(id);
            ui.ctx().request_repaint();
        }
        self.editor.sync_project();
        if before
            != (
                self.editor.active,
                self.editor.path.clone(),
                self.editor.appearance_mode,
            )
        {
            self.editor.simulation.release_momentary();
            self.editor.simulation.keyboard_focus = None;
            self.appearance.reset_context();
            self.canvas.reset();
            self.fit_pending = true;
            ui.ctx().request_repaint();
        }
    }
    fn show_inner(&mut self, ui: &mut Ui) {
        if ui.is_enabled()
            && !ui.ctx().any_popup_open()
            && ui.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                    egui::Key::E,
                )
            })
        {
            self.finish_edit();
            self.editor.appearance_mode = !self.editor.appearance_mode;
        }
        if ui.is_enabled() && !ui.ctx().any_popup_open() {
            if ui.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                    egui::Key::N,
                )
            }) {
                self.circuit_command(CircuitCommand::Add);
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft)) {
                self.editor.leave_instance();
            }
        }
        if ui.is_enabled() && !ui.ctx().any_popup_open() {
            for (modifiers, key, action) in [
                (
                    egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                    egui::Key::S,
                    FileAction::SaveAs,
                ),
                (egui::Modifiers::COMMAND, egui::Key::S, FileAction::Save),
                (egui::Modifiers::COMMAND, egui::Key::O, FileAction::Open),
                (egui::Modifiers::COMMAND, egui::Key::N, FileAction::New),
                (egui::Modifiers::COMMAND, egui::Key::Q, FileAction::Close),
            ] {
                if ui.input_mut(|i| i.consume_key(modifiers, key)) {
                    self.file_actions.push(action);
                }
            }
        }
        Panel::top("menus")
            .exact_size(29.0)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 2)),
            )
            .show(ui, |ui| self.menus(ui));

        Panel::top("tools")
            .min_size(42.0)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 5)),
            )
            .show(ui, |ui| self.toolbar(ui));

        let available_width = ui.available_width();
        Panel::left("inspector")
            .default_size((available_width * 0.25).clamp(230.0, 360.0))
            .size_range(230.0..=(available_width * 0.5).max(230.0))
            .frame(Frame::NONE.fill(CHROME))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| {
                if std::mem::take(&mut self.appearance.fit_requested) {
                    self.fit_pending = true;
                }
                if self.fit_pending {
                    let mut bounds = egui::Rect::NOTHING;
                    for c in &self.editor.circuit.components {
                        bounds = bounds.union(drawing::bounds(c));
                    }
                    for w in &self.editor.circuit.wires {
                        bounds.extend_with(drawing::pos(w.a));
                        bounds.extend_with(drawing::pos(w.b));
                    }
                    if self.editor.appearance_mode {
                        let (a, b) = appearance::AppearanceEditor::current(&self.editor).bounds();
                        bounds =
                            egui::Rect::from_two_pos(drawing::pos(a), drawing::pos(b)).expand(20.0);
                    }
                    if bounds.is_finite() {
                        let available =
                            (ui.available_size() - vec2(80.0, 80.0)).max(vec2(1.0, 1.0));
                        let zoom = (available.x / bounds.width().max(1.0))
                            .min(available.y / bounds.height().max(1.0))
                            .clamp(
                                MIN_ZOOM,
                                if self.editor.appearance_mode {
                                    3.0
                                } else {
                                    1.0
                                },
                            );
                        self.canvas.camera.zoom = zoom;
                        self.canvas.camera.offset = vec2(40.0, 40.0) - bounds.min.to_vec2() * zoom;
                    }
                    self.fit_pending = false;
                }
                self.canvas.typing = self.editor.simulation.keyboard_focus.is_some()
                    && self.editor.tool == Tool::Poke;
                let response = self.canvas.show(ui);
                if self.editor.appearance_mode {
                    self.appearance
                        .show(ui, &mut self.editor, &mut self.canvas, &response);
                } else {
                    self.editor.show(ui, &mut self.canvas, &response);
                }
            });

        egui::Window::new(tr("About logisim-rs"))
            .id(egui::Id::unique("about-window"))
            .open(&mut self.about_open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.heading("Logisim rs");
                ui.label(tr("A cleanroom digital circuit workspace."));
                ui.separator();
                ui.label(tr("Component editing, wiring and canvas navigation are available."));
                ui.label(tr("Combinational and sequential simulation is available. Open and save the supported .circ components."));
            });
    }

    pub(super) fn zoom_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.small_button("−").on_hover_text(tr("Zoom out")).clicked() {
                self.canvas.set_zoom(self.canvas.camera.zoom / 1.1);
            }
            let mut percent = self.canvas.camera.zoom * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut percent)
                        .range(MIN_ZOOM * 100.0..=MAX_ZOOM * 100.0)
                        .suffix("%")
                        .speed(1.0),
                )
                .on_hover_text(tr("Drag or type a zoom percentage"))
                .changed()
            {
                self.canvas.set_zoom(percent / 100.0);
            }
            if ui.small_button("+").on_hover_text(tr("Zoom in")).clicked() {
                self.canvas.set_zoom(self.canvas.camera.zoom * 1.1);
            }
            ui.separator();
            if icons::button(ui, Icon::Home, false, true, "Reset canvas · Ctrl+0").clicked() {
                self.canvas.reset();
            }
            ui.checkbox(&mut self.canvas.show_grid, tr("Grid"));
        });
    }
}

//! Workspace layout; document and simulation commands stay in their existing layers.
use crate::{
    FileAction, Workspace, appearance,
    canvas::{MAX_ZOOM, MIN_ZOOM},
    drawing,
    editor::Tool,
    i18n::{self, tr},
    icons::{self, Icon},
    project::CircuitCommand,
    theme::{self, CHROME},
};
use egui::{Frame, Margin, Panel, Ui, vec2};

impl Workspace {
    pub fn show(&mut self, ui: &mut Ui) {
        self.preferences.apply_appearance(ui.ctx());
        i18n::with_language(self.preferences.language, || self.show_localized(ui));
    }
    fn show_localized(&mut self, ui: &mut Ui) {
        let before = self.editor.session.context.clone();
        let modal = self.dialog_open();
        if modal || !ui.is_enabled() || ui.input(|i| !i.focused) {
            self.editor.cancel_slider();
            self.editor.simulation.release_keyboard();
        }
        if modal
            || self.editor.session.context.appearance_mode
            || !ui.is_enabled()
            || ui.input(|i| !i.focused || !i.pointer.primary_down())
        {
            self.editor.simulation.release_momentary();
        }
        ui.add_enabled_ui(!modal, |ui| self.show_inner(ui));
        if ui.is_enabled() {
            self.show_dialogs(ui.ctx());
        }
        if let Some(id) = self.editor.open_instance.take() {
            self.editor.enter_instance(id);
        }
        if let Some(id) = self.editor.analyze_circuit.take() {
            self.request_circuit_analysis(id);
            ui.ctx().request_repaint();
        }
        self.editor.sync_project();
        if before != self.editor.session.context {
            self.editor.simulation.release_momentary();
            self.editor.simulation.release_keyboard();
            self.appearance.reset_context();
            self.reset_canvas_for_navigation();
            ui.ctx().request_repaint();
        }
    }
    pub(crate) fn reset_canvas_for_navigation(&mut self) {
        self.canvas.reset();
        self.fit_pending = self.editor.session.context.appearance_mode
            || self.preferences.default_zoom == crate::DefaultZoom::Fit;
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
            self.editor.session.context.appearance_mode =
                !self.editor.session.context.appearance_mode;
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
        let m = theme::metrics(ui.ctx());
        Panel::top("menus")
            .exact_size(m.menu)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 1)),
            )
            .show(ui, |ui| self.menus(ui));

        let width = ui.available_width() - 16.;
        let toolbar = super::toolbar::ToolbarLayout::for_width(width);
        let height = toolbar.height(m, self.preferences.show_telemetry);
        Panel::top("tools")
            .exact_size(height)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, toolbar.padding(m))),
            )
            .show(ui, |ui| self.toolbar(ui, toolbar));

        let sidebar = m.sidebar;
        let canonical = crate::Density::Comfortable.metrics().sidebar;
        // Reserve the drawing region in window points even when interface text is enlarged.
        // Only project the on-screen divider; keep its stored, density-independent width.
        let scale = ui.ctx().zoom_factor().max(1.);
        let side_max =
            (ui.available_width() - 452. / scale).clamp(sidebar.min / scale, sidebar.max);
        let side_min = sidebar.min.min(side_max);
        let size = canonical
            .project(self.layout.sidebar_width, sidebar)
            .min(side_max);
        let panel = prepare_panel(ui, "inspector", size, &mut self.navigation.last_width);
        Panel::left("inspector")
            .default_size(size)
            .size_range(side_min..=side_max)
            .frame(Frame::NONE.fill(CHROME))
            .show(ui, |ui| {
                let mut bounded = panel_contents(ui);
                self.sidebar(&mut bounded);
            });
        if let Some(new_size) = panel.committed(ui, 0) {
            self.layout.sidebar_width =
                sidebar.project(new_size.clamp(side_min, side_max), canonical);
        }

        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| {
                if std::mem::take(&mut self.appearance.fit_requested) {
                    self.fit_pending = true;
                }
                if self.fit_pending
                    && ui.ctx().zoom_factor()
                        == f32::from(self.preferences.ui_scale.percent()) / 100.0
                {
                    let mut bounds = egui::Rect::NOTHING;
                    for c in &self.editor.session.circuit.components {
                        bounds = bounds.union(drawing::bounds(c));
                    }
                    for w in &self.editor.session.circuit.wires {
                        bounds.extend_with(drawing::pos(w.a));
                        bounds.extend_with(drawing::pos(w.b));
                    }
                    if self.editor.session.context.appearance_mode {
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
                                if self.editor.session.context.appearance_mode {
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
                self.canvas.typing =
                    self.editor.simulation.typing() && self.editor.tool == Tool::Poke;
                let response = self.canvas.show(ui, &self.preferences);
                if self.editor.session.context.appearance_mode {
                    self.appearance
                        .show(ui, &mut self.editor, &mut self.canvas, &response);
                } else {
                    self.editor.show(
                        ui,
                        &mut self.canvas,
                        &response,
                        &self.plugins,
                        &mut self.preferences.show_grid,
                    );
                }
            });
    }

    pub(super) fn zoom_controls(&mut self, ui: &mut Ui) {
        let tight = ui.available_width() < theme::metrics(ui.ctx()).sidebar.min;
        ui.horizontal(|ui| {
            if tight {
                ui.spacing_mut().item_spacing.x = 1.;
                ui.spacing_mut().button_padding.x = 2.;
            }
            if icons::button(ui, Icon::Minus, false, true, "Zoom out").clicked() {
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
            if icons::button(ui, Icon::Plus, false, true, "Zoom in").clicked() {
                self.canvas.set_zoom(self.canvas.camera.zoom * 1.1);
            }
            if !tight {
                ui.separator();
            }
            if icons::button(ui, Icon::Home, false, true, "Reset canvas · Ctrl+0").clicked() {
                self.canvas.reset();
            }
            ui.checkbox(&mut self.preferences.show_grid, tr("Grid"));
        });
    }
}

/// Keep the native panel at its assigned size even when a child requests more room.
pub(super) fn panel_contents(ui: &mut Ui) -> Ui {
    let rect = ui.available_rect_before_wrap();
    ui.expand_to_include_rect(rect);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.shrink_clip_rect(rect);
    child
}

/// Re-seed egui's native resize handle when the projected preference changes,
/// e.g. after a host resize or a layout reset. Keep its normal drag and keyboard behavior.
pub(super) fn prepare_panel(ui: &Ui, salt: &str, size: f32, last: &mut f32) -> PanelSize {
    use egui::containers::panel::PanelState;
    let id = ui.scope_id().with(salt);
    if *last != size {
        ui.data_mut(|data| data.remove::<PanelState>(id));
        *last = size;
    }
    PanelSize {
        id,
        before: PanelState::load(ui.ctx(), id).map(|p| p.size()),
    }
}

pub(super) struct PanelSize {
    id: egui::Id,
    before: Option<egui::Vec2>,
}
impl PanelSize {
    pub(super) fn committed(self, ui: &Ui, axis: usize) -> Option<f32> {
        let before = self.before?[axis];
        let after = egui::containers::panel::PanelState::load(ui.ctx(), self.id)?.size()[axis];
        // PanelState retains the pre-drag size until the native handle commits.
        ((before - after).abs() > 0.01).then_some(after)
    }
}

//! Editor layout based on runtime observation, with an independent canvas viewport.
use crate::i18n::tr;

mod appearance;
mod appearance_properties;
mod artwork;
mod bus;
mod canvas;
mod document;
mod latches;
mod parity;
mod transistors;
mod ttl;
pub use document::{DocumentView, FileAction};
mod drawing;
mod editor;
#[cfg(test)]
mod editor_tests;
pub mod i18n;
#[cfg(test)]
mod i18n_tests;
mod icons;
mod io;
mod terminal;
pub use terminal::TtyView;
mod settings;
pub use i18n::Language;
mod project;
mod properties;
use project::{CircuitCommand, NameDialog};
mod arithmetic;
mod memory;
mod pla;
mod plexers;
mod sequential;
mod simulation;
pub use simulation::{
    InputValue, SignalTone, SignalView, SimulationAction, SimulationIssue, SimulationView,
};

use canvas::{Canvas, MAX_ZOOM, MIN_ZOOM};
use editor::{Editor, Tool};
use egui::{Color32, Frame, Margin, Panel, RichText, ScrollArea, Stroke, Ui, vec2};
use icons::Icon;
use logisim_core::{Gate, Kind};

const INK: Color32 = Color32::from_rgb(51, 65, 85);
const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
const BORDER: Color32 = Color32::from_rgb(220, 225, 233);
const CHROME: Color32 = Color32::from_rgb(246, 248, 251);

/// Apply the application's light visual style once, when creating the window.
pub fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    let mut style = (*ctx.global_style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = CHROME;
    style.visuals.selection.bg_fill = Color32::from_rgb(225, 235, 255);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.spacing.item_spacing = vec2(6.0, 5.0);
    style.spacing.button_padding = vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style.animation_time = 0.12;
    ctx.set_global_style(style);
}

/// Circuit editing and viewport state. Signal simulation is not part of this layer.
#[derive(Default)]
pub struct Workspace {
    memory_editor: memory::MemoryEditor,
    pla_editor: pla::PlaEditor,
    language: Language,
    settings_open: bool,
    settings_error: Option<String>,
    retry_settings: bool,
    appearance: appearance::AppearanceEditor,
    canvas: Canvas,
    editor: Editor,
    about_open: bool,
    document: DocumentView,
    file_actions: Vec<FileAction>,
    fit_pending: bool,
    name_dialog: Option<NameDialog>,
    remove_dialog: bool,
}

impl Workspace {
    pub fn set_document(&mut self, document: DocumentView) {
        self.document = document;
    }
    pub fn take_file_actions(&mut self) -> Vec<FileAction> {
        std::mem::take(&mut self.file_actions)
    }
    pub fn finish_edit(&mut self) {
        self.appearance.cancel();
        self.editor.cancel_gesture();
        self.editor.commit_properties();
        self.editor.sync_project();
    }
    pub fn replace_circuit(&mut self, circuit: logisim_core::Circuit) {
        self.replace_project(logisim_core::Project::single("main", circuit));
    }
    pub fn circuit(&self) -> &logisim_core::Circuit {
        &self.editor.circuit
    }
    pub fn set_simulation(&mut self, view: SimulationView) {
        self.editor.simulation.view = view;
    }
    pub fn take_simulation_actions(&mut self) -> Vec<SimulationAction> {
        std::mem::take(&mut self.editor.simulation.actions)
    }

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
            self.settings(ui.ctx());
            self.memory_editor.show(ui.ctx(), &mut self.editor);
            self.pla_editor.show(ui.ctx(), &mut self.editor);
        }
        if let Some(id) = self.editor.open_instance.take() {
            self.editor.enter_instance(id);
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
            .exact_size(42.0)
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

    fn menus(&mut self, ui: &mut Ui) {
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

    fn toolbar(&mut self, ui: &mut Ui) {
        if self.editor.appearance_mode {
            self.appearance.toolbar(ui, &mut self.editor);
            return;
        }
        ui.horizontal(|ui| {
            for (icon, tool, hint) in [
                (
                    Icon::Poke,
                    Tool::Poke,
                    "Poke · P · toggle inputs / inspect signals",
                ),
                (Icon::Select, Tool::Select, "Select · V"),
                (
                    Icon::Wire,
                    Tool::Wire,
                    "Wire · W · drag or click two points",
                ),
                (Icon::Text, Tool::Place(Kind::Text), "Text · T"),
                (Icon::Pin, Tool::Place(Kind::Pin), "Pin"),
                (Icon::Tunnel, Tool::Place(Kind::Tunnel), "Tunnel · U"),
                (Icon::Not, Tool::Place(Kind::Gate(Gate::Not)), "NOT Gate"),
                (Icon::And, Tool::Place(Kind::Gate(Gate::And)), "AND Gate"),
                (Icon::Or, Tool::Place(Kind::Gate(Gate::Or)), "OR Gate"),
            ] {
                if icons::button(ui, icon, self.editor.tool == tool, true, hint).clicked() {
                    self.editor.set_tool(tool);
                }
            }
            ui.separator();
            if ui
                .add_enabled(self.editor.can_rotate(), egui::Button::new(tr("Rotate")))
                .on_hover_text(tr("Rotate clockwise · R"))
                .clicked()
            {
                self.editor.rotate();
            }
            ui.separator();
            self.editor.simulation.toolbar(ui);
            if ui.available_width() > 180.0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(self.editor.simulation.status())
                            .size(11.0)
                            .weak(),
                    );
                });
            }
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
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
                                for group in [
                                    "Wiring",
                                    "Gates",
                                    "TTL",
                                    "Plexers",
                                    "Arithmetic",
                                    "Memory",
                                    "Input/Output",
                                    "Base",
                                ] {
                                    egui::CollapsingHeader::new(tr(group)).id_salt(group).show(ui, |ui| match group {
                                        "Gates" => {
                                            for gate in Gate::ALL {
                                                self.library_tool(ui, Kind::Gate(gate));
                                            }
                                            for parity in logisim_core::Parity::ALL {
                                                self.library_tool(ui, Kind::Parity(parity));
                                            }
                                            self.library_tool(ui, Kind::ControlledBuffer);
                                            self.library_tool(ui, Kind::ControlledInverter);
                                        }
                                        "Wiring" => {
                                            for kind in [
                                                Kind::Splitter,
                                                Kind::Pin,
                                                Kind::Probe,
                                                Kind::Tunnel,
                                                Kind::PullResistor,
                                                Kind::Clock,
                                                Kind::Constant,
                                                Kind::Power,
                                                Kind::Ground,
                                                Kind::Transistor,
                                                Kind::TransmissionGate,
                                                Kind::BitExtender,
                                            ] {
                                                self.library_tool(ui, kind);
                                            }
                                        }
                                        "TTL" => {
                                            for t in logisim_core::Ttl::ALL { self.library_tool(ui, Kind::Ttl(t)); }
                                        }
                                        "Arithmetic" => {
                                    for a in logisim_core::Arithmetic::ALL { self.library_tool(ui, Kind::Arithmetic(a)); }
                                }
                                        "Plexers" => {
                                    for p in logisim_core::Plexer::ALL { self.library_tool(ui, Kind::Plexer(p)); }
                                }
                                "Memory" => {
                                            for latch in logisim_core::Latch::ALL { self.library_tool(ui, Kind::Latch(latch)); }
                                            for ff in logisim_core::FlipFlop::ALL { self.library_tool(ui, Kind::FlipFlop(ff)); }
                                            self.library_tool(ui, Kind::Register);
                                            self.library_tool(ui, Kind::Counter);
                                            self.library_tool(ui, Kind::ShiftRegister);
                                            self.library_tool(ui, Kind::Random);
                                            self.library_tool(ui, Kind::Rom);
                                            self.library_tool(ui, Kind::PlaRom);
                                            self.library_tool(ui, Kind::Ram);
                                        }
                                        "Input/Output" => {
                                            for kind in [Kind::Button, Kind::Switch, Kind::DipSwitch, Kind::Slider, Kind::Joystick, Kind::Keyboard, Kind::Tty, Kind::Led, Kind::RgbLed, Kind::SevenSegment, Kind::HexDisplay, Kind::DotMatrix] {
                                                self.library_tool(ui, kind);
                                            }
                                        }
                                        "Base" => self.library_tool(ui, Kind::Text),
                                        _ => {
                                            ui.weak(tr("No tools available yet"));
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

    fn zoom_controls(&mut self, ui: &mut Ui) {
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

fn disabled_actions(ui: &mut Ui, labels: &[&str]) {
    for label in labels {
        ui.add_enabled(false, egui::Button::new(tr(label)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{
        Context, Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput,
        Rect, TouchPhase, Vec2, pos2,
    };

    fn frame(ctx: &Context, app: &mut Workspace, size: Vec2, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        // These tests exercise layout and input without a renderer.
        output.textures_delta.clear();
    }

    fn wheel() -> Event {
        Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, 1.0),
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn canvas_keeps_the_right_hand_layout_at_both_window_sizes() {
        for size in [vec2(1200.0, 820.0), vec2(720.0, 480.0)] {
            let ctx = Context::default();
            configure(&ctx);
            let mut app = Workspace::default();
            frame(&ctx, &mut app, size, vec![]);
            let canvas = app.canvas.rect;
            assert!(canvas.left() >= 230.0 && canvas.left() < size.x * 0.5);
            assert!(canvas.top() >= 65.0 && canvas.top() < 90.0);
            assert_eq!(canvas.right(), size.x);
            assert_eq!(
                canvas.bottom(),
                size.y,
                "No full-width bottom bar may shrink the canvas"
            );
            assert!(canvas.width() > 450.0);
        }
    }

    #[test]
    fn wheel_zooms_only_canvas_and_keeps_the_pointer_anchor() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let pointer = pos2(620.0, 310.0);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
        let world = app.canvas.camera.world_at(pointer - app.canvas.rect.min);
        frame(&ctx, &mut app, size, vec![wheel()]);
        assert!((app.canvas.camera.zoom - 1.1).abs() < 0.001);
        assert!(
            (app.canvas.camera.world_at(pointer - app.canvas.rect.min) - world).length() < 0.001
        );
        let zoom = app.canvas.camera.zoom;
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(pos2(80.0, 160.0)), wheel()],
        );
        assert_eq!(
            app.canvas.camera.zoom, zoom,
            "Scrolling the tree must not zoom the canvas"
        );
    }

    #[test]
    fn reset_shortcut_works_after_the_canvas_takes_focus() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let pos = pos2(620.0, 310.0);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                size,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
            );
        }
        app.canvas.camera.pan(vec2(120.0, -70.0));
        app.canvas.set_zoom(2.0);
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::Key {
                key: egui::Key::Num0,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        assert_eq!(app.canvas.camera.zoom, 1.0);
        assert_eq!(app.canvas.camera.offset, canvas::Camera::default().offset);
    }

    #[test]
    fn middle_button_drag_moves_the_viewport() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let start = pos2(620.0, 310.0);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(start)]);
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerButton {
                pos: start,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        let before = app.canvas.camera.offset;
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(start + vec2(80.0, 45.0))],
        );
        assert!((app.canvas.camera.offset - before - vec2(80.0, 45.0)).length() < 0.001);
    }
    #[test]
    fn file_shortcuts_emit_one_command_and_modal_disabling_blocks_them() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        for (key, modifiers, action) in [
            (egui::Key::O, Modifiers::COMMAND, FileAction::Open),
            (egui::Key::S, Modifiers::COMMAND, FileAction::Save),
            (
                egui::Key::S,
                Modifiers::COMMAND.plus(Modifiers::SHIFT),
                FileAction::SaveAs,
            ),
            (egui::Key::N, Modifiers::COMMAND, FileAction::New),
            (egui::Key::Q, Modifiers::COMMAND, FileAction::Close),
        ] {
            let event = Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            };
            frame(&ctx, &mut app, size, vec![event.clone()]);
            assert_eq!(app.take_file_actions(), vec![action]);
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    events: vec![event],
                    ..Default::default()
                },
                |ui| {
                    ui.add_enabled_ui(false, |ui| app.show(ui));
                },
            );
            output.textures_delta.clear();
            assert!(app.take_file_actions().is_empty());
        }
    }
    #[test]
    fn loading_resets_history_selection_and_fits_negative_coordinates() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.editor.place(logisim_core::Point::new(100, 100));
        assert!(app.editor.can_undo());
        let mut circuit = logisim_core::Circuit::default();
        circuit.add_component(logisim_core::Component::new(
            Kind::Text,
            logisim_core::Point::new(-1000, -800),
        ));
        app.replace_circuit(circuit.clone());
        frame(&ctx, &mut app, vec2(1200.0, 820.0), vec![]);
        assert!(!app.editor.can_undo());
        assert!(app.editor.selection.is_empty());
        assert_eq!(app.circuit(), &circuit);
        assert!(
            app.canvas
                .rect
                .contains(app.canvas.screen(circuit.components[0].position))
        );
    }
    #[test]
    fn add_circuit_shortcut_takes_precedence_over_new_file_and_modal_blocks_other_commands() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let key = |key, modifiers| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(
            &ctx,
            &mut app,
            size,
            vec![key(egui::Key::N, Modifiers::COMMAND.plus(Modifiers::SHIFT))],
        );
        assert!(app.name_dialog.is_some());
        assert!(app.take_file_actions().is_empty());
        frame(
            &ctx,
            &mut app,
            size,
            vec![key(egui::Key::S, Modifiers::COMMAND)],
        );
        assert!(app.take_file_actions().is_empty());
        frame(&ctx, &mut app, size, vec![Event::Text("Pass".into())]);
        frame(
            &ctx,
            &mut app,
            size,
            vec![key(egui::Key::Enter, Modifiers::NONE)],
        );
        assert!(app.name_dialog.is_none());
        assert_eq!(app.project().circuits.len(), 2);
        assert_eq!(app.circuit_name(), "Pass");
        app.editor.undo();
        assert_eq!(app.project().circuits.len(), 1);
        app.editor.redo();
        assert_eq!(app.circuit_name(), "Pass");
    }
}

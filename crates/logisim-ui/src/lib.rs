//! Editor layout based on runtime observation, with an independent canvas viewport.

mod analysis;
mod appearance;
mod appearance_properties;
mod artwork;
mod bus;
mod canvas;
mod document;
mod fonts;
pub mod generation;
mod latches;
mod parity;
mod transistors;
mod ttl;
mod workspace;
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
pub use settings::SimulationMode;
mod libraries;
mod plugins;
mod project;
mod properties;
use project::NameDialog;
mod arithmetic;
mod memory;
pub use memory::MemoryImportTarget;
mod pla;
mod plexers;
mod sequential;
mod simulation;
pub use simulation::{
    DEFAULT_TICK_HZ, InputValue, MAX_TICK_HZ, SignalTone, SignalView, SimulationAction,
    SimulationIssue, SimulationView,
};

use canvas::Canvas;
use editor::Editor;
use egui::{Color32, Stroke, vec2};

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
    analysis: analysis::AnalysisUi,
    generator: generation::Generator,
    generated_expressions: generation::Memory,
    plugins: Vec<logisim_core::plugin::api::Manifest>,
    libraries: Vec<libraries::LibraryGroup>,
    memory_editor: memory::MemoryEditor,
    pla_editor: pla::PlaEditor,
    language: Language,
    simulation_mode: SimulationMode,
    truth_table_generation: bool,
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
    /// Update host telemetry without rebuilding or cloning the displayed snapshot.
    pub fn simulation_mut(&mut self) -> &mut SimulationView {
        &mut self.editor.simulation.view
    }
    pub fn take_simulation_actions(&mut self) -> Vec<SimulationAction> {
        std::mem::take(&mut self.editor.simulation.actions)
    }
}

//! Editor layout based on runtime observation, with an independent canvas viewport.

mod analysis;
mod appearance;
mod appearance_properties;
mod artwork;
mod bus;
mod canvas;
pub mod dialog;
mod document;
mod fonts;
pub mod generation;
mod latches;
mod parity;
mod transistors;
mod workspace;
pub use document::{DocumentView, FileAction};
pub use workspace::{LayoutTab, WorkspaceLayout};
mod drawing;
mod editor;
pub mod i18n;
mod icons;
mod io;
mod terminal;
#[cfg(test)]
mod tests;
pub use terminal::TtyView;
mod settings;
pub use i18n::Language;
pub use settings::{
    CONFIG_VERSION, DefaultZoom, FileSettings, Settings, SignalPalette, SimulationMode, TickHz,
    UiScale, WorkspaceSettings,
};
mod libraries;
mod plugins;
pub use plugins::{PluginImportTarget, PluginInputTarget};
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
mod theme;
#[cfg(test)]
use egui::vec2;
pub use theme::configure;
/// Circuit editing and viewport state. Signal simulation is not part of this layer.
#[derive(Default)]
pub struct Workspace {
    navigation: workspace::Navigation,
    layout: WorkspaceLayout,
    reset_window: bool,
    analysis: analysis::AnalysisUi,
    generator: generation::Generator,
    generated_expressions: generation::Memory,
    plugins: Vec<logisim_core::plugin::api::presentation::Library>,
    libraries: Vec<libraries::LibraryGroup>,
    memory_editor: memory::MemoryEditor,
    pla_editor: pla::PlaEditor,
    preferences: Settings,
    config_path: Option<String>,
    settings_open: bool,
    settings_page: settings::Page,
    settings_error: Option<String>,
    retry_settings: bool,
    appearance: appearance::AppearanceEditor,
    canvas: Canvas,
    editor: Editor,
    about_open: bool,
    document: DocumentView,
    recent: Vec<String>,
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
        &self.editor.session.circuit
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

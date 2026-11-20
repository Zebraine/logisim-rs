mod registry;
pub const CONFIG_VERSION: u32 = 2;
mod page;
pub(crate) use page::Page;
mod model;
use crate::{Language, Workspace, i18n::tr, theme};
pub use model::{DefaultZoom, Settings, TickHz, UiScale, WorkspaceSettings};

/// Host execution preference, independent of the document and its undo history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SimulationMode {
    #[default]
    Stable,
    Event,
}
impl SimulationMode {
    pub const ALL: [Self; 2] = [Self::Stable, Self::Event];
    pub fn code(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Event => "event",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.code() == code)
    }
    pub fn name(self) -> &'static str {
        tr(match self {
            Self::Stable => "Stable state",
            Self::Event => "Event propagation",
        })
    }
}

impl Workspace {
    pub fn preferences(&self) -> Settings {
        self.preferences
    }
    pub fn apply_settings(&mut self, settings: Settings) {
        if self.preferences.truth_table != settings.truth_table {
            self.set_truth_table_generation(settings.truth_table);
        }
        self.editor
            .simulation
            .set_tick_rate(&mut self.preferences.tick_hz, settings.tick_hz);
        self.preferences = settings;
    }
    pub fn set_config_path(&mut self, path: Option<String>) {
        self.config_path = path;
    }

    pub fn truth_table_generation(&self) -> bool {
        self.preferences.truth_table
    }
    pub fn set_truth_table_generation(&mut self, enabled: bool) {
        self.preferences.truth_table = enabled;
        if !enabled {
            self.generator.hide_truth_table();
        }
    }
    pub fn simulation_mode(&self) -> SimulationMode {
        self.preferences.simulation_mode
    }
    pub fn set_simulation_mode(&mut self, mode: SimulationMode) {
        self.preferences.simulation_mode = mode;
    }
    pub fn language(&self) -> Language {
        self.preferences.language
    }
    pub fn set_language(&mut self, language: Language) {
        self.preferences.language = language;
    }
    pub fn set_settings_error(&mut self, error: Option<String>) {
        self.settings_error = error;
    }
    pub fn take_settings_retry(&mut self) -> bool {
        std::mem::take(&mut self.retry_settings)
    }
    pub(crate) fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut values = self.preferences;
        self.settings_page.show(
            ctx,
            &mut self.settings_open,
            &mut values,
            self.config_path.as_deref(),
            self.settings_error.as_deref(),
            &mut self.retry_settings,
        );
        if !self.settings_open {
            self.settings_page.clear_transient();
        }
        if values != self.preferences {
            self.apply_settings(values);
            ctx.request_repaint();
        }
    }
}

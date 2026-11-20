mod recovery;
mod registry;
pub use recovery::{RecoveryView, SnapshotState};
pub const CONFIG_VERSION: u32 = 9;
mod page;
pub(crate) use page::Page;
mod model;
pub use crate::theme::SignalPalette;
use crate::{Language, Workspace, i18n::tr, theme};
pub use model::{DefaultZoom, FileSettings, Settings, TickHz, UiScale, WorkspaceSettings};
pub(crate) use model::{EditorDisplay, PlacementDefaults};

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
    pub fn set_recovery_view(&mut self, view: RecoveryView) {
        self.settings_page.set_recovery(view);
    }
    pub fn recovery_settings_open(&self) -> bool {
        self.settings_open && self.settings_page.showing_files()
    }
    pub fn preferences(&self) -> Settings {
        self.preferences
    }
    pub fn apply_settings(&mut self, mut settings: Settings) {
        if self.preferences.truth_table != settings.truth_table {
            self.set_truth_table_generation(settings.truth_table);
        }
        self.editor
            .simulation
            .set_tick_rate(&mut self.preferences.tick_hz, settings.tick_hz);
        self.editor.display = settings.editor_display();
        let defaults = settings.placement_defaults();
        self.editor
            .configure_placement(defaults, settings.remember_last_properties);
        settings.default_pin_bits = defaults.pin_bits;
        settings.default_tunnel_width = defaults.tunnel_bits;
        settings.workspace.recent_limit = settings.workspace.bounded_recent_limit();
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
    pub fn take_window_reset(&mut self) -> bool {
        std::mem::take(&mut self.reset_window)
    }
    pub fn take_settings_retry(&mut self) -> bool {
        std::mem::take(&mut self.retry_settings)
    }
    pub(crate) fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut values = self.preferences;
        let action = self.settings_page.show(
            ctx,
            &mut self.settings_open,
            &mut values,
            page::PageInfo {
                document_dirty: self.document.dirty,
                path: self.config_path.as_deref(),
                error: self.settings_error.as_deref(),
                recent: &self.recent,
            },
        );
        if let Some(action) = action {
            match action {
                page::PageAction::Retry => self.retry_settings = true,
                page::PageAction::OpenRecent(index) => {
                    self.settings_open = false;
                    self.file_actions.push(crate::FileAction::OpenRecent(index));
                }
                page::PageAction::ClearRecovery { disabled_autosave } => self
                    .file_actions
                    .push(crate::FileAction::ClearRecovery { disabled_autosave }),
                page::PageAction::ClearRecent => {
                    self.file_actions.push(crate::FileAction::ClearRecent)
                }
                page::PageAction::ResetLayout => {
                    self.set_layout(crate::WorkspaceLayout::default());
                    self.reset_window = true;
                    ctx.request_repaint();
                }
            }
        }
        if !self.settings_open {
            self.settings_page.clear_transient();
        }
        if values != self.preferences {
            self.apply_settings(values);
            ctx.request_repaint();
        }
    }
}

mod model;
use crate::{Language, Workspace, i18n::tr, theme};
pub use model::{DefaultZoom, Settings, TickHz, UiScale};

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
        let mut truth_table_generation = self.preferences.truth_table;
        egui::Window::new(tr("Settings"))
            .id(egui::Id::unique("settings-window"))
            .open(&mut self.settings_open)
            .default_pos(ctx.content_rect().center() - egui::vec2(226.0, 166.0))
            .collapsible(false)
            .resizable(false)
            .default_height(332.0)
            .max_height(ctx.content_rect().height()-56.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width()-64.0).min(420.0));
                if let Some(error) = &self.settings_error {
                    ui.colored_label(theme::DANGER, tr("Settings need attention."));
                    ui.label(tr(error));
                    ui.weak(tr("Your choice applies now. Retry to save it for the next launch."));
                    if ui.button(tr("Retry")).clicked() {
                        self.retry_settings = true;
                    }
                    ui.separator();
                }
                theme::section(ui,tr("Interface"));
                egui::Grid::new("settings-interface").min_col_width(130.0).show(ui,|ui|{
                    ui.label(tr("Language"));
                    egui::ComboBox::from_id_salt("language").width(200.0)
                        .selected_text(self.preferences.language.name()).show_ui(ui,|ui|{
                            for language in Language::ALL{
                                if ui.selectable_value(&mut self.preferences.language,language,language.name()).changed(){ctx.request_repaint();}
                            }
                        });
                    ui.end_row();
                });
                ui.weak(tr("Language changes apply immediately and are saved for the next launch."));
                if !ui.fonts_mut(|fonts|fonts.has_glyphs(&egui::FontId::proportional(14.0),"简体中文设置")){
                    // Keep the font installation hint readable without CJK glyphs.
                    let hint = crate::i18n::with_language(Language::English, || tr("Chinese characters require a system CJK font (for example Noto Sans CJK SC)."));
                    ui.colored_label(theme::DANGER, hint);
                }
                ui.add_space(4.0);
                ui.separator();
                theme::section(ui,tr("Simulation"));
                egui::Grid::new("settings-simulation").min_col_width(130.0).show(ui,|ui|{
                    ui.label(tr("Simulation mode"));
                    egui::ComboBox::from_id_salt("simulation-mode").width(200.0)
                        .selected_text(self.preferences.simulation_mode.name()).show_ui(ui,|ui|{
                            for mode in SimulationMode::ALL{ui.selectable_value(&mut self.preferences.simulation_mode,mode,mode.name());}
                        });
                    ui.end_row();
                });
                ui.weak(tr("Stable state settles combinational logic before sampling state. Unsupported circuits use event propagation."));
                ui.weak(tr("Changes apply without resetting the circuit and are saved for the next launch. Manual propagation steps always use events."));
                ui.add_space(4.0);
                ui.separator();
                ui.checkbox(&mut truth_table_generation,tr("Enable truth table generation"));

            });
        if truth_table_generation != self.preferences.truth_table {
            self.set_truth_table_generation(truth_table_generation);
        }
    }
}

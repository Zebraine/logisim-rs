use crate::{Language, Workspace, i18n::tr};

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
    pub fn simulation_mode(&self) -> SimulationMode {
        self.simulation_mode
    }
    pub fn set_simulation_mode(&mut self, mode: SimulationMode) {
        self.simulation_mode = mode;
    }
    pub fn language(&self) -> Language {
        self.language
    }
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }
    pub fn set_settings_error(&mut self, error: Option<String>) {
        self.settings_error = error;
    }
    pub fn take_settings_retry(&mut self) -> bool {
        std::mem::take(&mut self.retry_settings)
    }
    pub(crate) fn settings(&mut self, ctx: &egui::Context) {
        egui::Window::new(tr("Settings"))
            .id(egui::Id::unique("settings-window"))
            .open(&mut self.settings_open)
            .default_pos(ctx.content_rect().center() - egui::vec2(220.0, 80.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 48.0).min(440.0));
                ui.horizontal(|ui| {
                    ui.label(tr("Language"));
                    egui::ComboBox::from_id_salt("language")
                        .width(180.0)
                        .selected_text(self.language.name())
                        .show_ui(ui, |ui| {
                            for language in Language::ALL {
                                if ui.selectable_value(&mut self.language, language, language.name()).changed() {
                                    ctx.request_repaint();
                                }
                            }
                        });
                });
                ui.label(tr("Language changes apply immediately and are saved for the next launch."));
                if !ui.fonts_mut(|fonts| fonts.has_glyphs(&egui::FontId::proportional(14.0), "简体中文设置")) {
                    // Keep this actionable even when the missing font makes Chinese unreadable.
                    ui.colored_label(egui::Color32::DARK_RED, "Chinese characters require a system CJK font (for example Noto Sans CJK SC).");
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(tr("Simulation mode"));
                    egui::ComboBox::from_id_salt("simulation-mode")
                        .width(180.0)
                        .selected_text(self.simulation_mode.name())
                        .show_ui(ui, |ui| {
                            for mode in SimulationMode::ALL {
                                ui.selectable_value(&mut self.simulation_mode, mode, mode.name());
                            }
                        });
                });
                ui.label(tr("Stable state settles combinational logic before sampling state. Unsupported circuits use event propagation."));
                ui.label(tr("Changes apply without resetting the circuit and are saved for the next launch. Manual propagation steps always use events."));
                if let Some(error) = &self.settings_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::DARK_RED, tr("Settings need attention."));
                    ui.label(tr(error));
                    ui.label(tr("Your choice applies now. Retry to save it for the next launch."));
                    if ui.button(tr("Retry")).clicked() { self.retry_settings = true; }
                }
            });
    }
}

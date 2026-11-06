use crate::{Language, Workspace, i18n::tr};

impl Workspace {
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

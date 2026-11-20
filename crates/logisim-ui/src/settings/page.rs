use super::{
    registry::{Category, Entry, REGISTRY},
    *,
};
use crate::{
    dialog::{Dialog, Dismiss},
    icons::{self, Icon},
    tf,
};
use egui::{Align, Context, Id, Key, Layout, Modifiers, RichText, Ui, vec2};

pub(super) struct PageInfo<'a> {
    pub path: Option<&'a str>,
    pub error: Option<&'a str>,
    pub recent: &'a [String],
}
pub(super) enum PageAction {
    Retry,
    OpenRecent(usize),
    ClearRecent,
    ClearRecovery,
    ResetLayout,
}

#[derive(Default)]
pub(crate) struct Page {
    category: Category,
    query: String,
    confirm_reset: bool,
    confirm_clear: bool,
    recovery: RecoveryView,
    frequency_draft: Option<u32>,
}
impl Page {
    pub(super) fn showing_files(&self) -> bool {
        self.category == Category::Files && self.query.trim().is_empty()
    }
    pub(super) fn set_recovery(&mut self, view: RecoveryView) {
        if self.recovery != view {
            self.confirm_clear = false;
            self.recovery = view;
        }
    }
    pub(super) fn clear_transient(&mut self) {
        self.confirm_clear = false;
        self.query.clear();
        self.confirm_reset = false;
        self.frequency_draft = None;
    }
    pub(super) fn show(
        &mut self,
        ctx: &Context,
        open: &mut bool,
        values: &mut Settings,
        info: PageInfo<'_>,
    ) -> Option<PageAction> {
        let PageInfo {
            path,
            error,
            recent,
        } = info;
        let mut action = None;
        let screen = ctx.content_rect();
        let search_id = Id::unique("preference-search");
        let find = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F));
        if find {
            ctx.memory_mut(|m| m.request_focus(search_id));
        }
        let m = theme::metrics(ctx);
        let dismiss = Dialog::editor("settings-window", m.settings_size.x, m.settings_size.y).show(
            ctx,
            tr("Settings"),
            |dialog| {
                dialog.panes(|ui| {
                    let compact =
                        ui.available_width() < 610. || screen.width() * ctx.zoom_factor() <= 720.;
                    ui.horizontal(|ui| {
                        let search = ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .id(search_id)
                                .desired_width((ui.available_width() - 4.).min(330.))
                                .hint_text(tr("Search settings · Ctrl+F")),
                        );
                        theme::focus_ring(ui, &search);
                        if search.changed() {
                            self.confirm_clear = false;
                            self.confirm_reset = false;
                        }
                    });
                    ui.separator();
                    if compact {
                        egui::ComboBox::from_id_salt("settings-category")
                            .width(168.)
                            .selected_text(if self.query.trim().is_empty() {
                                self.category.label()
                            } else {
                                tr("Search results")
                            })
                            .show_ui(ui, |ui| {
                                for cat in Category::ALL {
                                    if ui
                                        .selectable_label(self.category == cat, cat.label())
                                        .clicked()
                                    {
                                        self.select(cat);
                                    }
                                }
                            });
                    }
                    let size = ui.available_size();
                    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Min), |ui| {
                        ui.set_height(size.y);
                        if !compact {
                            ui.allocate_ui_with_layout(
                                vec2(158., size.y),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.set_width(158.);
                                    for cat in Category::ALL {
                                        let r = ui.add_sized(
                                            [156., m.row],
                                            egui::Button::selectable(
                                                self.category == cat && self.query.is_empty(),
                                                cat.label(),
                                            ),
                                        );
                                        theme::focus_ring(ui, &r);
                                        if r.clicked() {
                                            self.select(cat);
                                        }
                                    }
                                },
                            );
                            ui.separator();
                        }
                        ui.allocate_ui_with_layout(
                            ui.available_size(),
                            Layout::top_down(Align::Min),
                            |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt((
                                        "settings-content",
                                        self.category,
                                        self.query.is_empty(),
                                    ))
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        if let Some(error) = error {
                                            egui::Frame::NONE
                                                .fill(theme::SUNKEN)
                                                .corner_radius(6)
                                                .inner_margin(m.card_margin)
                                                .show(ui, |ui| {
                                                    ui.colored_label(
                                                        theme::DANGER,
                                                        tr("Settings need attention."),
                                                    );
                                                    ui.label(tr(error));
                                                    path_line(ui, path);
                                                    if ui.button(tr("Retry")).clicked() {
                                                        action = Some(PageAction::Retry);
                                                    }
                                                });
                                            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
                                        }
                                        self.content(ui, values, path, recent, &mut action);
                                    });
                            },
                        );
                    });
                });
                dialog.footer("Close", |ui| {
                    ui.label(
                        RichText::new(tr(if error.is_some() {
                            "Settings need attention."
                        } else {
                            "Saved automatically"
                        }))
                        .size(12.)
                        .color(if error.is_some() {
                            theme::DANGER
                        } else {
                            theme::MUTED
                        }),
                    );
                    if let Some(path) = path {
                        let r = ui.add(
                            egui::Label::new(RichText::new(path).size(12.).color(theme::MUTED))
                                .truncate()
                                .sense(egui::Sense::click()),
                        );
                        if r.on_hover_text(tr("Copy path")).clicked() {
                            ctx.copy_text(path.to_owned());
                        }
                    }
                });
            },
        );
        match dismiss {
            Dismiss::Escape if self.confirm_clear => self.confirm_clear = false,
            Dismiss::Escape if self.confirm_reset => self.confirm_reset = false,
            Dismiss::Escape if !self.query.is_empty() => self.query.clear(),
            Dismiss::Escape | Dismiss::Close => *open = false,
            Dismiss::Open => {}
        }
        action
    }
    fn select(&mut self, cat: Category) {
        self.category = cat;
        self.confirm_clear = false;
        self.query.clear();
        self.confirm_reset = false;
        self.frequency_draft = None;
    }
    fn content(
        &mut self,
        ui: &mut Ui,
        values: &mut Settings,
        path: Option<&str>,
        recent: &[String],
        action: &mut Option<PageAction>,
    ) {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() && self.category == Category::Advanced {
            theme::card(ui, tr("Configuration"), |ui| {
                path_line(ui, path);
                theme::chip(ui, &tf!("Configuration version {}", super::CONFIG_VERSION));
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
            theme::card(ui, tr("Reset preferences"), |ui| {
                ui.label(tr("Restore default preferences. Your current circuit and workspace layout are preserved."));
                if self.confirm_reset {
                    ui.strong(tr("Reset all settings to their defaults?"));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Confirm reset")).clicked() {
                            *values = Settings::default();
                            self.frequency_draft = None;
                            self.confirm_reset = false;
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            self.confirm_reset = false;
                        }
                    });
                } else if ui.button(tr("Reset all")).clicked() {
                    self.confirm_reset = true;
                }
            });
            return;
        }
        let mut found = false;
        let active = self.category;
        for category in Category::ALL {
            let mut entries = REGISTRY
                .iter()
                .filter(|s| s.category == category && s.visible(active, &q))
                .peekable();
            if entries.peek().is_none() {
                continue;
            }
            found = true;
            theme::card(ui, category.label(), |ui| {
                for (i, e) in entries.enumerate() {
                    if i > 0 {
                        ui.separator();
                    }
                    ui.push_id(e.key, |ui| self.row(ui, e, values));
                }
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
        }
        if self.showing_files() {
            theme::card(ui, tr("Recovery snapshot"), |ui| {
                self.recovery.details(ui);
                if self.recovery.can_clear() {
                    if self.confirm_clear {
                        ui.label(tr("Delete this recovery copy?"));
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(tr("Confirm clear")).clicked() {
                                self.confirm_clear = false;
                                *action = Some(PageAction::ClearRecovery);
                            }
                            if ui.button(tr("Cancel")).clicked() {
                                self.confirm_clear = false;
                            }
                        });
                    } else if ui.button(tr("Clear recovery")).clicked() {
                        self.confirm_clear = true;
                    }
                }
            });
        }
        if active == Category::Workspace && q.is_empty() {
            theme::card(ui, tr("Recent Projects"), |ui| {
                if recent.is_empty() {
                    ui.weak(tr("No recent projects"));
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("settings-recent-projects")
                        .max_height(120.)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (index, path) in recent
                                .iter()
                                .take(values.workspace.recent_limit)
                                .enumerate()
                            {
                                if crate::document::recent_button(ui, path) {
                                    *action = Some(PageAction::OpenRecent(index));
                                }
                            }
                        });
                    if ui.button(tr("Clear recent")).clicked() {
                        *action = Some(PageAction::ClearRecent);
                    }
                }
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
            theme::card(ui, tr("Workspace layout"), |ui| {
                ui.weak(tr(
                    "Center the window and restore the default inspector layout.",
                ));
                if ui.button(tr("Reset workspace layout")).clicked() {
                    *action = Some(PageAction::ResetLayout);
                }
            });
        }
        if !found {
            ui.label(tr("No settings match"));
            if ui.button(tr("Clear")).clicked() {
                self.confirm_clear = false;
                self.query.clear();
            }
        }
    }
    fn row(&mut self, ui: &mut Ui, e: &Entry, values: &mut Settings) {
        let width = ui.available_width();
        let right = if matches!(e.control, super::registry::Control::SignalPalette) {
            (width - 120.).clamp(216., 282.)
        } else {
            (width * 0.37).clamp(140., 190.)
        };
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2((width - right - 4.).max(40.), theme::metrics(ui.ctx()).row),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.label(RichText::new(tr(e.label)).strong());
                    ui.weak(tr(e.help));
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let defaults = Settings::default();
                let enabled = !matches!(e.control, super::registry::Control::AutosaveMinutes)
                    || values.files.autosave;
                if e.changed(*values, defaults) {
                    if icons::button(ui, Icon::Restore, false, enabled, "Reset to default")
                        .clicked()
                    {
                        e.copy(values, defaults);
                        self.frequency_draft = None;
                    }
                } else {
                    ui.add_space(theme::metrics(ui.ctx()).icon_button);
                }
                e.show(ui, values, &mut self.frequency_draft);
            });
        });
        if matches!(e.control, super::registry::Control::Language)
            && !ui.fonts_mut(|fonts| {
                fonts.has_glyphs(&egui::FontId::proportional(14.0), "简体中文设置")
            })
        {
            let hint = crate::i18n::with_language(Language::English, || {
                tr("Chinese characters require a system CJK font (for example Noto Sans CJK SC).")
            });
            ui.colored_label(theme::DANGER, hint);
        }
    }
}
fn path_line(ui: &mut Ui, path: Option<&str>) {
    if let Some(path) = path {
        ui.horizontal(|ui| {
            if ui.button(tr("Copy path")).clicked() {
                ui.ctx().copy_text(path.to_owned());
            }
            ui.add(egui::Label::new(path).truncate())
                .on_hover_text(path);
        });
    } else {
        ui.weak(tr("User configuration directory is unavailable."));
    }
}

#[cfg(test)]
mod tests;

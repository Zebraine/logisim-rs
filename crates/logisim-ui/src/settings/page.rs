use super::{
    registry::{Category, Entry, REGISTRY},
    *,
};
use crate::{
    icons::{self, Icon},
    tf,
};
use egui::{Align, Context, Id, Key, Layout, Modifiers, RichText, Ui, vec2};

#[derive(Default)]
pub(crate) struct Page {
    category: Category,
    query: String,
    confirm_reset: bool,
    frequency_draft: Option<u32>,
}
impl Page {
    pub(super) fn clear_transient(&mut self) {
        self.query.clear();
        self.confirm_reset = false;
        self.frequency_draft = None;
    }
    pub(super) fn show(
        &mut self,
        ctx: &Context,
        open: &mut bool,
        values: &mut Settings,
        path: Option<&str>,
        error: Option<&str>,
        retry: &mut bool,
    ) {
        let screen = ctx.content_rect();
        let limit = screen.size() * 0.9;
        let margin = egui::Vec2::splat(26.0);
        let maximum = limit - margin;
        let target = vec2(760., 560.).min(limit);
        let min = vec2(640., 440.).min(limit) - margin;
        let compact = screen.width() < 850.;
        let search_id = Id::unique("preference-search");
        let find = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F));
        if !ctx.any_popup_open() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.confirm_reset {
                self.confirm_reset = false;
            } else if !self.query.is_empty() {
                self.query.clear();
            } else {
                *open = false;
                return;
            }
        }
        if find {
            ctx.memory_mut(|m| m.request_focus(search_id));
        }
        egui::Window::new(tr("Settings"))
            .id(Id::unique("settings-window"))
            .title_bar(false)
            .collapsible(false)
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(12))
            .default_pos(screen.center() - target * 0.5)
            .constrain_to(egui::Rect::from_center_size(screen.center(), limit))
            .min_size(min)
            .max_size(maximum)
            .default_size(target - margin)
            .resizable(true)
            .show(ctx, |ui| {
                let available = ui.available_size().clamp(min, maximum);
                ui.set_min_size(available);
                ui.set_max_size(available);
                ui.horizontal(|ui| {
                    ui.heading(tr("Settings"));
                    ui.add_space(8.);
                    let search = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id(search_id)
                            .desired_width((ui.available_width() - 4.).min(330.))
                            .hint_text(tr("Search settings · Ctrl+F")),
                    );
                    theme::focus_ring(ui, &search);
                    if search.changed() {
                        self.confirm_reset = false;
                    }
                });
                ui.separator();
                if compact {
                    egui::ComboBox::from_id_salt("settings-category")
                        .width(168.)
                        .selected_text(self.category.label())
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
                let size = vec2(ui.available_width(), (ui.available_height() - 46.).max(32.));
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
                                        [156., 30.],
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
                                .id_salt(("settings-content", self.category, self.query.is_empty()))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if let Some(error) = error {
                                        egui::Frame::NONE
                                            .fill(theme::SUNKEN)
                                            .corner_radius(6)
                                            .inner_margin(8)
                                            .show(ui, |ui| {
                                                ui.colored_label(
                                                    theme::DANGER,
                                                    tr("Settings need attention."),
                                                );
                                                ui.label(tr(error));
                                                path_line(ui, path);
                                                if ui.button(tr("Retry")).clicked() {
                                                    *retry = true;
                                                }
                                            });
                                        ui.add_space(4.);
                                    }
                                    self.content(ui, values, path);
                                });
                        },
                    );
                });
                ui.separator();
                ui.horizontal(|ui| {
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
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr("Close")).clicked() {
                            *open = false;
                        }
                        if let Some(path) = path {
                            let r = ui.add(
                                egui::Label::new(RichText::new(path).size(12.).color(theme::MUTED))
                                    .truncate()
                                    .sense(egui::Sense::click()),
                            );
                            if r.clone().on_hover_text(tr("Copy path")).clicked() {
                                ctx.copy_text(path.to_owned());
                            }
                        }
                    });
                });
            });
    }
    fn select(&mut self, cat: Category) {
        self.category = cat;
        self.query.clear();
        self.confirm_reset = false;
        self.frequency_draft = None;
    }
    fn content(&mut self, ui: &mut Ui, values: &mut Settings, path: Option<&str>) {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() && self.category == Category::Advanced {
            theme::card(ui, tr("Configuration"), |ui| {
                path_line(ui, path);
                theme::chip(ui, &tf!("Configuration version {}", super::CONFIG_VERSION));
            });
            ui.add_space(4.);
            theme::card(ui, tr("Reset preferences"), |ui| {
                ui.label(tr("Reset language, appearance, editor, simulation and generation preferences. Your current circuit is preserved."));
                if self.confirm_reset {
                    ui.strong(tr("Reset all settings to their defaults?"));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Confirm reset")).clicked() {
                            *values = Settings::default();
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
            ui.add_space(4.);
        }
        if !found {
            ui.label(tr("No settings match"));
            if ui.button(tr("Clear")).clicked() {
                self.query.clear();
            }
        }
    }
    fn row(&mut self, ui: &mut Ui, e: &Entry, values: &mut Settings) {
        let width = ui.available_width();
        let right = (width * 0.37).clamp(140., 190.);
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2((width - right - 4.).max(40.), 30.),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.label(RichText::new(tr(e.label)).strong());
                    ui.weak(tr(e.help));
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let defaults = Settings::default();
                if e.changed(*values, defaults) {
                    if icons::button(ui, Icon::Restore, false, true, "Reset to default").clicked() {
                        e.copy(values, defaults);
                        self.frequency_draft = None;
                    }
                } else {
                    ui.add_space(28.);
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

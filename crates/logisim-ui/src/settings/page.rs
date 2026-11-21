mod bindings;
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
    pub document_dirty: bool,
    pub plugin_fallback_directory: Option<&'a std::path::Path>,
    pub path: Option<&'a str>,
    pub error: Option<&'a str>,
    pub recent: &'a [String],
}
pub(super) enum PageAction {
    Retry,
    OpenRecent(usize),
    ClearRecent,
    ClearRecovery { disabled_autosave: bool },
    ResetLayout,
    ChoosePluginFallback,
    ClearPluginFallback,
    SettingsFile(crate::SettingsFileAction),
}

#[derive(Default)]
pub(crate) struct Page {
    category: Category,
    query: String,
    confirm_reset: bool,
    pub(crate) requested_engine: WasmEnginePreference,
    active_engine: WasmEnginePreference,
    wasmtime_available: bool,
    confirm_engine: bool,
    bindings: bindings::BindingEditor,
    confirm_clear: bool,
    recovery: RecoveryView,
    frequency_draft: Option<u32>,
}
impl Page {
    pub(super) fn set_runtime(
        &mut self,
        requested: WasmEnginePreference,
        active: WasmEnginePreference,
        available: bool,
    ) {
        self.requested_engine = requested;
        self.active_engine = active;
        self.wasmtime_available = available;
    }

    fn engine_card(&mut self, ui: &mut Ui, values: &mut Settings) {
        use WasmEnginePreference::{Wasmi, Wasmtime};
        const WARNING: &str = "After restart, the first course plugin load with an empty cache takes about 0.5 s. Fuel can pause execution at a different point. The Agent uses wasmi.";
        let running = match self.active_engine {
            Wasmi => "wasmi",
            Wasmtime => "Wasmtime",
        };
        theme::card(ui, tr("Wasm plugin engine"), |ui| {
            ui.label(tf!("Session engine preference: {}", running));
            ui.label(tr("Plugin ABI 1–3: wasmi only."));
            ui.push_id("plugins.wasm_engine", |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(values.wasm_engine == Wasmi, tr("wasmi (default)"))
                        .clicked()
                    {
                        values.wasm_engine = Wasmi;
                        self.confirm_engine = false;
                    }
                    let wasmtime =
                        egui::Button::selectable(values.wasm_engine == Wasmtime, tr("Wasmtime"));
                    if ui.add_enabled(self.wasmtime_available, wasmtime).clicked()
                        && values.wasm_engine != Wasmtime
                    {
                        self.confirm_engine = true;
                    }
                });
            });
            if !self.wasmtime_available {
                ui.label(tr("This build does not include Wasmtime."));
            }
            ui.label(tr(WARNING));
            if self.confirm_engine {
                ui.label(tr("Use Wasmtime on the next desktop start?"));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr("Confirm")).clicked() {
                        values.wasm_engine = Wasmtime;
                        self.confirm_engine = false;
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        self.confirm_engine = false;
                    }
                });
            }
            if values.wasm_engine != self.active_engine && self.wasmtime_available {
                ui.label(tr("Restart the desktop application to apply this change."));
            }
        });
    }
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
        self.bindings = Default::default();
        self.confirm_clear = false;
        self.query.clear();
        self.confirm_reset = false;
        self.confirm_engine = false;
        self.frequency_draft = None;
    }
    pub(super) fn show(
        &mut self,
        ctx: &Context,
        open: &mut bool,
        values: &mut Settings,
        info: PageInfo<'_>,
    ) -> Option<PageAction> {
        let PageInfo { path, error, .. } = info;
        let mut action = None;
        let screen = ctx.content_rect();
        let search_id = Id::unique("preference-search");
        self.bindings.input(ctx, &mut values.shortcuts);
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
                            self.bindings = Default::default();
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
                                                .fill(theme::palette(ui.ctx()).sunken)
                                                .corner_radius(6)
                                                .inner_margin(m.card_margin)
                                                .show(ui, |ui| {
                                                    ui.colored_label(
                                                        theme::palette(ui.ctx()).danger,
                                                        tr("Settings need attention."),
                                                    );
                                                    ui.label(tr(error));
                                                    path_line(ui, path, compact);
                                                    if ui.button(tr("Retry")).clicked() {
                                                        action = Some(PageAction::Retry);
                                                    }
                                                });
                                            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
                                        }
                                        self.content(ui, values, &info, &mut action);
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
                            theme::palette(ui.ctx()).danger
                        } else {
                            theme::palette(ui.ctx()).muted
                        }),
                    );
                    if let Some(path) = path {
                        let r = ui.add(
                            egui::Label::new(
                                RichText::new(path)
                                    .size(12.)
                                    .color(theme::palette(ui.ctx()).muted),
                            )
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
            Dismiss::Escape if self.bindings.confirm_reset => self.bindings.confirm_reset = false,
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
        self.bindings = Default::default();
        self.confirm_clear = false;
        self.query.clear();
        self.confirm_reset = false;
        self.confirm_engine = false;
        self.frequency_draft = None;
    }
    fn content(
        &mut self,
        ui: &mut Ui,
        values: &mut Settings,
        info: &PageInfo<'_>,
        action: &mut Option<PageAction>,
    ) {
        let PageInfo {
            path,
            recent,
            document_dirty,
            ..
        } = *info;
        let q = self.query.trim().to_lowercase();
        let compact = values.density == crate::Density::Compact;
        if q.is_empty() && self.category == Category::Advanced {
            theme::card(ui, tr("Configuration"), |ui| {
                path_line(ui, path, compact);
                theme::chip(ui, &tf!("Configuration version {}", super::CONFIG_VERSION));
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
            theme::card(ui, tr("Reset preferences"), |ui| {
                let help = "Restore default preferences. Your current circuit and workspace layout are preserved.";
                if !compact {
                    ui.label(tr(help));
                }
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
                } else if action_with_help(ui, compact, "Reset all", help) {
                    self.confirm_reset = true;
                }
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
            theme::card(ui, tr("Settings file"), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (label, command) in [
                        ("Export settings…", crate::SettingsFileAction::Export),
                        ("Import settings…", crate::SettingsFileAction::Import),
                    ] {
                        if ui.button(tr(label)).clicked() {
                            *action = Some(PageAction::SettingsFile(command));
                        }
                    }
                    if compact {
                        help_button(ui, "Settings file", "Transfer preferences as JSON. Recent projects, window geometry and panel layout stay on this computer.");
                    }
                });
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
            fallback_card(ui, info.plugin_fallback_directory, compact, action);
            self.engine_card(ui, values);
            return;
        }
        let mut found = !q.is_empty()
            && [
                "plugins.fallback_directory",
                FALLBACK_LABEL,
                tr(FALLBACK_LABEL),
            ]
            .iter()
            .any(|s| s.to_lowercase().contains(&q));
        if found {
            fallback_card(ui, info.plugin_fallback_directory, compact, action);
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
        }
        if !q.is_empty()
            && [
                "plugins.wasm_engine",
                "wasmi",
                "wasmtime",
                "Wasm plugin engine",
                tr("Wasm plugin engine"),
            ]
            .iter()
            .any(|s| s.to_lowercase().contains(&q))
        {
            self.engine_card(ui, values);
            found = true;
        }
        let active = self.category;
        let files_page = self.showing_files();
        let recovery_first = files_page
            && (self.recovery.error.is_some() || self.recovery.state != SnapshotState::Empty);
        if recovery_first {
            self.recovery_card(ui, values, document_dirty, action);
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
        }
        for category in Category::ALL {
            if category == Category::Shortcuts {
                if active == category || !q.is_empty() {
                    found |= self.bindings.show(ui, &mut values.shortcuts, &q);
                }
                continue;
            }
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
        if files_page && !recovery_first {
            self.recovery_card(ui, values, document_dirty, action);
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
                    if action_with_help(
                        ui,
                        compact,
                        "Clear recent",
                        "Remove the entries from this list. Project files stay on disk.",
                    ) {
                        *action = Some(PageAction::ClearRecent);
                    }
                }
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
            theme::card(ui, tr("Workspace layout"), |ui| {
                let help = "Center the window and restore the default inspector layout.";
                if !compact {
                    ui.weak(tr(help));
                }
                if action_with_help(ui, compact, "Reset workspace layout", help) {
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
    fn recovery_card(
        &mut self,
        ui: &mut Ui,
        values: &mut Settings,
        document_dirty: bool,
        action: &mut Option<PageAction>,
    ) {
        theme::card(ui, tr("Recovery snapshot"), |ui| {
            self.recovery.details(ui);
            let owner_error = self.recovery.error.as_deref() == Some(RecoveryView::FOREIGN_OWNER);
            if self.recovery.can_clear() {
                if self.confirm_clear {
                    let disabled_autosave = document_dirty && values.files.autosave;
                    ui.label(tr("Delete this recovery copy?"));
                    if disabled_autosave {
                        ui.label(tr("Autosave is on. Clearing will also turn Autosave off so this copy stays removed."));
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(tr("Confirm clear")).clicked() {
                            self.confirm_clear = false;
                            if disabled_autosave {
                                values.files.autosave = false;
                            }
                            *action = Some(PageAction::ClearRecovery { disabled_autosave });
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            self.confirm_clear = false;
                        }
                    });
                } else if action_with_help(
                    ui,
                    values.density == crate::Density::Compact,
                    if owner_error {
                        "Retry"
                    } else {
                        "Clear recovery"
                    },
                    "Review the recovery copy before confirming its removal.",
                ) {
                    self.confirm_clear = true;
                }
            } else if matches!(self.recovery.state, SnapshotState::Available { .. }) && !owner_error
            {
                ui.label(tr(RecoveryView::FOREIGN_OWNER));
            }
        });
    }
    fn row(&mut self, ui: &mut Ui, e: &Entry, values: &mut Settings) {
        let width = ui.available_width();
        let right = if matches!(
            e.control,
            super::registry::Control::SignalPalette | super::registry::Control::Accent
        ) {
            (width - 120.).clamp(216., 282.)
        } else {
            (width * 0.37).clamp(140., 190.)
        };
        let enabled = !matches!(e.control, super::registry::Control::AutosaveMinutes)
            || values.files.autosave;
        ui.horizontal(|ui| {
            let label_size = vec2((width - right - 4.).max(40.), theme::metrics(ui.ctx()).row);
            let label = RichText::new(tr(e.label)).strong();
            let help = tr(e.help);
            if values.density == crate::Density::Compact {
                ui.allocate_ui_with_layout(
                    label_size,
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        let button_width = theme::metrics(ui.ctx()).icon_button;
                        let text_width =
                            (label_size.x - button_width - ui.spacing().item_spacing.x).max(1.);
                        ui.allocate_ui_with_layout(
                            vec2(text_width, label_size.y),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.add(egui::Label::new(label).truncate())
                                    .on_hover_ui(|ui| {
                                        ui.strong(tr(e.label));
                                        ui.label(help);
                                    });
                            },
                        );
                        help_button(ui, e.label, e.help);
                    },
                );
            } else {
                ui.allocate_ui_with_layout(label_size, Layout::top_down(Align::Min), |ui| {
                    ui.label(label);
                    ui.weak(help);
                });
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let defaults = Settings::default();
                if e.changed(*values, defaults) {
                    if icons::button(ui, Icon::Restore, false, enabled)
                        .on_hover_text(tr("Reset to default"))
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
            ui.colored_label(theme::palette(ui.ctx()).danger, hint);
        }
    }
}
const FALLBACK_LABEL: &str = "Plugin fallback directory";
const FALLBACK_HELP: &str = "When both project paths fail, try the same .wasm file name in this folder. Subfolders are skipped. Confirm in Project Options to update the project path.";

fn fallback_card(
    ui: &mut Ui,
    directory: Option<&std::path::Path>,
    compact: bool,
    action: &mut Option<PageAction>,
) {
    ui.push_id("plugins.fallback_directory", |ui| {
        theme::card(ui, tr(FALLBACK_LABEL), |ui| {
            if let Some(dir) = directory {
                let path = dir.to_string_lossy();
                ui.add(egui::Label::new(path.as_ref()).truncate())
                    .on_hover_text(path);
            } else {
                ui.weak(tr("Not set"));
            }
            if !compact {
                ui.label(tr(FALLBACK_HELP));
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr("Choose…")).clicked() {
                    *action = Some(PageAction::ChoosePluginFallback);
                }
                if ui
                    .add_enabled(directory.is_some(), egui::Button::new(tr("Clear")))
                    .clicked()
                {
                    *action = Some(PageAction::ClearPluginFallback);
                }
                if compact {
                    help_button(ui, FALLBACK_LABEL, FALLBACK_HELP);
                }
            });
        })
    });
}

// The button is active even when the setting it describes is disabled.
fn help_button(ui: &mut Ui, label: &str, help: &str) {
    let side = theme::metrics(ui.ctx()).icon_button;
    let button = ui.add_sized(vec2(side, side), egui::Button::new("?"));
    button.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, tf!("Help for {}", tr(label)))
    });
    if button.clicked() {
        button.request_focus();
    }
    theme::focus_ring(ui, &button);
    if button.has_focus() {
        button.show_tooltip_ui(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            ui.label(tr(help));
        });
    } else {
        button.on_hover_text(tr(help));
    }
}

fn action_with_help(ui: &mut Ui, compact: bool, label: &str, help: &str) -> bool {
    if !compact {
        return ui.button(tr(label)).clicked();
    }
    ui.horizontal_wrapped(|ui| {
        let clicked = ui.button(tr(label)).clicked();
        help_button(ui, label, help);
        clicked
    })
    .inner
}

fn path_line(ui: &mut Ui, path: Option<&str>, compact: bool) {
    if let Some(path) = path {
        ui.horizontal(|ui| {
            if ui.button(tr("Copy path")).clicked() {
                ui.ctx().copy_text(path.to_owned());
            }
            if compact {
                help_button(
                    ui,
                    "Copy path",
                    "Copy the location of this computer’s preferences file.",
                );
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

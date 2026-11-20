//! Dialog rendering returns typed intentions; it never reads or writes project files.
use super::picker::Picker;
use eframe::egui;
use logisim_ui::{
    MemoryImportTarget,
    dialog::{Dialog as Window, Dismiss},
    i18n::tr,
};
use std::path::PathBuf;

pub(super) enum Dialog {
    Recovery {
        cli: Option<PathBuf>,
        view: logisim_ui::RecoveryView,
    },
    Plugins(Box<crate::plugins::Resolution>),
    PluginPicker {
        picker: Picker,
        resolution: Box<crate::plugins::Resolution>,
        index: usize,
    },
    Picker(Picker),
    MemoryPicker {
        picker: Picker,
        target: Box<MemoryImportTarget>,
    },
    PluginImportPicker {
        picker: Picker,
        target: Box<logisim_ui::PluginImportTarget>,
    },
    Unsaved(super::NextDocument),
    Replace {
        path: PathBuf,
        bytes: Vec<u8>,
    },
    ReplaceImage(super::image::ImageTarget),
    ImageMessage {
        title: &'static str,
        body: String,
    },
    ReplaceSettings {
        path: PathBuf,
        bytes: Vec<u8>,
    },
    ImportSettings {
        path: PathBuf,
        settings: Box<crate::preferences::ValidatedSettings>,
    },
    SettingsMessage {
        title: &'static str,
        body: String,
    },
    Message {
        title: String,
        body: String,
    },
}
pub(super) enum Response {
    Import,
    Recover,
    ConfirmPlugins,
    BrowsePlugin(usize),
    CheckPlugin(usize),
    Wait,
    Cancel,
    Save,
    Discard,
    Replace,
    Pick(PathBuf),
}
impl Dialog {
    pub(super) fn is_image(&self) -> bool {
        matches!(
            self,
            Self::Picker(Picker {
                mode: super::Mode::ExportImage,
                ..
            }) | Self::ReplaceImage(_)
                | Self::ImageMessage { .. }
        )
    }
    pub(super) fn is_settings(&self) -> bool {
        matches!(
            self,
            Self::Picker(Picker {
                mode: super::Mode::Settings(_) | super::Mode::Directory,
                ..
            }) | Self::ReplaceSettings { .. }
                | Self::ImportSettings { .. }
                | Self::SettingsMessage { .. }
        )
    }
    pub(super) fn show(&mut self, ctx: &egui::Context) -> Response {
        let mut answer = Response::Wait;
        let dismiss = match self {
            Dialog::Picker(picker)
            | Dialog::MemoryPicker { picker, .. }
            | Dialog::PluginPicker { picker, .. }
            | Dialog::PluginImportPicker { picker, .. } => return picker.show(ctx),
            Dialog::Plugins(resolution) => resolve_plugins(ctx, resolution, &mut answer),
            Dialog::Recovery { view, .. } => Window::prompt("startup-recovery").show(
                ctx,
                tr("Recover unsaved changes?"),
                |dialog| {
                    dialog.body(|ui| {
                        view.details(ui);
                        ui.weak(tr("Discard deletes the recovery copy."));
                    });
                    dialog.actions(|ui| {
                        if ui
                            .add_enabled(
                                matches!(view.state, logisim_ui::SnapshotState::Available { .. }),
                                egui::Button::new(tr("Recover")),
                            )
                            .clicked()
                        {
                            answer = Response::Recover;
                        }
                        if ui.button(tr("Discard")).clicked() {
                            answer = Response::Discard;
                        }
                    });
                },
            ),
            Dialog::Unsaved(_) => {
                Window::prompt("unsaved-project").show(ctx, tr("Save changes?"), |dialog| {
                    dialog.body(|ui| ui.label(tr("This project has unsaved changes.")));
                    dialog.footer("Cancel", |ui| {
                        if ui.button(tr("Save")).clicked() {
                            answer = Response::Save;
                        }
                        if ui.button(tr("Discard changes")).clicked() {
                            answer = Response::Discard;
                        }
                    });
                })
            }
            Dialog::ImportSettings { path, .. } => Window::prompt("confirm-settings-import").confirm(
                ctx, tr("Import settings?"),
                &logisim_ui::tf!("Replace your current preferences with the settings in {}? Your circuit, recent projects, window and panel layout are preserved.", path.file_name().expect("selected file").to_string_lossy()),
                tr("Import settings"), || answer = Response::Import
            ),
            Dialog::ReplaceImage(super::image::ImageTarget { path, .. }) | Dialog::ReplaceSettings { path, .. } | Dialog::Replace { path, .. } => Window::prompt("replace-file").confirm(
                ctx,
                tr("Replace existing file?"),
                &path.to_string_lossy(),
                tr("Replace"),
                || answer = Response::Replace,
            ),
            Dialog::ImageMessage {title, body} | Dialog::SettingsMessage {title, body} => Window::prompt("settings-file-message").message(ctx,tr(title),body),
            Dialog::Message { title, body } => {
                Window::prompt("file-message").message(ctx, tr(title), body)
            }
        };
        if matches!(answer, Response::Wait) && dismiss != Dismiss::Open {
            Response::Cancel
        } else {
            answer
        }
    }
}

fn resolve_plugins(
    ctx: &egui::Context,
    resolution: &mut crate::plugins::Resolution,
    answer: &mut Response,
) -> Dismiss {
    let window = Window::standard("locate-plugins", ctx);
    window.show(ctx, tr("Locate circuit plugins"), |dialog| {
        dialog.body(|ui| {
            ui.label(tr(
                "Choose missing plugin locations. Confirm can continue with static graphics for unresolved components.",
            ));
            for (index, row) in resolution.rows.iter_mut().enumerate() {
                let id = egui::IdSalt::new(("plugin-source", &row.source.plugin, &row.source.version));
                ui.push_id(id, |ui| {
                    plugin_row(ui, row, index, answer);
                });
            }
        });
        dialog.footer("Cancel", |ui| {
            if ui.button(tr("Confirm")).clicked() {
                *answer = Response::ConfirmPlugins;
            }
        });
    })
}

fn plugin_row(
    ui: &mut egui::Ui,
    row: &mut crate::plugins::Row,
    index: usize,
    answer: &mut Response,
) {
    ui.separator();
    ui.strong(format!("{} {}", row.source.plugin, row.source.version));
    if !row.source.path.is_empty() {
        ui.label(&row.source.path);
    }
    ui.horizontal_wrapped(|ui| {
        let width = (ui.available_width() - 152.).max(80.);
        if ui
            .add(
                egui::TextEdit::singleline(&mut row.location)
                    .desired_width(width)
                    .char_limit(4096)
                    .hint_text(tr("Actual plugin location")),
            )
            .changed()
        {
            row.loaded = None;
            row.found = None;
            row.from_directory = false;
            row.error = Some(tr("Location changed; Check or Confirm to validate.").into());
        }
        if ui.button(tr("Browse…")).clicked() {
            *answer = Response::BrowsePlugin(index);
        }
        if ui.button(tr("Check")).clicked() {
            *answer = Response::CheckPlugin(index);
        }
    });
    if let Some(error) = &row.error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    } else {
        ui.label(tr("Plugin ready"));
        if row.from_directory
            && let Some(path) = &row.found
        {
            ui.label(tr("Confirm to use this plugin path in the project."));
            let path = path.to_string_lossy();
            ui.add(egui::Label::new(path.as_ref()).truncate())
                .on_hover_text(path);
        }
    }
}

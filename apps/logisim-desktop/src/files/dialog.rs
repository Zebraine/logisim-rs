//! Dialog rendering returns typed intentions; it never reads or writes project files.
use crate::file_picker::{Choice, Picker};
use eframe::egui;
use logisim_ui::{FileAction, MemoryImportTarget, i18n::tr};
use std::path::PathBuf;

pub(super) enum Dialog {
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
    Unsaved(FileAction),
    Replace {
        path: PathBuf,
        bytes: Vec<u8>,
    },
    Message {
        title: String,
        body: String,
    },
}
pub(super) enum Response {
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
    pub(super) fn show(&mut self, ctx: &egui::Context) -> Response {
        let mut answer = Response::Wait;
        let modal = egui::Modal::new(egui::Id::unique("file-dialog")).show(ctx, |ui| match self {
            Dialog::Picker(picker) | Dialog::MemoryPicker { picker, .. } | Dialog::PluginPicker { picker, .. } => {
                answer = match picker.show(ui) {
                    Choice::Wait => Response::Wait,
                    Choice::Cancel => Response::Cancel,
                    Choice::Pick(path) => Response::Pick(path),
                };
            }
            Dialog::Plugins(resolution) => {
                ui.set_width(650.0);
                ui.heading(tr("Locate circuit plugins"));
                ui.label(tr("Choose missing plugin locations. Confirm can continue with static graphics for unresolved components."));
                egui::ScrollArea::vertical().max_height(450.0).show(ui, |ui| {
                    for (index, row) in resolution.rows.iter_mut().enumerate() {
                        ui.push_id(("plugin-source", &row.source.plugin, &row.source.version), |ui| {
                            ui.separator();
                            ui.strong(format!("{} {}", row.source.plugin, row.source.version));
                            if !row.source.path.is_empty() { ui.label(&row.source.path); }
                            ui.horizontal(|ui| {
                                if ui.add(egui::TextEdit::singleline(&mut row.location).desired_width(440.0).char_limit(4096).hint_text(tr("Actual plugin location"))).changed() {
                                    row.loaded = None; row.found = None;
                                    row.error = Some(tr("Location changed; Check or Confirm to validate.").into());
                                }
                                if ui.button(tr("Browse…")).clicked() { answer = Response::BrowsePlugin(index); }
                                if ui.button(tr("Check")).clicked() { answer = Response::CheckPlugin(index); }
                            });
                            if let Some(error) = &row.error { ui.colored_label(egui::Color32::DARK_RED,error); }
                            else { ui.colored_label(egui::Color32::DARK_GREEN,tr("Plugin ready")); }
                        });
                    }
                });
                if ui.button(tr("Confirm")).clicked() { answer = Response::ConfirmPlugins; }
            }
            Dialog::Unsaved(_) => {
                ui.heading(tr("Save changes?"));
                ui.label(tr("This project has unsaved changes."));
                ui.horizontal(|ui| {
                    if ui.button(tr("Save")).clicked() {
                        answer = Response::Save;
                    }
                    if ui.button(tr("Discard changes")).clicked() {
                        answer = Response::Discard;
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        answer = Response::Cancel;
                    }
                });
            }
            Dialog::Replace { path, .. } => {
                ui.heading(tr("Replace existing file?"));
                ui.label(path.to_string_lossy());
                ui.horizontal(|ui| {
                    if ui.button(tr("Replace")).clicked() {
                        answer = Response::Replace;
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        answer = Response::Cancel;
                    }
                });
            }
            Dialog::Message { title, body } => {
                ui.set_max_width(620.0);
                ui.heading(tr(title.as_str()));
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        ui.label(body.as_str());
                    });
                if ui.button(tr("OK")).clicked() {
                    answer = Response::Cancel;
                }
            }
        });
        if matches!(answer, Response::Wait) && modal.should_close() {
            Response::Cancel
        } else {
            answer
        }
    }
}

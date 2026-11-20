//! Small egui file browser, usable without a desktop portal or an external process.
use super::dialog::Response;
use eframe::egui;
use logisim_ui::{
    dialog::{Dialog, Dismiss},
    i18n::tr,
};
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Open,
    Save,
    Memory,
    Library,
    Plugin,
    ExportImage,
    Settings(logisim_ui::SettingsFileAction),
}
impl Mode {
    fn extension(self) -> &'static str {
        match self {
            Self::Settings(_) => "json",
            Self::Plugin => "wasm",
            Self::ExportImage => "png",
            Self::Memory => "hex",
            Self::Open | Self::Save | Self::Library => "circ",
        }
    }
    fn saving(self) -> bool {
        matches!(
            self,
            Self::Save | Self::ExportImage | Self::Settings(logisim_ui::SettingsFileAction::Export)
        )
    }
}
pub(super) struct Picker {
    pub mode: Mode,
    directory: PathBuf,
    path: String,
    entries: Vec<(PathBuf, bool)>,
    error: Option<String>,
    focus: bool,
}
impl Picker {
    pub fn new(mode: Mode, directory: PathBuf, suggested: &str) -> Self {
        let mut this = Self {
            mode,
            directory,
            path: suggested.into(),
            entries: vec![],
            error: None,
            focus: true,
        };
        this.refresh();
        this
    }
    fn refresh(&mut self) {
        self.entries.clear();
        self.error = None;
        match std::fs::read_dir(&self.directory) {
            Ok(entries) => {
                for entry in entries.take(10_000) {
                    let Ok(entry) = entry else { continue };
                    let path = entry.path();
                    let dir = path.is_dir();
                    if dir
                        || path
                            .extension()
                            .is_some_and(|s| s.eq_ignore_ascii_case(self.mode.extension()))
                    {
                        self.entries.push((path, dir));
                    }
                }
                self.entries
                    .sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn enter(&mut self, path: &Path) {
        self.directory = path.to_path_buf();
        self.refresh();
        if !self.mode.saving() {
            self.path.clear();
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) -> Response {
        let (id, title) = match self.mode {
            Mode::Settings(logisim_ui::SettingsFileAction::Export) => {
                ("export-settings", "Export settings…")
            }
            Mode::Settings(logisim_ui::SettingsFileAction::Import) => {
                ("import-settings", "Import settings…")
            }
            Mode::ExportImage => ("export-image", "Export Image…"),
            Mode::Open => ("open-circuit", "Open circuit"),
            Mode::Save => ("save-circuit", "Save circuit as"),
            Mode::Memory => ("import-memory", "Import memory image"),
            Mode::Library => ("load-library", "Load .circ Library…"),
            Mode::Plugin => ("load-plugin", "Load Wasm Plugin…"),
        };
        let mut picked = None;
        let mut submit = false;
        let dismiss = Dialog::standard(id, ctx).show(ctx, tr(title), |dialog| {
            let list_height = (dialog.body_height() - 144.).max(56.);
            dialog.body(|ui| {
                ui.label(tr(if matches!(self.mode, Mode::Settings(_)) {
                    "Settings files (.json)"
                } else if self.mode == Mode::ExportImage {
                    "Current canvas viewport (.png)"
                } else if self.mode == Mode::Plugin {
                    "Wasm plugins provide component graphics and behavior."
                } else if self.mode == Mode::Library {
                    "Import an editable snapshot; saved projects do not need the source library."
                } else if self.mode == Mode::Memory {
                    "Logisim .hex · v2.0 raw · replaces all words"
                } else {
                    "Logisim .circ · Wasm components require logisim-rs"
                }));
                ui.horizontal(|ui| {
                    if ui.button(tr("Up")).clicked()
                        && let Some(parent) = self.directory.parent().map(Path::to_path_buf)
                    {
                        self.enter(&parent);
                    }
                    if ui.button(tr("Refresh")).clicked() {
                        self.refresh();
                    }
                    let path = self.directory.to_string_lossy();
                    ui.add(egui::Label::new(path.as_ref()).truncate())
                        .on_hover_text(path);
                });
                let mut enter = None;
                egui::ScrollArea::vertical()
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show_rows(
                        ui,
                        ui.spacing().interact_size.y,
                        self.entries.len(),
                        |ui, range| {
                            for (path, dir) in &self.entries[range] {
                                let name = path.file_name().unwrap_or_default().to_string_lossy();
                                let label = if *dir {
                                    format!("{name}/")
                                } else {
                                    name.to_string()
                                };
                                let response = ui
                                    .add(
                                        egui::Button::selectable(self.path == name, label)
                                            .truncate(),
                                    )
                                    .on_hover_ui(|ui| {
                                        ui.label(path.to_string_lossy());
                                    });
                                if response.clicked() {
                                    if *dir {
                                        enter = Some(path.clone());
                                    } else {
                                        self.path = name.into_owned();
                                    }
                                }
                                if !*dir && response.double_clicked() {
                                    picked = Some(path.clone());
                                }
                            }
                        },
                    );
                if let Some(path) = enter {
                    self.enter(&path);
                }
                ui.separator();
                ui.label(tr("File name or full path"));
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .desired_width(f32::INFINITY)
                        .id_salt("file-path"),
                );
                if self.focus {
                    edit.request_focus();
                    self.focus = false;
                }
                submit = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
            dialog.footer("Cancel", |ui| {
                submit |= ui
                    .button(tr(if self.mode.saving() { "Save" } else { "Open" }))
                    .clicked();
            });
        });
        if dismiss != Dismiss::Open {
            return Response::Cancel;
        }
        if submit {
            if self.path.trim().is_empty() {
                self.error = Some(tr("Enter a file name.").into());
                return Response::Wait;
            }
            let path = PathBuf::from(&self.path);
            picked = Some(if path.is_absolute() {
                path
            } else {
                self.directory.join(path)
            });
        }
        if let Some(mut path) = picked {
            if path.is_dir() {
                if self.mode == Mode::ExportImage
                    && path
                        .extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
                {
                    self.error = Some(tr("Choose a file, not a directory.").into());
                    return Response::Wait;
                }
                self.enter(&path);
                return Response::Wait;
            }
            if matches!(self.mode, Mode::Settings(_) | Mode::ExportImage) {
                let extension = self.mode.extension();
                if self.mode.saving() && path.extension().is_none() {
                    path.set_extension(extension);
                }
                if !path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case(extension))
                {
                    self.error = Some(
                        tr(if self.mode == Mode::ExportImage {
                            "Choose a .png image file."
                        } else {
                            "Choose a .json settings file."
                        })
                        .into(),
                    );
                    return Response::Wait;
                }
            } else if self.mode == Mode::Save
                && !path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("circ"))
            {
                path.as_mut_os_string().push(".circ");
            }
            return Response::Pick(path);
        }
        Response::Wait
    }
}

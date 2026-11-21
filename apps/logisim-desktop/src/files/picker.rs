//! Small egui file browser, usable without a desktop portal or an external process.
use super::dialog::Response;
use eframe::egui;
use logisim_file::ProjectFormat;
use logisim_ui::{
    dialog::{Dialog, Dismiss},
    i18n::tr,
};
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Open,
    OpenNative,
    Copy(ProjectFormat),
    Save(ProjectFormat),
    Memory,
    Library,
    Plugin,
    ExportImage,
    Directory,
    Settings(logisim_ui::SettingsFileAction),
}
impl Mode {
    fn extension(self) -> Option<&'static str> {
        match self {
            Self::Directory => None,
            Self::Settings(_) => Some("json"),
            Self::Plugin => Some("wasm"),
            Self::ExportImage => Some("png"),
            Self::Memory => Some("hex"),
            Self::OpenNative => Some("lrs"),
            Self::Copy(format) | Self::Save(format) => Some(format.extension()),
            Self::Open | Self::Library => Some("circ"),
        }
    }
    fn saving(self) -> bool {
        matches!(
            self,
            Self::Copy(_)
                | Self::Save(_)
                | Self::ExportImage
                | Self::Settings(logisim_ui::SettingsFileAction::Export)
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
    fn select_format(&mut self, format: ProjectFormat) {
        let path = Path::new(&self.path);
        if ProjectFormat::from_extension(path).is_some() {
            self.path = path
                .with_extension(format.extension())
                .to_string_lossy()
                .into_owned();
        }
        self.mode = Mode::Save(format);
        self.refresh();
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
                        || (self.mode == Mode::Open
                            && ProjectFormat::from_extension(&path).is_some())
                        || path
                            .extension()
                            .zip(self.mode.extension())
                            .is_some_and(|(s, ext)| s.eq_ignore_ascii_case(ext))
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
            Mode::Directory => ("choose-plugin-directory", "Plugin fallback directory"),
            Mode::Open => ("open-circuit", "Open circuit"),
            Mode::OpenNative => ("open-native", "Open .lrs…"),
            Mode::Copy(ProjectFormat::Lrs) => ("copy-native", "Convert to .lrs…"),
            Mode::Copy(ProjectFormat::Circ) => ("copy-legacy", "Export .circ…"),
            Mode::Save(_) => ("save-circuit", "Save circuit as"),
            Mode::Memory => ("import-memory", "Import memory image"),
            Mode::Library => ("load-library", "Load .circ Library…"),
            Mode::Plugin => ("load-plugin", "Load Wasm Plugin…"),
        };
        let mut picked = None;
        let mut submit = false;
        let dismiss = Dialog::standard(id, ctx).show(ctx, tr(title), |dialog| {
            let extra = if matches!(self.mode, Mode::Save(_) | Mode::Copy(ProjectFormat::Circ)) { 52. } else { 0. };
            let list_height = (dialog.body_height() - 144. - extra).max(56.);
            dialog.body(|ui| {
                if let Mode::Save(mut format) = self.mode {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr("Save as type"));
                        egui::ComboBox::from_id_salt("project-save-format")
                            .selected_text(format_label(format))
                            .show_ui(ui, |ui| {
                                for choice in [ProjectFormat::Lrs, ProjectFormat::Circ] {
                                    ui.selectable_value(&mut format, choice, format_label(choice));
                                }
                            });
                    });
                    if self.mode != Mode::Save(format) {
                        self.select_format(format);
                    }
                } else {
                    let description = match self.mode {
                        Mode::Settings(_) => "Settings files (.json)",
                        Mode::Open => "Project files (.circ, .lrs)",
                        Mode::OpenNative | Mode::Copy(ProjectFormat::Lrs) => "Native project (.lrs)",
                        Mode::Copy(ProjectFormat::Circ) => "Legacy Logisim (*.circ)",
                        Mode::Directory => "Choose the folder containing the plugin files.",
                        Mode::ExportImage => "Current canvas viewport (.png)",
                        Mode::Plugin => "Wasm plugins provide component graphics and behavior.",
                        Mode::Library => "Import an editable snapshot; saved projects do not need the source library.",
                        Mode::Memory => "Logisim .hex · v2.0 raw · replaces all words",
                        Mode::Save(_) => unreachable!(),
                    };
                    ui.label(tr(description));
                }
                if matches!(self.mode, Mode::Save(ProjectFormat::Circ) | Mode::Copy(ProjectFormat::Circ)) {
                    ui.weak(tr("For older tools. Saving stops and lists any data the legacy format cannot preserve."));
                }
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
                ui.label(tr(if self.mode == Mode::Directory {
                    "Folder path"
                } else {
                    "File name or full path"
                }));
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
                    .button(tr(if self.mode == Mode::Directory {
                        "Use this folder"
                    } else if self.mode.saving() {
                        "Save"
                    } else {
                        "Open"
                    }))
                    .clicked();
            });
        });
        if dismiss != Dismiss::Open {
            return Response::Cancel;
        }
        if submit {
            if self.mode != Mode::Directory && self.path.trim().is_empty() {
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
            if self.mode == Mode::Directory {
                return match crate::preferences::canonical_plugin_directory(&path) {
                    Ok(dir) => Response::Pick(dir),
                    Err(error) => {
                        self.error = Some(tr(&error.to_string()).into());
                        Response::Wait
                    }
                };
            }
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
            if matches!(
                self.mode,
                Mode::Save(_) | Mode::Copy(_) | Mode::Open | Mode::OpenNative
            ) {
                if let Mode::Copy(format) | Mode::Save(format) = self.mode
                    && path.extension().is_none()
                {
                    path.set_extension(format.extension());
                }
                let format = ProjectFormat::from_extension(&path);
                let valid = match self.mode {
                    Mode::Open => format.is_some(),
                    Mode::OpenNative => format == Some(ProjectFormat::Lrs),
                    Mode::Copy(target) | Mode::Save(target) => format == Some(target),
                    _ => unreachable!(),
                };
                if !valid {
                    self.error =
                        Some(tr("Choose a project file with the indicated extension.").into());
                    return Response::Wait;
                }
            } else if matches!(self.mode, Mode::Settings(_) | Mode::ExportImage) {
                let extension = self.mode.extension().expect("file export/import extension");
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
            }
            return Response::Pick(path);
        }
        Response::Wait
    }
}

fn format_label(format: ProjectFormat) -> &'static str {
    tr(match format {
        ProjectFormat::Lrs => "Logisim-rs Project (*.lrs)",
        ProjectFormat::Circ => "Legacy Logisim (*.circ)",
    })
}

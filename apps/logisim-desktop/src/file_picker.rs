//! Small egui file browser, usable without a desktop portal or an external process.
use eframe::egui;
use logisim_ui::i18n::tr;
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Open,
    Save,
    Memory,
}
pub(crate) struct Picker {
    pub mode: Mode,
    directory: PathBuf,
    path: String,
    entries: Vec<(PathBuf, bool)>,
    error: Option<String>,
    focus: bool,
}
pub(crate) enum Choice {
    Wait,
    Cancel,
    Pick(PathBuf),
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
                        || path.extension().is_some_and(|s| {
                            s.eq_ignore_ascii_case(if self.mode == Mode::Memory {
                                "hex"
                            } else {
                                "circ"
                            })
                        })
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
        if self.mode != Mode::Save {
            self.path.clear();
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) -> Choice {
        ui.set_min_width(520.0);
        ui.heading(match self.mode {
            Mode::Open => tr("Open circuit"),
            Mode::Save => tr("Save circuit as"),
            Mode::Memory => tr("Import memory image"),
        });
        ui.label(tr(if self.mode == Mode::Memory {
            "Logisim .hex · v2.0 raw · replaces all words"
        } else {
            "Logisim-ITA compatible .circ · supported components only"
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
            ui.label(self.directory.to_string_lossy());
        });
        let mut picked = None;
        let mut enter = None;
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (path, dir) in &self.entries {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    let response = ui.selectable_label(
                        self.path == name,
                        format!("{} {name}", if *dir { "▸" } else { "·" }),
                    );
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
            });
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
        let mut submit = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::DARK_RED, error);
        }
        let mut cancel = false;
        ui.horizontal(|ui| {
            submit |= ui
                .button(if self.mode != Mode::Save {
                    tr("Open")
                } else {
                    tr("Save")
                })
                .clicked();
            cancel = ui.button(tr("Cancel")).clicked();
        });
        if cancel {
            return Choice::Cancel;
        }
        if submit {
            if self.path.trim().is_empty() {
                self.error = Some(tr("Enter a file name.").into());
                return Choice::Wait;
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
                self.enter(&path);
                return Choice::Wait;
            }
            if self.mode == Mode::Save
                && !path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("circ"))
            {
                path.as_mut_os_string().push(".circ");
            }
            return Choice::Pick(path);
        }
        Choice::Wait
    }
}

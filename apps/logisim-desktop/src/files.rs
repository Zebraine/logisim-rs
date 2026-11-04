use crate::{
    file_picker::{Choice, Mode, Picker},
    storage::{self, SaveMode, Storage},
};
use eframe::egui;
use logisim_sim::Simulator;
use logisim_ui::{DocumentView, FileAction, Workspace};
use std::path::{Path, PathBuf};

enum Dialog {
    Picker(Picker),
    Unsaved(FileAction),
    Replace { path: PathBuf, bytes: Vec<u8> },
    Message { title: String, body: String },
}
pub(crate) struct Files {
    pub storage: Storage,
    dialog: Option<Dialog>,
    after_save: Option<FileAction>,
    directory: PathBuf,
    pub allow_close: bool,
}
impl Default for Files {
    fn default() -> Self {
        Self {
            storage: Storage::new(),
            dialog: None,
            after_save: None,
            directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            allow_close: false,
        }
    }
}
impl Files {
    pub fn busy(&self) -> bool {
        self.dialog.is_some()
    }
    pub fn present(&self, circuit: &logisim_core::Circuit) -> DocumentView {
        DocumentView {
            title: self
                .storage
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into()),
            circuit_name: self.storage.name.clone(),
            dirty: self.storage.dirty(circuit),
            partial: self.storage.protected.is_some(),
        }
    }
    fn error(&mut self, error: String) {
        self.after_save = None;
        self.dialog = Some(Dialog::Message {
            title: "File operation failed".into(),
            body: error,
        });
    }
    pub fn load(&mut self, path: &Path, workspace: &mut Workspace, sim: &mut Simulator) {
        match self.storage.load(path) {
            Ok((circuit, warnings)) => {
                self.directory = self
                    .storage
                    .path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .unwrap_or(Path::new("."))
                    .to_path_buf();
                workspace.replace_circuit(circuit);
                *sim = Simulator::default();
                if !warnings.is_empty() {
                    self.dialog = Some(Dialog::Message {
                        title: "Imported supported content only".into(),
                        body: format!(
                            "The original file is protected. Use Save As to write the supported main-circuit subset to a different file.\n\n{}",
                            warnings.join("\n")
                        ),
                    });
                }
            }
            Err(e) => self.error(format!("{}\n\n{e}", path.display())),
        }
    }
    pub fn request(
        &mut self,
        action: FileAction,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut Simulator,
    ) {
        if self.busy() {
            return;
        }
        workspace.finish_edit();
        if matches!(
            action,
            FileAction::New | FileAction::Open | FileAction::Close
        ) && self.storage.dirty(workspace.circuit())
        {
            self.dialog = Some(Dialog::Unsaved(action));
        } else {
            self.execute(action, ctx, workspace, sim);
        }
    }
    fn picker(&mut self, mode: Mode) {
        let suggested = if mode == Mode::Save {
            self.storage
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled.circ".into())
        } else {
            String::new()
        };
        self.dialog = Some(Dialog::Picker(Picker::new(
            mode,
            self.directory.clone(),
            &suggested,
        )));
    }
    fn execute(
        &mut self,
        action: FileAction,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut Simulator,
    ) {
        match action {
            FileAction::New => {
                self.storage = Storage::new();
                workspace.replace_circuit(Default::default());
                *sim = Simulator::default();
            }
            FileAction::Open => self.picker(Mode::Open),
            FileAction::SaveAs => self.picker(Mode::Save),
            FileAction::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            FileAction::Save => {
                if let Some(path) = self
                    .storage
                    .path
                    .clone()
                    .filter(|_| self.storage.protected.is_none())
                {
                    self.save(&path, SaveMode::Current, ctx, workspace, sim);
                } else {
                    self.picker(Mode::Save);
                }
            }
        }
    }
    fn save(
        &mut self,
        path: &Path,
        mode: SaveMode<'_>,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut Simulator,
    ) {
        match self.storage.save(workspace.circuit(), path, mode) {
            Ok(()) => {
                if let Some(parent) = path.parent() {
                    self.directory = parent.to_path_buf();
                }
                if let Some(action) = self.after_save.take() {
                    self.execute(action, ctx, workspace, sim);
                }
            }
            Err(e) => self.error(format!("{}\n\n{e}", path.display())),
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, workspace: &mut Workspace, sim: &mut Simulator) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut choice = Choice::Wait;
        let mut answer = None;
        let response =
            egui::Modal::new(egui::Id::unique("file-dialog")).show(ctx, |ui| match &mut dialog {
                Dialog::Picker(picker) => choice = picker.show(ui),
                Dialog::Unsaved(_) => {
                    ui.heading("Save changes?");
                    ui.label("This circuit has unsaved changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            answer = Some(0);
                        }
                        if ui.button("Discard changes").clicked() {
                            answer = Some(1);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(2);
                        }
                    });
                }
                Dialog::Replace { path, .. } => {
                    ui.heading("Replace existing file?");
                    ui.label(path.to_string_lossy());
                    ui.horizontal(|ui| {
                        if ui.button("Replace").clicked() {
                            answer = Some(0);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(2);
                        }
                    });
                }
                Dialog::Message { title, body } => {
                    ui.set_max_width(620.0);
                    ui.heading(title.as_str());
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show(ui, |ui| {
                            ui.label(body.as_str());
                        });
                    if ui.button("OK").clicked() {
                        answer = Some(2);
                    }
                }
            });
        if matches!(choice, Choice::Wait) && answer.is_none() && response.should_close() {
            choice = Choice::Cancel;
        }
        if matches!(choice, Choice::Cancel) || answer == Some(2) {
            self.after_save = None;
            return;
        }
        match (dialog, choice, answer) {
            (Dialog::Unsaved(action), _, Some(0)) => {
                self.after_save = Some(action);
                self.execute(FileAction::Save, ctx, workspace, sim);
            }
            (Dialog::Unsaved(action), _, Some(1)) => self.execute(action, ctx, workspace, sim),
            (Dialog::Replace { path, bytes }, _, Some(0)) => {
                self.save(&path, SaveMode::Selected(Some(&bytes)), ctx, workspace, sim)
            }
            (Dialog::Picker(picker), Choice::Pick(path), _) => {
                if picker.mode == Mode::Open {
                    self.load(&path, workspace, sim);
                } else {
                    match storage::destination(&path) {
                        Ok(path) if self.storage.protected.as_ref() == Some(&path) => self.error(
                            "Choose a different file: this source contains unsupported content."
                                .into(),
                        ),
                        Ok(path) if path.exists() => match storage::read(&path) {
                            Ok(bytes) => self.dialog = Some(Dialog::Replace { path, bytes }),
                            Err(e) => self.error(e),
                        },
                        Ok(path) => self.save(&path, SaveMode::Selected(None), ctx, workspace, sim),
                        Err(e) => self.error(e),
                    }
                }
            }
            (dialog, _, _) => self.dialog = Some(dialog),
        }
        ctx.request_repaint();
    }
}

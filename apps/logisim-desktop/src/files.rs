use crate::{
    file_picker::{Choice, Mode, Picker},
    storage::{self, SaveMode, Storage},
};
use eframe::egui;
use logisim_sim::ProjectSimulator as Simulator;
use logisim_ui::{DocumentView, FileAction, MemoryImportTarget, Workspace};
use logisim_ui::{i18n::tr, tf};
use std::path::{Path, PathBuf};

enum Dialog {
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
    pub fn present(&self, workspace: &Workspace) -> DocumentView {
        DocumentView {
            untitled: self.storage.path.is_none(),
            title: self
                .storage
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into()),
            circuit_name: workspace.circuit_name().to_owned(),
            dirty: self.storage.dirty(workspace.project()),
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
                workspace.replace_project(circuit);
                *sim = Simulator::default();
                if !warnings.is_empty() {
                    self.dialog = Some(Dialog::Message {
                        title: "Imported supported content only".into(),
                        body: tf!(
                            "The original file is protected. Use Save As to write the supported project subset to a different file.\n\n{}",
                            warnings.join("\n")
                        ),
                    });
                }
            }
            Err(e) => self.error(format!("{}\n\n{}", path.display(), tr(&e))),
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
        ) && self.storage.dirty(workspace.project())
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
                workspace.replace_project(Default::default());
                *sim = Simulator::default();
            }
            FileAction::Open => self.picker(Mode::Open),
            FileAction::ImportMemory => {
                if let Some(target) = workspace.memory_import_target() {
                    self.dialog = Some(Dialog::MemoryPicker {
                        picker: Picker::new(Mode::Memory, self.directory.clone(), ""),
                        target: Box::new(target),
                    });
                }
            }
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
        match self.storage.save(workspace.project(), path, mode) {
            Ok(()) => {
                if let Some(parent) = path.parent() {
                    self.directory = parent.to_path_buf();
                }
                if let Some(action) = self.after_save.take() {
                    self.execute(action, ctx, workspace, sim);
                }
            }
            Err(e) => self.error(format!("{}\n\n{}", path.display(), tr(&e))),
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
                Dialog::Picker(picker) | Dialog::MemoryPicker { picker, .. } => {
                    choice = picker.show(ui)
                }
                Dialog::Unsaved(_) => {
                    ui.heading(tr("Save changes?"));
                    ui.label(tr("This project has unsaved changes."));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Save")).clicked() {
                            answer = Some(0);
                        }
                        if ui.button(tr("Discard changes")).clicked() {
                            answer = Some(1);
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            answer = Some(2);
                        }
                    });
                }
                Dialog::Replace { path, .. } => {
                    ui.heading(tr("Replace existing file?"));
                    ui.label(path.to_string_lossy());
                    ui.horizontal(|ui| {
                        if ui.button(tr("Replace")).clicked() {
                            answer = Some(0);
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            answer = Some(2);
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
            (Dialog::MemoryPicker { target, .. }, Choice::Pick(path), _) => {
                match crate::memory_import::load(&path, &target, workspace, sim) {
                    Ok(()) => {
                        if let Some(parent) = path.parent() {
                            self.directory = parent.to_path_buf();
                        }
                    }
                    Err(e) => self.error(format!("{}\n\n{}", path.display(), e)),
                }
            }
            (Dialog::Picker(picker), Choice::Pick(path), _) => {
                if picker.mode == Mode::Open {
                    self.load(&path, workspace, sim);
                } else {
                    match storage::destination(&path) {
                        Ok(path) if self.storage.protected.as_ref() == Some(&path) => self.error(
                            tr("Choose a different file: this source contains unsupported content.")
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

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::file_tests::Temp;
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};
    use logisim_core::{Circuit, Component, Kind, Point};

    #[derive(Default)]
    struct Harness {
        ctx: egui::Context,
        ws: Workspace,
        files: Files,
        sim: Simulator,
    }
    impl Harness {
        fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Pos2)> {
            self.sim
                .synchronize(self.ws.project(), self.ws.simulation_root());
            self.sim.advance(64);
            self.ws.set_simulation(crate::simulation::present_project(
                self.ws.circuit(),
                &self.sim,
                self.ws.simulation_path(),
            ));
            let mut out = self.ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 820.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.add_enabled_ui(!self.files.busy(), |ui| self.ws.show(ui));
                    for action in self.ws.take_file_actions() {
                        self.files
                            .request(action, &self.ctx, &mut self.ws, &mut self.sim);
                    }
                    self.files.show(&self.ctx, &mut self.ws, &mut self.sim);
                },
            );
            fn collect(shape: Shape, text: &mut Vec<(String, Pos2)>) {
                match shape {
                    Shape::Text(t) => {
                        text.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0))
                    }
                    Shape::Vec(v) => v.into_iter().for_each(|s| collect(s, text)),
                    _ => (),
                }
            }
            out.textures_delta.clear();
            let mut text = vec![];
            for shape in out.shapes {
                collect(shape.shape, &mut text);
            }
            text
        }
        fn click(&mut self, label: &str) {
            for _ in 0..2 {
                self.frame(vec![]);
            }
            let labels = self.frame(vec![]);
            let pos = labels
                .iter()
                .find(|(s, _)| s == label)
                .unwrap_or_else(|| panic!("Missing {label}: {labels:?}"))
                .1;
            for pressed in [true, false] {
                self.frame(vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ]);
            }
        }
        fn open_image(&mut self, path: &Path) {
            self.click("Import .hex…");
            assert!(matches!(
                self.files.dialog,
                Some(Dialog::MemoryPicker { .. })
            ));
            self.frame(vec![Event::Text(path.to_string_lossy().into_owned())]);
            self.frame(vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]);
            self.frame(vec![]);
        }
    }

    #[test]
    fn real_memory_picker_cancels_retries_and_preserves_document_file_and_ram() {
        let dir = Temp::new();
        let source = dir.path("source.circ");
        let image = dir.path("contents.HEX");
        let invalid = dir.path("bad.hex");
        std::fs::write(&image, "v2.0 raw\n12 AB 3*cd").unwrap();
        std::fs::write(&invalid, "v2.0 raw\n12 zz").unwrap();
        let mut circuit = Circuit::default();
        let mut c = Component::new(Kind::Rom, Point::new(200, 100));
        c.memory.address_bits = 4;
        c.memory.contents.fill(0, 16, 0xaa);
        let rom = circuit.add_component(c);
        let mut c = Component::new(Kind::Ram, Point::new(500, 100));
        c.memory.address_bits = 4;
        let ram = circuit.add_component(c);
        let mut h = Harness::default();
        logisim_ui::configure(&h.ctx);
        h.ws.replace_circuit(circuit);
        h.files
            .storage
            .save(h.ws.project(), &source, SaveMode::Current)
            .unwrap();
        h.files.directory = dir.0.clone();
        let original_disk = std::fs::read(&source).unwrap();
        let original = h.ws.project().clone();
        h.click("ROM 16 × 8");
        h.click("Edit contents…");
        h.click("Import .hex…");
        let labels = h.frame(vec![]);
        assert!(labels.iter().any(|(s, _)| s == "· contents.HEX"));
        assert!(!labels.iter().any(|(s, _)| s == "· source.circ"));
        h.click("Cancel");
        assert_eq!(h.ws.project(), &original);
        h.open_image(&invalid);
        assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
        assert_eq!(h.ws.project(), &original);
        h.click("OK");
        h.open_image(&image);
        assert!(!h.files.busy());
        assert_eq!(
            h.ws.circuit()
                .component(rom)
                .unwrap()
                .memory
                .contents
                .get(1),
            0xab
        );
        assert_eq!(
            h.ws.circuit()
                .component(rom)
                .unwrap()
                .memory
                .contents
                .get(15),
            0
        );
        assert_eq!(std::fs::read(&source).unwrap(), original_disk);
        assert!(h.files.storage.dirty(h.ws.project()));
        h.click("Close");
        h.files
            .storage
            .save(h.ws.project(), &source, SaveMode::Current)
            .unwrap();
        let saved = h.ws.project().clone();
        h.click("RAM 16 × 8");
        h.click("Edit contents…");
        h.open_image(&image);
        assert_eq!(
            h.sim
                .view(&[])
                .unwrap()
                .memory_contents(ram)
                .unwrap()
                .get(1),
            0xab
        );
        assert_eq!(h.ws.project(), &saved);
        assert!(!h.files.storage.dirty(h.ws.project()));
        h.open_image(&invalid);
        assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
        assert_eq!(
            h.sim
                .view(&[])
                .unwrap()
                .memory_contents(ram)
                .unwrap()
                .get(1),
            0xab
        );
        h.click("OK");
        let target = h.ws.memory_import_target().unwrap();
        h.click("Close");
        assert!(crate::memory_import::load(&image, &target, &mut h.ws, &mut h.sim).is_err());
        let (loaded, warnings) = Storage::new().load(&source).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            loaded
                .main_circuit()
                .circuit
                .component(rom)
                .unwrap()
                .memory
                .contents
                .get(1),
            0xab
        );
        assert_eq!(
            loaded
                .main_circuit()
                .circuit
                .component(ram)
                .unwrap()
                .memory
                .contents,
            Default::default()
        );
        h.sim.reset();
        h.sim.advance(64);
        assert_eq!(
            h.sim
                .view(&[])
                .unwrap()
                .memory_contents(ram)
                .unwrap()
                .get(1),
            0
        );
    }
}

//! File workflow: unsaved actions, protected imports and transactional saves.
mod dialog;
#[cfg(test)]
mod tests;

use dialog::{Dialog, Response};

use crate::simulation::Host;
use crate::{
    file_picker::{Mode, Picker},
    storage::{self, SaveMode, Storage},
};
use eframe::egui;
use logisim_ui::{DocumentView, FileAction, Workspace};
use logisim_ui::{i18n::tr, tf};
use std::path::{Path, PathBuf};

pub(crate) struct Files {
    pub plugins: logisim_plugin::Registry,
    pub storage: Storage,
    dialog: Option<Dialog>,
    after_save: Option<FileAction>,
    directory: PathBuf,
    pub allow_close: bool,
}
impl Default for Files {
    fn default() -> Self {
        Self {
            plugins: Default::default(),
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
    fn import_warnings(&mut self, warnings: Vec<String>) {
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
    fn resolve_plugins(
        &mut self,
        workspace: &mut Workspace,
        sim: &mut impl Host,
        warnings: Vec<String>,
    ) {
        let resolution = crate::plugins::Resolution::new(
            workspace.project(),
            crate::plugins::base(self.storage.path.as_deref()),
            warnings,
        );
        self.plugins = resolution.registry();
        workspace.set_plugin_catalogue(self.plugins.manifests().cloned().collect());
        sim.set_plugins(self.plugins.clone());
        if resolution.missing() {
            self.dialog = Some(Dialog::Plugins(Box::new(resolution)));
        } else {
            self.import_warnings(resolution.warnings);
        }
    }
    pub fn load(&mut self, path: &Path, workspace: &mut Workspace, sim: &mut impl Host) {
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
                sim.new_document();
                self.resolve_plugins(workspace, sim, warnings);
            }
            Err(e) => self.error(format!("{}\n\n{}", path.display(), tr(&e))),
        }
    }
    pub fn request(
        &mut self,
        action: FileAction,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
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
        sim: &mut impl Host,
    ) {
        match action {
            FileAction::New => {
                self.plugins = Default::default();
                workspace.set_plugin_catalogue(vec![]);
                self.storage = Storage::new();
                workspace.replace_project(Default::default());
                sim.new_document();
            }
            FileAction::Open => self.picker(Mode::Open),
            FileAction::ImportLibrary => self.picker(Mode::Library),
            FileAction::LoadPlugin => self.picker(Mode::Plugin),
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
        sim: &mut impl Host,
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
    pub fn show(&mut self, ctx: &egui::Context, workspace: &mut Workspace, sim: &mut impl Host) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let response = dialog.show(ctx);
        self.respond(dialog, response, ctx, workspace, sim);
    }
    fn respond(
        &mut self,
        dialog: Dialog,
        response: Response,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        if matches!(response, Response::Cancel) {
            self.after_save = None;
            match dialog {
                Dialog::PluginPicker { resolution, .. } => {
                    self.dialog = Some(Dialog::Plugins(resolution))
                }
                Dialog::Plugins(resolution) => self.import_warnings(resolution.warnings),
                _ => {}
            }
            return;
        }
        match (dialog, response) {
            (Dialog::Plugins(mut resolution), Response::ConfirmPlugins) => {
                for i in 0..resolution.rows.len() {
                    resolution.check(i);
                }
                let sources = resolution.sources(&workspace.project().plugin_sources);
                workspace.set_plugin_sources(sources);
                self.plugins = resolution.registry();
                workspace.set_plugin_catalogue(self.plugins.manifests().cloned().collect());
                sim.set_plugins(self.plugins.clone());
                self.import_warnings(resolution.warnings);
            }
            (Dialog::Plugins(mut resolution), Response::CheckPlugin(index)) => {
                resolution.check(index);
                self.dialog = Some(Dialog::Plugins(resolution));
            }
            (Dialog::Plugins(resolution), Response::BrowsePlugin(index)) => {
                self.dialog = Some(Dialog::PluginPicker {
                    picker: Picker::new(Mode::Plugin, resolution.base.clone(), ""),
                    resolution,
                    index,
                });
            }
            (
                Dialog::PluginPicker {
                    mut resolution,
                    index,
                    ..
                },
                Response::Pick(path),
            ) => {
                resolution.rows[index].location = path.to_string_lossy().into_owned();
                resolution.check(index);
                self.dialog = Some(Dialog::Plugins(resolution));
            }
            (Dialog::Unsaved(action), Response::Save) => {
                self.after_save = Some(action);
                self.execute(FileAction::Save, ctx, workspace, sim);
            }
            (Dialog::Unsaved(action), Response::Discard) => {
                self.execute(action, ctx, workspace, sim)
            }
            (Dialog::Replace { path, bytes }, Response::Replace) => {
                self.save(&path, SaveMode::Selected(Some(&bytes)), ctx, workspace, sim)
            }
            (Dialog::MemoryPicker { target, .. }, Response::Pick(path)) => {
                match crate::memory_import::load(&path, &target, workspace, sim) {
                    Ok(()) => {
                        if let Some(parent) = path.parent() {
                            self.directory = parent.to_path_buf();
                        }
                    }
                    Err(e) => self.error(format!("{}\n\n{}", path.display(), e)),
                }
            }
            (Dialog::Picker(picker), Response::Pick(path)) => {
                if picker.mode == Mode::Plugin {
                    match storage::read_file(&path, "Choose a regular .wasm file.").and_then(
                        |bytes| {
                            let plugin = logisim_plugin::Plugin::load(&bytes)?;
                            let manifest = plugin.manifest();
                            for c in workspace
                                .project()
                                .circuits
                                .iter()
                                .flat_map(|d| &d.circuit.components)
                                .filter_map(|c| c.plugin.as_ref())
                                .filter(|c| {
                                    c.plugin == manifest.id && c.version == manifest.version
                                })
                            {
                                plugin.check_component(c)?;
                            }
                            let path = path.canonicalize().map_err(|e| e.to_string())?;
                            let source = crate::plugins::located(
                                &manifest.id,
                                &manifest.version,
                                &path,
                                &crate::plugins::base(self.storage.path.as_deref()),
                            );
                            let mut project = workspace.project().clone();
                            project.plugin_sources.retain(|s| {
                                s.plugin != source.plugin || s.version != source.version
                            });
                            project.plugin_sources.push(source);
                            project.validate()?;
                            self.plugins.insert(plugin)?;
                            workspace.set_plugin_sources(project.plugin_sources);
                            Ok(())
                        },
                    ) {
                        Ok(()) => {
                            workspace
                                .set_plugin_catalogue(self.plugins.manifests().cloned().collect());
                            sim.set_plugins(self.plugins.clone());
                            if let Some(parent) = path.parent() {
                                self.directory = parent.to_path_buf();
                            }
                        }
                        Err(error) => self.error(format!("{}\n\n{error}", path.display())),
                    }
                } else if picker.mode == Mode::Library {
                    match crate::library_import::load(&path, workspace.project()) {
                        Ok(imported) => {
                            workspace.apply_library_import(imported);
                            self.resolve_plugins(workspace, sim, vec![]);
                            if let Some(parent) = path.parent() {
                                self.directory = parent.to_path_buf();
                            }
                        }
                        Err(error) => self.error(format!("{}\n\n{error}", path.display())),
                    }
                } else if picker.mode == Mode::Open {
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
            (dialog, _) => self.dialog = Some(dialog),
        }
        ctx.request_repaint();
    }
}

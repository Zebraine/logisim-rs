//! File workflow: unsaved actions, protected imports and transactional saves.
mod capture;
mod dialog;
mod image;
mod picker;
mod print;
mod settings;
#[cfg(test)]
mod tests;

use dialog::{Dialog, Response};
use picker::{Mode, Picker};

use crate::plugins::Resolution;
use crate::simulation::Host;
use crate::storage::{self, SaveMode, Storage};
use eframe::egui;
use logisim_ui::{DocumentView, FileAction, Workspace};
use logisim_ui::{i18n::tr, tf};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) enum NextDocument {
    New,
    Browse,
    Recent(PathBuf),
    Close,
}

enum ResolutionReason {
    Document(Vec<String>),
    Repair,
}

pub(crate) struct Files {
    engine: logisim_plugin::WasmEngine,
    engine_warning_pending: bool,
    pub plugins: logisim_plugin::Registry,
    pub storage: Storage,
    pub autosave: crate::recovery::Autosave,
    dialog: Option<Dialog>,
    capture: Option<capture::Capture>,
    after_save: Option<NextDocument>,
    directory: PathBuf,
    pub allow_close: bool,
    recovery_page_open: bool,
}
impl Default for Files {
    fn default() -> Self {
        Self {
            engine: Default::default(),
            engine_warning_pending: false,
            plugins: Default::default(),
            storage: Storage::new(),
            autosave: Default::default(),
            dialog: None,
            capture: None,
            after_save: None,
            directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            allow_close: false,
            recovery_page_open: false,
        }
    }
}
impl Files {
    pub fn for_desktop(directory: Option<PathBuf>, workspace: &mut Workspace) -> Self {
        use logisim_plugin::WasmEngine;
        use logisim_ui::WasmEnginePreference;
        let requested = workspace.preferences().wasm_engine;
        let available = WasmEngine::Wasmtime.available();
        let experimental = requested == WasmEnginePreference::Wasmtime;
        let engine = if experimental && available {
            WasmEngine::Wasmtime
        } else {
            WasmEngine::Wasmi
        };
        workspace.set_plugin_runtime(
            match engine {
                WasmEngine::Wasmi => WasmEnginePreference::Wasmi,
                WasmEngine::Wasmtime => WasmEnginePreference::Wasmtime,
            },
            available,
        );
        Self {
            engine,
            engine_warning_pending: experimental && !available,
            ..Self::with_recovery(directory)
        }
    }

    pub fn with_recovery(directory: Option<PathBuf>) -> Self {
        Self {
            autosave: crate::recovery::Autosave::at(directory),
            ..Default::default()
        }
    }
    pub fn present_recovery(&mut self, w: &mut Workspace) {
        let open = w.recovery_settings_open();
        if open && !self.recovery_page_open {
            self.autosave.refresh();
        }
        self.recovery_page_open = open;
        self.autosave.publish(w);
    }
    pub fn busy(&self) -> bool {
        self.dialog.is_some() || self.capture.is_some()
    }
    pub fn capturing_image(&self) -> bool {
        self.capture.is_some()
    }
    pub fn suspends_simulation(&self) -> bool {
        self.dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.is_settings() && !dialog.is_image())
    }
    pub fn present(&self, workspace: &Workspace) -> DocumentView {
        DocumentView {
            path: self.storage.path.clone(),
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
        reason: ResolutionReason,
    ) {
        let repair = matches!(reason, ResolutionReason::Repair);
        let warnings = match reason {
            ResolutionReason::Document(warnings) => warnings,
            ResolutionReason::Repair => Vec::new(),
        };
        let resolution = Resolution::new(
            self.engine,
            workspace.project(),
            crate::plugins::base(self.storage.path.as_deref()),
            workspace.plugin_fallback_directory.as_deref(),
            warnings,
        );
        let needs_confirmation =
            resolution.missing() || (repair && resolution.rows.iter().any(|r| r.from_directory));
        // A new document needs its own registry immediately. A repair can be cancelled.
        if !repair || !needs_confirmation {
            self.activate_plugins(&resolution, workspace, sim);
        }
        if needs_confirmation {
            self.dialog = Some(Dialog::Plugins(Box::new(resolution)));
        } else {
            self.import_warnings(resolution.warnings);
        }
    }
    fn activate_plugins(
        &mut self,
        resolution: &Resolution,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        self.plugins = resolution.registry();
        workspace.set_fallback_plugin_sources(
            resolution
                .rows
                .iter()
                .filter(|row| row.from_directory)
                .map(|row| row.source.clone())
                .collect(),
        );
        workspace.set_plugin_catalogue(self.plugins.libraries().collect());
        sim.set_plugins(self.plugins.clone());
    }
    pub fn startup(
        &mut self,
        cli: Option<PathBuf>,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        if self.autosave.pending() {
            self.dialog = Some(Dialog::Recovery {
                cli,
                view: self.autosave.view(),
            });
        } else {
            self.open_startup(cli, workspace, sim);
        }
    }
    fn open_startup(
        &mut self,
        cli: Option<PathBuf>,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        if let Some(path) = cli {
            self.load(&path, workspace, sim);
        } else if workspace.preferences().workspace.reopen_last
            && let Some(path) = workspace.recent().first().map(PathBuf::from)
            && !self.load(&path, workspace, sim)
        {
            crate::preferences::recent::remove(workspace, &path);
        }
    }
    pub fn load(&mut self, path: &Path, workspace: &mut Workspace, sim: &mut impl Host) -> bool {
        if storage::destination(path).is_ok_and(|p| self.autosave.reserved(&p)) {
            self.error(tr("Open the snapshot through recovery.").into());
            return false;
        }
        match self.storage.load(path) {
            Ok((circuit, warnings)) => {
                self.autosave.finish_document();
                crate::preferences::recent::record(
                    workspace,
                    self.storage.path.as_deref().expect("loaded document path"),
                );
                self.directory = self
                    .storage
                    .path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .unwrap_or(Path::new("."))
                    .to_path_buf();
                workspace.replace_project(circuit);
                sim.new_document();
                self.resolve_plugins(workspace, sim, ResolutionReason::Document(warnings));
                true
            }
            Err(e) => {
                if matches!(path.try_exists(), Ok(false)) {
                    crate::preferences::recent::remove(workspace, path);
                }
                self.error(format!("{}\n\n{}", path.display(), tr(&e)));
                false
            }
        }
    }
    pub fn request(
        &mut self,
        action: FileAction,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        if action == FileAction::Close && matches!(self.dialog, Some(Dialog::Recovery { .. })) {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let clear_recovery = matches!(action, FileAction::ClearRecovery { .. });
        if self.busy() && !clear_recovery {
            return;
        }
        if !clear_recovery
            && !matches!(
                action,
                FileAction::ClearRecent
                    | FileAction::Settings(_)
                    | FileAction::ChoosePluginFallback
                    | FileAction::Print
                    | FileAction::ExportImage
            )
        {
            workspace.finish_edit();
        }
        match action {
            FileAction::New => self.navigate(NextDocument::New, ctx, workspace, sim),
            FileAction::Open => self.navigate(NextDocument::Browse, ctx, workspace, sim),
            FileAction::Close => self.navigate(NextDocument::Close, ctx, workspace, sim),
            FileAction::OpenRecent(i) => {
                if let Some(path) = workspace.recent().get(i).map(PathBuf::from) {
                    self.navigate(NextDocument::Recent(path), ctx, workspace, sim);
                }
            }
            FileAction::ImportLibrary => self.picker(Mode::Library),
            FileAction::LoadPlugin => self.picker(Mode::Plugin),
            FileAction::ResolvePlugins => {
                if workspace.needs_plugin_resolution() {
                    self.resolve_plugins(workspace, sim, ResolutionReason::Repair);
                }
            }
            FileAction::ImportPlugin { component, import } => {
                if let Some(target) = workspace.plugin_import_target(component, import) {
                    self.dialog = Some(Dialog::PluginImportPicker {
                        picker: Picker::new(Mode::Memory, self.directory.clone(), ""),
                        target: Box::new(target),
                    });
                }
            }
            FileAction::ImportMemory => {
                if let Some(target) = workspace.memory_import_target() {
                    self.dialog = Some(Dialog::MemoryPicker {
                        picker: Picker::new(Mode::Memory, self.directory.clone(), ""),
                        target: Box::new(target),
                    });
                }
            }
            FileAction::ChoosePluginFallback => {
                let dir = workspace
                    .plugin_fallback_directory
                    .as_ref()
                    .unwrap_or(&self.directory);
                self.dialog = Some(Dialog::Picker(Picker::new(
                    Mode::Directory,
                    dir.clone(),
                    "",
                )));
            }
            FileAction::Settings(action) => self.picker(Mode::Settings(action)),
            FileAction::Print => {
                self.capture = Some(capture::Capture::new(capture::Target::Print, ctx))
            }
            FileAction::ExportImage => self.picker(Mode::ExportImage),
            FileAction::SaveAs => self.picker(Mode::Save),
            FileAction::ClearRecovery { disabled_autosave } => {
                let result = if self.busy() {
                    Err("Recovery storage is busy.".into())
                } else {
                    self.autosave.clear_from_settings()
                };
                if let Err(error) = result {
                    if disabled_autosave {
                        let mut settings = workspace.preferences();
                        settings.files.autosave = true;
                        workspace.apply_settings(settings);
                    }
                    self.autosave.report(error);
                }
                self.autosave.publish(workspace);
                ctx.request_repaint();
            }
            FileAction::ClearRecent => workspace.set_recent(Vec::new()),
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
    fn navigate(
        &mut self,
        target: NextDocument,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        if self.storage.dirty(workspace.project()) {
            self.dialog = Some(Dialog::Unsaved(target));
        } else {
            self.proceed(target, ctx, workspace, sim);
        }
    }
    fn proceed(
        &mut self,
        target: NextDocument,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        match target {
            NextDocument::New => {
                self.autosave.finish_document();
                self.plugins = Default::default();
                workspace.set_plugin_catalogue(vec![]);
                self.storage = Storage::new();
                workspace.replace_project(Default::default());
                sim.new_document();
            }
            NextDocument::Browse => self.picker(Mode::Open),
            NextDocument::Recent(path) => {
                self.load(&path, workspace, sim);
            }
            NextDocument::Close => {
                self.autosave.finish_document();
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
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
        } else if mode == Mode::ExportImage {
            let stem = self.storage.path.as_deref().and_then(Path::file_stem);
            format!(
                "{}.png",
                stem.unwrap_or("circuit".as_ref()).to_string_lossy()
            )
        } else if mode == Mode::Settings(logisim_ui::SettingsFileAction::Export) {
            "logisim-settings.json".into()
        } else {
            String::new()
        };
        self.dialog = Some(Dialog::Picker(Picker::new(
            mode,
            self.directory.clone(),
            &suggested,
        )));
    }
    fn save(
        &mut self,
        path: &Path,
        mode: SaveMode<'_>,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
    ) {
        let save_as = matches!(mode, SaveMode::Selected(_));
        if storage::destination(path).is_ok_and(|p| self.autosave.reserved(&p)) {
            self.error(tr("Recovery files cannot be used as a project destination.").into());
            return;
        }
        match self.storage.save(
            workspace.project(),
            path,
            mode,
            workspace.preferences().files.backup_before_save,
        ) {
            Ok(()) => {
                self.autosave.finish_document();
                if save_as {
                    crate::preferences::recent::record(
                        workspace,
                        self.storage.path.as_deref().expect("saved document path"),
                    );
                }
                if let Some(parent) = path.parent() {
                    self.directory = parent.to_path_buf();
                }
                if let Some(action) = self.after_save.take() {
                    self.proceed(action, ctx, workspace, sim);
                }
            }
            Err(e) => self.error(format!("{}\n\n{}", path.display(), tr(&e))),
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        sim: &mut impl Host,
        preferences: &mut crate::preferences::Preferences,
    ) {
        if let Some(mut capture) = self.capture.take() {
            if matches!(capture.target, capture::Target::Print)
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                return;
            }
            if let Some(result) = capture.poll(ctx, workspace.canvas_rect()) {
                self.captured(ctx, capture.target, result);
            } else {
                self.capture = Some(capture);
            }
            return;
        }
        if self.dialog.is_none() && std::mem::take(&mut self.engine_warning_pending) {
            self.dialog = Some(Dialog::SettingsMessage {
                title: "Wasm plugin engine",
                body: tr("This build does not include Wasmtime. Plugins will run with wasmi for this session.").into(),
            });
        }
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let response = dialog.show(ctx);
        if matches!(dialog, Dialog::PrintPreview(_) | Dialog::Printing(_)) {
            self.respond_to_print(dialog, response, ctx, workspace.language());
        } else if dialog.is_image() {
            self.respond_to_image(dialog, response, preferences, ctx);
        } else if dialog.is_settings() {
            self.respond_to_settings(dialog, response, preferences, workspace);
        } else {
            self.respond(dialog, response, ctx, workspace, sim);
        }
    }
    fn captured(
        &mut self,
        ctx: &egui::Context,
        target: capture::Target,
        pixels: Result<egui::ColorImage, String>,
    ) {
        match target {
            capture::Target::Export(target) => {
                self.image_result(&target.path, pixels.and_then(|image| target.write(&image)))
            }
            capture::Target::Print => match pixels {
                Ok(image) => {
                    self.dialog = Some(Dialog::PrintPreview(print::Preview::new(ctx, image)))
                }
                Err(error) => self.print_result(Err(tr(&error).to_owned()).into()),
            },
        }
        ctx.request_repaint();
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
                d @ Dialog::Recovery { .. } => self.dialog = Some(d),
                Dialog::PluginPicker { resolution, .. } => {
                    self.dialog = Some(Dialog::Plugins(resolution))
                }
                Dialog::Plugins(resolution) => self.import_warnings(resolution.warnings),
                _ => {}
            }
            return;
        }
        match (dialog, response) {
            (Dialog::Recovery { cli, .. }, Response::Recover) => {
                match self.autosave.recover(&mut self.storage) {
                    Ok(project) => {
                        if let Some(parent) = self.storage.path.as_deref().and_then(Path::parent) {
                            self.directory = parent.into();
                        }
                        workspace.replace_project(project);
                        sim.new_document();
                        self.resolve_plugins(
                            workspace,
                            sim,
                            ResolutionReason::Document(Vec::new()),
                        );
                    }
                    Err(_) => {
                        self.dialog = Some(Dialog::Recovery {
                            cli,
                            view: self.autosave.view(),
                        });
                    }
                }
            }
            (Dialog::Recovery { cli, .. }, Response::Discard) => {
                if let Err(error) = self.autosave.discard() {
                    self.autosave.report(error);
                }
                self.open_startup(cli, workspace, sim);
            }
            (Dialog::Plugins(mut resolution), Response::ConfirmPlugins) => {
                for i in 0..resolution.rows.len() {
                    resolution.check(i);
                }
                let sources = resolution.sources(&workspace.project().plugin_sources);
                workspace.set_plugin_sources(sources);
                self.activate_plugins(&resolution, workspace, sim);
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
                self.request(FileAction::Save, ctx, workspace, sim);
            }
            (Dialog::Unsaved(action), Response::Discard) => {
                self.proceed(action, ctx, workspace, sim)
            }
            (Dialog::Replace { path, bytes }, Response::Replace) => {
                self.save(&path, SaveMode::Selected(Some(&bytes)), ctx, workspace, sim)
            }
            (Dialog::PluginImportPicker { target, .. }, Response::Pick(path)) => {
                match crate::memory_import::load_plugin(&path, &target, workspace) {
                    Ok(()) => {
                        if let Some(parent) = path.parent() {
                            self.directory = parent.to_path_buf();
                        }
                    }
                    Err(e) => self.error(format!("{}\n\n{}", path.display(), e)),
                }
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
                            let plugin =
                                logisim_plugin::Plugin::load_with_engine(&bytes, self.engine)?;
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
                            workspace.set_plugin_catalogue(self.plugins.libraries().collect());
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
                            self.resolve_plugins(
                                workspace,
                                sim,
                                ResolutionReason::Document(vec![]),
                            );
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

//! Atomic, versioned application preferences. Invalid input is read-only until repaired.
pub(crate) mod recent;
mod schema;
use crate::window::WindowState;
use logisim_ui::WorkspaceLayout;
use logisim_ui::{Language, Settings, SimulationMode, Workspace};
use schema::{Config, PluginSettings};
use std::time::{Duration, Instant};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(crate) fn canonical_plugin_directory(path: &Path) -> std::io::Result<PathBuf> {
    use std::io::{Error, ErrorKind};
    if !path.is_absolute() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Choose an absolute directory.",
        ));
    }
    let dir = path.canonicalize()?;
    if !dir.metadata()?.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Choose an existing directory.",
        ));
    }
    if dir.to_str().is_none() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Choose a directory with a UTF-8 path.",
        ));
    }
    Ok(dir)
}

impl PluginSettings {
    fn directory(&self) -> Option<&Path> {
        self.fallback_directory.as_deref().map(Path::new)
    }

    fn from_workspace(workspace: &Workspace) -> Self {
        Self {
            fallback_directory: workspace
                .plugin_fallback_directory
                .as_deref()
                .map(|p| p.to_string_lossy().into_owned()),
        }
    }
}

#[derive(Default)]
struct Loaded {
    config: Config,
    json: Option<Vec<u8>>,
}
struct Issue {
    path: PathBuf,
    message: &'static str,
}
impl Loaded {
    fn read(path: &Path) -> Result<Self, Issue> {
        let read = |path: &Path| match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e)
                if e.kind() == ErrorKind::NotFound
                    && fs::symlink_metadata(path)
                        .is_err_and(|e| e.kind() == ErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(_) => Err(Issue {
                path: path.to_owned(),
                message: "Cannot read the configuration file.",
            }),
        };
        if let Some(bytes) = read(path)? {
            let config = schema::decode(&bytes).map_err(|message| Issue {
                path: path.to_owned(),
                message,
            })?;
            return Ok(Self {
                config,
                json: Some(bytes),
            });
        }
        let old = path.with_file_name("preferences");
        let Some(bytes) = read(&old)? else {
            return Ok(Self::default());
        };
        let values = std::str::from_utf8(&bytes)
            .ok()
            .and_then(legacy)
            .ok_or(Issue {
                path: old,
                message: "Unknown or invalid settings.",
            })?;
        Ok(Self {
            config: Config {
                settings: values,
                ..Default::default()
            },
            json: None,
        })
    }
}
/// A portable settings file that passed the same schema validation as local preferences.
pub(crate) struct ValidatedSettings(Settings);

impl ValidatedSettings {
    pub(crate) fn requests_wasmtime(&self) -> bool {
        self.0.wasm_engine == logisim_ui::WasmEnginePreference::Wasmtime
    }
}

pub(crate) struct Preferences {
    path: Option<PathBuf>,
    loaded: Loaded,
    blocked: bool,
    window: WindowState,
    panels: WorkspaceLayout,
    layout_due: Option<Instant>,
}
impl Preferences {
    pub fn load(workspace: &mut Workspace) -> Self {
        Self::at(config_path(), workspace)
    }
    pub(crate) fn at(path: Option<PathBuf>, workspace: &mut Workspace) -> Self {
        let mut state = Self {
            path,
            loaded: Loaded::default(),
            blocked: false,
            window: WindowState::default(),
            panels: WorkspaceLayout::default(),
            layout_due: None,
        };
        if let Some(loaded) = state.read_settings(workspace) {
            state.loaded = loaded;
        }
        workspace.apply_settings(state.loaded.config.settings);
        workspace.plugin_fallback_directory =
            state.loaded.config.plugins.directory().map(Path::to_owned);
        workspace.set_recent(state.loaded.config.recent.clone());
        state.window = state.loaded.config.window;
        state.panels = state.loaded.config.panels;
        if workspace.preferences().workspace.restore_panels {
            workspace.set_layout(state.panels);
        }
        state
    }
    pub(crate) fn is_config_path(&self, destination: &Path) -> bool {
        self.path.as_deref().is_some_and(|path| {
            crate::storage::destination(path).is_ok_and(|config| config == destination)
        })
    }

    pub(crate) fn export_settings(workspace: &Workspace) -> Vec<u8> {
        schema::encode_settings(workspace.preferences())
    }

    pub(crate) fn validate_import(bytes: &[u8]) -> Result<ValidatedSettings, String> {
        schema::decode_settings(bytes).map(ValidatedSettings)
    }

    /// Commit the exact settings approved in the dialog before publishing them to the UI.
    pub(crate) fn import_settings(
        &mut self,
        ValidatedSettings(settings): ValidatedSettings,
        workspace: &mut Workspace,
    ) -> Result<(), String> {
        if self.blocked {
            return Err(
                "Repair the configuration file and retry before importing settings.".into(),
            );
        }
        self.persist(Config {
            plugins: PluginSettings::from_workspace(workspace),
            settings,
            recent: workspace.recent().to_vec(),
            window: self.window,
            panels: self.panels,
        })?;
        self.layout_due = None;
        workspace.apply_settings(settings);
        Ok(())
    }

    pub fn recovery_directory(&self) -> Option<PathBuf> {
        self.path
            .as_ref()
            .map(|path| path.with_file_name("recovery"))
    }
    pub fn initial_window(&self, workspace: &Workspace) -> WindowState {
        if workspace.preferences().workspace.restore_window {
            self.window
        } else {
            WindowState::default()
        }
    }
    pub fn observe_layout(
        &mut self,
        window: Option<WindowState>,
        panels: Option<WorkspaceLayout>,
        now: Instant,
    ) {
        let window = window.unwrap_or(self.window);
        let panels = panels.unwrap_or(self.panels);
        if (window, panels) != (self.window, self.panels) {
            self.window = window;
            self.panels = panels;
            self.layout_due = Some(now + Duration::from_millis(500));
        }
    }
    pub fn flush_layout(&mut self) {
        self.layout_due = Some(Instant::now());
    }
    fn read_settings(&mut self, workspace: &mut Workspace) -> Option<Loaded> {
        let Some(path) = self.path.as_ref() else {
            self.blocked = true;
            workspace
                .set_settings_error(Some("User configuration directory is unavailable.".into()));
            return None;
        };
        match Loaded::read(path) {
            Ok(loaded) => {
                self.blocked = false;
                workspace.set_config_path(Some(path.display().to_string()));
                workspace.set_settings_error(None);
                Some(loaded)
            }
            Err(issue) => {
                self.blocked = true;
                workspace.set_config_path(Some(issue.path.display().to_string()));
                workspace.set_settings_error(Some(issue.message.into()));
                None
            }
        }
    }
    /// Re-read the file before retrying a write, preserving independent disk edits.
    fn retry(&mut self, workspace: &mut Workspace) {
        let baseline = self.loaded.config.settings;
        let directory_changed = workspace.plugin_fallback_directory.as_deref()
            != self.loaded.config.plugins.directory();
        let recent_changed = workspace.recent() != self.loaded.config.recent;
        if self.path.is_none() {
            self.path = config_path();
        }
        let local_window = self.window != self.loaded.config.window;
        let local_panels = self.panels != self.loaded.config.panels;
        let Some(loaded) = self.read_settings(workspace) else {
            return;
        };
        let Ok(merged) = loaded
            .config
            .settings
            .with_changes(baseline, workspace.preferences())
        else {
            self.blocked = true;
            workspace.set_settings_error(Some(
                "A local shortcut conflicts with a binding on disk. Change the binding and retry."
                    .into(),
            ));
            return;
        };
        if !local_window {
            self.window = loaded.config.window;
        }
        if !local_panels {
            self.panels = loaded.config.panels;
        }
        if !directory_changed {
            workspace.plugin_fallback_directory =
                loaded.config.plugins.directory().map(Path::to_owned);
        }
        if !recent_changed {
            workspace.set_recent(loaded.config.recent.clone());
        }
        workspace.apply_settings(merged);
        self.loaded = loaded;
    }
    pub fn synchronize(&mut self, workspace: &mut Workspace) {
        if workspace.take_settings_retry() {
            self.retry(workspace);
        }
        let values = workspace.preferences();
        if values.workspace.recent_limit != self.loaded.config.settings.workspace.recent_limit
            || workspace.recent() != self.loaded.config.recent
        {
            recent::truncate(workspace);
        }
        let layout_due = self.layout_due.is_some_and(|at| Instant::now() >= at);
        if self.blocked
            || !values.files.valid_interval()
            || (values == self.loaded.config.settings
                && workspace.plugin_fallback_directory.as_deref()
                    == self.loaded.config.plugins.directory()
                && workspace.recent() == self.loaded.config.recent
                && !(layout_due
                    && (self.window != self.loaded.config.window
                        || self.panels != self.loaded.config.panels)))
        {
            return;
        }
        let config = Config {
            plugins: PluginSettings::from_workspace(workspace),
            window: if layout_due {
                self.window
            } else {
                self.loaded.config.window
            },
            panels: if layout_due {
                self.panels
            } else {
                self.loaded.config.panels
            },
            settings: values,
            recent: workspace.recent().to_vec(),
        };
        match self.persist(config) {
            Ok(()) => {
                if layout_due {
                    self.layout_due = None;
                }
            }
            Err(_) => {
                self.blocked = true;
                workspace.set_settings_error(Some(
                    "Could not save settings. Check the file and its permissions, then retry."
                        .into(),
                ));
            }
        }
    }
    fn persist(&mut self, config: Config) -> Result<(), String> {
        let path = self.path.as_ref().expect("loaded configuration path");
        let bytes = schema::encode(&config);
        fs::create_dir_all(path.parent().expect("configuration directory"))
            .map_err(|err| err.to_string())?;
        logisim_preferences::storage::write_preferences(path, &bytes, self.loaded.json.as_deref())
            .map_err(|e| e.to_string())?;
        self.loaded = Loaded {
            config,
            json: Some(bytes),
        };
        Ok(())
    }
}
fn legacy(text: &str) -> Option<Settings> {
    let mut language = None;
    let mut mode = None;
    let mut truth_table_generation = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (key, value) = line.split_once('=')?;
        match key.trim() {
            "language" if language.is_none() => language = Some(Language::from_code(value.trim())?),
            "simulation_mode" if mode.is_none() => {
                mode = Some(SimulationMode::from_code(value.trim())?)
            }
            "truth_table_generation" if truth_table_generation.is_none() => {
                truth_table_generation = Some(value.trim().parse::<bool>().ok()?)
            }
            _ => return None,
        }
    }
    Some(Settings {
        language: language?,
        simulation_mode: mode.unwrap_or_default(),
        truth_table: truth_table_generation.unwrap_or_default(),
        ..Settings::default()
    })
}
fn config_path() -> Option<PathBuf> {
    let env = |key| {
        std::env::var_os(key)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(target_os = "windows") {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|p| p.join("Library/Application Support"))
    } else {
        env("XDG_CONFIG_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| env("HOME").map(|p| p.join(".config")))
    };
    base.map(|p| p.join("logisim-rs/preferences.json"))
}

#[cfg(test)]
mod tests;

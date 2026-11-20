//! Atomic, versioned application preferences. Invalid input is read-only until repaired.
pub(crate) mod recent;
mod schema;
use crate::window::WindowState;
use logisim_ui::WorkspaceLayout;
use logisim_ui::{Language, Settings, SimulationMode, Workspace};
use schema::Config;
use std::time::{Duration, Instant};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

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
    fn at(path: Option<PathBuf>, workspace: &mut Workspace) -> Self {
        let mut state = Self {
            path,
            loaded: Loaded::default(),
            blocked: false,
            window: WindowState::default(),
            panels: WorkspaceLayout::default(),
            layout_due: None,
        };
        state.reload(workspace);
        workspace.apply_settings(state.loaded.config.settings);
        workspace.set_recent(state.loaded.config.recent.clone());
        state.window = state.loaded.config.window;
        state.panels = state.loaded.config.panels;
        if workspace.preferences().workspace.restore_panels {
            workspace.set_layout(state.panels);
        }
        state
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
    fn reload(&mut self, workspace: &mut Workspace) -> bool {
        let Some(path) = self.path.as_ref() else {
            self.blocked = true;
            workspace
                .set_settings_error(Some("User configuration directory is unavailable.".into()));
            return false;
        };
        match Loaded::read(path) {
            Ok(loaded) => {
                self.loaded = loaded;
                self.blocked = false;
                workspace.set_config_path(Some(path.display().to_string()));
                workspace.set_settings_error(None);
                true
            }
            Err(issue) => {
                self.blocked = true;
                workspace.set_config_path(Some(issue.path.display().to_string()));
                workspace.set_settings_error(Some(issue.message.into()));
                false
            }
        }
    }
    /// Re-read the file before retrying a write, preserving independent disk edits.
    fn retry(&mut self, workspace: &mut Workspace) {
        let baseline = self.loaded.config.settings;
        let recent_changed = workspace.recent() != self.loaded.config.recent;
        if self.path.is_none() {
            self.path = config_path();
        }
        let local_window = self.window != self.loaded.config.window;
        let local_panels = self.panels != self.loaded.config.panels;
        if self.reload(workspace) {
            if !local_window {
                self.window = self.loaded.config.window;
            }
            if !local_panels {
                self.panels = self.loaded.config.panels;
            }
            if !recent_changed {
                workspace.set_recent(self.loaded.config.recent.clone());
            }
            workspace.apply_settings(
                self.loaded
                    .config
                    .settings
                    .with_changes(baseline, workspace.preferences()),
            );
        }
    }
    pub fn synchronize(&mut self, workspace: &mut Workspace) {
        if workspace.take_settings_retry() {
            self.retry(workspace);
        }
        let values = workspace.preferences();
        recent::truncate(workspace);
        let layout_due = self.layout_due.is_some_and(|at| Instant::now() >= at);
        if self.blocked
            || !values.files.valid_interval()
            || (values == self.loaded.config.settings
                && workspace.recent() == self.loaded.config.recent
                && !(layout_due
                    && (self.window != self.loaded.config.window
                        || self.panels != self.loaded.config.panels)))
        {
            return;
        }
        let path = self.path.as_ref().expect("loaded configuration path");
        let config = Config {
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
        let bytes = schema::encode(&config);
        let save = fs::create_dir_all(path.parent().expect("configuration directory"))
            .map_err(|err| err.to_string())
            .and_then(|()| crate::storage::atomic_write(path, &bytes, self.loaded.json.as_deref()));
        match save {
            Ok(()) => {
                if layout_due {
                    self.layout_due = None;
                }
                self.loaded = Loaded {
                    config,
                    json: Some(bytes),
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

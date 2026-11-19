//! Atomic, versioned application preferences. Invalid input is read-only until repaired.
mod schema;
use logisim_ui::{Language, Settings, SimulationMode, Workspace};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[derive(Default)]
struct Loaded {
    values: Settings,
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
            let values = schema::decode(&bytes).map_err(|message| Issue {
                path: path.to_owned(),
                message,
            })?;
            return Ok(Self {
                values,
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
        Ok(Self { values, json: None })
    }
}
pub(crate) struct Preferences {
    path: Option<PathBuf>,
    loaded: Loaded,
    blocked: bool,
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
        };
        state.reload(workspace);
        workspace.apply_settings(state.loaded.values);
        state
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
        let baseline = self.loaded.values;
        if self.path.is_none() {
            self.path = config_path();
        }
        if self.reload(workspace) {
            workspace.apply_settings(
                self.loaded
                    .values
                    .with_changes(baseline, workspace.preferences()),
            );
        }
    }
    pub fn synchronize(&mut self, workspace: &mut Workspace) {
        if workspace.take_settings_retry() {
            self.retry(workspace);
        }
        let values = workspace.preferences();
        if self.blocked || values == self.loaded.values {
            return;
        }
        let path = self.path.as_ref().expect("loaded configuration path");
        let bytes = schema::encode(values);
        let save = fs::create_dir_all(path.parent().expect("configuration directory"))
            .map_err(|err| err.to_string())
            .and_then(|()| crate::storage::atomic_write(path, &bytes, self.loaded.json.as_deref()));
        match save {
            Ok(()) => {
                self.loaded = Loaded {
                    values,
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

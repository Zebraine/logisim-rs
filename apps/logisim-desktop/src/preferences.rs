//! User preferences are separate from circuits, undo history and project storage.
use logisim_ui::{Language, SimulationMode, Workspace};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Values {
    language: Language,
    simulation_mode: SimulationMode,
    truth_table_generation: bool,
}
impl Values {
    fn from_workspace(workspace: &Workspace) -> Self {
        Self {
            language: workspace.language(),
            simulation_mode: workspace.simulation_mode(),
            truth_table_generation: workspace.truth_table_generation(),
        }
    }
}
pub(crate) struct Preferences {
    path: Option<PathBuf>,
    attempted: Values,
}
impl Preferences {
    pub fn load(workspace: &mut Workspace) -> Self {
        Self::at(config_path(), workspace)
    }
    fn at(path: Option<PathBuf>, workspace: &mut Workspace) -> Self {
        let values = match path.as_ref().map(fs::read_to_string).transpose() {
            Ok(Some(text)) => match decode(&text) {
                Some(values) => values,
                None => {
                    workspace.set_settings_error(Some("Unknown or invalid settings.".into()));
                    Values::default()
                }
            },
            Ok(None) => Values::default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Values::default(),
            Err(e) => {
                workspace.set_settings_error(Some(e.to_string()));
                Values::default()
            }
        };
        workspace.set_language(values.language);
        workspace.set_simulation_mode(values.simulation_mode);
        workspace.set_truth_table_generation(values.truth_table_generation);
        Self {
            path,
            attempted: values,
        }
    }
    pub fn synchronize(&mut self, workspace: &mut Workspace) {
        let retry = workspace.take_settings_retry();
        let values = Values::from_workspace(workspace);
        if values == self.attempted && !retry {
            return;
        }
        self.attempted = values;
        let result = self
            .path
            .as_ref()
            .ok_or_else(|| "User configuration directory is unavailable.".to_owned())
            .and_then(|path| save(path, values));
        workspace.set_settings_error(result.err());
    }
}
fn decode(text: &str) -> Option<Values> {
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
    Some(Values {
        language: language?,
        simulation_mode: mode.unwrap_or_default(),
        truth_table_generation: truth_table_generation.unwrap_or_default(),
    })
}
fn save(path: &Path, values: Values) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let expected = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    crate::storage::atomic_write(
        path,
        format!(
            "language={}\nsimulation_mode={}\ntruth_table_generation={}\n",
            values.language.code(),
            values.simulation_mode.code(),
            values.truth_table_generation
        )
        .as_bytes(),
        expected.as_deref(),
    )
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
    base.map(|p| p.join("logisim-rs/preferences"))
}

#[cfg(test)]
mod tests;

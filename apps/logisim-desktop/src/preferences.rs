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
}
impl Values {
    fn from_workspace(workspace: &Workspace) -> Self {
        Self {
            language: workspace.language(),
            simulation_mode: workspace.simulation_mode(),
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
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (key, value) = line.split_once('=')?;
        match key.trim() {
            "language" if language.is_none() => language = Some(Language::from_code(value.trim())?),
            "simulation_mode" if mode.is_none() => {
                mode = Some(SimulationMode::from_code(value.trim())?)
            }
            _ => return None,
        }
    }
    Some(Values {
        language: language?,
        simulation_mode: mode.unwrap_or_default(),
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
            "language={}\nsimulation_mode={}\n",
            values.language.code(),
            values.simulation_mode.code()
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "logisim-language-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn language_persists_in_both_directions_without_dirtying_or_reserializing_the_project() {
        let temp = Temp::new();
        let path = temp.0.join("settings/preferences");
        let mut workspace = Workspace::default();
        let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
        assert_eq!(workspace.language(), Language::English);
        assert!(!path.exists());
        let before = logisim_file::serialize(workspace.project()).unwrap();
        for (language, mode) in [
            (Language::SimplifiedChinese, SimulationMode::Stable),
            (Language::SimplifiedChinese, SimulationMode::Event),
            (Language::English, SimulationMode::Event),
            (Language::English, SimulationMode::Stable),
        ] {
            workspace.set_language(language);
            workspace.set_simulation_mode(mode);
            prefs.synchronize(&mut workspace);
            let mut restarted = Workspace::default();
            Preferences::at(Some(path.clone()), &mut restarted);
            assert_eq!(restarted.language(), language);
            assert_eq!(restarted.simulation_mode(), mode);
            assert_eq!(
                before,
                logisim_file::serialize(workspace.project()).unwrap()
            );
            assert!(!crate::storage::Storage::new().dirty(workspace.project()));
        }
    }
    #[test]
    fn invalid_settings_and_failed_writes_do_not_break_the_editor_or_replace_files() {
        let temp = Temp::new();
        let path = temp.0.join("preferences");
        fs::write(&path, "language=unknown\n").unwrap();
        let mut workspace = Workspace::default();
        let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
        assert_eq!(workspace.language(), Language::English);
        prefs.synchronize(&mut workspace);
        assert_eq!(fs::read_to_string(&path).unwrap(), "language=unknown\n");
        let blocked = path.join("preferences"); // parent is a file
        assert!(
            save(
                &blocked,
                Values {
                    language: Language::SimplifiedChinese,
                    ..Values::default()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "language=unknown\n");
        workspace.set_language(Language::SimplifiedChinese);
        prefs.synchronize(&mut workspace);
        assert_eq!(
            decode(&fs::read_to_string(path).unwrap()),
            Some(Values {
                language: Language::SimplifiedChinese,
                ..Values::default()
            })
        );
    }
    #[test]
    fn language_only_preferences_keep_their_language_and_invalid_modes_are_not_overwritten() {
        let temp = Temp::new();
        let path = temp.0.join("preferences");
        fs::write(&path, "language=zh-CN\n").unwrap();
        let mut workspace = Workspace::default();
        let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
        assert_eq!(workspace.language(), Language::SimplifiedChinese);
        assert_eq!(workspace.simulation_mode(), SimulationMode::Stable);
        prefs.synchronize(&mut workspace);
        assert_eq!(fs::read_to_string(&path).unwrap(), "language=zh-CN\n");
        workspace.set_simulation_mode(SimulationMode::Event);
        prefs.synchronize(&mut workspace);
        assert_eq!(
            decode(&fs::read_to_string(&path).unwrap()),
            Some(Values {
                language: Language::SimplifiedChinese,
                simulation_mode: SimulationMode::Event,
            })
        );
        for text in [
            "language=en\nsimulation_mode=invalid\n",
            "language=en\nsimulation_mode=event\nsimulation_mode=stable\n",
        ] {
            fs::write(&path, text).unwrap();
            let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
            assert_eq!(workspace.simulation_mode(), SimulationMode::Stable);
            prefs.synchronize(&mut workspace);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
    }
}

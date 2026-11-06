//! User preferences are separate from circuits, undo history and project storage.
use logisim_ui::{Language, Workspace};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct Preferences {
    path: Option<PathBuf>,
    attempted: Language,
}
impl Preferences {
    pub fn load(workspace: &mut Workspace) -> Self {
        Self::at(config_path(), workspace)
    }
    fn at(path: Option<PathBuf>, workspace: &mut Workspace) -> Self {
        let language = match path.as_ref().map(fs::read_to_string).transpose() {
            Ok(Some(text)) => match decode(&text) {
                Some(language) => language,
                None => {
                    workspace
                        .set_settings_error(Some("Unknown or invalid language preference.".into()));
                    Language::English
                }
            },
            Ok(None) => Language::English,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Language::English,
            Err(e) => {
                workspace.set_settings_error(Some(e.to_string()));
                Language::English
            }
        };
        workspace.set_language(language);
        Self {
            path,
            attempted: language,
        }
    }
    pub fn synchronize(&mut self, workspace: &mut Workspace) {
        let retry = workspace.take_settings_retry();
        let language = workspace.language();
        if language == self.attempted && !retry {
            return;
        }
        self.attempted = language;
        let result = self
            .path
            .as_ref()
            .ok_or_else(|| "User configuration directory is unavailable.".to_owned())
            .and_then(|path| save(path, language));
        workspace.set_settings_error(result.err());
    }
}
fn decode(text: &str) -> Option<Language> {
    Language::from_code(text.trim().strip_prefix("language=")?)
}
fn save(path: &Path, language: Language) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let expected = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    crate::storage::atomic_write(
        path,
        format!("language={}\n", language.code()).as_bytes(),
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
        for language in [Language::SimplifiedChinese, Language::English] {
            workspace.set_language(language);
            prefs.synchronize(&mut workspace);
            let mut restarted = Workspace::default();
            Preferences::at(Some(path.clone()), &mut restarted);
            assert_eq!(restarted.language(), language);
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
        assert!(save(&blocked, Language::SimplifiedChinese).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "language=unknown\n");
        workspace.set_language(Language::SimplifiedChinese);
        prefs.synchronize(&mut workspace);
        assert_eq!(
            decode(&fs::read_to_string(path).unwrap()),
            Some(Language::SimplifiedChinese)
        );
    }
}

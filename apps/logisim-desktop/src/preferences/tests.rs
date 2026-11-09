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
fn preferences_persist_in_both_directions_without_dirtying_or_reserializing_the_project() {
    let temp = Temp::new();
    let path = temp.0.join("settings/preferences");
    let mut workspace = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
    assert_eq!(workspace.language(), Language::English);
    assert!(!workspace.truth_table_generation());
    assert!(!path.exists());
    let before = logisim_file::serialize(workspace.project()).unwrap();
    for (language, mode, truth_table) in [
        (Language::English, SimulationMode::Stable, true),
        (Language::English, SimulationMode::Stable, false),
        (Language::SimplifiedChinese, SimulationMode::Stable, true),
        (Language::SimplifiedChinese, SimulationMode::Event, true),
        (Language::English, SimulationMode::Event, false),
        (Language::English, SimulationMode::Stable, false),
    ] {
        workspace.set_language(language);
        workspace.set_simulation_mode(mode);
        workspace.set_truth_table_generation(truth_table);
        prefs.synchronize(&mut workspace);
        let mut restarted = Workspace::default();
        Preferences::at(Some(path.clone()), &mut restarted);
        assert_eq!(restarted.language(), language);
        assert_eq!(restarted.simulation_mode(), mode);
        assert_eq!(restarted.truth_table_generation(), truth_table);
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
fn older_preferences_keep_defaults_and_invalid_settings_are_not_overwritten() {
    let temp = Temp::new();
    let path = temp.0.join("preferences");
    fs::write(&path, "language=zh-CN\n").unwrap();
    let mut workspace = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
    assert_eq!(workspace.language(), Language::SimplifiedChinese);
    assert_eq!(workspace.simulation_mode(), SimulationMode::Stable);
    assert!(!workspace.truth_table_generation());
    prefs.synchronize(&mut workspace);
    assert_eq!(fs::read_to_string(&path).unwrap(), "language=zh-CN\n");
    fs::write(&path, "language=zh-CN\nsimulation_mode=stable\n").unwrap();
    let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
    assert!(!workspace.truth_table_generation());
    prefs.synchronize(&mut workspace);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "language=zh-CN\nsimulation_mode=stable\n"
    );
    workspace.set_simulation_mode(SimulationMode::Event);
    prefs.synchronize(&mut workspace);
    assert_eq!(
        decode(&fs::read_to_string(&path).unwrap()),
        Some(Values {
            language: Language::SimplifiedChinese,
            simulation_mode: SimulationMode::Event,
            ..Values::default()
        })
    );
    for text in [
        "language=en\nsimulation_mode=invalid\n",
        "language=en\nsimulation_mode=event\nsimulation_mode=stable\n",
        "language=en\ntruth_table_generation=yes\n",
        "language=en\ntruth_table_generation=true\ntruth_table_generation=false\n",
    ] {
        fs::write(&path, text).unwrap();
        let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
        assert_eq!(workspace.simulation_mode(), SimulationMode::Stable);
        assert!(!workspace.truth_table_generation());
        prefs.synchronize(&mut workspace);
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

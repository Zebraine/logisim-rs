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
    let path = temp.0.join("settings/preferences.json");
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
fn migrates_legacy_only_on_edit_and_always_prefers_json() {
    let temp = Temp::new();
    let old = temp.0.join("preferences");
    let json = old.with_extension("json");
    let mut workspace = Workspace::default();
    for (text, truth, mode) in [
        ("language=zh-CN\n", false, SimulationMode::Stable),
        (
            "language=zh-CN\nsimulation_mode=event\ntruth_table_generation=true\n",
            true,
            SimulationMode::Event,
        ),
    ] {
        fs::write(&old, text).unwrap();
        let mut prefs = Preferences::at(Some(json.clone()), &mut workspace);
        assert_eq!(workspace.language(), Language::SimplifiedChinese);
        assert_eq!(workspace.simulation_mode(), mode);
        assert_eq!(workspace.truth_table_generation(), truth);
        prefs.synchronize(&mut workspace);
        assert!(!json.exists());
        assert_eq!(fs::read_to_string(&old).unwrap(), text);
    }
    let mut prefs = Preferences::at(Some(json.clone()), &mut workspace);
    workspace.set_language(Language::English);
    prefs.synchronize(&mut workspace);
    let mut restarted = Workspace::default();
    Preferences::at(Some(json.clone()), &mut restarted);
    assert_eq!(restarted.preferences(), workspace.preferences());
    assert!(restarted.truth_table_generation());
    assert!(
        fs::read_to_string(old)
            .unwrap()
            .starts_with("language=zh-CN")
    );
}
#[test]
fn rejects_invalid_legacy_without_creating_a_replacement() {
    let temp = Temp::new();
    let old = temp.0.join("preferences");
    let json = old.with_extension("json");
    for text in [
        "language=unknown",
        "language=en\nsimulation_mode=invalid",
        "language=en\nsimulation_mode=stable\nsimulation_mode=event",
        "language=en\ntruth_table_generation=yes",
        "language=en\ntruth_table_generation=true\ntruth_table_generation=false",
    ] {
        fs::write(&old, text).unwrap();
        let mut workspace = Workspace::default();
        let mut prefs = Preferences::at(Some(json.clone()), &mut workspace);
        assert_eq!(workspace.preferences(), Settings::default());
        workspace.set_simulation_mode(SimulationMode::Event);
        prefs.synchronize(&mut workspace);
        assert!(!json.exists());
        assert_eq!(fs::read_to_string(&old).unwrap(), text);
    }
}
#[test]
fn missing_fields_default_and_invalid_json_stays_intact_after_edits() {
    let defaults = Settings::default();
    assert_eq!(
        schema::decode(br#"{"version":1,"appearance":{},"generation":{}}"#).unwrap(),
        defaults
    );
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    fs::write(temp.0.join("preferences"), "language=zh-CN").unwrap();
    for json in [
        "{",
        "{}",
        r#"{"version":2}"#,
        r#"{"version":1,"other":0}"#,
        r#"{"version":1,"simulation":{"typo":4}}"#,
        r#"{"version":1,"simulation":{"tick_hz":32769}}"#,
        r#"{"version":1,"simulation":{"tick_hz":0}}"#,
        r#"{"version":1,"appearance":{"ui_scale":1.01}}"#,
        r#"{"version":1,"general":{"language":"other"}}"#,
        r#"{"version":1,"generation":{"truth_table":"yes"}}"#,
    ] {
        fs::write(&path, json).unwrap();
        let mut workspace = Workspace::default();
        let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
        assert_eq!(workspace.preferences(), defaults);
        workspace.set_truth_table_generation(true);
        prefs.synchronize(&mut workspace);
        assert_eq!(fs::read_to_string(&path).unwrap(), json);
    }
}
#[test]
fn all_fields_restart_reset_and_atomic_conflict_protection() {
    use logisim_ui::{DefaultZoom, TickHz, UiScale};
    let alternative = Settings {
        language: Language::SimplifiedChinese,
        ui_scale: UiScale::P125,
        reduce_motion: true,
        show_telemetry: false,
        show_grid: false,
        default_zoom: DefaultZoom::Actual,
        simulation_mode: SimulationMode::Event,
        tick_hz: TickHz::new(32768).unwrap(),
        truth_table: true,
    };
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut w = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut w);
    for desired in [alternative, Settings::default()] {
        w.apply_settings(desired);
        prefs.synchronize(&mut w);
        prefs = Preferences::at(Some(path.clone()), &mut w);
        assert_eq!(w.preferences(), desired);
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    }
    fs::write(&path, "bad external edit").unwrap();
    w.apply_settings(alternative);
    prefs.synchronize(&mut w);
    assert!(prefs.blocked);
    assert_eq!(fs::read_to_string(path).unwrap(), "bad external edit");
}

#[test]
fn retry_after_write_failure_replays_only_local_changes_on_repaired_file() {
    let temp = Temp::new();
    let directory = temp.0.join("prefs");
    let path = directory.join("preferences.json");
    let mut w = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut w);
    fs::write(&directory, b"blocks directory creation").unwrap();
    w.set_truth_table_generation(true);
    prefs.synchronize(&mut w);
    assert!(prefs.blocked);
    assert!(w.truth_table_generation());
    assert_eq!(fs::read(&directory).unwrap(), b"blocks directory creation");
    fs::remove_file(&directory).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(&path, b"invalid").unwrap();
    prefs.retry(&mut w);
    prefs.synchronize(&mut w);
    assert!(prefs.blocked);
    assert_eq!(fs::read(&path).unwrap(), b"invalid");
    fs::write(&path, br#"{"version":1,"general":{"language":"zh-CN"}}"#).unwrap();
    prefs.retry(&mut w);
    prefs.synchronize(&mut w);
    assert!(!prefs.blocked);
    let mut restarted = Workspace::default();
    Preferences::at(Some(path), &mut restarted);
    assert!(restarted.truth_table_generation());
    assert_eq!(restarted.language(), Language::SimplifiedChinese);
    assert_eq!(w.preferences(), restarted.preferences());
}

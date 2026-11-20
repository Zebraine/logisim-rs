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
        schema::decode(br#"{"version":1,"appearance":{},"generation":{}}"#)
            .unwrap()
            .settings,
        defaults
    );
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    fs::write(temp.0.join("preferences"), "language=zh-CN").unwrap();
    for json in [
        "{",
        "{}",
        r#"{"version":7}"#,
        r#"{"version":6,"appearance":{"density":"dense"}}"#,
        r##"{"version":6,"appearance":{"accent":"#ffffff"}}"##,
        r#"{"version":6,"appearance":{"density":null}}"#,
        r#"{"version":5,"files":{"autosave_minutes":0}}"#,
        r#"{"version":5,"files":{"autosave_minutes":31}}"#,
        r#"{"version":5,"files":{"autosave_minutes":"5"}}"#,
        r#"{"version":5,"files":{"autosave_minutes":1.5}}"#,
        r#"{"version":5,"files":{"autosave":1}}"#,
        r#"{"version":4,"appearance":{"signal_palette":"neon"}}"#,
        r#"{"version":3,"editor":{"default_probe_radix":"decimal"}}"#,
        r#"{"version":3,"editor":{"default_pin_bits":0}}"#,
        r#"{"version":3,"editor":{"default_pin_bits":33}}"#,
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
        files: logisim_ui::FileSettings {
            autosave: false,
            autosave_minutes: 30,
        },
        workspace: Default::default(),
        language: Language::SimplifiedChinese,
        density: logisim_ui::Density::Compact,
        accent: logisim_ui::Accent::Teal,
        ui_scale: UiScale::P125,
        reduce_motion: true,
        show_telemetry: false,
        signal_palette: logisim_ui::SignalPalette::ColorblindSafe,
        show_wire_values: false,
        show_origin_marker: false,
        show_grid: false,
        default_zoom: DefaultZoom::Actual,
        highlight_net: false,
        show_connection_markers: false,
        invert_wheel_zoom: true,
        default_probe_radix: logisim_core::Radix::Hex,
        default_pin_bits: 8,
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

#[test]
fn v1_migrates_and_v2_history_is_validated_and_limited_on_edit() {
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut ws = Workspace::default();
    fs::write(&path, r#"{"version":1,"editor":{"show_grid":false}}"#).unwrap();
    let mut prefs = Preferences::at(Some(path.clone()), &mut ws);
    assert!(!ws.preferences().show_grid);
    let a = temp.0.join("a.circ");
    let b = temp.0.join("b.circ");
    recent::record(&mut ws, &a);
    recent::record(&mut ws, &b);
    recent::record(&mut ws, &a);
    recent::record(&mut ws, Path::new(""));
    assert_eq!(ws.recent(), &[a.to_str().unwrap(), b.to_str().unwrap()]);
    let mut values = ws.preferences();
    values.workspace.recent_limit = 1;
    ws.apply_settings(values);
    prefs.synchronize(&mut ws);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["version"],
        logisim_ui::CONFIG_VERSION
    );
    let saved = schema::decode(&bytes).unwrap();
    assert_eq!(saved.recent, [a.to_str().unwrap()]);
    assert_eq!(saved.settings, values);
    for bad in [
        r#"{"version":2,"recent":["relative.circ"]}"#,
        r#"{"version":2,"workspace":{"recent_limit":21}}"#,
        r#"{"version":2,"workspace":{"recent_limit":0}}"#,
        r#"{"version":2,"workspace":null}"#,
        r#"{"version":1,"recent":[]}"#,
        r#"{"version":2,"workspace":{"unknown":true}}"#,
    ] {
        assert!(schema::decode(bad.as_bytes()).is_err(), "{bad}");
    }
}

#[test]
fn layout_waits_for_idle_or_close_and_restores_only_when_enabled() {
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut ui = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut ui);
    let panels = WorkspaceLayout {
        sidebar_width: 367.,
        split: 0.67,
        tab: logisim_ui::LayoutTab::Components,
    };
    let now = Instant::now();
    prefs.observe_layout(None, Some(panels), now);
    prefs.synchronize(&mut ui);
    assert!(!path.exists());
    prefs.flush_layout();
    prefs.synchronize(&mut ui);
    let mut restarted = Workspace::default();
    let mut opened = Preferences::at(Some(path.clone()), &mut restarted);
    assert_eq!(restarted.layout(), panels);
    let mut options = restarted.preferences();
    options.workspace.restore_panels = false;
    restarted.apply_settings(options);
    opened.synchronize(&mut restarted);
    let mut fresh = Workspace::default();
    Preferences::at(Some(path.clone()), &mut fresh);
    assert_eq!(fresh.layout(), WorkspaceLayout::default());
    assert_eq!(
        schema::decode(&fs::read(path).unwrap()).unwrap().panels,
        panels
    );
}

#[test]
fn earlier_preferences_migrate_to_v6_and_all_radix_codes_round_trip() {
    for version in 1..=5 {
        let json = serde_json::json!({"version":version,"editor":{"show_grid":false}});
        let config = schema::decode(json.to_string().as_bytes()).unwrap();
        assert_eq!(
            config.settings,
            Settings {
                show_grid: false,
                ..Settings::default()
            }
        );
        let encoded: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
        assert_eq!(encoded["version"], 6);
        assert_eq!(encoded["appearance"]["density"], "comfortable");
        assert_eq!(encoded["appearance"]["accent"], "indigo");
        assert_eq!(
            encoded["files"],
            serde_json::json!({"autosave":true,"autosave_minutes":5})
        );
        assert_eq!(encoded["appearance"]["signal_palette"], "default");
        assert_eq!(encoded["appearance"]["show_wire_values"], true);
        assert_eq!(encoded["appearance"]["show_origin_marker"], true);
        assert_eq!(encoded["editor"]["default_probe_radix"], "binary");
        assert_eq!(encoded["editor"]["default_pin_bits"], 1);
    }
    for (code, radix) in ["binary", "octal", "signed", "unsigned", "hex"]
        .into_iter()
        .zip(logisim_core::Radix::ALL)
    {
        let json = serde_json::json!({"version":3,"editor":{"default_probe_radix":code,"default_pin_bits":32}});
        let config = schema::decode(json.to_string().as_bytes()).unwrap();
        assert_eq!(config.settings.default_probe_radix, radix);
        let encoded: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
        assert_eq!(encoded["editor"]["default_probe_radix"], code);
        assert_eq!(encoded["editor"]["default_pin_bits"], 32);
    }
}

#[test]
fn appearance_codes_round_trip_without_changing_other_preferences() {
    for (code, palette) in ["default", "high-contrast", "colorblind-safe"]
        .into_iter()
        .zip(logisim_ui::SignalPalette::ALL)
    {
        let appearance = serde_json::json!({"signal_palette":code,"show_wire_values":false,"show_origin_marker":false});
        let json = serde_json::json!({"version":4,"appearance":appearance});
        let config = schema::decode(json.to_string().as_bytes()).unwrap();
        assert_eq!(
            config.settings,
            Settings {
                signal_palette: palette,
                show_wire_values: false,
                show_origin_marker: false,
                ..Settings::default()
            }
        );
        let encoded: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
        for (key, value) in appearance.as_object().unwrap() {
            assert_eq!(&encoded["appearance"][key], value);
        }
    }
}

#[test]
fn individual_file_preferences_default_independently() {
    for (json, enabled, minutes) in [
        (r#"{"version":5,"files":{"autosave":false}}"#, false, 5),
        (r#"{"version":5,"files":{"autosave_minutes":1}}"#, true, 1),
        (r#"{"version":5,"files":{"autosave_minutes":30}}"#, true, 30),
    ] {
        let config = schema::decode(json.as_bytes()).unwrap();
        assert_eq!(
            config.settings.files,
            logisim_ui::FileSettings {
                autosave: enabled,
                autosave_minutes: minutes
            }
        );
    }
}

#[test]
fn density_and_accent_round_trip_independently_in_v6() {
    for density in ["comfortable", "compact"] {
        for accent in ["indigo", "blue", "teal"] {
            let appearance = serde_json::json!({"density": density, "accent": accent});
            let bytes =
                serde_json::to_vec(&serde_json::json!({"version": 6, "appearance": appearance}))
                    .unwrap();
            let config = schema::decode(&bytes).unwrap();
            let encoded: serde_json::Value =
                serde_json::from_slice(&schema::encode(&config)).unwrap();
            assert_eq!(encoded["appearance"]["density"], density);
            assert_eq!(encoded["appearance"]["accent"], accent);
        }
    }
}

#[test]
fn programmatic_limits_bound_history_and_never_persist_an_invalid_interval() {
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut ws = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut ws);
    for (input, expected) in [(0, 1), (usize::MAX, 20)] {
        let mut values = ws.preferences();
        values.workspace.recent_limit = input;
        values.default_pin_bits = u8::MAX;
        ws.apply_settings(values);
        let entries: Vec<_> = (0..25)
            .map(|i| temp.0.join(format!("{i}.circ")).display().to_string())
            .collect();
        ws.set_recent(entries.clone());
        recent::record(&mut ws, Path::new(&entries[0]));
        assert_eq!(ws.recent(), &entries[..expected]);
        ws.set_recent(entries.clone());
        prefs.synchronize(&mut ws);
        let bytes = fs::read(&path).unwrap();
        let config = schema::decode(&bytes).unwrap();
        assert_eq!(config.recent, entries[..expected]);
        assert_eq!(config.settings.default_pin_bits, 32);
        assert_eq!(config.settings.workspace.recent_limit, expected);
        for invalid in [0, 31, u8::MAX] {
            let mut values = ws.preferences();
            values.files.autosave_minutes = invalid;
            ws.apply_settings(values);
            prefs.synchronize(&mut ws);
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        let mut corrected = ws.preferences();
        corrected.files.autosave_minutes = 2;
        ws.apply_settings(corrected);
        prefs.synchronize(&mut ws);
        assert_eq!(
            schema::decode(&fs::read(&path).unwrap()).unwrap().settings,
            corrected
        );
    }
}

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
        r#"{"version":16}"#,
        r#"{"version":15,"appearance":{"dark_artwork_contrast":"false"}}"#,
        r#"{"version":15,"appearance":{"dark_artwork_contrast":null}}"#,
        r#"{"version":13,"appearance":{"theme":"system"}}"#,
        r#"{"version":13,"appearance":{"theme":null}}"#,
        r#"{"version":12,"files":{"backup_before_save":"true"}}"#,
        r#"{"version":12,"files":{"backup_before_save":1}}"#,
        r#"{"version":12,"files":{"backup_before_save":null}}"#,
        r#"{"version":11,"editor":{"default_font_size":7}}"#,
        r#"{"version":11,"editor":{"default_font_size":33}}"#,
        r#"{"version":11,"editor":{"default_font_size":14.5}}"#,
        r#"{"version":11,"editor":{"default_font_size":"auto"}}"#,
        r#"{"version":11,"editor":{"default_gate_inputs":null}}"#,
        r#"{"version":11,"editor":{"default_gate_inputs":1}}"#,
        r#"{"version":11,"editor":{"default_gate_inputs":33}}"#,
        r#"{"version":11,"editor":{"default_gate_inputs":4.5}}"#,
        r#"{"version":11,"editor":{"default_gate_inputs":"4"}}"#,
        r#"{"version":8,"editor":{"default_gate_size":"large"}}"#,
        r#"{"version":8,"editor":{"default_tunnel_width":0}}"#,
        r#"{"version":8,"editor":{"default_tunnel_width":33}}"#,
        r#"{"version":8,"editor":{"default_tunnel_width":"8"}}"#,
        r#"{"version":8,"editor":{"remember_last_properties":1}}"#,
        r#"{"version":10,"shortcuts":{"appearance.invalid":"f3"}}"#,
        r#"{"version":10,"shortcuts":{"appearance.copy":"ctrl+??"}}"#,
        r#"{"version":10,"shortcuts":{"appearance.copy":"delete"}}"#,
        r#"{"version":10,"shortcuts":{"appearance.copy":"f3","appearance.cut":"f3"}}"#,
        r#"{"version":10,"shortcuts":{"appearance.copy":"primary+s"}}"#,
        r#"{"version":9,"shortcuts":{"file.save":"primary+shift+up"}}"#,
        r#"{"version":9,"shortcuts":{"tool.missing":"f3"}}"#,
        r#"{"version":9,"shortcuts":{"tool.text":"primary+bad"}}"#,
        r#"{"version":9,"shortcuts":{"tool.poke":"f6","tool.poke":"p"}}"#,
        r#"{"version":9,"shortcuts":{"tool.poke":"f6","tool.wire":"f6"}}"#,
        r#"{"version":9,"shortcuts":{"tool.poke":"primary+s"}}"#,
        r#"{"version":9,"shortcuts":{"tool.rotate":"space"}}"#,
        r#"{"version":7,"shortcuts":null}"#,
        r#"{"version":7,"shortcuts":{"edit.unknown":"f3"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"primary+maybe"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"shift+shift+n"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"ctrl+y"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"controlleft"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"shift+insert"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"primary++"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"ctrl+s"}}"#,
        r#"{"version":7,"shortcuts":{"file.new":"f3","file.new":"f4"}}"#,
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
    let mut shortcuts = logisim_ui::shortcuts::Shortcuts::default();
    shortcuts
        .bind(logisim_ui::shortcuts::Command::Save, "f5".parse().unwrap())
        .unwrap();
    let alternative = Settings {
        shortcuts,
        files: logisim_ui::FileSettings {
            autosave: false,
            autosave_minutes: 30,
            backup_before_save: true,
        },
        workspace: logisim_ui::WorkspaceSettings {
            recent_limit: 3,
            restore_window: false,
            restore_panels: false,
            reopen_last: true,
        },
        language: Language::SimplifiedChinese,
        density: logisim_ui::Density::Compact,
        theme: logisim_ui::ThemeMode::Dark,
        accent: logisim_ui::Accent::Teal,
        dark_artwork_contrast: false,
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
        default_gate_size: logisim_core::GateSize::Wide,
        default_font_size: Some(24),
        default_gate_inputs: 4,
        default_tunnel_width: 32,
        remember_last_properties: true,
        simulation_mode: SimulationMode::Event,
        tick_hz: TickHz::new(32768).unwrap(),
        truth_table: true,
    };
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut w = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut w);
    for desired in [alternative, Settings::default()] {
        let bytes = schema::encode_settings(desired);
        assert_eq!(schema::decode_settings(&bytes).unwrap(), desired);
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 9);
        for key in [
            "version",
            "general",
            "appearance",
            "editor",
            "files",
            "workspace",
            "shortcuts",
            "simulation",
            "generation",
        ] {
            assert!(json.get(key).is_some(), "{key}");
        }
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
    w.apply_settings(Settings {
        files: logisim_ui::FileSettings {
            backup_before_save: true,
            ..w.preferences().files
        },
        truth_table: true,
        default_font_size: Some(24),
        default_gate_inputs: 4,
        default_gate_size: logisim_core::GateSize::Medium,
        default_tunnel_width: 17,
        remember_last_properties: true,
        ..w.preferences()
    });
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
    assert!(restarted.preferences().files.backup_before_save);
    assert_eq!(
        restarted.preferences().default_gate_size,
        logisim_core::GateSize::Medium
    );
    assert_eq!(restarted.preferences().default_font_size, Some(24));
    assert_eq!(restarted.preferences().default_gate_inputs, 4);
    assert_eq!(restarted.preferences().default_tunnel_width, 17);
    assert!(restarted.preferences().remember_last_properties);
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
fn earlier_preferences_migrate_to_v15_and_editor_defaults_round_trip() {
    for version in 1..=14 {
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
        assert_eq!(encoded["version"], 15);
        assert_eq!(encoded["appearance"]["dark_artwork_contrast"], true);
        assert!(config.plugins.fallback_directory.is_none());
        assert_eq!(encoded["appearance"]["theme"], "light");
        assert_eq!(encoded["appearance"]["density"], "comfortable");
        assert_eq!(encoded["appearance"]["accent"], "indigo");
        assert_eq!(
            encoded["files"],
            serde_json::json!({"autosave":true,"autosave_minutes":5,"backup_before_save":false})
        );
        assert_eq!(encoded["appearance"]["signal_palette"], "default");
        assert_eq!(encoded["appearance"]["show_wire_values"], true);
        assert_eq!(encoded["appearance"]["show_origin_marker"], true);
        assert_eq!(encoded["editor"]["default_probe_radix"], "binary");
        assert_eq!(encoded["editor"]["default_pin_bits"], 1);
        assert_eq!(encoded["editor"]["default_gate_size"], "narrow");
        assert_eq!(encoded["editor"]["default_tunnel_width"], 1);
        assert_eq!(
            encoded["editor"].get("default_font_size"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(encoded["editor"]["default_gate_inputs"], 2);
        assert_eq!(encoded["editor"]["remember_last_properties"], false);
    }
    for (font, inputs) in [(None, 2), (Some(8), 32), (Some(32), 4)] {
        let json = serde_json::json!({"version":11,"editor":{"default_font_size":font,"default_gate_inputs":inputs}});
        let config = schema::decode(json.to_string().as_bytes()).unwrap();
        assert_eq!(config.settings.default_font_size, font);
        assert_eq!(config.settings.default_gate_inputs, inputs);
        assert_eq!(
            schema::decode(&schema::encode(&config)).unwrap().settings,
            config.settings
        );
    }
    for (code, size) in [
        ("narrow", logisim_core::GateSize::Narrow),
        ("medium", logisim_core::GateSize::Medium),
        ("wide", logisim_core::GateSize::Wide),
    ] {
        let json = serde_json::json!({"version":8,"editor":{"default_gate_size":code,"default_tunnel_width":32,"remember_last_properties":true}});
        let config = schema::decode(json.to_string().as_bytes()).unwrap();
        assert_eq!(config.settings.default_gate_size, size);
        assert_eq!(config.settings.default_tunnel_width, 32);
        assert!(config.settings.remember_last_properties);
        let encoded: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
        assert_eq!(encoded["editor"]["default_gate_size"], code);
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
                autosave_minutes: minutes,
                backup_before_save: false,
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

#[test]
fn partial_shortcut_bindings_round_trip_and_survive_retry() {
    use logisim_ui::shortcuts::Command;
    let bytes = br#"{"version":7,"shortcuts":{"file.save":"primary+shift+f8"}}"#;
    let mut shortcuts = schema::decode(bytes).unwrap().settings.shortcuts;
    shortcuts
        .bind(Command::ToolPoke, "f6".parse().unwrap())
        .unwrap();
    shortcuts
        .bind(Command::AppearanceDuplicate, "f6".parse().unwrap())
        .unwrap();
    assert_eq!(shortcuts.get(Command::New), Command::New.default_chord());
    assert_eq!(shortcuts.get(Command::Save).label(false), "Ctrl+Shift+F8");
    assert_eq!(shortcuts.get(Command::Save).label(true), "Cmd+Shift+F8");
    let tmp = Temp::new();
    let path = tmp.0.join("preferences.json");
    fs::write(&path, b"invalid").unwrap();
    let mut w = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut w);
    w.apply_settings(Settings {
        shortcuts,
        ..w.preferences()
    });
    prefs.synchronize(&mut w);
    assert_eq!(fs::read(&path).unwrap(), b"invalid");
    let clash = br#"{"version":7,"shortcuts":{"file.open":"primary+shift+f8"}}"#;
    fs::write(&path, clash).unwrap();
    prefs.retry(&mut w);
    prefs.synchronize(&mut w);
    assert_eq!(fs::read(&path).unwrap(), clash);
    assert_eq!(w.preferences().shortcuts, shortcuts);
    let repair = br#"{"version":7,"general":{"language":"zh-CN"},"shortcuts":{"file.open":"f10"}}"#;
    fs::write(&path, repair).unwrap();
    prefs.retry(&mut w);
    prefs.synchronize(&mut w);
    let saved = fs::read(path).unwrap();
    let mut expected = shortcuts;
    expected
        .bind(Command::Open, "f10".parse().unwrap())
        .unwrap();
    assert_eq!(schema::decode(&saved).unwrap().settings.shortcuts, expected);
    assert_eq!(w.language(), Language::SimplifiedChinese);
    let json: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(json["version"], logisim_ui::CONFIG_VERSION);
    assert_eq!(json["shortcuts"].as_object().unwrap().len(), Command::COUNT);
}

#[test]
fn v9_shift_arrow_save_migrates_alongside_restored_layer_defaults() {
    let config = schema::decode(br#"{"version":9,"shortcuts":{"file.save":"shift+up"}}"#).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
    assert_eq!(saved["version"], logisim_ui::CONFIG_VERSION);
    let bindings = saved["shortcuts"].as_object().unwrap();
    assert_eq!(bindings.len(), 37);
    assert_eq!(bindings["file.save"], "shift+up");
    assert_eq!(bindings["appearance.front"], "primary+shift+up");
    assert_eq!(bindings["appearance.back"], "primary+shift+down");
    assert!(
        schema::decode(br#"{"version":9,"shortcuts":{"file.save":"primary+shift+up"}}"#).is_err()
    );
}

#[test]
fn explicit_v10_layer_bindings_survive_the_default_correction() {
    let config = schema::decode(br#"{"version":10,"shortcuts":{"appearance.front":"shift+up","appearance.back":"shift+down"}}"#).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
    assert_eq!(saved["shortcuts"]["appearance.front"], "shift+up");
    assert_eq!(saved["shortcuts"]["appearance.back"], "shift+down");
}

#[test]
fn complete_v8_and_v9_shortcuts_keep_custom_keys_and_gain_missing_defaults() {
    use logisim_ui::shortcuts::{Command, Group};
    for version in [8, 9] {
        let mut legacy = serde_json::Map::new();
        for c in Command::ALL.into_iter().filter(|c| {
            c.group() != Group::Appearance && (version >= 9 || c.group() != Group::Tools)
        }) {
            legacy.insert(c.id().into(), c.default_chord().to_string().into());
        }
        assert_eq!(legacy.len(), if version == 8 { 20 } else { 26 });
        if version == 9 {
            legacy.insert("tool.poke".into(), "f5".into());
        }
        legacy.insert("file.save".into(), "f2".into());
        let file = serde_json::json!({"version":version,"shortcuts":legacy});
        let config = schema::decode(file.to_string().as_bytes()).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&schema::encode(&config)).unwrap();
        assert_eq!(saved["version"], logisim_ui::CONFIG_VERSION);
        for (id, chord) in file["shortcuts"].as_object().unwrap() {
            assert_eq!(&saved["shortcuts"][id], chord, "{id}");
        }
        for (id, chord) in [
            ("tool.select", "v"),
            ("tool.poke", if version == 8 { "p" } else { "f5" }),
            ("tool.wire", "w"),
            ("tool.text", "t"),
            ("tool.tunnel", "u"),
            ("tool.rotate", "r"),
        ] {
            assert_eq!(saved["shortcuts"][id], chord);
        }
        assert_eq!(saved["shortcuts"].as_object().unwrap().len(), 37);
        for c in Command::ALL
            .into_iter()
            .filter(|c| c.group() == Group::Appearance)
        {
            assert_eq!(saved["shortcuts"][c.id()], c.default_chord().to_string());
        }
    }
}

#[test]
fn portable_files_migrate_validate_bindings_and_ignore_host_metadata() {
    for version in 1..=logisim_ui::CONFIG_VERSION {
        let json = serde_json::json!({"version":version, "recent":["private path"], "window":null, "panels":{}});
        let settings = schema::decode_settings(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&schema::encode_settings(settings))
                .unwrap()["version"],
            15
        );
    }
    for json in [
        "",
        "{}",
        r#"{"version":16}"#,
        r#"{"version":15,"appearance":{"dark_artwork_contrast":"false"}}"#,
        r#"{"version":15,"appearance":{"dark_artwork_contrast":null}}"#,
        r#"{"version":13,"unknown":null}"#,
        r#"{"version":13,"editor":{"new_option":2}}"#,
        r#"{"version":13,"appearance":{"theme":"system"}}"#,
        r#"{"version":13,"editor":{"default_font_size":33}}"#,
        r#"{"version":13,"version":12}"#,
        r#"{"version":13,"shortcuts":{"file.save":"primary+o"}}"#,
    ] {
        assert!(schema::decode_settings(json.as_bytes()).is_err(), "{json}");
    }
    assert!(schema::decode_settings(&vec![b' '; logisim_file::MAX_BYTES + 1]).is_err());
}

#[test]
fn importing_commits_only_validated_preferences_and_keeps_local_history_and_geometry() {
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut ws = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut ws);
    let recent: Vec<_> = ["one.circ", "two.circ"]
        .map(|name| temp.0.join(name).display().to_string())
        .into();
    ws.set_recent(recent.clone());
    let window: WindowState =
        serde_json::from_str(r#"{"width":1080,"height":640,"x":41,"y":29}"#).unwrap();
    let panels = WorkspaceLayout {
        sidebar_width: 287.,
        split: 0.61,
        ..Default::default()
    };
    ws.set_layout(panels);
    prefs.observe_layout(Some(window), Some(panels), Instant::now());
    prefs.synchronize(&mut ws);
    let project = ws.project().clone();
    let root = ws.simulation_root();
    let active = ws.active_circuit();
    let before = fs::read(&path).unwrap();
    assert!(
        Preferences::validate_import(br#"{"version":13,"general":{"language":"XX"}}"#).is_err()
    );
    assert_eq!(ws.preferences(), Settings::default());
    assert_eq!(fs::read(&path).unwrap(), before);
    let imported = Preferences::validate_import(br#"{
        "version":12, "general":{"language":"zh-CN"}, "appearance":{"accent":"teal","density":"compact","theme":"dark"},
        "workspace":{"recent_limit":1}, "files":{"backup_before_save":true}, "editor":{"default_font_size":24,"default_gate_inputs":4},
        "shortcuts":{"file.save":"f5"}, "simulation":{"mode":"event"}, "generation":{"truth_table":true},
        "recent":[],"window":{"width":720},"panels":{"split":0.25}
    }"#).unwrap();
    let expected = imported.0;
    prefs.import_settings(imported, &mut ws).unwrap();
    prefs.synchronize(&mut ws);
    assert_eq!(ws.preferences(), expected);
    assert_eq!(ws.project(), &project);
    assert_eq!((ws.active_circuit(), ws.simulation_root()), (active, root));
    assert_eq!(ws.layout(), panels);
    assert_eq!(ws.recent(), recent);
    let exported = Preferences::export_settings(&ws);
    assert_eq!(schema::decode_settings(&exported).unwrap(), expected);
    let persisted = schema::decode(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        (persisted.window, persisted.panels, persisted.recent),
        (window, panels, recent.clone())
    );
    let mut restarted = Workspace::default();
    let prefs = Preferences::at(Some(path.clone()), &mut restarted);
    assert!(prefs.is_config_path(&path));
    assert_eq!(restarted.preferences(), expected);
    assert_eq!(restarted.recent(), recent);
}

#[test]
fn refused_import_preserves_live_settings_and_conflicting_or_protected_config() {
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut ws = Workspace::default();
    let settings = Settings {
        theme: logisim_ui::ThemeMode::Dark,
        ..Default::default()
    };
    let mut prefs = Preferences::at(Some(path.clone()), &mut ws);
    fs::write(&path, b"external edit").unwrap();
    for _ in 0..2 {
        let validated = Preferences::validate_import(&schema::encode_settings(settings)).unwrap();
        assert!(prefs.import_settings(validated, &mut ws).is_err());
        assert_eq!(ws.preferences(), Settings::default());
        assert_eq!(fs::read(&path).unwrap(), b"external edit");
        prefs = Preferences::at(Some(path.clone()), &mut ws);
    }
}

#[test]
fn fallback_directory_is_validated_local_state_and_survives_portable_transfers() {
    let temp = Temp::new();
    let dir = temp.0.join("modules");
    fs::create_dir(&dir).unwrap();
    let conf = temp.0.join("preferences.json");
    for value in [
        serde_json::json!(""),
        serde_json::json!("relative"),
        serde_json::json!(temp.0.join("absent")),
        serde_json::json!(false),
        serde_json::json!([1]),
        serde_json::Value::Null,
    ] {
        let input = serde_json::json!({"version":14,"plugins":{"fallback_directory":value}});
        assert!(
            schema::decode(&serde_json::to_vec(&input).unwrap())
                .unwrap()
                .plugins
                .fallback_directory
                .is_none()
        );
    }
    fs::write(
        &conf,
        serde_json::to_vec(
            &serde_json::json!({"version":14,"plugins":{"fallback_directory":dir.join(".")}}),
        )
        .unwrap(),
    )
    .unwrap();
    let mut ws = Workspace::default();
    let mut prefs = Preferences::at(Some(conf.clone()), &mut ws);
    assert_eq!(
        ws.plugin_fallback_directory,
        Some(dir.canonicalize().unwrap())
    );
    let portable: serde_json::Value =
        serde_json::from_slice(&Preferences::export_settings(&ws)).unwrap();
    assert!(portable.get("plugins").is_none());
    let other = serde_json::json!({"version":14,"general":{"language":"zh-CN"},"plugins":{"fallback_directory":"private-on-another-machine"}});
    prefs
        .import_settings(
            Preferences::validate_import(&serde_json::to_vec(&other).unwrap()).unwrap(),
            &mut ws,
        )
        .unwrap();
    assert_eq!(
        ws.plugin_fallback_directory,
        Some(dir.canonicalize().unwrap())
    );
    assert_eq!(
        schema::decode(&fs::read(&conf).unwrap())
            .unwrap()
            .plugins
            .directory(),
        ws.plugin_fallback_directory.as_deref()
    );
    ws.plugin_fallback_directory = None;
    prefs.synchronize(&mut ws);
    assert!(
        schema::decode(&fs::read(conf).unwrap())
            .unwrap()
            .plugins
            .directory()
            .is_none()
    );
}

#[test]
fn failed_local_directory_write_preserves_the_previous_path_on_disk() {
    let temp = Temp::new();
    let previous = temp.0.join("previous");
    fs::create_dir(&previous).unwrap();
    let path = temp.0.join("preferences.json");
    let mut ws = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut ws);
    let previous = previous.canonicalize().unwrap();
    ws.plugin_fallback_directory = Some(previous.clone());
    prefs.synchronize(&mut ws);
    let mut external = fs::read(&path).unwrap();
    external.push(b' ');
    fs::write(&path, &external).unwrap();
    ws.plugin_fallback_directory = None;
    prefs.synchronize(&mut ws);
    assert!(prefs.blocked);
    assert_eq!(fs::read(&path).unwrap(), external);
    assert_eq!(
        schema::decode(&external).unwrap().plugins.directory(),
        Some(previous.as_path())
    );
    prefs.retry(&mut ws);
    prefs.synchronize(&mut ws);
    assert!(
        schema::decode(&fs::read(path).unwrap())
            .unwrap()
            .plugins
            .directory()
            .is_none()
    );
}

#[test]
fn dark_artwork_contrast_round_trips_in_local_and_portable_settings() {
    let temp = Temp::new();
    let path = temp.0.join("preferences.json");
    let mut workspace = Workspace::default();
    let mut prefs = Preferences::at(Some(path.clone()), &mut workspace);
    for enabled in [false, true] {
        let expected = Settings {
            dark_artwork_contrast: enabled,
            ..workspace.preferences()
        };
        let portable = schema::encode_settings(expected);
        let value: serde_json::Value = serde_json::from_slice(&portable).unwrap();
        assert_eq!(value["appearance"]["dark_artwork_contrast"], enabled);
        prefs
            .import_settings(
                Preferences::validate_import(&portable).unwrap(),
                &mut workspace,
            )
            .unwrap();
        assert_eq!(workspace.preferences(), expected);
        let mut restarted = Workspace::default();
        Preferences::at(Some(path.clone()), &mut restarted);
        assert_eq!(restarted.preferences(), expected);
        assert_eq!(Preferences::export_settings(&restarted), portable);
    }
}

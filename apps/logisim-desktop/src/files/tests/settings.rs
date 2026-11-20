use super::*;
use logisim_ui::{Settings, SettingsFileAction};
use std::fs;

impl Harness {
    fn pick_settings(&mut self, action: SettingsFileAction, path: &Path) {
        self.files.request(
            FileAction::Settings(action),
            &self.ctx,
            &mut self.ws,
            &mut self.sim,
        );
        assert!(!self.files.suspends_simulation());
        self.settings_response(Response::Pick(path.to_owned()));
    }

    fn settings_response(&mut self, response: Response) {
        let dialog = self.files.dialog.take().unwrap();
        assert!(dialog.is_settings());
        self.files
            .respond_to_settings(dialog, response, &mut self.preferences, &mut self.ws);
    }
}

#[test]
fn settings_export_confirms_exact_bytes_and_preserves_protected_destinations() {
    let mut h = Harness::default();
    let temp = crate::tests::support::Temp::new();
    let path = temp.path("portable.json");
    let export = SettingsFileAction::Export;
    h.pick_settings(export, &path);
    let original = fs::read(&path).unwrap();
    let settings = Settings {
        theme: logisim_ui::ThemeMode::Dark,
        ..h.ws.preferences()
    };
    h.ws.apply_settings(settings);
    h.settings_response(Response::Cancel);
    h.pick_settings(export, &path);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::ReplaceSettings { .. })
    ));
    h.settings_response(Response::Cancel);
    assert_eq!(fs::read(&path).unwrap(), original);
    h.pick_settings(export, &path);
    fs::write(&path, "external edit").unwrap();
    h.settings_response(Response::Replace);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::SettingsMessage {
            title: "Settings file error",
            ..
        })
    ));
    assert_eq!(fs::read(&path).unwrap(), b"external edit");
    h.settings_response(Response::Cancel);
    h.pick_settings(export, &path);
    h.settings_response(Response::Replace);
    assert_eq!(
        fs::read(&path).unwrap(),
        crate::preferences::Preferences::export_settings(&h.ws)
    );
    h.settings_response(Response::Cancel);

    let mut permissions = fs::metadata(&path).unwrap().permissions();
    let writable = permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let protected = fs::read(&path).unwrap();
    h.pick_settings(export, &path);
    h.settings_response(Response::Replace);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::SettingsMessage {
            title: "Settings file error",
            ..
        })
    ));
    assert_eq!(fs::read(&path).unwrap(), protected);
    fs::set_permissions(&path, writable).unwrap();
    h.settings_response(Response::Cancel);

    h.files.autosave = crate::recovery::Autosave::at(Some(temp.0.clone()));
    for reserved in [
        temp.path("current.json"),
        h.config_dir.path("preferences.json"),
        temp.path("wrong.circ"),
        temp.path("missing/target.json"),
    ] {
        h.pick_settings(export, &reserved);
        assert!(matches!(
            h.files.dialog,
            Some(Dialog::SettingsMessage {
                title: "Settings file error",
                ..
            })
        ));
        assert!(!reserved.exists());
        h.settings_response(Response::Cancel);
    }
    assert!(h.ws.recent().is_empty());
}

#[test]
fn import_cancel_invalid_and_changed_sources_do_not_mutate_the_document() {
    let mut h = Harness::default();
    let temp = crate::tests::support::Temp::new();
    let path = temp.path("new.json");
    h.ws.simulation_mut().ticks = 142;
    let before = h.ws.project().clone();
    let dirty = h.files.present(&h.ws).dirty;
    let candidate = br#"{"version":13,"files":{"backup_before_save":true}}"#;
    fs::write(&path, candidate).unwrap();
    let import = SettingsFileAction::Import;
    h.pick_settings(import, &path);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::ImportSettings { .. })
    ));
    h.settings_response(Response::Cancel);
    assert_eq!(h.ws.preferences(), Settings::default());
    assert!(!h.config_dir.path("preferences.json").exists());
    for bytes in [
        &b""[..],
        b"not json",
        br#"{"version":13,"shortcuts":{"file.save":"primary+o"}}"#,
    ] {
        fs::write(&path, bytes).unwrap();
        h.pick_settings(import, &path);
        assert!(matches!(
            h.files.dialog,
            Some(Dialog::SettingsMessage {
                title: "Settings file error",
                ..
            })
        ));
        h.settings_response(Response::Cancel);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!h.config_dir.path("preferences.json").exists());
        assert_eq!(h.ws.preferences(), Settings::default());
    }
    h.pick_settings(import, &temp.0);
    assert!(matches!(
        h.files.dialog,
        Some(Dialog::SettingsMessage {
            title: "Settings file error",
            ..
        })
    ));
    h.settings_response(Response::Cancel);
    fs::write(&path, candidate).unwrap();
    h.pick_settings(import, &path);
    fs::write(&path, b"changed after validation").unwrap();
    h.settings_response(Response::Import);
    assert!(h.ws.preferences().files.backup_before_save);
    assert_eq!(fs::read(&path).unwrap(), b"changed after validation");
    assert_eq!(h.ws.project(), &before);
    assert_eq!(h.files.present(&h.ws).dirty, dirty);
    assert_eq!(h.ws.simulation_mut().ticks, 142);
}

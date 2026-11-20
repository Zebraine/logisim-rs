//! Explicit transfers of preferences. No document or recovery transaction is involved.
use super::{Dialog, Files, Mode, Response};
use crate::{preferences::Preferences, storage};
use logisim_ui::{SettingsFileAction, Workspace, i18n::tr};
use std::{fs, path::Path};

impl Files {
    pub(super) fn respond_to_settings(
        &mut self,
        dialog: Dialog,
        response: Response,
        preferences: &mut Preferences,
        workspace: &mut Workspace,
    ) {
        let (path, result) = match (dialog, response) {
            (dialog, Response::Wait) => {
                self.dialog = Some(dialog);
                return;
            }
            (_, Response::Cancel) => return,
            (Dialog::Picker(picker), Response::Pick(path)) if picker.mode == Mode::Directory => {
                workspace.plugin_fallback_directory = Some(path);
                return;
            }
            (Dialog::Picker(picker), Response::Pick(path)) => {
                let result = match picker.mode {
                    Mode::Settings(SettingsFileAction::Import) => {
                        storage::read_file(&path, "Choose a .json settings file.")
                            .and_then(|bytes| Preferences::validate_import(&bytes))
                            .map(|settings| Dialog::ImportSettings {
                                path: path.clone(),
                                settings: Box::new(settings),
                            })
                    }
                    Mode::Settings(SettingsFileAction::Export) => self
                        .export_destination(&path, preferences)
                        .and_then(|target| {
                            if target.exists() {
                                let bytes =
                                    storage::read_file(&target, "Choose a .json settings file.")?;
                                Ok(Dialog::ReplaceSettings {
                                    path: target,
                                    bytes,
                                })
                            } else {
                                export(&target, None, workspace)
                            }
                        }),
                    _ => unreachable!("only settings pickers enter the settings workflow"),
                };
                (path, result)
            }
            (Dialog::ReplaceSettings { path, bytes }, Response::Replace) => {
                let result = self
                    .export_destination(&path, preferences)
                    .and_then(|target| {
                        if target != path {
                            return Err(
                                "The export destination changed. Choose the file again.".into()
                            );
                        }
                        export(&target, Some(&bytes), workspace)
                    });
                (path, result)
            }
            (Dialog::ImportSettings { path, settings }, Response::Import) => {
                let result = preferences
                    .import_settings(*settings, workspace)
                    .map(|()| message("Settings imported", &path));
                (path, result)
            }
            _ => unreachable!("settings dialog returned an unrelated response"),
        };
        self.dialog = Some(result.unwrap_or_else(|error| Dialog::SettingsMessage {
            title: "Settings file error",
            body: format!("{}\n\n{}", path.display(), tr(&error)),
        }));
    }

    fn export_destination(
        &self,
        path: &Path,
        preferences: &Preferences,
    ) -> Result<std::path::PathBuf, String> {
        let target = storage::destination(path)?;
        if !target
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("json"))
        {
            return Err("Choose a .json settings file.".into());
        }
        if preferences.is_config_path(&target)
            || self.autosave.reserved(&target)
            || self.storage.path.as_ref() == Some(&target)
            || self.storage.protected.as_ref() == Some(&target)
        {
            return Err("Choose a separate file for the settings export.".into());
        }
        Ok(target)
    }
}

fn export(path: &Path, expected: Option<&[u8]>, workspace: &Workspace) -> Result<Dialog, String> {
    if expected.is_some() {
        // Rename permissions alone would allow replacement of a read-only file.
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        if file
            .metadata()
            .map_err(|e| e.to_string())?
            .permissions()
            .readonly()
        {
            return Err("The selected file is read-only.".into());
        }
    }
    storage::atomic_write(path, &Preferences::export_settings(workspace), expected)?;
    Ok(message("Settings exported", path))
}

fn message(title: &'static str, path: &Path) -> Dialog {
    Dialog::SettingsMessage {
        title,
        body: path.display().to_string(),
    }
}

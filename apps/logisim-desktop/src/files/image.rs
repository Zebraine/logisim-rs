//! Save a viewport image without entering the circuit save/recovery transaction.
use super::{Dialog, Files, Mode, Response};
use crate::{preferences::Preferences, storage};
use ::image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use eframe::egui;
use logisim_ui::i18n::tr;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub(super) struct ImageTarget {
    pub(super) path: PathBuf,
    existing: Option<(u64, SystemTime)>,
}

impl ImageTarget {
    fn inspect(path: &Path) -> Result<Option<(u64, SystemTime)>, String> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => Ok(Some((
                meta.len(),
                meta.modified().map_err(|e| e.to_string())?,
            ))),
            Ok(_) => Err("Choose a .png image file.".into()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub(super) fn write(&self, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(rgba, width, height, ColorType::Rgba8.into())
            .map_err(|e| e.to_string())?;
        storage::atomic_replace(&self.path, &png, None, || {
            if storage::destination(&self.path)? != self.path
                || Self::inspect(&self.path)? != self.existing
            {
                return Err("The export destination changed. Choose the file again.".into());
            }
            if self.existing.is_some() {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .open(&self.path)
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
            Ok(())
        })
    }
}

impl Files {
    fn image_target(
        &self,
        selected: &Path,
        preferences: &Preferences,
    ) -> Result<ImageTarget, String> {
        let path = storage::destination(selected)?;
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            return Err("Choose a .png image file.".into());
        }
        let recovery = preferences
            .recovery_directory()
            .and_then(|dir| dir.canonicalize().ok());
        if recovery.is_some_and(|dir| path.starts_with(dir))
            || preferences.is_config_path(&path)
            || self.storage.path.as_ref() == Some(&path)
            || self.storage.protected.as_ref() == Some(&path)
        {
            return Err("Choose a separate file outside the recovery directory.".into());
        }
        Ok(ImageTarget {
            existing: ImageTarget::inspect(&path)?,
            path,
        })
    }

    pub(super) fn respond_to_image(
        &mut self,
        dialog: Dialog,
        response: Response,
        preferences: &Preferences,
        ctx: &egui::Context,
    ) {
        let target = match (dialog, response) {
            (dialog, Response::Wait) => {
                self.dialog = Some(dialog);
                return;
            }
            (_, Response::Cancel) => return,
            (Dialog::Picker(picker), Response::Pick(path)) if picker.mode == Mode::ExportImage => {
                match self.image_target(&path, preferences) {
                    Ok(target) if target.existing.is_some() => {
                        self.dialog = Some(Dialog::ReplaceImage(target));
                        return;
                    }
                    Ok(target) => target,
                    Err(e) => {
                        self.image_result(&path, Err(e));
                        return;
                    }
                }
            }
            (Dialog::ReplaceImage(target), Response::Replace) => target,
            _ => unreachable!("image export dialog returned an unrelated response"),
        };
        self.image_export = Some(super::capture::Capture::new(target));
        ctx.request_repaint();
    }

    pub(super) fn image_result(&mut self, path: &Path, result: Result<(), String>) {
        let (title, body) = match result {
            Ok(()) => ("Image exported", path.display().to_string()),
            Err(error) => (
                "Image export failed",
                format!("{}\n\n{}", path.display(), tr(&error)),
            ),
        };
        self.dialog = Some(Dialog::ImageMessage { title, body });
    }
}

#[cfg(test)]
mod tests;

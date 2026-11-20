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

    pub(super) fn write(&self, image: &egui::ColorImage) -> Result<(), String> {
        let mut png = Vec::new();
        encode_png(&mut png, image)?;
        storage::atomic_replace(&self.path, &png, None, || {
            if storage::destination(&self.path)? != self.path
                || Self::inspect(&self.path)? != self.existing
            {
                return Err("The export destination changed. Choose the file again.".into());
            }
            if self.existing.is_some() {
                if fs::metadata(&self.path)
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .readonly()
                {
                    return Err("The selected file is read-only.".into());
                }
                fs::OpenOptions::new()
                    .write(true)
                    .open(&self.path)
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
}

pub(super) fn encode_png(
    writer: impl std::io::Write,
    image: &egui::ColorImage,
) -> Result<(), String> {
    let rgba: Vec<_> = image
        .pixels
        .iter()
        .flat_map(|p| p.to_srgba_unmultiplied())
        .collect();
    PngEncoder::new(writer)
        .write_image(
            &rgba,
            image.width() as u32,
            image.height() as u32,
            ColorType::Rgba8.into(),
        )
        .map_err(|e| e.to_string())
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
        self.capture = Some(super::capture::Capture::new(
            super::capture::Target::Export(target),
            ctx,
        ));
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

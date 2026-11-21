//! A session advances its disk baseline only after atomic replacement succeeds.
use super::Session;
use crate::{Error, Result, digest, io};
use logisim_preferences::storage::{WriteError, lock_target, replace_checked};
use serde_json::{Value, json};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

fn write_error(e: WriteError) -> Error {
    match e {
        WriteError::Conflict => Error::new(
            "conflict",
            "Target changed or another writer holds the lock.",
        ),
        WriteError::Io(s) => Error::new("io", s),
    }
}
fn backup(path: &Path, original: &[u8]) -> Result<PathBuf> {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    let backup = PathBuf::from(name);
    logisim_preferences::storage::write_backup(path, &backup, original, logisim_file::MAX_BYTES)
        .map_err(|cause| {
            let mut error = Error::new("backup_failed", write_error(cause).message);
            error.details["path"] = json!(backup);
            error
        })?;
    Ok(backup)
}
impl Session {
    pub fn save(&mut self, id: &str, expected: Option<&str>, with_backup: bool) -> Result<Value> {
        self.check_editable(expected)?;
        let bytes = super::saveable(&self.project, &self.path)?;
        let _guard = lock_target(&self.path).map_err(write_error)?;
        let meta = fs::symlink_metadata(&self.path).map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                Error::new("conflict", "Source file was removed.")
            } else {
                io::file_error(e)
            }
        })?;
        if !meta.is_file() {
            return Err(Error::new(
                "conflict",
                "Source is no longer a regular file.",
            ));
        }
        let original = io::read(&self.path)?;
        if digest::sha256(&original) != self.sha256 {
            return Err(Error::new("conflict", "Source bytes changed on disk."));
        }
        if meta.permissions().readonly() {
            return Err(Error::new("io", "Source file is read-only."));
        }
        fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .map_err(io::file_error)?;
        let backup_path = if with_backup {
            Some(backup(&self.path, &original)?)
        } else {
            None
        };
        replace_checked(&self.path, &bytes, Some(&original), None).map_err(write_error)?;
        self.sha256 = digest::sha256(&bytes);
        Ok(
            json!({"session_id":id,"saved":true,"path":self.path,"source_sha256":self.sha256,"backup_path":backup_path}),
        )
    }
}

impl Session {
    pub fn convert(
        &self,
        id: &str,
        target: &Path,
        format: logisim_file::ProjectFormat,
        expected: Option<&str>,
        backup: bool,
    ) -> Result<Value> {
        self.check_editable(None)?;
        let path = std::path::absolute(target).map_err(io::file_error)?;
        let parent = path
            .parent()
            .ok_or_else(|| Error::invalid("Missing destination directory."))?
            .canonicalize()
            .map_err(io::file_error)?;
        let path = parent.join(
            path.file_name()
                .ok_or_else(|| Error::invalid("Missing destination name."))?,
        );
        if path == self.path || logisim_file::ProjectFormat::from_extension(&path) != Some(format) {
            return Err(Error::invalid(
                "Choose a separate destination with the requested extension.",
            ));
        }
        if self.sha256 != digest::sha256(&io::read(&self.path)?) {
            return Err(Error::new("conflict", "Source bytes changed on disk."));
        }
        let old = if let Some(hash) = expected {
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::invalid("Invalid destination SHA-256."));
            }
            let old = io::read(&path)?;
            if !hash.eq_ignore_ascii_case(&digest::sha256(&old)) {
                return Err(Error::new("conflict", "Destination SHA-256 differs."));
            }
            Some(old)
        } else {
            None
        };
        let mut portable = self.project.clone();
        let from = self.path.parent().expect("absolute source file");
        if from != parent {
            logisim_file::rebase_plugin_sources(&mut portable, from, &parent);
        }
        let bytes = format
            .export(&portable)
            .map_err(|e| Error::new("invalid_project", format!("{}: {e}", path.display())))?;
        logisim_preferences::storage::write_project_copy(
            &path,
            &bytes,
            old.as_deref(),
            backup,
            logisim_file::MAX_BYTES,
        )
        .map_err(write_error)?;
        Ok(
            json!({"session_id":id,"path":path,"format":format.extension(),"sha256":digest::sha256(&bytes)}),
        )
    }
}

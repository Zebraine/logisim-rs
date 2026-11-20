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
            let mut error = Error::new("backup_failed", cause);
            error.details["path"] = json!(backup);
            error
        })?;
    Ok(backup)
}
impl Session {
    pub fn save(&mut self, id: &str, expected: Option<&str>, with_backup: bool) -> Result<Value> {
        self.check_editable(expected)?;
        let xml = super::saveable(&self.project)?;
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
        replace_checked(&self.path, xml.as_bytes(), Some(&original), None).map_err(write_error)?;
        self.sha256 = digest::sha256(xml.as_bytes());
        Ok(
            json!({"session_id":id,"saved":true,"path":self.path,"source_sha256":self.sha256,"backup_path":backup_path}),
        )
    }
}

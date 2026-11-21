//! Explicit offline file operations; no session is opened or rebound.
use crate::{Error, Result, Service, digest, io};
use logisim_file::{ProjectFormat, native};
use logisim_preferences::storage::{CopyError, WriteError, write_project_copy};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

impl Service {
    fn new_output(&self, output: &Path, extension: &str) -> Result<PathBuf> {
        let path = std::path::absolute(self.resolve_path(output)?).map_err(io::file_error)?;
        if !path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case(extension))
        {
            return Err(Error::invalid(format!(
                "Expected a .{extension} output file."
            )));
        }
        if let Some(root) = &self.root {
            root.check_sidecars(&path, false)?;
        }
        let dir = path
            .parent()
            .ok_or_else(|| Error::invalid("Expected a destination directory."))?
            .canonicalize()
            .map_err(io::file_error)?;
        Ok(dir.join(
            path.file_name()
                .ok_or_else(|| Error::invalid("Expected a file name."))?,
        ))
    }

    pub fn migrate_lrs(&self, input: &Path, output: &Path) -> Result<Value> {
        let input = self
            .resolve_path(input)?
            .canonicalize()
            .map_err(io::file_error)?;
        let path = self.new_output(output, "lrs")?;
        if input == path {
            return Err(Error::invalid(
                "Choose a new destination for the binary copy.",
            ));
        }
        let mut project = native::parse_legacy_json(&io::read(&input)?).map_err(invalid)?;
        let from = input.parent().expect("absolute source file");
        let to = path.parent().expect("absolute destination file");
        if from != to {
            logisim_file::rebase_plugin_sources(&mut project, from, to);
        }
        write_new(
            &path,
            &ProjectFormat::Lrs.export(&project).map_err(invalid)?,
        )
    }

    pub fn inspect_lrs(&self, input: &Path, output: Option<&Path>) -> Result<Value> {
        let project =
            logisim_file::parse_native(&io::read(&self.resolve_path(input)?)?).map_err(invalid)?;
        let bytes = native::diagnostic_json(&project).map_err(invalid)?;
        match output {
            Some(output) => write_new(&self.new_output(output, "json")?, &bytes),
            None => serde_json::from_slice(&bytes).map_err(Error::invalid),
        }
    }
}
fn invalid(e: logisim_file::Error) -> Error {
    Error::new("invalid_project", e)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<Value> {
    write_project_copy(path, bytes, None, false, logisim_file::MAX_BYTES).map_err(|e| match e {
        CopyError::Destination(WriteError::Conflict) => {
            Error::new("conflict", "Choose a new output file.")
        }
        _ => Error::new("io", e),
    })?;
    Ok(json!({"path":path,"sha256":digest::sha256(bytes)}))
}

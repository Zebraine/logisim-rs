//! Strict, transactional .circ library snapshots. Never execute an external library.
use logisim_core::{LibraryImport, Project};
use logisim_ui::{i18n::tr, tf};
use std::path::Path;
#[cfg(test)]
mod tests;

pub(crate) fn load(path: &Path, destination: &Project) -> Result<LibraryImport, String> {
    let bytes = crate::storage::read(path)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", tr("File is not UTF-8")))?;
    let mut source = logisim_file::parse(text).map_err(|e| e.to_string())?;
    let absolute = path.canonicalize().map_err(|e| e.to_string())?;
    crate::plugins::absolutize(&mut source.document, &crate::plugins::base(Some(&absolute)));
    if !source.warnings.is_empty() {
        return Err(tf!(
            "Library import cancelled: unsupported content would be omitted.\n\n{}",
            source.warnings.join("\n")
        ));
    }
    let name = path.file_stem().unwrap_or_default().to_string_lossy();
    let imported = destination
        .with_library(&source.document, &name)
        .map_err(|e| tr(&e).to_owned())?;
    // Validate aggregate item/byte budgets and every serializable property before committing.
    logisim_file::serialize(&imported.project).map_err(|e| e.to_string())?;
    Ok(imported)
}

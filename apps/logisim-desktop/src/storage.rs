//! Disk transactions. A failed load or save never advances the document baseline.
use logisim_core::Project;
use logisim_file::MAX_BYTES;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
pub(crate) enum SaveMode<'a> {
    Current,
    /// The file chooser observed absence, or the user confirmed these exact bytes.
    Selected(Option<&'a [u8]>),
}
#[derive(Default)]
pub(crate) struct Storage {
    pub path: Option<PathBuf>,
    pub protected: Option<PathBuf>,
    baseline: Project,
    disk: Vec<u8>,
    recovered: bool,
    // The source changed between the last normal load/save and crash recovery.
    conflicting_source: Option<u64>,
}
impl Storage {
    pub fn source_checksum(&self) -> Option<u64> {
        self.path.as_ref().map(|_| {
            self.conflicting_source
                .unwrap_or_else(|| crate::recovery::checksum(&self.disk))
        })
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn dirty(&self, project: &Project) -> bool {
        self.recovered || *project != self.baseline
    }
    pub fn recovered(source: Option<&Path>, expected: Option<u64>) -> Result<Self> {
        let mut state = Self {
            recovered: true,
            ..Default::default()
        };
        if let Some(path) = source {
            match fs::metadata(path) {
                Ok(_) => {
                    let disk = read(path)?;
                    state.conflicting_source =
                        expected.filter(|hash| *hash != crate::recovery::checksum(&disk));
                    state.path = Some(path.to_owned());
                    state.disk = disk;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(state)
    }
    pub fn load(&mut self, path: &Path) -> Result<(Project, Vec<String>)> {
        let path = destination(path)?;
        let bytes = read(&path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("File is not UTF-8: {e}"))?;
        let mut import = logisim_file::parse(text).map_err(|e| e.to_string())?;
        crate::plugins::absolutize(&mut import.document, &crate::plugins::base(Some(&path)));
        self.recovered = false;
        self.conflicting_source = None;
        self.baseline = import.document.clone();
        self.protected = (!import.warnings.is_empty()).then(|| path.clone());
        self.path = Some(path);
        self.disk = bytes;
        Ok((import.document, import.warnings))
    }
    pub fn save(
        &mut self,
        project: &Project,
        path: &Path,
        mode: SaveMode<'_>,
        backup_before_save: bool,
    ) -> Result<()> {
        let path = destination(path)?;
        if self.protected.as_ref() == Some(&path) {
            return Err("This import omitted unsupported content. Save to a different file to protect the original.".into());
        }
        let expected = match mode {
            SaveMode::Current if self.conflicting_source.is_some() => return Err(CHANGED.into()),
            SaveMode::Current if self.path.as_ref() == Some(&path) => Some(self.disk.as_slice()),
            SaveMode::Current => None,
            SaveMode::Selected(expected) => expected,
        };
        let mut portable = project.clone();
        crate::plugins::rebase(
            &mut portable,
            &crate::plugins::base(self.path.as_deref()),
            &crate::plugins::base(Some(&path)),
        );
        let text = logisim_file::serialize(&portable).map_err(|e| e.to_string())?;
        if backup_before_save && let Some(original) = expected {
            // These are the exact bytes observed at load or Replace confirmation.
            // Verify them before rotating the backup; atomic_write checks the target again.
            check_expected(&path, Some(original))?;
            let mut name = path.as_os_str().to_owned();
            name.push(".bak");
            let backup = PathBuf::from(name);
            if self.protected.as_ref() == Some(&backup) {
                return Err("The backup would replace the protected source. Choose a different destination.".into());
            }
            logisim_preferences::storage::write_backup(&path, &backup, original, MAX_BYTES)
                .map_err(|cause| {
                    let message = match cause {
                        logisim_preferences::storage::WriteError::Conflict => CHANGED.into(),
                        logisim_preferences::storage::WriteError::Io(message) => message,
                    };
                    logisim_ui::tf!(
                        "Could not write backup {}. Save stopped.\n{}",
                        backup.display(),
                        logisim_ui::i18n::tr(&message)
                    )
                })?;
        }
        atomic_write(&path, text.as_bytes(), expected)?;
        self.path = Some(path);
        self.disk = text.into_bytes();
        self.recovered = false;
        self.conflicting_source = None;
        self.baseline = project.clone();
        self.protected = None;
        Ok(())
    }
}
pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    read_file(path, "Choose a regular .circ file.")
}
pub(crate) fn read_file(path: &Path, regular_file_message: &str) -> Result<Vec<u8>> {
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err(regular_file_message.into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    read_bytes(file, regular_file_message)
}
fn read_bytes(file: fs::File, regular_file_message: &str) -> Result<Vec<u8>> {
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err(regular_file_message.into());
    }
    let mut bytes = vec![];
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("File exceeds the 8 MiB limit.".into());
    }
    Ok(bytes)
}
pub(crate) fn destination(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        if !path.is_file() {
            return Err("Choose a file, not a directory.".into());
        }
        return path.canonicalize().map_err(|e| e.to_string());
    }
    // Do not follow dangling symlinks or overwrite them as if they were new files.
    if fs::symlink_metadata(path).is_ok() {
        return Err("Cannot save through a dangling symlink.".into());
    }
    let name = path.file_name().ok_or("Missing file name")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(parent.canonicalize().map_err(|e| e.to_string())?.join(name))
}
const CHANGED: &str = "The destination changed, cannot be read, or needs overwrite confirmation. Use Save As and choose the file again.";
fn check_expected(path: &Path, expected: Option<&[u8]>) -> Result<()> {
    match (read(path), expected) {
        (Ok(actual), Some(expected)) if actual == expected => Ok(()),
        (Err(_), None)
            if fs::symlink_metadata(path)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(())
        }
        _ => Err(CHANGED.into()),
    }
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8], expected: Option<&[u8]>) -> Result<()> {
    atomic_replace(path, bytes, None, || check_expected(path, expected))
}
pub(crate) use logisim_preferences::storage::atomic_replace;

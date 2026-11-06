//! Disk transactions. A failed load or save never advances the document baseline.
use logisim_core::Project;
use logisim_file::MAX_BYTES;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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
}
impl Storage {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn dirty(&self, project: &Project) -> bool {
        *project != self.baseline
    }
    pub fn load(&mut self, path: &Path) -> Result<(Project, Vec<String>)> {
        let path = destination(path)?;
        let bytes = read(&path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("File is not UTF-8: {e}"))?;
        let import = logisim_file::parse(text).map_err(|e| e.to_string())?;
        self.baseline = import.document.clone();
        self.protected = (!import.warnings.is_empty()).then(|| path.clone());
        self.path = Some(path);
        self.disk = bytes;
        Ok((import.document, import.warnings))
    }
    pub fn save(&mut self, project: &Project, path: &Path, mode: SaveMode<'_>) -> Result<()> {
        let path = destination(path)?;
        if self.protected.as_ref() == Some(&path) {
            return Err("This import omitted unsupported content. Save to a different file to protect the original.".into());
        }
        let expected = match mode {
            SaveMode::Current if self.path.as_ref() == Some(&path) => Some(self.disk.as_slice()),
            SaveMode::Current => None,
            SaveMode::Selected(expected) => expected,
        };
        let text = logisim_file::serialize(project).map_err(|e| e.to_string())?;
        atomic_write(&path, text.as_bytes(), expected)?;
        self.path = Some(path);
        self.disk = text.into_bytes();
        self.baseline = project.clone();
        self.protected = None;
        Ok(())
    }
}
pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a regular .circ file.".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a regular .circ file.".into());
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
fn check_expected(path: &Path, expected: Option<&[u8]>) -> Result<()> {
    match (read(path),expected) {
        (Ok(actual),Some(expected)) if actual==expected => Ok(()),
        (Err(_),None) if fs::symlink_metadata(path).is_err_and(|e|e.kind()==std::io::ErrorKind::NotFound) => Ok(()),
        _ => Err("The destination changed, cannot be read, or needs overwrite confirmation. Use Save As and choose the file again.".into()),
    }
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8], expected: Option<&[u8]>) -> Result<()> {
    check_expected(path, expected)?;
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or("Missing parent directory")?;
    let mut created = None;
    for _ in 0..100 {
        let temp = parent.join(format!(
            ".logisim-rs-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => {
                created = Some((temp, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let (temp, mut file) = created.ok_or("Could not create a temporary save file")?;
    let result = (|| {
        if let Ok(meta) = fs::metadata(path) {
            file.set_permissions(meta.permissions())
                .map_err(|e| e.to_string())?;
        }
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        check_expected(path, expected)?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

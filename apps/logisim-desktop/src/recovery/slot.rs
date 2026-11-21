//! One data file and one commit record. The checksum detects interrupted commits.
use crate::storage;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) const CHANGED: &str = "The recovery snapshot changed in another window.";

pub(super) const DATA: &str = "current.circ";
pub(super) const META: &str = "current.json";
pub(super) const LOCK: &str = ".recovery.lock";
pub(super) const OWNER: &str = ".recovery.owner";

/// The OS releases the exclusive writer lease on process exit, including after a crash.
pub(super) fn owner(dir: &Path) -> Result<fs::File> {
    try_lock(dir, OWNER)?.ok_or_else(|| logisim_ui::RecoveryView::FOREIGN_OWNER.into())
}

pub(super) fn try_lock(dir: &Path, name: &str) -> Result<Option<fs::File>> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.with_file_name(name))
        .map_err(|e| e.to_string())?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(e)) => Err(e.to_string()),
    }
}

type Result<T> = std::result::Result<T, String>;

/// FNV-1a is an on-disk integrity check for torn writes, not an authentication token.
pub(crate) fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Metadata {
    format: u8,
    #[serde(deserialize_with = "source_path")]
    pub source: Option<PathBuf>,
    pub saved_at: u64,
    app: String,
    version: String,
    circ_fnv1a64: u64,
    pub source_fnv1a64: Option<u64>,
}

// Unlike a missing field, JSON null explicitly identifies an untitled document.
fn source_path<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> std::result::Result<Option<PathBuf>, D::Error> {
    Option::deserialize(de)
}

#[derive(Debug, Default, PartialEq, Eq)]
enum Entry {
    #[default]
    Missing,
    Bytes(Vec<u8>),
    Invalid,
    Unreadable {
        error: String,
        len: u64,
        modified: SystemTime,
    },
}
impl Entry {
    fn read(path: &Path, limit: usize) -> Result<Self> {
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::Missing),
            Err(e) => Err(e.to_string()),
            Ok(m) if !m.is_file() || m.len() > limit as u64 => Ok(Self::Invalid),
            Ok(m) => Ok(match storage::read(path) {
                Ok(bytes) => Self::Bytes(bytes),
                Err(error) => Self::Unreadable {
                    error,
                    len: m.len(),
                    modified: m.modified().map_err(|e| e.to_string())?,
                },
            }),
        }
    }
    fn bytes(&self) -> Result<Option<&[u8]>> {
        match self {
            Self::Missing => Ok(None),
            Self::Bytes(b) => Ok(Some(b)),
            Self::Unreadable { error, .. } => Err(error.clone()),
            Self::Invalid => Err("The recovery file is invalid or too large.".into()),
        }
    }
}
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Pair {
    data: Entry,
    meta: Entry,
}
impl Pair {
    pub fn read(dir: &Path) -> Result<Self> {
        match fs::symlink_metadata(dir) {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
            Ok(m) if !m.is_dir() => return Err("Cannot access the recovery directory.".into()),
            Ok(_) => {}
        }
        Ok(Self {
            data: Entry::read(&dir.join(DATA), logisim_file::MAX_BYTES)?,
            meta: Entry::read(&dir.join(META), 64 * 1024)?,
        })
    }
    pub fn project(&self, dir: &Path) -> Result<logisim_core::Project> {
        let bytes = self.data.bytes()?.ok_or("Recovery circuit is missing.")?;
        let meta = self.metadata()?;
        let format = meta
            .source
            .as_deref()
            .map(logisim_file::ProjectFormat::binding)
            .unwrap_or(logisim_file::ProjectFormat::Circ);
        let mut import = format
            .parse(bytes)
            .map_err(|e| format!("Invalid recovery circuit: {e}"))?;
        if !import.warnings.is_empty() {
            return Err("The recovery circuit contains unsupported content.".into());
        }
        crate::plugins::absolutize(&mut import.document, dir);
        Ok(import.document)
    }
    pub fn unreadable(&self) -> bool {
        self.read_error().is_some()
    }
    pub fn read_error(&self) -> Option<&str> {
        match (&self.data, &self.meta) {
            (Entry::Unreadable { error, .. }, _) | (_, Entry::Unreadable { error, .. }) => {
                Some(error)
            }
            _ => None,
        }
    }
    pub fn empty(&self) -> bool {
        self.data == Entry::Missing && self.meta == Entry::Missing
    }
    pub fn metadata(&self) -> Result<Metadata> {
        let meta: Metadata =
            serde_json::from_slice(self.meta.bytes()?.ok_or("Recovery metadata is missing.")?)
                .map_err(|_| "Invalid recovery metadata.")?;
        let data = self.data.bytes()?.ok_or("Recovery circuit is missing.")?;
        if meta.format != 1
            || meta.app != "logisim-rs"
            || meta.version.is_empty()
            || meta.source.as_ref().is_some_and(|p| !p.is_absolute())
            || meta.source.is_some() != meta.source_fnv1a64.is_some()
            || meta.saved_at > 253402300799
            || checksum(data) != meta.circ_fnv1a64
        {
            return Err("The recovery files are damaged or incomplete.".into());
        }
        Ok(meta)
    }
    /// The lock covers both renames and cleanup across desktop instances. Compare
    /// the observed pair under the lock so another instance retains its snapshot.
    fn lock(dir: &Path) -> Result<fs::File> {
        try_lock(dir, LOCK)?.ok_or_else(|| "Recovery storage is busy.".into())
    }
    pub fn verify(&self, dir: &Path) -> Result<()> {
        if &Self::read(dir)? != self {
            return Err(CHANGED.into());
        }
        Ok(())
    }
    pub fn write(
        &mut self,
        dir: &Path,
        xml: Vec<u8>,
        source: Option<PathBuf>,
        original: Option<u64>,
    ) -> Result<Metadata> {
        let meta = Metadata {
            format: 1,
            source,
            source_fnv1a64: original,
            saved_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
            app: "logisim-rs".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            circ_fnv1a64: checksum(&xml),
        };
        let record = serde_json::to_vec_pretty(&meta).map_err(|e| e.to_string())?;
        if record.len() > 64 * 1024 {
            return Err("Recovery metadata is too large.".into());
        }
        self.replace(dir, xml, record, storage::atomic_write)?;
        Ok(meta)
    }
    fn replace(
        &mut self,
        dir: &Path,
        xml: Vec<u8>,
        record: Vec<u8>,
        write_record: impl FnOnce(&Path, &[u8], Option<&[u8]>) -> Result<()>,
    ) -> Result<()> {
        let _lock = Self::lock(dir)?;
        let current = Self::read(dir)?;
        // Startup Discard may have emptied our slot since the previous checkpoint.
        if !current.empty() && current != *self {
            return Err(CHANGED.into());
        }
        let data = dir.join(DATA);
        let old_data = current.data.bytes()?;
        let old_meta = current.meta.bytes()?;
        storage::atomic_write(&data, &xml, old_data)?;
        if let Err(e) = write_record(&dir.join(META), &record, old_meta) {
            let rollback = match old_data {
                Some(bytes) => storage::atomic_write(&data, bytes, Some(&xml)),
                None => remove(&data),
            };
            rollback?;
            return Err(e);
        }
        *self = Self {
            data: Entry::Bytes(xml),
            meta: Entry::Bytes(record),
        };
        Ok(())
    }
    pub fn clear(&mut self, dir: &Path, partial: Option<&Self>) -> Result<()> {
        let _lock = Self::lock(dir)?;
        let current = Self::read(dir)?;
        if current.empty() {
            *self = current;
            return Ok(());
        }
        if *self != current && partial != Some(&current) {
            return Err(CHANGED.into());
        }
        let meta = dir.join(META);
        let data = dir.join(DATA);
        match (&current.meta, &current.data) {
            (Entry::Missing, _) => remove(&data)?,
            (_, Entry::Missing) => remove(&meta)?,
            (Entry::Bytes(original), _) => remove_pair(&meta, &data, original)?,
            (_, Entry::Bytes(original)) => remove_pair(&data, &meta, original)?,
            // Neither entry can be buffered for rollback. A temporary hard link
            // preserves even an unreadable or oversized file if the second unlink fails.
            _ => {
                let temp = dir.join(format!(".logisim-clear-{}.tmp", std::process::id()));
                fs::hard_link(&meta, &temp).map_err(|e| e.to_string())?;
                if let Err(e) = remove(&meta) {
                    remove(&temp)?;
                    return Err(e);
                }
                if let Err(e) = remove(&data) {
                    fs::rename(&temp, &meta).map_err(|e| e.to_string())?;
                    return Err(e);
                }
                remove(&temp)?;
            }
        }
        *self = Self::default();
        Ok(())
    }
}
fn remove_pair(first: &Path, second: &Path, original: &[u8]) -> Result<()> {
    remove(first)?;
    if let Err(error) = remove(second) {
        storage::atomic_write(first, original, None)?;
        return Err(error);
    }
    Ok(())
}
fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests;

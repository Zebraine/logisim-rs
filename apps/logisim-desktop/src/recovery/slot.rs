//! One data file and one commit record. The checksum detects interrupted commits.
use crate::storage;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const DATA: &str = "current.circ";
pub(super) const META: &str = "current.json";
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
    pub source: Option<PathBuf>,
    pub saved_at: u64,
    app: String,
    version: String,
    circ_fnv1a64: u64,
    pub source_fnv1a64: Option<u64>,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum Entry {
    #[default]
    Missing,
    Bytes(Vec<u8>),
    Invalid,
}
impl Entry {
    fn read(path: &Path, limit: usize) -> Result<Self> {
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::Missing),
            Err(e) => Err(e.to_string()),
            Ok(m) if !m.is_file() || m.len() > limit as u64 => Ok(Self::Invalid),
            Ok(_) => storage::read(path).map(Self::Bytes),
        }
    }
    fn bytes(&self) -> Result<Option<&[u8]>> {
        match self {
            Self::Missing => Ok(None),
            Self::Bytes(b) => Ok(Some(b)),
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
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.with_file_name(".recovery.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|_| "Recovery storage is busy.")?;
        Ok(lock)
    }
    pub fn verify(&self, dir: &Path) -> Result<()> {
        if &Self::read(dir)? != self {
            return Err("The recovery snapshot changed in another window.".into());
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
        self.verify(dir)?;
        let data = dir.join(DATA);
        let old_data = self.data.bytes()?;
        let old_meta = self.meta.bytes()?;
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
    pub fn clear(&mut self, dir: &Path) -> Result<()> {
        let _lock = Self::lock(dir)?;
        let current = Self::read(dir)?;
        if current.empty() {
            *self = current;
            return Ok(());
        }
        if *self != current {
            return Err("The recovery snapshot changed in another window.".into());
        }
        remove(&dir.join(DATA))?;
        self.data = Entry::Missing;
        remove(&dir.join(META))?;
        self.meta = Entry::Missing;
        Ok(())
    }
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

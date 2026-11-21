//! Atomic replacement shared with desktop storage. Preference writers additionally
//! lock a stable sibling inode, so concurrent renames cannot invalidate the lock.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
type Result<T> = std::result::Result<T, String>;
pub fn atomic_replace(
    path: &Path,
    bytes: &[u8],
    permissions: Option<fs::Permissions>,
    mut verify: impl FnMut() -> Result<()>,
) -> Result<()> {
    verify()?;
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
        if let Some(permissions) =
            permissions.or_else(|| fs::metadata(path).ok().map(|m| m.permissions()))
        {
            file.set_permissions(permissions)
                .map_err(|e| e.to_string())?;
        }
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        verify()?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[derive(Debug)]
pub enum WriteError {
    Conflict,
    Io(String),
}
impl From<std::io::Error> for WriteError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict => f.write_str("Configuration changed or another writer is active."),
            Self::Io(e) => f.write_str(e),
        }
    }
}
pub fn write_preferences(
    path: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
) -> std::result::Result<(), WriteError> {
    use std::io::ErrorKind;
    let io = |e: std::io::Error| WriteError::Io(e.to_string());
    let target = match path.canonicalize() {
        Ok(target) => target,
        Err(e)
            if e.kind() == ErrorKind::NotFound
                && fs::symlink_metadata(path).is_err_and(|e| e.kind() == ErrorKind::NotFound) =>
        {
            path.to_owned()
        }
        Err(e) => return Err(io(e)),
    };
    let _lock = lock_target(&target)?;
    replace_checked(&target, bytes, expected, None)
}

/// Release the exclusive lock at the end of the transaction, even if a child
/// between fork and exec still holds a copy of the open file description.
#[derive(Debug)]
pub struct FileLock(fs::File);

impl FileLock {
    pub fn try_acquire(file: fs::File) -> std::result::Result<Self, fs::TryLockError> {
        file.try_lock()?;
        Ok(Self(file))
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Lock a stable sibling inode across backup and target replacements. The empty sidecar stays
/// in place after unlocking so all cooperating writers continue to lock the same inode.
pub fn lock_target(target: &Path) -> std::result::Result<FileLock, WriteError> {
    use std::fs::TryLockError;
    let io = |e: std::io::Error| WriteError::Io(e.to_string());
    let mut lock_name = target.as_os_str().to_owned();
    lock_name.push(".lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_name)
        .map_err(io)?;
    if !lock.metadata().map_err(io)?.is_file() {
        return Err(WriteError::Io("Expected a regular lock file.".into()));
    }
    FileLock::try_acquire(lock).map_err(|error| match error {
        TryLockError::WouldBlock => WriteError::Conflict,
        TryLockError::Error(e) => io(e),
    })
}

pub fn replace_checked(
    target: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
    permissions: Option<fs::Permissions>,
) -> std::result::Result<(), WriteError> {
    use std::io::ErrorKind;
    let io = |e: std::io::Error| WriteError::Io(e.to_string());
    let check = || -> std::result::Result<(), WriteError> {
        match (fs::symlink_metadata(target), expected) {
            (Ok(meta), Some(old)) if meta.is_file() => {
                let file = fs::File::open(target).map_err(io)?;
                let mut actual = Vec::new();
                file.take(old.len() as u64 + 1)
                    .read_to_end(&mut actual)
                    .map_err(io)?;
                if actual == old {
                    Ok(())
                } else {
                    Err(WriteError::Conflict)
                }
            }
            (Err(e), None) if e.kind() == ErrorKind::NotFound => Ok(()),
            (Err(e), _) if e.kind() != ErrorKind::NotFound => Err(io(e)),
            _ => Err(WriteError::Conflict),
        }
    };
    let mut reason = None;
    // Verify again just before the rename, including changes by non-cooperating editors.
    atomic_replace(target, bytes, permissions, || {
        check().map_err(|e| {
            let message = e.to_string();
            reason = Some(e);
            message
        })
    })
    .map_err(|e| reason.unwrap_or(WriteError::Io(e)))
}

/// Atomically rotate one generation using the target's original bytes. Existing backups
/// must be writable regular files and retain their permissions.
pub fn write_backup(
    target: &Path,
    backup: &Path,
    original: &[u8],
    max_bytes: usize,
) -> std::result::Result<(), WriteError> {
    const REGULAR: &str = "The backup path must be a writable regular file.";
    let (previous, permissions) = match fs::symlink_metadata(backup) {
        Ok(meta) => {
            if !meta.is_file() || meta.permissions().readonly() {
                return Err(WriteError::Io(REGULAR.into()));
            }
            let file = fs::OpenOptions::new().read(true).write(true).open(backup)?;
            if !file.metadata()?.is_file() {
                return Err(WriteError::Io(REGULAR.into()));
            }
            let mut data = Vec::new();
            file.take(max_bytes as u64 + 1).read_to_end(&mut data)?;
            if data.len() > max_bytes {
                return Err(WriteError::Io(format!(
                    "File exceeds the {} MiB limit.",
                    max_bytes / (1024 * 1024)
                )));
            }
            (Some(data), meta.permissions())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (None, fs::metadata(target)?.permissions())
        }
        Err(e) => return Err(e.into()),
    };
    replace_checked(backup, original, previous.as_deref(), Some(permissions))
}

#[derive(Debug)]
pub enum CopyError {
    Destination(WriteError),
    Backup(std::path::PathBuf, WriteError),
}
impl std::fmt::Display for CopyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Destination(e) => e.fmt(f),
            Self::Backup(path, e) => write!(f, "Backup failed ({}): {e}", path.display()),
        }
    }
}

fn confirm_target(target: &Path, old: &[u8]) -> std::result::Result<(), WriteError> {
    let metadata = fs::symlink_metadata(target)?;
    if !metadata.is_file() || metadata.permissions().readonly() {
        return Err(WriteError::Io(
            "Destination must be a writable regular file.".into(),
        ));
    }
    let mut actual = Vec::new();
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)?
        .take(old.len() as u64 + 1)
        .read_to_end(&mut actual)?;
    if actual != old {
        return Err(WriteError::Conflict);
    }
    Ok(())
}

/// Write an explicitly selected project copy. Hold the sibling lock through
/// verification of the confirmed bytes, backup rotation and checked rename.
pub fn write_project_copy(
    target: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
    backup: bool,
    max_bytes: usize,
) -> std::result::Result<(), CopyError> {
    let _lock = lock_target(target).map_err(CopyError::Destination)?;
    if let Some(old) = expected {
        confirm_target(target, old).map_err(CopyError::Destination)?;
        if backup {
            let mut name = target.as_os_str().to_owned();
            name.push(".bak");
            let path = std::path::PathBuf::from(name);
            write_backup(target, &path, old, max_bytes).map_err(|e| CopyError::Backup(path, e))?;
        }
    }
    replace_checked(target, bytes, expected, None).map_err(CopyError::Destination)
}

#[cfg(test)]
mod tests;

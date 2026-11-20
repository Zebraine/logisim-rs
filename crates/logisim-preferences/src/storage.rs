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
    use std::{fs::TryLockError, io::ErrorKind};
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
    let mut lock_name = target.as_os_str().to_owned();
    lock_name.push(".lock");
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_name)
        .map_err(io)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Err(WriteError::Conflict),
        Err(TryLockError::Error(e)) => return Err(io(e)),
    }
    // The on-disk lock file remains after unlocking. Removing it would let two writers
    // lock different inodes for the same configuration.
    let check = || -> std::result::Result<(), WriteError> {
        match (fs::File::open(&target), expected) {
            (Ok(file), Some(old)) => {
                if !file.metadata().map_err(io)?.is_file() {
                    return Err(WriteError::Conflict);
                }
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
    check()?;
    let mut reason = None;
    // Verify again just before the rename, including changes by non-cooperating editors.
    atomic_replace(&target, bytes, None, || {
        check().map_err(|e| {
            let message = e.to_string();
            reason = Some(e);
            message
        })
    })
    .map_err(|e| reason.unwrap_or(WriteError::Io(e)))
}

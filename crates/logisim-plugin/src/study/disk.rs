//! The release experiment provisions an empty 0700 directory under a trusted parent.
//! A Wasmtime cache holds executable code. A cache supplied by a project is never an input.
use super::*;
use std::path::Path;

#[cfg(target_os = "linux")]
pub(super) fn open(path: &Path) -> Result<backend::Cache> {
    use std::{fs, os::unix::fs::MetadataExt};
    let owner = fs::metadata("/proc/self").map_err(error)?.uid();
    let mount_owner = fs::metadata("/").map_err(error)?.uid();
    if !path.is_absolute() || path.canonicalize().map_err(error)? != path {
        return Err("Expected an absolute private cache directory without symlinks".into());
    }
    for dir in path.ancestors() {
        let meta = fs::symlink_metadata(dir).map_err(error)?;
        let private = meta.uid() == owner && meta.mode() & 0o077 == 0;
        // Root-owned mounts may have an unmapped uid in an unprivileged Linux namespace.
        // Accept the system root and sticky /tmp specifically, never an arbitrary unmapped owner.
        let trusted_owner = meta.uid() == 0
            || meta.uid() == owner
            || dir == Path::new("/")
            || (dir == Path::new("/tmp") && meta.uid() == mount_owner && meta.mode() & 0o1000 != 0);
        let trusted_parent =
            trusted_owner && (meta.mode() & 0o022 == 0 || meta.mode() & 0o1000 != 0);
        if !meta.is_dir() || !trusted_parent || (dir == path && !private) {
            return Err(
                "Native cache requires a private owned directory and trusted ancestors".into(),
            );
        }
    }
    let mut options = backend::CacheConfig::new();
    options.with_directory(path);
    backend::Cache::new(options).map_err(error)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn open(_: &Path) -> Result<backend::Cache> {
    Err("The private disk-cache experiment has been qualified only on Linux".into())
}

//! The startup root applies to caller-supplied paths. Plugin dependencies retain their own resolver.
use crate::{Error, Result, io};
use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct Root {
    requested: PathBuf,
    canonical: PathBuf,
}

fn outside() -> Error {
    Error::new(
        "path_outside_root",
        "Path is outside the configured root or its link destination cannot be verified.",
    )
}

impl Root {
    pub fn new(directory: &Path) -> Result<Self> {
        if directory.as_os_str().is_empty() {
            return Err(Error::invalid("--root requires an existing directory."));
        }
        let canonical = directory.canonicalize().map_err(io::file_error)?;
        if !canonical.is_dir() {
            return Err(Error::invalid("--root requires an existing directory."));
        }
        Ok(Self {
            requested: std::path::absolute(directory).map_err(io::file_error)?,
            canonical,
        })
    }

    pub fn resolve(&self, input: &Path) -> Result<PathBuf> {
        self.checked(input, false)
    }

    /// A recorded fallback can have missing parent directories. Authorize its whole
    /// path before persisting it, even when a different candidate already exists.
    pub fn check_candidate(&self, path: &Path) -> Result<()> {
        self.checked(path, true).map(|_| ())
    }

    fn checked(&self, input: &Path, allow_missing_parents: bool) -> Result<PathBuf> {
        let relative = if input.is_absolute() {
            input
                .strip_prefix(&self.canonical)
                .or_else(|_| input.strip_prefix(&self.requested))
                .map_err(|_| outside())?
        } else {
            input
        };
        // A long-lived session must not follow a root replaced since startup.
        if self.canonical.canonicalize().map_err(|_| outside())? != self.canonical {
            return Err(outside());
        }
        let mut resolved = self.canonical.clone();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    if resolved == self.canonical {
                        return Err(outside());
                    }
                    resolved.pop();
                }
                Component::Normal(name) => {
                    resolved.push(name);
                    // On Windows a prefixed component may replace the path during push.
                    if !resolved.starts_with(&self.canonical) {
                        return Err(outside());
                    }
                    match fs::symlink_metadata(&resolved) {
                        Ok(_) => {
                            resolved = resolved.canonicalize().map_err(|_| outside())?;
                            if !resolved.starts_with(&self.canonical) {
                                return Err(outside());
                            }
                        }
                        // Only the parent can be canonicalized for a new sidecar or a
                        // removed source. The save operation handles a missing source as a conflict.
                        Err(e)
                            if (allow_missing_parents || components.peek().is_none())
                                && e.kind() == ErrorKind::NotFound => {}
                        Err(e) => return Err(io::file_error(e)),
                    }
                }
                Component::Prefix(_) | Component::RootDir => return Err(outside()),
            }
        }
        Ok(resolved)
    }

    pub fn check_sidecars(&self, target: &Path, with_backup: bool) -> Result<()> {
        let suffixes: &[&str] = if with_backup {
            &[".lock", ".bak"]
        } else {
            &[".lock"]
        };
        for suffix in suffixes {
            let mut sibling = target.as_os_str().to_owned();
            sibling.push(suffix);
            self.resolve(Path::new(&sibling))?;
        }
        Ok(())
    }
}

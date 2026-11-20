//! Single-document crash recovery. All disk work happens on deadlines or explicit actions.
mod slot;
use crate::{plugins, storage::Storage};
use logisim_core::Project;
use logisim_ui::FileSettings;
pub(crate) use slot::checksum;
use slot::{Metadata, Pair};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct Autosave {
    directory: Option<PathBuf>,
    observed: Pair,
    metadata: Option<Metadata>,
    unavailable: bool,
    owned: bool,
    last_success: Option<Instant>,
    next_due: Option<Instant>,
    options: Option<FileSettings>,
    error: Option<String>,
}
impl Autosave {
    pub fn at(directory: Option<PathBuf>) -> Self {
        let mut state = Self {
            directory,
            ..Self::default()
        };
        match state.dir().and_then(Pair::read) {
            Ok(pair) => {
                if !pair.empty() {
                    match pair.metadata() {
                        Ok(meta) => state.metadata = Some(meta),
                        Err(e) => state.error = Some(e),
                    }
                }
                state.observed = pair;
            }
            Err(e) => {
                state.unavailable = true;
                state.error = Some(e);
            }
        }
        state
    }
    fn dir(&self) -> Result<&Path, String> {
        self.directory
            .as_deref()
            .ok_or_else(|| "User configuration directory is unavailable.".into())
    }
    pub fn reserved(&self, path: &Path) -> bool {
        self.directory.as_ref().is_some_and(|dir| {
            let base = dir.canonicalize().unwrap_or_else(|_| dir.clone());
            [slot::DATA, slot::META]
                .iter()
                .any(|name| path == base.join(name))
        })
    }
    pub fn update(
        &mut self,
        now: Instant,
        options: FileSettings,
        dirty: bool,
        storage: &Storage,
        project: &Project,
    ) {
        if self.options != Some(options) {
            self.next_due = None;
            self.options = Some(options);
        }
        if !options.autosave
            || !dirty
            || storage.protected.is_some()
            || self.unavailable
            || (!self.owned && !self.observed.empty())
        {
            self.next_due = None;
            return;
        }
        let interval = Duration::from_secs(u64::from(options.autosave_minutes) * 60);
        if now < *self.next_due.get_or_insert(now + interval) {
            return;
        }
        let result = self.write(storage, project);
        match result {
            Ok(()) => {
                self.owned = true;
                self.last_success = Some(now);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        self.next_due = Some(now + interval);
    }
    fn write(&mut self, storage: &Storage, project: &Project) -> Result<(), String> {
        let dir = self.dir()?.to_owned();
        if storage.path.as_ref().is_some_and(|p| self.reserved(p)) {
            return Err("Recovery files cannot be used as a project destination.".into());
        }
        let mut portable = project.clone();
        plugins::rebase(&mut portable, &plugins::base(storage.path.as_deref()), &dir);
        let xml = logisim_file::serialize(&portable).map_err(|e| e.to_string())?;
        let meta = self.observed.write(
            &dir,
            xml.into_bytes(),
            storage.path.clone(),
            storage.source_checksum(),
        )?;
        self.metadata = Some(meta);
        Ok(())
    }
    pub fn finish_document(&mut self) {
        self.next_due = None;
        self.last_success = None;
        if self.owned {
            self.clear();
            self.owned = false;
        }
    }
    fn clear(&mut self) -> bool {
        let result = self
            .dir()
            .map(Path::to_owned)
            .and_then(|dir| self.observed.clear(&dir));
        match result {
            Ok(()) => {
                self.metadata = None;
                self.owned = false;
                self.error = None;
                self.next_due = None;
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;

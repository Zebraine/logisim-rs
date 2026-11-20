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
    changed: bool,
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
        state.refresh();
        state
    }
    pub fn refresh(&mut self) {
        self.changed = true;
        match self.dir().and_then(Pair::read) {
            Ok(pair) => {
                let was_unavailable = std::mem::take(&mut self.unavailable);
                if pair != self.observed || was_unavailable {
                    self.owned &= pair == self.observed;
                    self.metadata = None;
                    self.error = None;
                    if !pair.empty() {
                        let inspected = pair.metadata().and_then(|meta| {
                            if meta.source.as_ref().is_some_and(|p| self.reserved(p)) {
                                return Err("Invalid recovery metadata.".into());
                            }
                            pair.project(self.dir()?)?;
                            Ok(meta)
                        });
                        match inspected {
                            Ok(meta) => self.metadata = Some(meta),
                            Err(e) => self.error = Some(e),
                        }
                    }
                    self.observed = pair;
                }
            }
            Err(e) => {
                self.unavailable = true;
                self.error = Some(e);
            }
        }
    }
    pub fn pending(&self) -> bool {
        !self.observed.empty()
    }
    pub fn view(&self) -> logisim_ui::RecoveryView {
        use logisim_ui::SnapshotState;
        let state = if self.unavailable || self.observed.unreadable() {
            SnapshotState::Unavailable {
                snapshot_present: self.pending(),
            }
        } else if let Some(meta) = &self.metadata {
            SnapshotState::Available {
                source: meta.source.as_ref().map(|p| p.display().to_string()),
                saved_at: meta.saved_at,
            }
        } else if self.pending() {
            SnapshotState::Damaged
        } else {
            SnapshotState::Empty
        };
        logisim_ui::RecoveryView {
            state,
            directory: self.directory.as_ref().map(|p| p.display().to_string()),
            error: self.error.clone(),
        }
    }
    pub fn publish(&mut self, w: &mut logisim_ui::Workspace) {
        if std::mem::take(&mut self.changed) {
            w.set_recovery_view(self.view());
        }
    }
    pub fn recover(&mut self, storage: &mut Storage) -> Result<Project, String> {
        let dir = self.dir()?;
        self.observed.verify(dir)?;
        let meta = self
            .metadata
            .as_ref()
            .ok_or("The recovery files are damaged or incomplete.")?;
        let project = self.observed.project(dir)?;
        *storage = Storage::recovered(meta.source.as_deref(), meta.source_fnv1a64)?;
        self.owned = true;
        self.error = None;
        self.changed = true;
        Ok(project)
    }
    pub fn report(&mut self, error: String) {
        self.error = Some(error);
        self.changed = true;
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
            || (!self.owned && !self.observed.empty())
        {
            self.next_due = None;
            return;
        }
        let interval = Duration::from_secs(u64::from(options.autosave_minutes) * 60);
        if now < *self.next_due.get_or_insert(now + interval) {
            return;
        }
        if self.unavailable {
            self.refresh();
            if self.unavailable || self.pending() {
                self.next_due = Some(now + interval);
                return;
            }
        }
        self.changed = true;
        let result = self.write(storage, project);
        match result {
            Ok(()) => {
                self.owned = true;
                self.last_success = Some(now);
                self.error = None;
            }
            Err(e) => {
                self.refresh();
                self.report(e);
            }
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
        }
    }
    pub fn clear(&mut self) {
        self.changed = true;
        self.owned = false;
        let result = self
            .dir()
            .map(Path::to_owned)
            .and_then(|dir| self.observed.clear(&dir));
        match result {
            Ok(()) => {
                self.metadata = None;
                self.error = None;
                self.next_due = None;
            }
            Err(e) => {
                self.refresh();
                if !self.observed.empty() && self.observed.metadata().is_err() {
                    self.metadata = None;
                }
                self.report(e);
            }
        }
    }
}

#[cfg(test)]
mod tests;

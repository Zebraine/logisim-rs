//! Single-document crash recovery. All disk work happens on deadlines or explicit actions.
mod slot;
use crate::{plugins, storage::Storage};
use logisim_core::Project;
use logisim_ui::FileSettings;
use slot::CHANGED;
pub(crate) use slot::checksum;
use slot::{Metadata, Pair};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const INVALID_INTERVAL: &str = "Autosave interval must be between 1 and 30 minutes.";

/// Keep the last verified pair for recovery and the most recent observation for
/// an explicit clear while a file is still unreadable.
#[derive(Default)]
enum Access {
    #[default]
    Readable,
    UnreadableFile(Pair),
    UnavailableDirectory,
}

#[derive(Default)]
pub(crate) struct Autosave {
    directory: Option<PathBuf>,
    observed: Pair,
    metadata: Option<Metadata>,
    access: Access,
    changed: bool,
    owned: Option<std::fs::File>,
    written: Option<Project>,
    last_success: Option<Instant>,
    next_due: Option<Instant>,
    schedule: Option<(bool, u8)>,
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
                // A read error cannot establish that our previously readable files changed.
                if !self.observed.empty()
                    && !self.observed.unreadable()
                    && let Some(error) = pair.read_error()
                {
                    self.error = Some(error.to_owned());
                    self.access = Access::UnreadableFile(pair);
                    return;
                }
                let was_unavailable = !matches!(std::mem::take(&mut self.access), Access::Readable);
                if pair != self.observed || was_unavailable {
                    if pair != self.observed {
                        self.owned = None;
                        self.written = None;
                    }
                    self.metadata = None;
                    self.error = None;
                    if !pair.empty() {
                        match self.inspect(&pair) {
                            Ok((meta, _)) => self.metadata = Some(meta),
                            Err(e) => self.error = Some(e),
                        }
                    }
                    self.observed = pair;
                }
            }
            Err(e) => {
                self.access = Access::UnavailableDirectory;
                self.error = Some(e);
            }
        }
    }
    pub fn pending(&self) -> bool {
        !self.observed.empty()
    }
    pub fn view(&self) -> logisim_ui::RecoveryView {
        use logisim_ui::SnapshotState;
        let state = if !matches!(self.access, Access::Readable) || self.observed.unreadable() {
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
            owned: self.owned.is_some(),
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
        self.recover_with_lease(storage)
            .inspect_err(|error| self.report(error.clone()))
    }

    fn inspect(&self, pair: &Pair) -> Result<(Metadata, Project), String> {
        let meta = pair.metadata()?;
        if meta.source.as_ref().is_some_and(|p| self.reserved(p)) {
            return Err("Invalid recovery metadata.".into());
        }
        Ok((meta, pair.project(self.dir()?)?))
    }

    fn recover_with_lease(&mut self, storage: &mut Storage) -> Result<Project, String> {
        let dir = self.dir()?;
        let lease = slot::owner(dir)?;
        // Only a new click after the changed-snapshot warning can accept a
        // replacement. Stage it locally until parsing and source checks succeed.
        let replacement = if self.error.as_deref() == Some(CHANGED) {
            Some(Pair::read(dir)?)
        } else {
            None
        };
        let pair = replacement.as_ref().unwrap_or(&self.observed);
        pair.verify(dir)?;
        let (meta, project) = self.inspect(pair)?;
        *storage = Storage::recovered(meta.source.as_deref(), meta.source_fnv1a64)?;
        if let Some(pair) = replacement {
            self.observed = pair;
        }
        self.metadata = Some(meta);
        self.access = Access::Readable;
        self.owned = Some(lease);
        self.error = None;
        self.changed = true;
        self.written = Some(project.clone());
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
            path == base.with_file_name(slot::LOCK)
                || path == base.with_file_name(slot::OWNER)
                || [slot::DATA, slot::META]
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
        let schedule = (options.autosave, options.autosave_minutes);
        if self.schedule != Some(schedule) {
            self.next_due = None;
            self.schedule = Some(schedule);
            if !options.valid_interval() {
                self.report(INVALID_INTERVAL.into());
            } else if self.error.as_deref() == Some(INVALID_INTERVAL) {
                self.error = None;
                self.changed = true;
            }
        }
        if !options.valid_interval()
            || !options.autosave
            || !dirty
            || storage.protected.is_some()
            || (self.owned.is_none() && !self.observed.empty())
        {
            self.next_due = None;
            return;
        }
        let interval = Duration::from_secs(u64::from(options.autosave_minutes) * 60);
        if now < *self.next_due.get_or_insert(now + interval) {
            return;
        }
        if !matches!(self.access, Access::Readable) {
            self.refresh();
            if !matches!(self.access, Access::Readable) || (self.pending() && self.owned.is_none())
            {
                self.next_due = Some(now + interval);
                return;
            }
        }
        self.next_due = Some(now + interval);
        if self.written.as_ref() == Some(project) {
            return;
        }
        self.changed = true;
        let result = self.write(storage, project);
        match result {
            Ok(()) => {
                self.written = Some(project.clone());
                self.last_success = Some(now);
                self.error = None;
            }
            Err(e) => {
                self.refresh();
                self.report(e);
            }
        }
    }
    fn write(&mut self, storage: &Storage, project: &Project) -> Result<(), String> {
        let dir = self.dir()?.to_owned();
        if storage.path.as_ref().is_some_and(|p| self.reserved(p)) {
            return Err("Recovery files cannot be used as a project destination.".into());
        }
        let lease = if self.owned.is_none() {
            Some(slot::owner(&dir)?)
        } else {
            None
        };
        let mut portable = project.clone();
        plugins::rebase(&mut portable, &plugins::base(storage.path.as_deref()), &dir);
        let bytes = storage.encode(storage.bound_format(), &mut portable)?;
        let meta =
            self.observed
                .write(&dir, bytes, storage.path.clone(), storage.source_checksum())?;
        if let Some(lease) = lease {
            self.owned = Some(lease);
        }
        self.metadata = Some(meta);
        Ok(())
    }
    pub fn finish_document(&mut self) {
        self.next_due = None;
        self.last_success = None;
        self.written = None;
        if self.owned.is_some()
            && let Err(error) = self.clear_owned()
        {
            self.report(error);
        }
    }

    pub fn clear_owned(&mut self) -> Result<(), String> {
        if self.owned.is_none() {
            return Err(logisim_ui::RecoveryView::FOREIGN_OWNER.into());
        }
        self.remove_observed()
    }

    /// Settings can also remove explicitly confirmed, damaged files. The same
    /// compare-under-lock protects a repaired or replaced pair.
    pub fn clear_from_settings(&mut self) -> Result<(), String> {
        if self.owned.is_some() {
            return self.clear_owned();
        }
        if !self.pending() || (matches!(self.access, Access::Readable) && self.metadata.is_some()) {
            return Err(logisim_ui::RecoveryView::FOREIGN_OWNER.into());
        }
        // Hold OWNER across verification and both removals. A writer cannot
        // acquire the slot between our ownership check and the operation lock.
        let _lease = slot::owner(self.dir()?)?;
        self.remove_observed()
    }

    pub fn discard(&mut self) -> Result<(), String> {
        self.remove_observed()
    }

    fn remove_observed(&mut self) -> Result<(), String> {
        let dir = self.dir()?.to_owned();
        let partial = match &self.access {
            Access::UnreadableFile(pair) => Some(pair),
            Access::Readable | Access::UnavailableDirectory => None,
        };
        if let Err(error) = self.observed.clear(&dir, partial) {
            self.refresh();
            return Err(error);
        }
        self.owned = None;
        self.metadata = None;
        self.access = Access::Readable;
        self.error = None;
        self.next_due = None;
        self.last_success = None;
        self.written = None;
        self.changed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

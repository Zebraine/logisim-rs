//! One bounded undo timeline shared by all document editing surfaces.
use super::session::{EditSession, Snapshot};
use std::collections::VecDeque;

const LIMIT: usize = 100;

struct Edit {
    before: Snapshot,
    after: Snapshot,
}

#[derive(Default)]
pub(super) struct History {
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    properties: Option<Snapshot>,
}

impl History {
    /// The session must be synchronized and normalized before recording.
    /// Navigation and selection alone do not create edits or invalidate Redo.
    pub fn record(&mut self, before: Snapshot, session: &EditSession) {
        if before.project == session.project {
            return;
        }
        self.undo.push_back(Edit {
            before,
            after: session.snapshot(),
        });
        if self.undo.len() > LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    pub fn begin_properties(&mut self, session: &EditSession) {
        self.properties.get_or_insert_with(|| session.snapshot());
    }

    pub fn take_properties(&mut self) -> Option<Snapshot> {
        self.properties.take()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.properties.is_some()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty() && self.properties.is_none()
    }

    pub fn undo(&mut self) -> Option<Snapshot> {
        let edit = self.undo.pop_back()?;
        let snapshot = edit.before.clone();
        self.redo.push(edit);
        Some(snapshot)
    }

    pub fn redo(&mut self) -> Option<Snapshot> {
        let edit = self.redo.pop()?;
        let snapshot = edit.after.clone();
        self.undo.push_back(edit);
        Some(snapshot)
    }
}

#[cfg(test)]
mod tests;

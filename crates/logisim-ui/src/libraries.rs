//! Session palette groups for imported, editable project snapshots.
use crate::Workspace;
use logisim_core::{Id, LibraryImport};

pub(crate) struct LibraryGroup {
    pub(crate) name: String,
    pub(crate) circuits: Vec<Id>,
}

impl Workspace {
    /// Commit an import prepared from `project()` while file editing is modal.
    /// The host checks file-format limits before handing the result to the editor.
    pub fn apply_library_import(&mut self, import: LibraryImport) {
        self.finish_edit();
        let before = self.editor.snapshot();
        self.editor.session.replace_project(import.project);
        self.editor.record(before);
        self.libraries.push(LibraryGroup {
            name: import.name,
            circuits: import.circuits,
        });
    }
}

#[cfg(test)]
mod tests;

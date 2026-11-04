//! File commands and document labels; filesystem work belongs to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    New,
    Open,
    Save,
    SaveAs,
    Close,
}
#[derive(Clone, Debug)]
pub struct DocumentView {
    pub title: String,
    pub circuit_name: String,
    pub dirty: bool,
    pub partial: bool,
}
impl Default for DocumentView {
    fn default() -> Self {
        Self {
            title: "Untitled".into(),
            circuit_name: "main".into(),
            dirty: false,
            partial: false,
        }
    }
}
impl DocumentView {
    pub(crate) fn label(&self) -> String {
        format!(
            "{}{}{}",
            self.title,
            if self.dirty { " *" } else { "" },
            if self.partial { " (partial)" } else { "" }
        )
    }
}

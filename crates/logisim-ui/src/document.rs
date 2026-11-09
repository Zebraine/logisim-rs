//! File commands and document labels; filesystem work belongs to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    New,
    Open,
    Save,
    SaveAs,
    ImportMemory,
    ImportPlugin {
        component: logisim_core::Id,
        import: usize,
    },
    ImportLibrary,
    LoadPlugin,
    Close,
}
#[derive(Clone, Debug)]
pub struct DocumentView {
    pub title: String,
    pub untitled: bool,
    pub circuit_name: String,
    pub dirty: bool,
    pub partial: bool,
}
impl Default for DocumentView {
    fn default() -> Self {
        Self {
            title: "Untitled".into(),
            untitled: true,
            circuit_name: "main".into(),
            dirty: false,
            partial: false,
        }
    }
}
impl DocumentView {
    pub fn display_title(&self) -> &str {
        if self.untitled {
            crate::i18n::tr("Untitled")
        } else {
            &self.title
        }
    }
    pub(crate) fn label(&self) -> String {
        format!(
            "{}{}{}",
            self.display_title(),
            if self.dirty { " *" } else { "" },
            if self.partial {
                crate::i18n::tr(" (partial)")
            } else {
                ""
            }
        )
    }
}

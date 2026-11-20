//! File commands and document labels; filesystem work belongs to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    New,
    Open,
    OpenRecent(usize),
    ClearRecent,
    ClearRecovery {
        disabled_autosave: bool,
    },
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
    pub path: Option<std::path::PathBuf>,
    pub untitled: bool,
    pub circuit_name: String,
    pub dirty: bool,
    pub partial: bool,
}
impl Default for DocumentView {
    fn default() -> Self {
        Self {
            title: "Untitled".into(),
            path: None,
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

impl crate::Workspace {
    pub fn recent(&self) -> &[String] {
        &self.recent
    }
    pub fn set_recent(&mut self, recent: Vec<String>) {
        self.recent = recent;
    }
}

pub(crate) fn recent_button(ui: &mut egui::Ui, path: &str) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    ui.push_id(path, |ui| {
        ui.add(egui::Button::new(name).truncate())
            .on_hover_text(path)
            .clicked()
    })
    .inner
}

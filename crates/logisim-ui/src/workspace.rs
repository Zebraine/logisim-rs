//! Workspace regions, tool catalogue and frame-level interaction tests.
mod dialogs;
mod layout;
mod library;
mod menus;
mod sidebar;
pub(crate) use sidebar::Navigation;
mod catalogue;
#[cfg(test)]
mod tests;
mod toolbar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutTab {
    #[default]
    Circuits,
    Components,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspaceLayout {
    pub sidebar_width: f32,
    pub split: f32,
    pub tab: LayoutTab,
}
impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            sidebar_width: 320.,
            split: 0.52,
            tab: LayoutTab::Circuits,
        }
    }
}
impl crate::Workspace {
    pub fn layout(&self) -> WorkspaceLayout {
        self.layout
    }
    pub fn set_layout(&mut self, layout: WorkspaceLayout) {
        self.layout = layout;
    }
}

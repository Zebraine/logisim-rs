//! Workspace regions, tool catalogue and frame-level interaction tests.
mod dialogs;
mod layout;
mod library;
mod menus;
mod shortcuts;
mod sidebar;
pub(crate) use sidebar::Navigation;
mod catalogue;
#[cfg(test)]
mod tests;
mod toolbar;

pub use logisim_preferences::host::{LayoutTab, WorkspaceLayout};
impl crate::Workspace {
    pub fn layout(&self) -> WorkspaceLayout {
        self.layout
    }
    pub fn set_layout(&mut self, layout: WorkspaceLayout) {
        self.layout = layout;
    }
}

//! Workspace regions, tool catalogue and frame-level interaction tests.
mod layout;
mod library;
mod menus;
mod sidebar;
pub(crate) use sidebar::Navigation;
mod catalogue;
#[cfg(test)]
mod tests;
mod toolbar;

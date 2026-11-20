//! Host rules for the recent-project list. The UI receives changes only on file actions.
use logisim_ui::Workspace;
use std::path::Path;

pub(super) fn valid(path: &str) -> bool {
    Path::new(path).is_absolute() && !path.contains('\0') && path.chars().count() <= 4096
}
pub(crate) fn record(workspace: &mut Workspace, path: &Path) {
    let Some(path) = path.to_str().filter(|s| valid(s)) else {
        return;
    };
    let limit = workspace.preferences().workspace.recent_limit;
    if workspace.recent().first().is_some_and(|p| p == path) {
        return;
    }
    let recent = std::iter::once(path.to_owned())
        .chain(workspace.recent().iter().filter(|p| *p != path).cloned())
        .take(limit)
        .collect();
    workspace.set_recent(recent);
}
pub(crate) fn remove(workspace: &mut Workspace, path: &Path) {
    if let Some(index) = workspace.recent().iter().position(|p| Path::new(p) == path) {
        let mut recent = workspace.recent().to_vec();
        recent.remove(index);
        workspace.set_recent(recent);
    }
}
pub(super) fn truncate(workspace: &mut Workspace) {
    let limit = workspace.preferences().workspace.recent_limit;
    if workspace.recent().len() > limit {
        workspace.set_recent(workspace.recent()[..limit].to_vec());
    }
}

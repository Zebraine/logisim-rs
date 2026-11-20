use serde::{Deserialize, Serialize};
use std::path::Path;
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
}
impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1200.,
            height: 820.,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

impl WindowState {
    pub fn valid(self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && self.x.is_none_or(f32::is_finite)
            && self.y.is_none_or(f32::is_finite)
    }
}
pub fn valid_recent(path: &str) -> bool {
    Path::new(path).is_absolute() && !path.contains('\0') && path.chars().count() <= 4096
}
pub fn validate(
    window: WindowState,
    panels: WorkspaceLayout,
    recent: &[String],
) -> Result<(), &'static str> {
    if !window.valid()
        || !(230.0..=380.0).contains(&panels.sidebar_width)
        || !(0.25..=0.75).contains(&panels.split)
        || recent.len() > 20
        || recent
            .iter()
            .enumerate()
            .any(|(i, p)| !valid_recent(p) || recent[..i].contains(p))
    {
        return Err("Unknown or invalid settings.");
    }
    Ok(())
}

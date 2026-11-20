//! Window geometry in native logical pixels, independent of the workbench UI scale.
use eframe::egui::{Context, Pos2, Vec2, ViewportCommand, ViewportInfo, pos2, vec2};
use serde::{Deserialize, Serialize};

const MIN: Vec2 = vec2(720., 480.);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct WindowState {
    width: f32,
    height: f32,
    x: Option<f32>,
    y: Option<f32>,
    maximized: bool,
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
    fn size(self) -> Vec2 {
        vec2(self.width, self.height)
    }
    fn position(self) -> Option<Pos2> {
        Some(pos2(self.x?, self.y?))
    }

    pub fn valid(self) -> bool {
        self.size().is_finite()
            && self.x.is_none_or(f32::is_finite)
            && self.y.is_none_or(f32::is_finite)
    }
    pub fn bounded(mut self, monitor: Option<Vec2>) -> Self {
        let mut size = self.size().max(MIN);
        if let Some(monitor) = monitor {
            size = size.min(monitor);
            let room = monitor - size;
            let pos = self
                .position()
                .filter(|p| p.x >= 0. && p.y >= 0. && p.x <= room.x && p.y <= room.y)
                .unwrap_or_else(|| (room * 0.5).to_pos2());
            self.x = Some(pos.x);
            self.y = Some(pos.y);
        }
        self.width = size.x;
        self.height = size.y;
        self
    }

    pub fn options(self) -> eframe::NativeOptions {
        let mut viewport = eframe::egui::ViewportBuilder::default()
            .with_inner_size(self.size())
            .with_min_inner_size(MIN)
            .with_app_id("logisim-rs");
        if let Some(pos) = self.position() {
            viewport = viewport.with_position(pos);
        }
        eframe::NativeOptions {
            viewport,
            centered: self.position().is_none(),
            ..Default::default()
        }
    }

    /// Apply the normal rectangle first so unmaximizing returns to that rectangle.
    pub fn restore(self, ctx: &Context) -> Self {
        let zoom = ctx.zoom_factor();
        let (monitor, positioned, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.monitor_size.map(|m| m * zoom),
                v.outer_rect.is_some(),
                v.maximized == Some(true),
            )
        });
        let mut target = self.bounded(monitor);
        if maximized {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
        }
        if let Some(monitor) = monitor {
            ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MIN.min(monitor) / zoom));
        }
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(target.size() / zoom));
        if positioned {
            if let Some(pos) = target.position() {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos / zoom));
            }
        } else {
            target.x = None;
            target.y = None;
        }
        if target.maximized {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
        }
        target
    }

    pub fn observe(&mut self, info: &ViewportInfo, size: Vec2, zoom: f32) {
        if info.minimized == Some(true) || info.fullscreen == Some(true) {
            return;
        }
        self.maximized = info.maximized.unwrap_or(self.maximized);
        if self.maximized {
            return;
        }
        let size = (size * zoom).round();
        self.width = size.x;
        self.height = size.y;
        let pos = info.outer_rect.map(|r| (r.min * zoom).round());
        self.x = pos.map(|p| p.x);
        self.y = pos.map(|p| p.y);
    }
}

#[cfg(test)]
mod tests;

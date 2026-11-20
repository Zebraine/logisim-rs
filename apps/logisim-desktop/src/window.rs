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
mod tests {
    use super::*;
    use eframe::egui::Rect;

    #[test]
    fn bounds_center_missing_or_unreachable_positions_and_keep_valid_positions() {
        let monitor = vec2(1024., 768.);
        let centered = WindowState {
            width: 20.,
            height: -2.,
            x: Some(10000.),
            ..Default::default()
        }
        .bounded(Some(monitor));
        assert_eq!(centered.size(), MIN);
        assert_eq!(centered.position(), Some(pos2(152., 144.)));
        assert_eq!(
            WindowState::default().bounded(Some(monitor)).size(),
            monitor
        );
        let visible = WindowState {
            x: Some(56.),
            y: Some(28.),
            ..centered
        };
        assert_eq!(visible.bounded(Some(monitor)), visible);
        assert!(WindowState::default().bounded(None).position().is_none());
    }

    #[test]
    fn maximized_keeps_normal_rectangle_and_positionless_backend_captures_size() {
        let mut normal = WindowState::default();
        let mut info = ViewportInfo {
            outer_rect: Some(Rect::from_min_size(pos2(32., 56.), MIN)),
            ..Default::default()
        };
        normal.observe(&info, vec2(840., 520.), 1.25);
        let original = normal;
        assert_eq!(normal.size(), vec2(1050., 650.));
        info.maximized = Some(true);
        normal.observe(&info, vec2(1600., 900.), 1.);
        assert_eq!(
            normal,
            WindowState {
                maximized: true,
                ..original
            }
        );
        normal.observe(
            &ViewportInfo {
                maximized: Some(false),
                ..Default::default()
            },
            MIN,
            1.,
        );
        assert_eq!(normal.size(), MIN);
        assert_eq!(normal.position(), None);
    }
}

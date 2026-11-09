//! Viewport navigation, independent of circuit or simulator state.

use egui::{Color32, Event, MouseWheelUnit, PointerButton, Rect, Sense, Ui, Vec2, vec2};

pub(crate) const MIN_ZOOM: f32 = 0.2;
pub(crate) const MAX_ZOOM: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Camera {
    pub zoom: f32,
    pub offset: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: vec2(24.0, 24.0),
        }
    }
}

impl Camera {
    pub fn world_at(self, local: Vec2) -> Vec2 {
        (local - self.offset) / self.zoom
    }

    pub fn zoom_at(&mut self, zoom: f32, anchor: Vec2) {
        if !zoom.is_finite() || !anchor.is_finite() {
            return;
        }
        let world = self.world_at(anchor);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = anchor - world * self.zoom;
    }

    pub fn pan(&mut self, delta: Vec2) {
        if delta.is_finite() {
            self.offset = (self.offset + delta).clamp(Vec2::splat(-1e6), Vec2::splat(1e6));
        }
    }
}

pub(crate) struct Canvas {
    pub camera: Camera,
    pub show_grid: bool,
    pub typing: bool,
    pub rect: Rect,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            show_grid: true,
            typing: false,
            rect: Rect::NOTHING,
        }
    }
}

impl Canvas {
    pub fn world(&self, screen: egui::Pos2) -> Vec2 {
        self.camera.world_at(screen - self.rect.min)
    }

    pub fn screen(&self, point: logisim_core::Point) -> egui::Pos2 {
        self.screen_vec(vec2(point.x as f32, point.y as f32))
    }

    pub fn screen_vec(&self, world: Vec2) -> egui::Pos2 {
        self.rect.min + self.camera.offset + world * self.camera.zoom
    }
    pub fn reset(&mut self) {
        self.camera = Camera::default();
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        let anchor = if self.rect.is_positive() {
            self.rect.size() * 0.5
        } else {
            Vec2::ZERO
        };
        self.camera.zoom_at(zoom, anchor);
    }

    pub fn show(&mut self, ui: &mut Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.rect = rect;
        if response.clicked() {
            response.request_focus();
        }
        if ui.is_enabled()
            && !ui.ctx().any_popup_open()
            && (response.has_focus()
                || (response.hovered() && !ui.ctx().egui_wants_keyboard_input()))
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0))
        {
            self.reset();
        }

        let space = !self.typing && ui.input(|i| i.key_down(egui::Key::Space));
        let dragging = response.dragged_by(PointerButton::Middle)
            || (space && response.dragged_by(PointerButton::Primary));
        if ui.is_enabled() && dragging {
            self.camera.pan(ui.input(|i| i.pointer.delta()));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() && space {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        if ui.is_enabled() && response.hovered() && !ui.ctx().any_popup_open() {
            let anchor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center()) - rect.min;
            ui.input(|i| {
                for event in &i.events {
                    match event {
                        Event::MouseWheel { unit, delta, .. } => {
                            let steps = match unit {
                                MouseWheelUnit::Line => delta.y,
                                MouseWheelUnit::Point => delta.y / 40.0,
                                MouseWheelUnit::Page => delta.y * 5.0,
                            };
                            self.camera.zoom_at(
                                self.camera.zoom * 1.1_f32.powf(steps.clamp(-8.0, 8.0)),
                                anchor,
                            );
                        }
                        Event::Zoom(factor) => {
                            self.camera.zoom_at(self.camera.zoom * factor, anchor);
                        }
                        _ => {}
                    }
                }
            });
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0, Color32::WHITE);
        if self.show_grid {
            self.paint_grid(&painter, rect);
        }

        // A subtle world-origin marker makes an otherwise empty viewport navigable.
        let origin = rect.min + self.camera.offset;
        if rect.shrink(12.0).contains(origin) {
            let stroke = egui::Stroke::new(1.0, Color32::from_rgb(160, 174, 193));
            painter.line_segment([origin - vec2(5.0, 0.0), origin + vec2(5.0, 0.0)], stroke);
            painter.line_segment([origin - vec2(0.0, 5.0), origin + vec2(0.0, 5.0)], stroke);
        }
        response
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
        let mut spacing = 10.0 * self.camera.zoom;
        // Thin the grid at low zoom, keeping drawing work bounded by viewport size.
        while spacing < 7.0 {
            spacing *= 2.0;
        }
        let start = rect.min
            + vec2(
                self.camera.offset.x.rem_euclid(spacing),
                self.camera.offset.y.rem_euclid(spacing),
            );
        let columns = (rect.width() / spacing).ceil() as usize;
        let rows = (rect.height() / spacing).ceil() as usize;
        let mut mesh = egui::Mesh::default();
        let color = Color32::from_rgb(207, 215, 226);
        for column in 0..=columns {
            for row in 0..=rows {
                let point = start + vec2(column as f32, row as f32) * spacing;
                if rect.contains(point) {
                    mesh.add_colored_rect(Rect::from_center_size(point, Vec2::splat(1.0)), color);
                }
            }
        }
        painter.add(mesh);
    }
}

#[cfg(test)]
mod tests;

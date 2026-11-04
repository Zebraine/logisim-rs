//! Small original line icons; no upstream artwork is bundled.

use egui::{Color32, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Poke,
    Select,
    Text,
    Pin,
    Probe,
    Not,
    And,
    Or,
    Plus,
    Up,
    Down,
    Trash,
    Home,
}

pub(crate) fn button(
    ui: &mut Ui,
    icon: Icon,
    selected: bool,
    enabled: bool,
    hint: &str,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
        let visuals = ui.style().interact_selectable(&response, selected);
        if response.hovered() || selected {
            ui.painter()
                .rect_filled(rect.shrink(1.0), 5, visuals.bg_fill);
        }
        let color = if selected {
            super::ACCENT
        } else if enabled {
            super::INK
        } else {
            Color32::from_rgb(155, 165, 179)
        };
        paint(ui.painter(), rect.shrink(7.0), icon, color);
        response.on_hover_text(hint)
    })
    .inner
}

fn paint(p: &egui::Painter, r: Rect, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let point = |x: f32, y: f32| pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    match icon {
        Icon::Select => {
            p.add(Shape::closed_line(
                vec![
                    point(0.1, 0.0),
                    point(0.9, 0.55),
                    point(0.55, 0.6),
                    point(0.35, 1.0),
                ],
                stroke,
            ));
        }
        Icon::Poke => {
            p.add(Shape::line(
                vec![
                    point(0.2, 0.55),
                    point(0.2, 0.4),
                    point(0.4, 0.55),
                    point(0.4, 0.05),
                    point(0.55, 0.05),
                    point(0.55, 0.4),
                    point(0.85, 0.5),
                    point(0.8, 0.9),
                    point(0.4, 1.0),
                    point(0.2, 0.55),
                ],
                stroke,
            ));
        }
        Icon::Text => {
            line((0.1, 0.1), (0.9, 0.1));
            line((0.5, 0.1), (0.5, 0.95));
            line((0.3, 0.95), (0.7, 0.95));
        }
        Icon::Pin => {
            p.rect_stroke(r.shrink(2.0), 2, stroke, StrokeKind::Inside);
            line((0.5, 0.35), (0.5, 0.7));
            line((0.9, 0.5), (1.1, 0.5));
        }
        Icon::Probe => {
            p.circle_stroke(r.center(), r.width() * 0.35, stroke);
            line((0.85, 0.5), (1.1, 0.5));
        }
        Icon::Not => {
            p.add(Shape::closed_line(
                vec![point(0.1, 0.1), point(0.1, 0.9), point(0.75, 0.5)],
                stroke,
            ));
            p.circle_stroke(point(0.9, 0.5), 1.5, stroke);
        }
        Icon::And => {
            let mut points = vec![point(0.1, 0.05), point(0.5, 0.05)];
            for step in 0..=16 {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + step as f32 * std::f32::consts::PI / 16.0;
                points.push(point(0.5 + 0.45 * angle.cos(), 0.5 + 0.45 * angle.sin()));
            }
            points.push(point(0.1, 0.95));
            p.add(Shape::closed_line(points, stroke));
        }
        Icon::Or => {
            p.add(Shape::closed_line(
                vec![
                    point(0.05, 0.05),
                    point(0.5, 0.12),
                    point(0.9, 0.5),
                    point(0.5, 0.88),
                    point(0.05, 0.95),
                    point(0.25, 0.5),
                ],
                stroke,
            ));
        }
        Icon::Plus => {
            line((0.5, 0.1), (0.5, 0.9));
            line((0.1, 0.5), (0.9, 0.5));
        }
        Icon::Up | Icon::Down => {
            let (tip, end) = if matches!(icon, Icon::Up) {
                (0.15, 0.85)
            } else {
                (0.85, 0.15)
            };
            line((0.5, tip), (0.5, end));
            line((0.2, 0.5), (0.5, tip));
            line((0.8, 0.5), (0.5, tip));
        }
        Icon::Trash => {
            p.rect_stroke(
                Rect::from_min_max(point(0.2, 0.25), point(0.8, 0.95)),
                1,
                stroke,
                StrokeKind::Inside,
            );
            line((0.1, 0.15), (0.9, 0.15));
            line((0.4, 0.0), (0.6, 0.0));
        }
        Icon::Home => {
            p.add(Shape::closed_line(
                vec![
                    point(0.1, 0.45),
                    point(0.5, 0.1),
                    point(0.9, 0.45),
                    point(0.8, 0.45),
                    point(0.8, 0.95),
                    point(0.2, 0.95),
                    point(0.2, 0.45),
                ],
                stroke,
            ));
        }
    }
}

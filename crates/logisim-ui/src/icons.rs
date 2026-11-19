//! Small original line icons; no upstream artwork is bundled.

use egui::{Color32, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Poke,
    Select,
    Text,
    Pin,
    Wire,
    Tunnel,
    Not,
    And,
    Or,
    Plus,
    More,
    Document,
    Chip,
    Search,
    Restore,
    Minus,
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
        let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
        let visuals = ui.style().interact_selectable(&response, selected);
        if response.hovered() || selected || response.has_focus() {
            ui.painter()
                .rect_filled(rect.shrink(1.0), 4, visuals.bg_fill);
        }
        crate::theme::focus_ring(ui, &response);
        let color = if selected {
            crate::theme::ACCENT
        } else if enabled {
            crate::theme::INK
        } else {
            crate::theme::MUTED
        };
        paint(ui.painter(), rect.shrink(7.0), icon, color);
        response.on_hover_text(crate::i18n::tr(hint))
    })
    .inner
}

pub(crate) fn paint(p: &egui::Painter, r: Rect, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let point = |x: f32, y: f32| pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    match icon {
        Icon::Restore => {
            let center = r.center();
            let radius = r.width() * 0.35;
            let arc = (0..=20)
                .map(|i| {
                    let angle = -2.55 + i as f32 * (5.65 / 20.0);
                    center + egui::Vec2::angled(angle) * radius
                })
                .collect();
            p.add(Shape::line(arc, stroke));
            line((0.16, 0.10), (0.16, 0.36));
            line((0.16, 0.36), (0.43, 0.36));
        }
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
        Icon::Wire => {
            line((0.05, 0.85), (0.55, 0.85));
            line((0.55, 0.85), (0.55, 0.15));
            line((0.55, 0.15), (0.95, 0.15));
            p.circle_filled(point(0.05, 0.85), 2.0, color);
            p.circle_filled(point(0.95, 0.15), 2.0, color);
        }
        Icon::Tunnel => {
            p.add(Shape::closed_line(
                vec![
                    point(0.05, 0.2),
                    point(0.65, 0.2),
                    point(1.0, 0.5),
                    point(0.65, 0.8),
                    point(0.05, 0.8),
                ],
                stroke,
            ));
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
        Icon::More => {
            for x in [0.12, 0.5, 0.88] {
                p.circle_filled(point(x, 0.5), 1.5, color);
            }
        }
        Icon::Minus => line((0.1, 0.5), (0.9, 0.5)),
        Icon::Search => {
            p.circle_stroke(point(0.4, 0.4), r.width() * 0.31, stroke);
            line((0.63, 0.63), (0.95, 0.95));
        }
        Icon::Document => {
            p.rect_stroke(r.shrink2(vec2(2.0, 0.0)), 2, stroke, StrokeKind::Inside);
            line((0.32, 0.38), (0.68, 0.38));
            line((0.32, 0.65), (0.58, 0.65));
        }
        Icon::Chip => {
            p.rect_stroke(r.shrink(r.width() * 0.18), 2, stroke, StrokeKind::Inside);
            for n in [0.32, 0.68] {
                line((0.0, n), (0.18, n));
                line((0.82, n), (1.0, n));
                line((n, 0.0), (n, 0.18));
                line((n, 0.82), (n, 1.0));
            }
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

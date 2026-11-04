//! Original vector artwork and editor hit geometry.
use crate::{ACCENT, INK, canvas::Canvas};
use egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use logisim_core::{Circuit, Component, Gate, Id, Kind, Point, Wire};
use std::collections::BTreeSet;

pub(crate) fn pos(p: Point) -> Pos2 {
    pos2(p.x as f32, p.y as f32)
}
fn local(c: &Component, point: Vec2) -> Vec2 {
    let p = match c.facing {
        logisim_core::Facing::East => point,
        logisim_core::Facing::South => vec2(-point.y, point.x),
        logisim_core::Facing::West => -point,
        logisim_core::Facing::North => vec2(point.y, -point.x),
    };
    pos(c.position).to_vec2() + p
}
fn text_width(c: &Component) -> f32 {
    (if c.kind == Kind::Tunnel && c.label.is_empty() {
        "(unnamed)"
    } else {
        &c.label
    }
    .lines()
    .map(|l| l.chars().count())
    .max()
    .unwrap_or(0)
    .max(1) as f32
        * f32::from(c.font_size)
        * 0.62)
        .max(24.0)
}
fn half_height(c: &Component) -> f32 {
    (f32::from(c.input_count().saturating_sub(1)) * 10.0 + 10.0).max(20.0)
}
pub(crate) fn bounds(c: &Component) -> Rect {
    if c.kind == Kind::Text {
        return Rect::from_min_size(
            pos(c.position),
            vec2(
                text_width(c),
                f32::from(c.font_size) * 1.35 * c.label.lines().count().max(1) as f32,
            ),
        );
    }
    let (width, height) = match c.kind {
        Kind::Gate(_) => (60.0, half_height(c)),
        Kind::Tunnel => (
            text_width(c) + 22.0,
            (f32::from(c.font_size) * 0.7 + 5.0).max(16.0),
        ),
        _ => (32.0, 16.0),
    };
    let mut rect = Rect::NOTHING;
    for p in [
        vec2(-width, -height),
        vec2(0.0, -height),
        vec2(-width, height),
        vec2(0.0, height),
    ] {
        rect.extend_with(local(c, p).to_pos2());
    }
    rect
}
pub(crate) fn wire_distance(w: Wire, point: Vec2) -> f32 {
    let a = pos(w.a).to_vec2();
    let b = pos(w.b).to_vec2();
    let segment = b - a;
    let t = ((point - a).dot(segment) / segment.length_sq().max(1.0)).clamp(0.0, 1.0);
    (point - (a + segment * t)).length()
}
pub(crate) fn circuit(p: &Painter, canvas: &Canvas, circuit: &Circuit, selection: &BTreeSet<Id>) {
    let mut net = BTreeSet::new();
    for id in selection {
        if !net.contains(id) && circuit.wires.iter().any(|w| w.id == *id) {
            net.extend(circuit.connected_wires(*id));
        }
    }
    for w in &circuit.wires {
        let selected = selection.contains(&w.id);
        let color = if selected {
            ACCENT
        } else if net.contains(&w.id) {
            Color32::from_rgb(14, 165, 180)
        } else {
            INK
        };
        p.line_segment(
            [canvas.screen(w.a), canvas.screen(w.b)],
            Stroke::new(if selected { 2.8 } else { 1.8 }, color),
        );
        if selected {
            for endpoint in [w.a, w.b] {
                p.rect_filled(
                    Rect::from_center_size(canvas.screen(endpoint), vec2(6.0, 6.0)),
                    1,
                    ACCENT,
                );
            }
        }
    }
    for point in circuit.junctions() {
        p.circle_filled(canvas.screen(point), 3.0, INK);
    }
    for c in &circuit.components {
        component(p, canvas, c, selection.contains(&c.id), false);
    }
}
pub(crate) fn component(p: &Painter, canvas: &Canvas, c: &Component, selected: bool, ghost: bool) {
    let color = if ghost {
        ACCENT.gamma_multiply(0.45)
    } else {
        Color32::from_rgb(c.color[0], c.color[1], c.color[2])
    };
    if !ghost && c.kind != Kind::Text {
        let body = bounds(c);
        p.rect_filled(
            Rect::from_min_max(
                canvas.screen_vec(body.min.to_vec2()),
                canvas.screen_vec(body.max.to_vec2()),
            ),
            0,
            Color32::WHITE,
        );
    }
    let stroke = Stroke::new(1.7, color);
    let at = |x, y| canvas.screen_vec(local(c, vec2(x, y)));
    let path = |points: Vec<Vec2>, closed| {
        let points = points.into_iter().map(|v| at(v.x, v.y)).collect();
        p.add(if closed {
            Shape::closed_line(points, stroke)
        } else {
            Shape::line(points, stroke)
        });
    };
    let font = FontId::proportional(f32::from(c.font_size) * canvas.camera.zoom);
    match c.kind {
        Kind::Gate(gate) => {
            let h = half_height(c);
            match gate {
                Gate::Not | Gate::Buffer => path(
                    vec![vec2(-50.0, -20.0), vec2(-10.0, 0.0), vec2(-50.0, 20.0)],
                    true,
                ),
                Gate::And | Gate::Nand => {
                    let mut points = vec![vec2(-50.0, -h), vec2(-30.0, -h)];
                    for step in 0..=24 {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + step as f32 * std::f32::consts::PI / 24.0;
                        points.push(vec2(-30.0 + 20.0 * angle.cos(), h * angle.sin()));
                    }
                    points.push(vec2(-50.0, h));
                    path(points, true);
                }
                Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor => {
                    let mut points = vec![];
                    for step in 0..=20 {
                        let t = step as f32 / 20.0;
                        points.push(vec2(-52.0 + 42.0 * t, -h * (1.0 - t * t)));
                    }
                    for step in 1..=20 {
                        let t = step as f32 / 20.0;
                        points.push(vec2(-10.0 - 42.0 * t, h * (2.0 * t - t * t)));
                    }
                    for step in 1..20 {
                        let t = step as f32 / 20.0;
                        points.push(vec2(
                            -52.0 + 10.0 * (std::f32::consts::PI * t).sin(),
                            h * (1.0 - 2.0 * t),
                        ));
                    }
                    path(points, true);
                    if matches!(gate, Gate::Xor | Gate::Xnor) {
                        path(
                            (0..=20)
                                .map(|i| {
                                    let t = i as f32 / 20.0;
                                    vec2(
                                        -58.0 + 10.0 * (std::f32::consts::PI * t).sin(),
                                        h * (1.0 - 2.0 * t),
                                    )
                                })
                                .collect(),
                            false,
                        );
                    }
                }
            }
            for port in c.local_ports().into_iter().skip(1) {
                let input_x = if matches!(gate, Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor) {
                    -52.0 + 10.0 * (std::f32::consts::FRAC_PI_2 * port.y as f32 / h).cos()
                } else {
                    -50.0
                };
                p.line_segment(
                    [at(port.x as f32, port.y as f32), at(input_x, port.y as f32)],
                    stroke,
                );
            }
            if gate.inverted() {
                p.circle_stroke(at(-5.0, 0.0), 5.0 * canvas.camera.zoom, stroke);
            } else {
                p.line_segment([at(-10.0, 0.0), at(0.0, 0.0)], stroke);
            }
        }
        Kind::Pin => {
            path(
                vec![
                    vec2(-32.0, -15.0),
                    vec2(-8.0, -15.0),
                    vec2(0.0, 0.0),
                    vec2(-8.0, 15.0),
                    vec2(-32.0, 15.0),
                ],
                true,
            );
            p.text(
                at(-19.0, 0.0),
                Align2::CENTER_CENTER,
                if c.output { "OUT" } else { "IN" },
                FontId::proportional(10.0 * canvas.camera.zoom),
                color,
            );
        }
        Kind::Tunnel => {
            let width = text_width(c) + 22.0;
            let height = (f32::from(c.font_size) * 0.7 + 5.0).max(16.0);
            path(
                vec![
                    vec2(-width, -height),
                    vec2(-9.0, -height),
                    vec2(0.0, 0.0),
                    vec2(-9.0, height),
                    vec2(-width, height),
                ],
                true,
            );
            let label = if c.label.is_empty() {
                "(unnamed)"
            } else {
                &c.label
            };
            let galley = p.layout_no_wrap(label.to_owned(), font.clone(), color);
            let center = at(-width * 0.5 - 3.0, 0.0);
            let angle = match c.facing {
                logisim_core::Facing::South => std::f32::consts::FRAC_PI_2,
                logisim_core::Facing::North => -std::f32::consts::FRAC_PI_2,
                _ => 0.0,
            };
            p.add(
                egui::epaint::TextShape::new(center - galley.size() * 0.5, galley, color)
                    .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
            );
        }
        Kind::Text => {
            p.text(
                canvas.screen(c.position),
                Align2::LEFT_TOP,
                &c.label,
                font.clone(),
                color,
            );
        }
    }
    if !matches!(c.kind, Kind::Text | Kind::Tunnel) && !c.label.is_empty() {
        let r = bounds(c);
        p.text(
            canvas.screen_vec(vec2(r.center().x, r.top() - 8.0)),
            Align2::CENTER_BOTTOM,
            &c.label,
            font,
            color,
        );
    }
    if c.bits > 1 && c.kind != Kind::Text {
        let r = bounds(c);
        p.text(
            canvas.screen_vec(vec2(r.center().x, r.bottom() + 6.0)),
            Align2::CENTER_TOP,
            format!("{} bit", c.bits),
            FontId::proportional(10.0 * canvas.camera.zoom),
            color,
        );
    }
    for port in c.ports() {
        p.circle_filled(canvas.screen(port), 2.6, if ghost { color } else { ACCENT });
    }
    if selected {
        let b = bounds(c);
        let r = Rect::from_min_max(
            canvas.screen_vec(b.min.to_vec2()),
            canvas.screen_vec(b.max.to_vec2()),
        )
        .expand(5.0);
        p.rect_stroke(
            r,
            2,
            Stroke::new(1.0, ACCENT.gamma_multiply(0.65)),
            StrokeKind::Outside,
        );
        for at in [
            r.left_top(),
            r.right_top(),
            r.left_bottom(),
            r.right_bottom(),
        ] {
            p.rect_filled(Rect::from_center_size(at, vec2(5.0, 5.0)), 1, ACCENT);
        }
    }
}

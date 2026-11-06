//! Original vector artwork and editor hit geometry.
use crate::{ACCENT, INK, canvas::Canvas, simulation::SimulationView};
use egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use logisim_core::{Circuit, Component, Extension, Facing, Gate, Id, Kind, Point, Radix, Wire};
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
fn text_origin(c: &Component) -> Pos2 {
    use logisim_core::{HorizontalAlign as H, VerticalAlign as V};
    let width = text_width(c);
    let height = f32::from(c.font_size) * 1.35 * c.label.lines().count().max(1) as f32;
    pos(c.position)
        - vec2(
            match c.text_horizontal {
                H::Left => 0.0,
                H::Center => width / 2.0,
                H::Right => width,
            },
            match c.text_vertical {
                V::Top => 0.0,
                V::Center => height / 2.0,
                V::Baseline => f32::from(c.font_size),
                V::Bottom => height,
            },
        )
}
fn pin_width(c: &Component) -> f32 {
    if c.bits <= 1 {
        32.0
    } else {
        (f32::from(c.bits.clamp(1, 32).div_ceil(4)) * 8.0 + 22.0).max(40.0)
    }
}
fn half_height(c: &Component) -> f32 {
    (c.gate_width() as f32 * 0.5)
        .max(c.local_ports().iter().map(|p| p.y.abs()).max().unwrap_or(0) as f32 + 5.0)
}
fn probe_width(c: &Component) -> f32 {
    match c.radix {
        Radix::Binary => 136.0,
        Radix::Octal => 100.0,
        Radix::Signed | Radix::Unsigned => 108.0,
        Radix::Hex => 84.0,
    }
}
pub(crate) fn bounds(c: &Component) -> Rect {
    if matches!(c.kind, Kind::Subcircuit(_)) {
        let mut rect = Rect::NOTHING;
        for p in [
            c.interface.min,
            c.interface.max,
            Point::new(c.interface.min.x, c.interface.max.y),
            Point::new(c.interface.max.x, c.interface.min.y),
        ] {
            rect.extend_with(local(c, pos(p).to_vec2()).to_pos2());
        }
        return rect;
    }

    if c.kind == Kind::Splitter {
        let mut rect = Rect::from_min_max(pos(c.position), pos(c.position));
        for point in c.ports() {
            rect.extend_with(pos(point));
        }
        return rect.expand(4.0);
    }
    if matches!(c.kind, Kind::Power | Kind::Ground) {
        let mut rect = Rect::NOTHING;
        for p in [
            vec2(0.0, -10.0),
            vec2(20.0, 10.0),
            vec2(0.0, 10.0),
            vec2(20.0, -10.0),
        ] {
            rect.extend_with(local(c, p).to_pos2());
        }
        return rect;
    }
    if c.kind == Kind::Text {
        return Rect::from_min_size(
            text_origin(c),
            vec2(
                text_width(c),
                f32::from(c.font_size) * 1.35 * c.label.lines().count().max(1) as f32,
            ),
        );
    }
    let (width, height) = match c.kind {
        Kind::Gate(_) => (c.gate_depth() as f32, half_height(c)),
        Kind::Probe => (probe_width(c), 20.0),
        Kind::BitExtender => (40.0, 20.0),
        Kind::Tunnel => (
            text_width(c) + 22.0,
            (f32::from(c.font_size) * 0.7 + 5.0).max(16.0),
        ),
        _ => (pin_width(c), 16.0),
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
pub(crate) fn circuit(
    p: &Painter,
    canvas: &Canvas,
    circuit: &Circuit,
    selection: &BTreeSet<Id>,
    simulation: &SimulationView,
) {
    let mut net = BTreeSet::new();
    for id in selection {
        if !net.contains(id) && circuit.wires.iter().any(|w| w.id == *id) {
            net.extend(circuit.connected_wires(*id));
        }
    }
    for w in &circuit.wires {
        let selected = selection.contains(&w.id);
        let segment = [canvas.screen(w.a), canvas.screen(w.b)];
        if selected || net.contains(&w.id) {
            p.line_segment(
                segment,
                Stroke::new(
                    if selected { 7.0 } else { 5.0 },
                    ACCENT.gamma_multiply(if selected { 0.25 } else { 0.12 }),
                ),
            );
        }
        let signal = simulation.wires.get(&w.id);
        p.line_segment(
            segment,
            Stroke::new(
                if signal.is_some_and(|s| s.binary.len() > 1) {
                    3.0
                } else {
                    1.8
                },
                signal.map_or(INK, |s| s.tone.color()),
            ),
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
        let color = circuit
            .wires
            .iter()
            .find(|w| w.contains(point))
            .and_then(|w| simulation.wires.get(&w.id))
            .map_or(INK, |s| s.tone.color());
        p.circle_filled(canvas.screen(point), 3.0, color);
    }
    for c in &circuit.components {
        component(
            p,
            canvas,
            c,
            selection.contains(&c.id),
            false,
            Some(simulation),
        );
    }
}
pub(crate) fn component(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    selected: bool,
    ghost: bool,
    simulation: Option<&SimulationView>,
) {
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
    let scale_x = if matches!(c.kind, Kind::Gate(_)) {
        c.gate_depth() as f32 / 60.0
    } else {
        1.0
    };
    let at = |x, y| canvas.screen_vec(local(c, vec2(x * scale_x, y)));
    let path = |points: Vec<Vec2>, closed| {
        let points = points.into_iter().map(|v| at(v.x, v.y)).collect();
        p.add(if closed {
            Shape::closed_line(points, stroke)
        } else {
            Shape::line(points, stroke)
        });
    };
    let font = FontId::proportional(f32::from(c.font_size) * canvas.camera.zoom);
    let body_text = |at: Pos2, text: String, color: Color32| {
        let galley = p.layout_no_wrap(text, FontId::monospace(12.0 * canvas.camera.zoom), color);
        let angle = match c.facing {
            Facing::South => std::f32::consts::FRAC_PI_2,
            Facing::North => -std::f32::consts::FRAC_PI_2,
            _ => 0.0,
        };
        p.add(
            egui::epaint::TextShape::new(at - galley.size() * 0.5, galley, color)
                .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
        );
    };
    match c.kind {
        Kind::Subcircuit(_) => {
            if let Some(appearance) = &c.interface.appearance {
                for shape in &appearance.shapes {
                    crate::artwork::paint(
                        p,
                        canvas,
                        shape,
                        |v| {
                            local(
                                c,
                                crate::artwork::rotate(
                                    appearance.facing.inverse(),
                                    v - crate::artwork::vector(appearance.anchor),
                                ),
                            )
                        },
                        crate::artwork::angle(c.facing) - crate::artwork::angle(appearance.facing),
                    );
                }
            } else {
                let a = c.interface.min;
                let b = c.interface.max;
                let rect = bounds(c);
                let screen = Rect::from_min_max(
                    canvas.screen_vec(rect.min.to_vec2()),
                    canvas.screen_vec(rect.max.to_vec2()),
                );
                p.rect(
                    screen,
                    4.0,
                    Color32::from_rgb(241, 245, 251),
                    stroke,
                    StrokeKind::Inside,
                );
                path(
                    vec![
                        vec2(a.x as f32 + 4.0, a.y as f32 + 3.0),
                        vec2(a.x as f32 + 4.0, a.y as f32 + 7.0),
                        vec2(a.x as f32 + 8.0, a.y as f32 + 7.0),
                    ],
                    false,
                );
                let max_chars = ((b.x - a.x - 4) / 6).max(1) as usize;
                let title: String = c.interface.name.chars().take(max_chars).collect();
                p.text(
                    screen.center(),
                    Align2::CENTER_CENTER,
                    title,
                    FontId::proportional(10.0 * canvas.camera.zoom),
                    color,
                );
            }
        }
        Kind::Gate(gate) => {
            let h = half_height(c);
            match gate {
                Gate::Not | Gate::Buffer => path(
                    vec![vec2(-50.0, -h), vec2(-10.0, 0.0), vec2(-50.0, h)],
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
                    [
                        at(port.x as f32 / scale_x, port.y as f32),
                        at(input_x, port.y as f32),
                    ],
                    stroke,
                );
            }
            if gate.inverted() {
                p.circle_stroke(at(-5.0, 0.0), 5.0 * scale_x * canvas.camera.zoom, stroke);
            } else {
                p.line_segment([at(-10.0, 0.0), at(0.0, 0.0)], stroke);
            }
        }
        Kind::Constant => {
            let width = pin_width(c);
            path(
                vec![
                    vec2(-width, -11.0),
                    vec2(-5.0, -11.0),
                    vec2(-5.0, 11.0),
                    vec2(-width, 11.0),
                ],
                true,
            );
            p.line_segment([at(-5.0, 0.0), at(0.0, 0.0)], stroke);
            body_text(
                at(-(width + 5.0) / 2.0, 0.0),
                format!("{:X}", c.constant),
                color,
            );
        }
        Kind::Power => {
            p.line_segment([at(0.0, 0.0), at(18.0, 0.0)], stroke);
            path(
                vec![vec2(10.0, -8.0), vec2(18.0, 0.0), vec2(10.0, 8.0)],
                false,
            );
        }
        Kind::Ground => {
            p.line_segment([at(0.0, 0.0), at(9.0, 0.0)], stroke);
            for (x, half) in [(9.0, 9.0), (14.0, 6.0), (19.0, 3.0)] {
                p.line_segment([at(x, -half), at(x, half)], stroke);
            }
        }
        Kind::Probe => {
            let width = probe_width(c);
            path(
                vec![
                    vec2(-width, -20.0),
                    vec2(-6.0, -20.0),
                    vec2(-6.0, 20.0),
                    vec2(-width, 20.0),
                ],
                true,
            );
            p.line_segment([at(-6.0, 0.0), at(0.0, 0.0)], stroke);
            let signal = simulation.and_then(|s| s.ports.get(&(c.id, 0)));
            let text = signal.map_or_else(
                || "—".into(),
                |s| {
                    if c.radix == Radix::Binary && s.binary.len() > 16 {
                        let split = s.binary.len() - 16;
                        format!("{}\n{}", &s.binary[..split], &s.binary[split..])
                    } else {
                        s.text.clone()
                    }
                },
            );
            body_text(
                at(-width / 2.0 - 3.0, 0.0),
                text,
                signal.map_or(color, |s| s.tone.color()),
            );
        }
        Kind::Splitter => {
            let ports = c.local_ports();
            let first = ports[1].y as f32;
            let last = ports[ports.len() - 1].y as f32;
            p.line_segment([at(0.0, 0.0), at(10.0, 0.0)], stroke);
            // Reversed branch numbering can reverse first/last without changing the stem.
            p.line_segment(
                [
                    at(10.0, first.min(last).min(0.0)),
                    at(10.0, first.max(last).max(0.0)),
                ],
                Stroke::new(3.5, color),
            );
            for (branch, port) in ports.iter().skip(1).enumerate() {
                p.line_segment([at(10.0, port.y as f32), at(20.0, port.y as f32)], stroke);
                let bits = c.splitter.branch_bits(c.bits, branch as u8);
                let text = bits.iter().map(u8::to_string).collect::<Vec<_>>().join(",");
                p.text(
                    at(23.0, port.y as f32),
                    Align2::LEFT_BOTTOM,
                    format!("{branch}:{}", if text.is_empty() { "—" } else { &text }),
                    FontId::proportional(8.0 * canvas.camera.zoom),
                    color,
                );
            }
        }
        Kind::BitExtender => {
            path(
                vec![
                    vec2(-40.0, -20.0),
                    vec2(0.0, -20.0),
                    vec2(0.0, 20.0),
                    vec2(-40.0, 20.0),
                ],
                true,
            );
            p.text(
                at(-20.0, -7.0),
                Align2::CENTER_CENTER,
                format!("{}→{}", c.input_bits, c.bits),
                FontId::monospace(10.0 * canvas.camera.zoom),
                color,
            );
            p.text(
                at(-20.0, 8.0),
                Align2::CENTER_CENTER,
                c.extension.name(),
                FontId::proportional(10.0 * canvas.camera.zoom),
                color,
            );
            if c.extension == Extension::Input {
                p.line_segment([at(-20.0, -20.0), at(-20.0, -15.0)], stroke);
            }
        }
        Kind::Pin => {
            let width = pin_width(c);
            path(
                vec![
                    vec2(-width, -15.0),
                    vec2(-8.0, -15.0),
                    vec2(0.0, 0.0),
                    vec2(-8.0, 15.0),
                    vec2(-width, 15.0),
                ],
                true,
            );
            let signal = simulation.and_then(|s| s.ports.get(&(c.id, 0)));
            let text = simulation
                .and_then(|s| s.inputs.get(&c.id))
                .map(|v| v.text())
                .or_else(|| signal.map(|s| s.text.clone()))
                .unwrap_or_else(|| if c.output { "OUT".into() } else { "IN".into() });
            let color = signal.map_or(color, |s| s.tone.color());
            let galley =
                p.layout_no_wrap(text, FontId::monospace(12.0 * canvas.camera.zoom), color);
            let angle = match c.facing {
                logisim_core::Facing::South => std::f32::consts::FRAC_PI_2,
                logisim_core::Facing::North => -std::f32::consts::FRAC_PI_2,
                _ => 0.0,
            };
            p.add(
                egui::epaint::TextShape::new(
                    at(-width * 0.5 - 3.0, 0.0) - galley.size() * 0.5,
                    galley,
                    color,
                )
                .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
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
                canvas.screen_vec(text_origin(c).to_vec2()),
                Align2::LEFT_TOP,
                &c.label,
                font.clone(),
                color,
            );
        }
    }
    if !matches!(c.kind, Kind::Text | Kind::Tunnel) && !c.label.is_empty() {
        let r = bounds(c);
        let (anchor, align) = if matches!(c.kind, Kind::Probe | Kind::Subcircuit(_)) {
            match c.label_facing {
                Facing::North => (vec2(r.center().x, r.top() - 8.0), Align2::CENTER_BOTTOM),
                Facing::South => (vec2(r.center().x, r.bottom() + 8.0), Align2::CENTER_TOP),
                Facing::East => (vec2(r.right() + 8.0, r.center().y), Align2::LEFT_CENTER),
                Facing::West => (vec2(r.left() - 8.0, r.center().y), Align2::RIGHT_CENTER),
            }
        } else {
            (vec2(r.center().x, r.top() - 8.0), Align2::CENTER_BOTTOM)
        };
        p.text(canvas.screen_vec(anchor), align, &c.label, font, color);
    }
    if c.bits > 1
        && !matches!(
            c.kind,
            Kind::Text | Kind::Probe | Kind::Splitter | Kind::BitExtender | Kind::Subcircuit(_)
        )
    {
        let r = bounds(c);
        p.text(
            canvas.screen_vec(vec2(r.center().x, r.bottom() + 6.0)),
            Align2::CENTER_TOP,
            format!("{} bit", c.bits),
            FontId::proportional(10.0 * canvas.camera.zoom),
            color,
        );
    }
    for (index, port) in c.ports().into_iter().enumerate() {
        let signal = simulation.and_then(|s| s.ports.get(&(c.id, index)));
        p.circle_filled(
            canvas.screen(port),
            2.6,
            if ghost {
                color
            } else {
                signal.map_or(ACCENT, |s| s.tone.color())
            },
        );
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

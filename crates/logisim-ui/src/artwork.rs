//! Shared appearance drawing for the package editor and rotated circuit instances.
use crate::canvas::Canvas;
use egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2, vec2};
use logisim_core::{Artwork, Facing, Geometry, Point, TextAnchor};

pub(crate) fn vector(p: Point) -> Vec2 {
    vec2(p.x as f32, p.y as f32)
}
pub(crate) fn rotate(f: Facing, p: Vec2) -> Vec2 {
    match f {
        Facing::East => p,
        Facing::South => vec2(-p.y, p.x),
        Facing::West => -p,
        Facing::North => vec2(p.y, -p.x),
    }
}
pub(crate) fn angle(f: Facing) -> f32 {
    match f {
        Facing::East => 0.0,
        Facing::South => std::f32::consts::FRAC_PI_2,
        Facing::West => std::f32::consts::PI,
        Facing::North => -std::f32::consts::FRAC_PI_2,
    }
}
fn color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
pub(crate) fn outline(g: &Geometry) -> (Vec<Vec2>, bool) {
    match g {
        Geometry::Poly { points, closed } => {
            (points.iter().copied().map(vector).collect(), *closed)
        }
        Geometry::Curve { a, control, b } => {
            let (a, c, b) = (vector(*a), vector(*control), vector(*b));
            (
                (0..=48)
                    .map(|i| {
                        let t = i as f32 / 48.0;
                        a * (1.0 - t).powi(2) + c * 2.0 * t * (1.0 - t) + b * t * t
                    })
                    .collect(),
                false,
            )
        }
        Geometry::Ellipse { a, b } => {
            let a = vector(*a);
            let b = vector(*b);
            let center = (a + b) * 0.5;
            let radii = (b - a).abs() * 0.5;
            (
                (0..64)
                    .map(|i| {
                        let t = i as f32 * std::f32::consts::TAU / 64.0;
                        center + vec2(t.cos(), t.sin()) * radii
                    })
                    .collect(),
                true,
            )
        }
        Geometry::Rect { a, b, radius } => {
            let a = vector(*a);
            let b = vector(*b);
            let min = a.min(b);
            let max = a.max(b);
            let r = (*radius as f32)
                .min((max.x - min.x) * 0.5)
                .min((max.y - min.y) * 0.5);
            let mut points = vec![];
            for (i, center) in [
                vec2(max.x - r, min.y + r),
                vec2(max.x - r, max.y - r),
                vec2(min.x + r, max.y - r),
                vec2(min.x + r, min.y + r),
            ]
            .into_iter()
            .enumerate()
            {
                for j in 0..=8 {
                    let t = (i as f32 - 1.0 + j as f32 / 8.0) * std::f32::consts::FRAC_PI_2;
                    points.push(center + vec2(t.cos(), t.sin()) * r);
                }
            }
            points.dedup();
            (points, true)
        }
        Geometry::Text { .. } => (vec![], false),
    }
}
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}
/// Ear clipping handles concave artwork without assuming every polygon is convex.
fn triangles(points: &[Pos2]) -> Vec<[usize; 3]> {
    let mut indices: Vec<_> = (0..points.len()).collect();
    let mut out = vec![];
    let area: f32 = (0..points.len())
        .map(|i| {
            cross(
                points[i].to_vec2(),
                points[(i + 1) % points.len()].to_vec2(),
            )
        })
        .sum();
    let sign = area.signum();
    if sign == 0.0 {
        return out;
    }
    while indices.len() > 2 {
        let mut found = false;
        for i in 0..indices.len() {
            let [a, b, c] = [
                indices[(i + indices.len() - 1) % indices.len()],
                indices[i],
                indices[(i + 1) % indices.len()],
            ];
            let (pa, pb, pc) = (points[a], points[b], points[c]);
            if cross(pb - pa, pc - pb) * sign <= 0.0001 {
                continue;
            }
            let contains = indices
                .iter()
                .copied()
                .filter(|v| ![a, b, c].contains(v))
                .any(|v| {
                    let p = points[v];
                    cross(pb - pa, p - pa) * sign >= 0.0
                        && cross(pc - pb, p - pb) * sign >= 0.0
                        && cross(pa - pc, p - pc) * sign >= 0.0
                });
            if !contains {
                out.push([a, b, c]);
                indices.remove(i);
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
    }
    out
}
pub(crate) fn fill_polygon(p: &Painter, points: &[Pos2], fill: Color32) {
    if fill == Color32::TRANSPARENT || points.len() < 3 {
        return;
    }
    let mut mesh = egui::epaint::Mesh::default();
    for point in points {
        mesh.colored_vertex(*point, fill);
    }
    for [a, b, c] in triangles(points) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    if !mesh.indices.is_empty() {
        p.add(Shape::mesh(mesh));
    }
}
pub(crate) fn paint(
    p: &Painter,
    canvas: &Canvas,
    art: &Artwork,
    map: impl Fn(Vec2) -> Vec2,
    text_angle: f32,
) {
    if let Geometry::Text {
        at,
        text,
        font,
        anchor,
    } = &art.geometry
    {
        let Some(fill) = art.paint.fill else {
            return;
        };
        let fill = color(fill);
        let style = crate::fonts::Text {
            text,
            font,
            color: fill,
            zoom: canvas.camera.zoom,
            angle: text_angle,
        };
        let galley = style.layout(p);
        let shift = match anchor {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => galley.size().x * 0.5,
            TextAnchor::End => galley.size().x,
        };
        let offset = vec2(-shift, -f32::from(font.size) * canvas.camera.zoom);
        let rot = egui::emath::Rot2::from_angle(text_angle);
        let origin = canvas.screen_vec(map(vector(*at))) + rot * offset;
        style.paint_galley(p, origin, galley);
        return;
    }
    let (outline, closed) = outline(&art.geometry);
    let points: Vec<_> = outline
        .into_iter()
        .map(|v| canvas.screen_vec(map(v)))
        .collect();
    if let Some(fill) = art.paint.fill {
        fill_polygon(p, &points, color(fill));
    }
    if let Some(stroke) = art.paint.stroke
        && points.len() >= 2
    {
        let stroke = Stroke::new(
            f32::from(art.paint.width) * canvas.camera.zoom,
            color(stroke),
        );
        p.add(if closed {
            Shape::closed_line(points, stroke)
        } else {
            Shape::line(points, stroke)
        });
    }
}
pub(crate) fn hit(art: &Artwork, at: Vec2, tolerance: f32) -> bool {
    if matches!(art.geometry, Geometry::Text { .. }) {
        let (a, b) = art.geometry.bounds();
        return egui::Rect::from_two_pos(vector(a).to_pos2(), vector(b).to_pos2())
            .expand(tolerance)
            .contains(at.to_pos2());
    }
    let (points, closed) = outline(&art.geometry);
    let mut inside = false;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if closed || i + 1 < points.len() {
            let t = ((at - a).dot(b - a) / (b - a).length_sq().max(0.0001)).clamp(0.0, 1.0);
            if (at - (a + (b - a) * t)).length() <= tolerance + f32::from(art.paint.width) * 0.5 {
                return true;
            }
        }
        if (a.y > at.y) != (b.y > at.y) && at.x < (b.x - a.x) * (at.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    art.paint.fill.is_some() && inside
}

#[cfg(test)]
mod tests;

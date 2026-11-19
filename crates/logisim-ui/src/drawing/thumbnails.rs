//! Normalized document geometry, rasterized once per display density.
//! Running signals and canvas camera changes never touch this cache.
use super::{bounds, half_height, local, pin_width, pos, text_width};
use crate::{artwork, theme};
use egui::{ColorImage, Painter, Pos2, Rect, Stroke, StrokeKind, TextureHandle, Vec2, pos2, vec2};
use logisim_core::{Circuit, Component, Gate, Geometry, Id, Kind, Project, plugin::api::Shape};
use std::collections::BTreeMap;

pub(crate) const SIZE: Vec2 = Vec2::new(56.0, 38.0);

#[derive(Default)]
pub(crate) struct Thumbnails {
    epoch: u64,
    entries: BTreeMap<Id, Preview>,
}

struct Preview {
    segments: Vec<[Pos2; 2]>,
    wire_count: usize,
    missing: bool,
    texture: Option<(f32, TextureHandle)>,
}

impl Thumbnails {
    /// Called only at document transaction boundaries. Carry unchanged definitions
    /// into the next epoch, including a definition whose row was merely renamed.
    pub fn advance(&mut self, before: &Project, after: &Project) {
        self.epoch += 1;
        self.entries.retain(|id,_| matches!((before.circuit(*id),after.circuit(*id)),(Some(a),Some(b)) if a.circuit==b.circuit));
    }

    pub fn paint(
        &mut self,
        p: &Painter,
        id: Id,
        circuit: &Circuit,
        rect: Rect,
        active: bool,
    ) -> bool {
        let preview = self
            .entries
            .entry(id)
            .or_insert_with(|| Preview::new(circuit));
        let density = p.pixels_per_point();
        if !matches!(preview.texture,Some((d,_)) if d==density) {
            let texture = p.ctx().load_texture(
                format!("circuit-{id}-{}", self.epoch),
                preview.image(density),
                egui::TextureOptions::LINEAR,
            );
            preview.texture = Some((density, texture));
        }
        let texture = &preview.texture.as_ref().expect("rasterized preview").1;
        p.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        p.rect_stroke(
            rect,
            3,
            Stroke::new(1.0, if active { theme::ACCENT } else { theme::BORDER }),
            StrokeKind::Inside,
        );
        preview.missing
    }
}

impl Preview {
    fn new(circuit: &Circuit) -> Self {
        let mut preview = Self {
            segments: circuit.wires.iter().map(|w| [pos(w.a), pos(w.b)]).collect(),
            wire_count: circuit.wires.len(),
            missing: false,
            texture: None,
        };
        for c in &circuit.components {
            preview.component(c);
        }
        if !preview.segments.is_empty() {
            let bounds = preview
                .segments
                .iter()
                .flatten()
                .fold(Rect::NOTHING, |mut r, &p| {
                    r.extend_with(p);
                    r
                });
            let scale = ((SIZE.x - 8.0) / bounds.width().max(1.0))
                .min((SIZE.y - 8.0) / bounds.height().max(1.0));
            for p in preview.segments.iter_mut().flatten() {
                *p = (SIZE * 0.5 + (*p - bounds.center()) * scale).to_pos2();
            }
        }
        preview
    }

    fn path(&mut self, points: impl IntoIterator<Item = Pos2>, closed: bool) {
        let mut points = points.into_iter();
        let Some(first) = points.next() else {
            return;
        };
        let mut last = first;
        for point in points {
            self.segments.push([last, point]);
            last = point;
        }
        if closed {
            self.segments.push([last, first]);
        }
    }

    fn rectangle(&mut self, r: Rect) {
        self.path(
            [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ],
            true,
        );
    }

    fn component(&mut self, c: &Component) {
        let world = |v| local(c, v).to_pos2();
        match c.kind {
            Kind::Text => (),
            Kind::Plugin => {
                let Some(plugin) = &c.plugin else {
                    self.missing = true;
                    return;
                };
                let point = |v: [i32; 2]| world(vec2(v[0] as f32, v[1] as f32));
                for shape in &plugin.tool.graphics {
                    match shape {
                        Shape::Text { .. } => (),
                        Shape::Rect { min, max, .. } => {
                            self.rectangle(Rect::from_two_pos(point(*min), point(*max)))
                        }
                        Shape::Line { a, b, .. } => self.segments.push([point(*a), point(*b)]),
                        Shape::Ellipse { min, max, .. } => {
                            self.ellipse(Rect::from_two_pos(point(*min), point(*max)))
                        }
                    }
                }
            }
            Kind::Subcircuit(_) => {
                if let Some(a) = &c.interface.appearance {
                    for shape in &a.shapes {
                        if matches!(shape.geometry, Geometry::Text { .. }) {
                            continue;
                        }
                        let (outline, closed) = artwork::outline(&shape.geometry);
                        self.path(
                            outline.into_iter().map(|v| {
                                world(artwork::rotate(
                                    a.facing.inverse(),
                                    v - artwork::vector(a.anchor),
                                ))
                            }),
                            closed,
                        );
                    }
                } else {
                    self.rectangle(bounds(c));
                }
            }
            Kind::Plexer(_) => self.path(
                c.plexer_body()
                    .into_iter()
                    .map(|p| world(artwork::vector(p))),
                true,
            ),
            Kind::Led | Kind::RgbLed => self.ellipse(bounds(c)),
            Kind::Pin | Kind::Tunnel => {
                let (w, h) = if c.kind == Kind::Pin {
                    (pin_width(c), 15.0)
                } else {
                    (
                        text_width(c) + 22.0,
                        (f32::from(c.font.size) * 0.7 + 5.0).max(16.0),
                    )
                };
                self.path(
                    [
                        vec2(-w, -h),
                        vec2(-8.0, -h),
                        Vec2::ZERO,
                        vec2(-8.0, h),
                        vec2(-w, h),
                    ]
                    .map(world),
                    true,
                );
            }
            Kind::Gate(g) => {
                let w = c.gate_width() as f32;
                let h = half_height(c);
                let outline = match g {
                    Gate::Buffer | Gate::Not => vec![vec2(-w, -h), vec2(0.0, 0.0), vec2(-w, h)],
                    Gate::And | Gate::Nand => {
                        let mut points = vec![vec2(-w, -h)];
                        points.extend((0..=12).map(|i| {
                            let t = std::f32::consts::PI * (i as f32 / 12.0 - 0.5);
                            vec2((t.cos() - 1.0) * w * 0.5, h * t.sin())
                        }));
                        points.push(vec2(-w, h));
                        points
                    }
                    Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor => vec![
                        vec2(-w, -h),
                        vec2(-w * 0.40, -h * 0.80),
                        Vec2::ZERO,
                        vec2(-w * 0.40, h * 0.80),
                        vec2(-w, h),
                        vec2(-w * 0.8, 0.0),
                    ],
                };
                self.path(outline.into_iter().map(world), true);
            }
            Kind::Parity(_)
            | Kind::ControlledBuffer
            | Kind::ControlledInverter
            | Kind::PullResistor
            | Kind::Transistor
            | Kind::TransmissionGate
            | Kind::Constant
            | Kind::Power
            | Kind::Ground
            | Kind::Probe
            | Kind::Splitter
            | Kind::BitExtender
            | Kind::Clock
            | Kind::FlipFlop(_)
            | Kind::Latch(_)
            | Kind::Register
            | Kind::Counter
            | Kind::ShiftRegister
            | Kind::Random
            | Kind::Button
            | Kind::Switch
            | Kind::DipSwitch
            | Kind::Slider
            | Kind::Joystick
            | Kind::Keyboard
            | Kind::Tty
            | Kind::SevenSegment
            | Kind::HexDisplay
            | Kind::DotMatrix
            | Kind::Rom
            | Kind::PlaRom
            | Kind::Ram
            | Kind::Arithmetic(_) => self.rectangle(bounds(c)),
        }
    }

    fn ellipse(&mut self, r: Rect) {
        self.path(
            (0..24).map(|i| {
                let angle = std::f32::consts::TAU * i as f32 / 24.0;
                r.center() + r.size() * 0.5 * vec2(angle.cos(), angle.sin())
            }),
            true,
        );
    }

    fn image(&self, density: f32) -> ColorImage {
        let size = (SIZE * density).round().max(Vec2::splat(1.0));
        let mut image = ColorImage::filled([size.x as usize, size.y as usize], theme::SURFACE);
        let spacing = (6.0 * density).round().max(1.0) as usize;
        for y in (spacing / 2..image.size[1]).step_by(spacing) {
            for x in (spacing / 2..image.size[0]).step_by(spacing) {
                image[(x, y)] = theme::BORDER;
            }
        }
        for (index, &[a, b]) in self.segments.iter().enumerate() {
            let wire = index < self.wire_count;
            let radius = if wire { 0.5 } else { 0.75 } * density;
            let color = if wire {
                theme::BORDER_STRONG
            } else {
                theme::MUTED
            };
            let (a, b) = (a * density, b * density);
            let bounds = Rect::from_two_pos(a, b).expand(radius + 0.5);
            let delta = b - a;
            let length = delta.length_sq().max(f32::EPSILON);
            for y in bounds.top() as usize..(bounds.bottom().ceil() as usize).min(image.size[1]) {
                for x in bounds.left() as usize..(bounds.right().ceil() as usize).min(image.size[0])
                {
                    let offset = pos2(x as f32 + 0.5, y as f32 + 0.5) - a;
                    let distance =
                        (offset - delta * (offset.dot(delta) / length).clamp(0.0, 1.0)).length();
                    let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
                    image[(x, y)] = image[(x, y)].lerp_to_gamma(color, coverage);
                }
            }
        }
        image
    }
}

#[cfg(test)]
mod tests;

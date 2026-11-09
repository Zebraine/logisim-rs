//! Editable vector artwork and stable pin bindings, independent of any renderer.
use crate::{CircuitDefinition, Facing, Id, Kind, Point};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SHAPES: usize = 2048;
pub type Rgb = [u8; 3];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paint {
    pub stroke: Option<Rgb>,
    pub fill: Option<Rgb>,
    pub width: u8,
}
impl Default for Paint {
    fn default() -> Self {
        Self {
            stroke: Some([0; 3]),
            fill: None,
            width: 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkFont {
    pub family: String,
    pub size: u8,
    pub bold: bool,
    pub italic: bool,
}
impl Default for ArtworkFont {
    fn default() -> Self {
        Self {
            family: "Dialog".into(),
            size: 12,
            bold: false,
            italic: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    #[default]
    Middle,
    End,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Geometry {
    Rect {
        a: Point,
        b: Point,
        radius: i32,
    },
    Ellipse {
        a: Point,
        b: Point,
    },
    Poly {
        points: Vec<Point>,
        closed: bool,
    },
    Curve {
        a: Point,
        control: Point,
        b: Point,
    },
    Text {
        at: Point,
        text: String,
        font: ArtworkFont,
        anchor: TextAnchor,
    },
}
impl Geometry {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rect { radius, .. } if *radius > 0 => "Rounded rectangle",
            Self::Rect { .. } => "Rectangle",
            Self::Ellipse { .. } => "Ellipse",
            Self::Poly { closed: true, .. } => "Polygon",
            Self::Poly { points, .. } if points.len() == 2 => "Line",
            Self::Poly { .. } => "Polyline",
            Self::Curve { .. } => "Curve",
            Self::Text { .. } => "Text",
        }
    }
    pub fn handles(&self) -> Vec<Point> {
        match self {
            Self::Rect { a, b, .. } | Self::Ellipse { a, b } => {
                vec![*a, Point::new(b.x, a.y), *b, Point::new(a.x, b.y)]
            }
            Self::Poly { points, .. } => points.clone(),
            Self::Curve { a, control, b } => vec![*a, *control, *b],
            Self::Text { at, .. } => vec![*at],
        }
    }
    pub fn set_handle(&mut self, index: usize, p: Point) {
        match self {
            Self::Rect { a, b, .. } | Self::Ellipse { a, b } => match index {
                0 => *a = p,
                1 => {
                    b.x = p.x;
                    a.y = p.y
                }
                2 => *b = p,
                3 => {
                    a.x = p.x;
                    b.y = p.y
                }
                _ => {}
            },
            Self::Poly { points, .. } => {
                if let Some(point) = points.get_mut(index) {
                    *point = p;
                }
            }
            Self::Curve { a, control, b } => match index {
                0 => *a = p,
                1 => *control = p,
                2 => *b = p,
                _ => {}
            },
            Self::Text { at, .. } if index == 0 => *at = p,
            _ => {}
        }
    }
    pub fn translate(&mut self, delta: Point) {
        match self {
            Self::Rect { a, b, .. } | Self::Ellipse { a, b } => {
                *a = *a + delta;
                *b = *b + delta;
            }
            Self::Poly { points, .. } => {
                for p in points {
                    *p = *p + delta;
                }
            }
            Self::Curve { a, control, b } => {
                *a = *a + delta;
                *control = *control + delta;
                *b = *b + delta;
            }
            Self::Text { at, .. } => *at = *at + delta,
        }
    }
    /// Conservative bounds for selection and fitting; text uses a font-independent estimate.
    pub fn bounds(&self) -> (Point, Point) {
        let points = if let Self::Text {
            at,
            text,
            font,
            anchor,
        } = self
        {
            let width = (text.chars().count() as i32 * i32::from(font.size) * 3 / 4).max(1);
            let left = at.x
                - match anchor {
                    TextAnchor::Start => 0,
                    TextAnchor::Middle => width / 2,
                    TextAnchor::End => width,
                };
            vec![
                Point::new(left, at.y - i32::from(font.size)),
                Point::new(left + width, at.y + 3),
            ]
        } else {
            self.handles()
        };
        let mut min = Point::new(i32::MAX, i32::MAX);
        let mut max = Point::new(i32::MIN, i32::MIN);
        for p in points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        (min, max)
    }
    pub fn valid(&self) -> bool {
        let points = self.handles();
        if points.is_empty()
            || points.len() > 256
            || points
                .iter()
                .any(|p| p.x.unsigned_abs() > 1_000_000 || p.y.unsigned_abs() > 1_000_000)
        {
            return false;
        }
        match self {
            Self::Rect { a, b, radius } => a.x != b.x && a.y != b.y && (0..=10000).contains(radius),
            Self::Ellipse { a, b } => a.x != b.x && a.y != b.y,
            Self::Poly { points, closed } => points.len() >= if *closed { 3 } else { 2 },
            Self::Text { text, font, .. } => {
                text.len() <= 16384
                    && !text.contains(['\n', '\r'])
                    && (1..=96).contains(&font.size)
                    && !font.family.is_empty()
                    && font.family.len() <= 256
            }
            _ => true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artwork {
    pub id: Id,
    pub geometry: Geometry,
    pub paint: Paint,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub shapes: Vec<Artwork>,
    pub ports: BTreeMap<Id, Point>,
    pub anchor: Point,
    pub facing: Facing,
}
impl Appearance {
    pub fn add(&mut self, geometry: Geometry, paint: Paint) -> Option<Id> {
        if self.shapes.len() >= MAX_SHAPES || !geometry.valid() {
            return None;
        }
        let id = self.shapes.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        self.shapes.push(Artwork {
            id,
            geometry,
            paint,
        });
        Some(id)
    }
    pub fn bounds(&self) -> (Point, Point) {
        let mut min = self.anchor;
        let mut max = self.anchor;
        for p in self
            .ports
            .values()
            .copied()
            .chain(self.shapes.iter().flat_map(|s| {
                let (a, b) = s.geometry.bounds();
                [a, b]
            }))
        {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        (min, max)
    }
    pub fn local(&self, p: Point) -> Point {
        self.facing.inverse().transform(p - self.anchor)
    }
    pub fn validate(&self, pins: &BTreeSet<Id>) -> Result<(), String> {
        if self.shapes.len() > MAX_SHAPES
            || self.ports.keys().copied().collect::<BTreeSet<_>>() != *pins
        {
            return Err("Appearance must bind every supported Pin exactly once.".into());
        }
        let mut ids = BTreeSet::new();
        if self
            .shapes
            .iter()
            .any(|s| !ids.insert(s.id) || !s.geometry.valid() || !(1..=64).contains(&s.paint.width))
        {
            return Err("Invalid appearance geometry or paint.".into());
        }
        if self
            .ports
            .values()
            .chain(std::iter::once(&self.anchor))
            .any(|p| {
                p.x.unsigned_abs() > 1_000_000
                    || p.y.unsigned_abs() > 1_000_000
                    || p.x % 10 != 0
                    || p.y % 10 != 0
            })
        {
            return Err("Appearance ports and anchor must be on the 10-unit grid.".into());
        }
        Ok(())
    }
}
impl CircuitDefinition {
    pub fn editable_appearance(&self) -> Appearance {
        if let Some(a) = &self.circuit.appearance {
            return a.clone();
        }
        let interface = self.default_interface();
        let anchor = Point::new(100, 100);
        let a = interface.min + anchor;
        let b = interface.max + anchor;
        let mut result = Appearance {
            shapes: vec![],
            ports: interface
                .ports
                .iter()
                .map(|p| (p.pin, p.offset + anchor))
                .collect(),
            anchor,
            facing: Facing::East,
        };
        result.add(
            Geometry::Rect { a, b, radius: 5 },
            Paint {
                fill: Some([255; 3]),
                width: 2,
                ..Default::default()
            },
        );
        let center = (a.x + b.x) / 2;
        result.add(
            Geometry::Curve {
                a: Point::new(center - 4, a.y + 2),
                control: Point::new(center, a.y + 11),
                b: Point::new(center + 4, a.y + 2),
            },
            Paint {
                stroke: Some([128; 3]),
                width: 2,
                ..Default::default()
            },
        );
        result
    }
    /// Preserve bound pins across circuit edits. New pins append to their side without moving existing artwork.
    pub fn reconcile_appearance(&mut self) {
        let Some(mut appearance) = self.circuit.appearance.take() else {
            return;
        };
        let pins: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Pin)
            .collect();
        appearance
            .ports
            .retain(|id, _| pins.iter().any(|p| p.id == *id));
        let defaults = self.default_interface();
        for pin in &pins {
            if appearance.ports.contains_key(&pin.id) {
                continue;
            }
            let vertical = matches!(pin.facing, Facing::East | Facing::West);
            let last = pins
                .iter()
                .filter(|p| p.facing == pin.facing)
                .filter_map(|p| appearance.ports.get(&p.id))
                .max_by_key(|p| if vertical { (p.y, p.x) } else { (p.x, p.y) })
                .copied();
            let mut at = if let Some(last) = last {
                last + if vertical {
                    Point::new(0, 10)
                } else {
                    Point::new(10, 0)
                }
            } else {
                defaults
                    .ports
                    .iter()
                    .find(|p| p.pin == pin.id)
                    .unwrap()
                    .offset
                    + appearance.anchor
            };
            while appearance.ports.values().any(|p| *p == at) {
                at = at
                    + if vertical {
                        Point::new(0, 10)
                    } else {
                        Point::new(10, 0)
                    };
            }
            appearance.ports.insert(pin.id, at);
        }
        self.circuit.appearance = Some(appearance);
    }
}

#[cfg(test)]
mod tests;

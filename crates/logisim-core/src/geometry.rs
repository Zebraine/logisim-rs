//! Grid coordinates, orientation and orthogonal segments.
use crate::Id;
use std::ops::{Add, Sub};

pub const GRID: i32 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self::new(0, 0);
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn snapped(x: f32, y: f32) -> Self {
        let snap = |v: f32| (v.clamp(-1e6, 1e6) / GRID as f32).round() as i32 * GRID;
        Self::new(snap(x), snap(y))
    }
}
impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x.saturating_add(rhs.x), self.y.saturating_add(rhs.y))
    }
}
impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x.saturating_sub(rhs.x), self.y.saturating_sub(rhs.y))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Facing {
    #[default]
    East,
    South,
    West,
    North,
}
impl Facing {
    pub const ALL: [Self; 4] = [Self::East, Self::South, Self::West, Self::North];
    pub fn name(self) -> &'static str {
        match self {
            Self::East => "East",
            Self::South => "South",
            Self::West => "West",
            Self::North => "North",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
        }
    }
    pub fn inverse(self) -> Self {
        match self {
            Self::South => Self::North,
            Self::North => Self::South,
            other => other,
        }
    }
    pub fn transform(self, p: Point) -> Point {
        match self {
            Self::East => p,
            Self::South => Point::new(p.y.saturating_neg(), p.x),
            Self::West => Point::new(p.x.saturating_neg(), p.y.saturating_neg()),
            Self::North => Point::new(p.y, p.x.saturating_neg()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    pub id: Id,
    pub a: Point,
    pub b: Point,
}
impl Wire {
    pub fn horizontal(self) -> bool {
        self.a.y == self.b.y
    }
    pub fn contains(self, p: Point) -> bool {
        if self.horizontal() {
            p.y == self.a.y && (self.a.x.min(self.b.x)..=self.a.x.max(self.b.x)).contains(&p.x)
        } else {
            p.x == self.a.x && (self.a.y.min(self.b.y)..=self.a.y.max(self.b.y)).contains(&p.y)
        }
    }
    pub fn touches(self, other: Self) -> bool {
        self.contains(other.a)
            || self.contains(other.b)
            || other.contains(self.a)
            || other.contains(self.b)
    }
}

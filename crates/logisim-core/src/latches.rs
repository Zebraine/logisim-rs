//! Latch attributes and terminal layouts measured from the running reference.
use crate::{Component, ControlPosition, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Latch {
    D,
    Sr,
}

impl Latch {
    pub const ALL: [Self; 2] = [Self::D, Self::Sr];
    pub fn name(self) -> &'static str {
        match self {
            Self::D => "D Latch",
            Self::Sr => "S-R Latch",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LatchEnable {
    #[default]
    High,
    Low,
    None,
}
impl LatchEnable {
    pub const ALL: [Self; 3] = [Self::High, Self::Low, Self::None];
    pub fn name(self) -> &'static str {
        match self {
            Self::High => "High level",
            Self::Low => "Low level",
            Self::None => "Without enable",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatchRole {
    Q,
    NotQ,
    Enable,
    D,
    S,
    R,
    Preset,
    Clear,
}
impl Component {
    pub fn latch_has_enable(&self) -> bool {
        self.kind != Kind::Latch(Latch::Sr) || self.latch_enable != LatchEnable::None
    }
    /// Q, /Q, optional enable, data input(s), preset, clear.
    pub fn latch_terminals(&self) -> Vec<(LatchRole, Point)> {
        use LatchRole::*;
        let paired = self.kind == Kind::Latch(Latch::Sr);
        let (depth, second) = if self.sequential.new_layout {
            (60, 40)
        } else {
            (40, 20)
        };
        let mut p = vec![(Q, Point::ZERO), (NotQ, Point::new(0, second))];
        if self.latch_has_enable() {
            p.push((
                Enable,
                Point::new(
                    -depth,
                    if !self.sequential.new_layout && paired {
                        10
                    } else {
                        0
                    },
                ),
            ));
        }
        if paired {
            p.push((
                S,
                Point::new(-depth, if self.sequential.new_layout { 20 } else { 0 }),
            ));
        }
        p.push((if paired { R } else { D }, Point::new(-depth, second)));
        let (top, bottom, center, left, right) = if self.sequential.new_layout {
            (-20, 60, -30, -40, -20)
        } else {
            (-10, 30, -20, -30, -10)
        };
        let (pre, clr) = match self.sequential.controls {
            ControlPosition::AboveBelow => (Point::new(center, top), Point::new(center, bottom)),
            ControlPosition::BelowAbove => (Point::new(center, bottom), Point::new(center, top)),
            ControlPosition::Legacy => (Point::new(left, bottom), Point::new(right, bottom)),
        };
        p.extend([(Preset, pre), (Clear, clr)]);
        p
    }
}

#[cfg(test)]
mod tests;

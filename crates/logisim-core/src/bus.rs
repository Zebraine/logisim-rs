//! Bus control attributes and terminal geometry, independent of signal evaluation.
use crate::{Component, GateSize, Kind, Point};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pull {
    #[default]
    Zero,
    One,
    Error,
}
impl Pull {
    pub const ALL: [Self; 3] = [Self::Zero, Self::One, Self::Error];
    pub fn name(self) -> &'static str {
        match self {
            Self::Zero => "Zero",
            Self::One => "One",
            Self::Error => "Error",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusControl {
    pub size: GateSize,
    pub control_left: bool,
    pub negate_control: bool,
    pub pull: Pull,
}

/// Controlled gate behavior, independent of control placement and symbol size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlledLogic {
    pub inverted: bool,
    pub bits: u8,
    pub negate_control: bool,
}
impl Kind {
    pub fn is_controlled(self) -> bool {
        matches!(self, Self::ControlledBuffer | Self::ControlledInverter)
    }
}
impl Component {
    pub fn controlled_logic(&self) -> Option<ControlledLogic> {
        self.kind.is_controlled().then_some(ControlledLogic {
            inverted: self.kind == Kind::ControlledInverter,
            bits: self.bits,
            negate_control: self.bus.negate_control,
        })
    }
    pub fn controlled_ports(&self) -> Vec<Point> {
        let depth = self.gate_width();
        vec![
            Point::ZERO,
            Point::new(-depth, 0),
            Point::new(10 - depth, if self.bus.control_left { -10 } else { 10 }),
        ]
    }
}

#[cfg(test)]
mod tests;

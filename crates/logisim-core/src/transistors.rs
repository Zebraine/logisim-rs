//! Attributes and terminal geometry observed through the reference UI.
use crate::{Component, Facing, Kind, Point};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransistorType {
    #[default]
    P,
    N,
}
impl TransistorType {
    pub const ALL: [Self; 2] = [Self::P, Self::N];
    pub fn name(self) -> &'static str {
        match self {
            Self::P => "P-Type",
            Self::N => "N-Type",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransistorOptions {
    pub transistor_type: TransistorType,
    pub gate_bottom_right: bool,
}
impl Kind {
    pub fn is_transistor(self) -> bool {
        matches!(self, Self::Transistor | Self::TransmissionGate)
    }
}
impl Component {
    /// Local control offset; the attribute refers to world top/left, not handedness.
    pub fn transistor_gate_y(&self) -> i32 {
        if self.transistor.gate_bottom_right ^ matches!(self.facing, Facing::South | Facing::West) {
            20
        } else {
            -20
        }
    }
    /// Output, data input, transistor control / P control, optional N control.
    pub fn transistor_ports(&self) -> Vec<Point> {
        let y = self.transistor_gate_y();
        let mut ports = vec![Point::ZERO, Point::new(-40, 0), Point::new(-20, y)];
        if self.kind == Kind::TransmissionGate {
            ports.push(Point::new(-20, -y));
        }
        ports
    }
}

#[cfg(test)]
mod tests;

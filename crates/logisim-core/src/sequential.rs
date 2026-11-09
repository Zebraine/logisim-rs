//! Observed terminal layouts and editable timing attributes; no runtime state.
use crate::{Component, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipFlop {
    D,
    T,
    Jk,
    Sr,
}
impl FlipFlop {
    pub const ALL: [Self; 4] = [Self::D, Self::T, Self::Jk, Self::Sr];
    pub fn name(self) -> &'static str {
        match self {
            Self::D => "D Flip-Flop",
            Self::T => "T Flip-Flop",
            Self::Jk => "J-K Flip-Flop",
            Self::Sr => "S-R Flip-Flop",
        }
    }
    pub fn paired(self) -> bool {
        matches!(self, Self::Jk | Self::Sr)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trigger {
    #[default]
    Rising,
    Falling,
    High,
    Low,
}
impl Trigger {
    pub const ALL: [Self; 4] = [Self::Rising, Self::Falling, Self::High, Self::Low];
    pub fn name(self) -> &'static str {
        match self {
            Self::Rising => "Rising edge",
            Self::Falling => "Falling edge",
            Self::High => "High level",
            Self::Low => "Low level",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlPosition {
    #[default]
    AboveBelow,
    BelowAbove,
    Legacy,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CounterOverflow {
    #[default]
    Wrap,
    Stay,
    Continue,
    Load,
}
impl CounterOverflow {
    pub const ALL: [Self; 4] = [Self::Wrap, Self::Stay, Self::Continue, Self::Load];
    pub fn name(self) -> &'static str {
        match self {
            Self::Wrap => "Wrap around",
            Self::Stay => "Stay at value",
            Self::Continue => "Continue counting",
            Self::Load => "Load next value",
        }
    }
}
impl ControlPosition {
    pub const ALL: [Self; 3] = [Self::AboveBelow, Self::BelowAbove, Self::Legacy];
    pub fn name(self) -> &'static str {
        match self {
            Self::AboveBelow => "Above / below",
            Self::BelowAbove => "Below / above",
            Self::Legacy => "Legacy",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sequential {
    pub trigger: Trigger,
    pub random_seed: i32,
    pub controls: ControlPosition,
    pub negate_controls: bool,
    pub new_layout: bool,
    pub select_high: bool,
    pub high_ticks: u32,
    pub low_ticks: u32,
    pub counter_max: u32,
    pub counter_overflow: CounterOverflow,
    pub counter_old: bool,
}
impl Default for Sequential {
    fn default() -> Self {
        Self {
            trigger: Trigger::Rising,
            random_seed: 0,
            controls: ControlPosition::AboveBelow,
            negate_controls: false,
            new_layout: true,
            select_high: false,
            high_ticks: 1,
            low_ticks: 1,
            counter_max: 255,
            counter_overflow: CounterOverflow::Wrap,
            counter_old: false,
        }
    }
}
impl Kind {
    pub fn is_storage(self) -> bool {
        matches!(
            self,
            Self::FlipFlop(_)
                | Self::Latch(_)
                | Self::Register
                | Self::Counter
                | Self::ShiftRegister
                | Self::Random
        )
    }
    pub fn is_sequential(self) -> bool {
        self.is_storage() || self == Self::Clock
    }
}
impl Component {
    /// FF: Q, /Q, CLK, data(s), PRE, CLR. Register: Q, D, CLK, EN, CS, PRE, CLR.
    pub fn sequential_ports(&self) -> Option<Vec<Point>> {
        let points = match self.kind {
            // Q, EN, CLK, CLR.
            Kind::Random => vec![(0, 0), (-30, 10), (-30, -10), (-20, 20)],
            Kind::Latch(_) => {
                return Some(self.latch_terminals().into_iter().map(|(_, p)| p).collect());
            }
            // Q, D, CLK, Load, Count/Reverse, Carry, PRE, CLR, optional EN.
            Kind::Counter => {
                let mut p = vec![
                    (0, 0),
                    (-30, 0),
                    (-20, 20),
                    (-30, -10),
                    (-30, 10),
                    (0, 10),
                    (-10, -20),
                    (-10, 20),
                ];
                if !self.sequential.counter_old {
                    p.push((-20, -20));
                }
                p
            }
            // The file anchor is D, rather than Q. Q, D, CLK, Mode, CLR.
            Kind::ShiftRegister => vec![(30, 0), (0, 0), (0, 10), (10, -20), (10, 20)],
            Kind::Register => vec![
                (0, 0),
                (-30, 0),
                (-20, 20),
                (-30, 10),
                (-30, -10),
                (-10, -20),
                (-10, 20),
            ],
            Kind::FlipFlop(ff) => {
                if self.sequential.new_layout {
                    let mut p = vec![(0, 0), (0, 40), (-60, 0)];
                    if ff.paired() {
                        p.push((-60, 20));
                    }
                    p.push((-60, 40));
                    p.extend(match self.sequential.controls {
                        ControlPosition::AboveBelow => [(-30, -20), (-30, 60)],
                        ControlPosition::BelowAbove => [(-30, 60), (-30, -20)],
                        ControlPosition::Legacy => [(-40, 60), (-20, 60)],
                    });
                    p
                } else {
                    let mut p = vec![(0, 0), (0, 20), (-40, if ff.paired() { 10 } else { 0 })];
                    if ff.paired() {
                        p.push((-40, 0));
                    }
                    p.push((-40, 20));
                    p.extend([(-30, 30), (-10, 30)]);
                    p
                }
            }
            _ => return None,
        };
        Some(points.into_iter().map(|(x, y)| Point::new(x, y)).collect())
    }
    pub fn drives_port(&self, index: usize) -> bool {
        if self.kind == Kind::Plugin {
            return self
                .plugin
                .as_ref()
                .and_then(|p| p.tool.ports.get(index))
                .is_some_and(|p| p.output);
        }
        if matches!(self.kind, Kind::Ttl(_)) {
            return self.ttl_output(index);
        }
        if matches!(self.kind, Kind::Joystick | Kind::Keyboard) {
            return index < 2;
        }
        if self.kind == Kind::DipSwitch {
            return index < usize::from(self.io.switch_count.clamp(1, 32));
        }
        if let Kind::Arithmetic(kind) = self.kind {
            if kind == crate::Arithmetic::BitAdder {
                return index == 0;
            }
            if kind == crate::Arithmetic::BitFinder {
                return index == 0 || index == 2;
            }
            return index == 0
                || (index == 4
                    && !matches!(
                        kind,
                        crate::Arithmetic::Shifter | crate::Arithmetic::Negator
                    ))
                || (index == 3 && kind == crate::Arithmetic::Comparator);
        }
        if matches!(self.kind, Kind::Plexer(_)) {
            return self
                .plexer_logic()
                .and_then(|logic| logic.port(index))
                .is_some_and(|p| p.role.is_output());
        }
        if let Kind::Subcircuit(_) = self.kind {
            return self.interface.ports.get(index).is_some_and(|p| p.output);
        }
        (index == 0 && self.drives_output())
            || (matches!(self.kind, Kind::FlipFlop(_) | Kind::Latch(_)) && index == 1)
            || (self.kind == Kind::Counter && index == 5)
    }
}

#[cfg(test)]
mod tests;

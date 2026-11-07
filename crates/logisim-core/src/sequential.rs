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
        if self.kind == Kind::RegisterFile {
            return index < 2;
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
            return self.plexer_terminals().get(index).is_some_and(|p| p.output);
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
mod tests {
    use super::*;
    #[test]
    fn counter_and_shift_geometry_is_fixed_across_width_and_mode_edits() {
        for bits in [1, 8, 32] {
            let mut c = Component::new(Kind::Counter, Point::ZERO);
            c.bits = bits;
            let ports = [
                (0, 0),
                (-30, 0),
                (-20, 20),
                (-30, -10),
                (-30, 10),
                (0, 10),
                (-10, -20),
                (-10, 20),
                (-20, -20),
            ]
            .map(|(x, y)| Point::new(x, y));
            assert_eq!(c.ports(), ports);
            for i in 0..9 {
                assert_eq!(c.port_width(i), Some(if i < 2 { bits } else { 1 }));
                assert_eq!(c.drives_port(i), i == 0 || i == 5);
            }
            let mut circuit = crate::Circuit::default();
            let id = circuit.add_component(c.clone());
            for port in ports {
                circuit.add_wire(port, port + Point::new(0, 100));
            }
            let wires = circuit.wires.clone();
            c.id = id;
            c.sequential.counter_old = true;
            assert_eq!(c.ports(), ports[..8]);
            circuit.replace_component(c);
            assert_eq!(circuit.wires, wires);
            let mut r = Component::new(Kind::ShiftRegister, Point::ZERO);
            r.bits = bits;
            assert_eq!(
                r.ports(),
                [(30, 0), (0, 0), (0, 10), (10, -20), (10, 20)].map(|(x, y)| Point::new(x, y))
            );
            assert_eq!(
                (0..5).map(|i| r.port_width(i).unwrap()).collect::<Vec<_>>(),
                [bits, bits, 1, 2, 1]
            );
        }
    }
    #[test]
    fn observed_layouts_keep_port_roles_widths_and_connection_edits() {
        for ff in FlipFlop::ALL {
            let mut c = Component::new(Kind::FlipFlop(ff), Point::ZERO);
            assert_eq!(c.ports()[0], Point::ZERO);
            assert_eq!(c.ports()[1], Point::new(0, 40));
            assert_eq!(c.ports()[2], Point::new(-60, 0));
            assert_eq!(
                c.ports()[3],
                Point::new(-60, if ff.paired() { 20 } else { 40 })
            );
            assert!(c.drives_port(0) && c.drives_port(1) && !c.drives_port(2));
            c.sequential.controls = ControlPosition::Legacy;
            assert_eq!(
                &c.ports()[c.ports().len() - 2..],
                &[Point::new(-40, 60), Point::new(-20, 60)]
            );
            c.sequential.new_layout = false;
            assert_eq!(
                c.ports()[2],
                Point::new(-40, if ff.paired() { 10 } else { 0 })
            );
            assert_eq!(
                &c.ports()[c.ports().len() - 2..],
                &[Point::new(-30, 30), Point::new(-10, 30)]
            );
        }
        let mut r = Component::new(Kind::Register, Point::ZERO);
        r.bits = 32;
        assert_eq!(
            r.ports(),
            [
                (0, 0),
                (-30, 0),
                (-20, 20),
                (-30, 10),
                (-30, -10),
                (-10, -20),
                (-10, 20)
            ]
            .map(|(x, y)| Point::new(x, y))
        );
        assert_eq!(
            (0..7).map(|i| r.port_width(i)).collect::<Vec<_>>(),
            vec![
                Some(32),
                Some(32),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1)
            ]
        );
        let mut d = crate::Circuit::default();
        let c = Component::new(Kind::FlipFlop(FlipFlop::D), Point::new(100, 100));
        let old = c.ports();
        let id = d.add_component(c);
        let end = Point::new(10, 80);
        d.add_wire(old[4], end);
        let mut changed = d.component(id).unwrap().clone();
        changed.sequential.controls = ControlPosition::BelowAbove;
        let new = changed.ports();
        d.replace_component(changed);
        assert!(d.wires.iter().any(|w| w.a == new[4] || w.b == new[4]));
        assert!(d.wires.iter().any(|w| w.a == end || w.b == end));
    }
}

#[cfg(test)]
mod random_tests {
    use super::*;
    #[test]
    fn random_fixed_geometry_and_port_widths_survive_property_edits() {
        let mut c = Component::new(Kind::Random, Point::new(200, 200));
        assert_eq!(c.bits, 8);
        assert!(!c.kind.rotatable());
        assert!(c.kind.has_label());
        let ports = [(200, 200), (170, 210), (170, 190), (180, 220)].map(|(x, y)| Point::new(x, y));
        assert_eq!(c.ports(), ports);
        let mut circuit = crate::Circuit::default();
        c.id = circuit.add_component(c.clone());
        for port in ports {
            circuit.add_wire(port, port + Point::new(60, 0));
        }
        let wires = circuit.wires.clone();
        for bits in [1, 8, 32] {
            c.bits = bits;
            c.sequential.random_seed = i32::MIN;
            c.sequential.trigger = Trigger::Falling;
            assert_eq!(c.ports(), ports);
            for i in 0..4 {
                assert_eq!(c.drives_port(i), i == 0);
                assert_eq!(c.port_width(i), Some(if i == 0 { bits } else { 1 }));
            }
            assert_eq!(c.port_width(4), None);
            circuit.replace_component(c.clone());
            assert_eq!(circuit.wires, wires);
        }
    }
}

//! Wiring attributes and terminal geometry, independent of simulation.
use crate::{Component, Kind, Point};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Radix {
    #[default]
    Binary,
    Octal,
    Signed,
    Unsigned,
    Hex,
}
impl Radix {
    pub const ALL: [Self; 5] = [
        Self::Binary,
        Self::Octal,
        Self::Signed,
        Self::Unsigned,
        Self::Hex,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Binary => "Binary",
            Self::Octal => "Octal",
            Self::Signed => "Signed decimal",
            Self::Unsigned => "Unsigned decimal",
            Self::Hex => "Hexadecimal",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Extension {
    #[default]
    Zero,
    One,
    Sign,
    Input,
}
impl Extension {
    pub const ALL: [Self; 4] = [Self::Zero, Self::One, Self::Sign, Self::Input];
    pub fn name(self) -> &'static str {
        match self {
            Self::Zero => "Zero",
            Self::One => "One",
            Self::Sign => "Sign",
            Self::Input => "Input",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitAppearance {
    #[default]
    Left,
    Right,
    Center,
    Legacy,
}
impl SplitAppearance {
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Center, Self::Legacy];
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left-handed",
            Self::Right => "Right-handed",
            Self::Center => "Centered",
            Self::Legacy => "Legacy",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splitter {
    pub fanout: u8,
    pub appearance: SplitAppearance,
    pub descending: bool,
    /// Combined bus bit -> branch index; None disconnects that bit.
    pub mapping: [Option<u8>; 32],
}
impl Default for Splitter {
    fn default() -> Self {
        let mut s = Self {
            fanout: 4,
            appearance: SplitAppearance::Left,
            descending: false,
            mapping: [None; 32],
        };
        s.distribute(4, false);
        s
    }
}
impl Splitter {
    pub fn distribute(&mut self, bits: u8, descending: bool) {
        let bits = bits.clamp(1, 32);
        self.fanout = self.fanout.clamp(1, 32);
        self.mapping.fill(None);
        self.descending = descending;
        let mut bit: u8 = 0;
        for branch in 0..self.fanout {
            let count = bits / self.fanout + u8::from(branch < bits % self.fanout);
            for _ in 0..count {
                self.mapping[usize::from(bit)] = Some(branch);
                bit += 1;
            }
        }
        if descending {
            self.mapping[..usize::from(bits)].reverse();
        }
    }
    pub fn branch_bits(&self, bits: u8, branch: u8) -> Vec<u8> {
        (0..bits.clamp(1, 32))
            .filter(|i| self.mapping[usize::from(*i)] == Some(branch))
            .collect()
    }
    pub fn ports(&self) -> Vec<Point> {
        let n = i32::from(self.fanout.clamp(1, 32));
        std::iter::once(Point::ZERO)
            .chain((0..n).map(|i| {
                Point::new(
                    20,
                    10 * match self.appearance {
                        SplitAppearance::Left => i - n,
                        SplitAppearance::Right => i + 1,
                        SplitAppearance::Center | SplitAppearance::Legacy => i - n / 2,
                    },
                )
            }))
            .collect()
    }
}
impl Component {
    /// None denotes a probe or pull resistor whose width is inferred; Some(0) is an empty splitter branch.
    pub fn port_width(&self, index: usize) -> Option<u8> {
        if matches!(self.kind, Kind::Subcircuit(_)) {
            return self.interface.ports.get(index).map(|p| p.bits);
        }
        match self.kind {
            Kind::Ttl(_) => (index < self.ttl_pins().len()).then_some(1),
            Kind::Parity(_) => {
                (index <= usize::from(self.input_count())).then_some(self.bits.clamp(1, 32))
            }
            Kind::Keyboard | Kind::Tty => {
                (index < self.io_ports().len()).then_some(if index == 0 { self.bits } else { 1 })
            }
            Kind::Joystick => (index < 2).then_some(self.bits.clamp(2, 5)),
            Kind::Button | Kind::DipSwitch | Kind::Led | Kind::SevenSegment => Some(1),
            Kind::HexDisplay => Some(if index == 0 { 4 } else { 1 }),
            Kind::RgbLed => (index < 3).then_some(if self.io.rgb_multibit { 8 } else { 1 }),
            Kind::DotMatrix => {
                (index < self.io_ports().len()).then_some(match self.io.matrix_input {
                    crate::MatrixInput::Columns => self.io.matrix_rows.clamp(1, 32),
                    crate::MatrixInput::Rows => self.io.matrix_columns.clamp(1, 32),
                    crate::MatrixInput::Select => {
                        if index == 0 {
                            self.io.matrix_columns.clamp(1, 32)
                        } else {
                            self.io.matrix_rows.clamp(1, 32)
                        }
                    }
                })
            }
            Kind::Plexer(_) => self.plexer_terminals().get(index).map(|p| p.bits),
            Kind::Arithmetic(kind) => Some(if index == 0 {
                self.arithmetic_output_bits()
            } else if kind == crate::Arithmetic::BitAdder {
                self.bits.clamp(1, 32)
            } else if (kind == crate::Arithmetic::BitFinder && index == 2)
                || (index >= 3
                    && !matches!(
                        kind,
                        crate::Arithmetic::Multiplier | crate::Arithmetic::Divider
                    ))
            {
                1
            } else if index == 2 && kind == crate::Arithmetic::Shifter {
                self.shift_amount_bits()
            } else {
                self.bits.clamp(1, 32)
            }),
            Kind::Latch(_) => (index < self.latch_terminals().len()).then_some(1),
            Kind::Clock | Kind::FlipFlop(_) => Some(1),
            Kind::PlaRom => match index {
                0 => Some(self.pla.dimensions().2),
                1 => Some(self.pla.dimensions().0),
                2 => Some(1),
                _ => None,
            },
            Kind::Rom | Kind::Ram => self.memory_terminals().get(index).map(|p| p.bits),
            Kind::Random => (index < 4).then_some(if index == 0 {
                self.bits.clamp(1, 32)
            } else {
                1
            }),
            Kind::ShiftRegister if index == 3 => Some(2),
            Kind::Register | Kind::Counter | Kind::ShiftRegister if index >= 2 => Some(1),
            Kind::Transistor | Kind::TransmissionGate => (index < self.transistor_ports().len())
                .then_some(if index < 2 { self.bits.clamp(1, 32) } else { 1 }),
            Kind::Probe | Kind::PullResistor => None,
            Kind::ControlledBuffer | Kind::ControlledInverter => {
                (index < 3).then_some(if index == 2 {
                    1
                } else {
                    self.bits.clamp(1, 32)
                })
            }
            Kind::Splitter if index > 0 => Some(
                self.splitter
                    .branch_bits(self.bits, (index - 1) as u8)
                    .len() as u8,
            ),
            Kind::BitExtender if index == 1 => Some(self.input_bits.clamp(1, 32)),
            Kind::BitExtender if index == 2 => Some(1),
            _ => Some(self.bits.clamp(1, 32)),
        }
    }
    pub fn drives_output(&self) -> bool {
        matches!(
            self.kind,
            Kind::Gate(_)
                | Kind::Parity(_)
                | Kind::ControlledBuffer
                | Kind::ControlledInverter
                | Kind::Transistor
                | Kind::TransmissionGate
                | Kind::Button
                | Kind::Switch
                | Kind::DipSwitch
                | Kind::Slider
                | Kind::Joystick
                | Kind::Constant
                | Kind::Power
                | Kind::Ground
                | Kind::BitExtender
                | Kind::Clock
                | Kind::FlipFlop(_)
                | Kind::Latch(_)
                | Kind::Register
                | Kind::Counter
                | Kind::ShiftRegister
                | Kind::Random
                | Kind::Rom
                | Kind::PlaRom
                | Kind::Ram
        ) || (self.kind == Kind::Pin && !self.output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circuit, Facing};
    #[test]
    fn uneven_descending_distribution_reverses_bit_order_not_branch_sizes() {
        let mut splitter = Splitter {
            fanout: 3,
            ..Splitter::default()
        };
        splitter.distribute(8, false);
        assert_eq!(
            &splitter.mapping[..8],
            &[
                Some(0),
                Some(0),
                Some(0),
                Some(1),
                Some(1),
                Some(1),
                Some(2),
                Some(2)
            ]
        );
        splitter.distribute(8, true);
        assert_eq!(
            &splitter.mapping[..8],
            &[
                Some(2),
                Some(2),
                Some(1),
                Some(1),
                Some(1),
                Some(0),
                Some(0),
                Some(0)
            ]
        );
        assert!(splitter.descending);
    }
    #[test]
    fn measured_orientations_keep_reference_branch_numbering() {
        let mut c = Component::new(Kind::Splitter, Point::new(100, 100));
        for (facing, expected) in [
            (Facing::East, [(120, 60), (120, 70), (120, 80), (120, 90)]),
            (
                Facing::South,
                [(140, 120), (130, 120), (120, 120), (110, 120)],
            ),
            (Facing::West, [(80, 110), (80, 120), (80, 130), (80, 140)]),
            (Facing::North, [(90, 80), (80, 80), (70, 80), (60, 80)]),
        ] {
            c.facing = facing;
            assert_eq!(c.ports()[1..], expected.map(|(x, y)| Point::new(x, y)));
        }
    }
    #[test]
    fn editing_splitter_layout_reroutes_surviving_branch_connections() {
        let mut d = Circuit::default();
        let c = Component::new(Kind::Splitter, Point::ZERO);
        let before = c.ports();
        let id = d.add_component(c);
        d.add_wire(before[1], Point::new(100, before[1].y));
        let mut c = d.component(id).unwrap().clone();
        c.splitter.appearance = SplitAppearance::Right;
        c.splitter.fanout = 3;
        c.splitter.distribute(c.bits, false);
        let after = c.ports();
        d.replace_component(c);
        assert!(d.wires.iter().any(|w| w.a == after[1] || w.b == after[1]));
        assert!(
            d.wires
                .iter()
                .all(|w| w.a != w.b && (w.a.x == w.b.x || w.a.y == w.b.y))
        );
    }
}

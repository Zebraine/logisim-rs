//! Terminal roles and geometry observed from the running reference.
use crate::{Component, Facing, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plexer {
    Multiplexer,
    Demultiplexer,
    Decoder,
    PriorityEncoder,
}
impl Plexer {
    pub const ALL: [Self; 4] = [
        Self::Multiplexer,
        Self::Demultiplexer,
        Self::Decoder,
        Self::PriorityEncoder,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Multiplexer => "Multiplexer",
            Self::Demultiplexer => "Demultiplexer",
            Self::Decoder => "Decoder",
            Self::PriorityEncoder => "Priority Encoder",
        }
    }
    pub fn has_data_bits(self) -> bool {
        matches!(self, Self::Multiplexer | Self::Demultiplexer)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plexing {
    pub select_bits: u8,
    pub top_right: bool,
    pub enable: bool,
    pub disabled_zero: bool,
    pub three_state: bool,
    pub no_input_zero: bool,
}
impl Default for Plexing {
    fn default() -> Self {
        Self {
            select_bits: 1,
            top_right: false,
            enable: false,
            disabled_zero: false,
            three_state: false,
            no_input_zero: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlexerRole {
    Input(u8),
    Output(u8),
    Select,
    Enable,
    Group,
    EnableOut,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlexerTerminal {
    pub role: PlexerRole,
    pub offset: Point,
    pub bits: u8,
    pub output: bool,
}
impl Component {
    pub fn plexer_count(&self) -> u8 {
        1 << self.plexing.select_bits.clamp(1, 5)
    }
    // Longitudinal coordinates follow facing; transverse numbering stays screen-ascending.
    fn plexer_axis(&self, longitudinal: i32, transverse: i32) -> Point {
        self.facing.inverse().transform(match self.facing {
            Facing::East => Point::new(longitudinal, transverse),
            Facing::West => Point::new(-longitudinal, transverse),
            Facing::South => Point::new(transverse, longitudinal),
            Facing::North => Point::new(transverse, -longitudinal),
        })
    }
    fn plexer_side(&self) -> i32 {
        let vertical = matches!(self.facing, Facing::North | Facing::South);
        if self.plexing.top_right == vertical {
            1
        } else {
            -1
        }
    }
    pub fn plexer_terminals(&self) -> Vec<PlexerTerminal> {
        use PlexerRole::*;
        let Kind::Plexer(kind) = self.kind else {
            return vec![];
        };
        let n = self.plexer_count();
        let half = i32::from(n) * 5;
        let depth = if n == 2 { 30 } else { 40 };
        let height = if n == 2 { 20 } else { half };
        let bits = self.bits.clamp(1, 32);
        let mut result = vec![];
        let mut add = |role, x, y, bits| {
            result.push(PlexerTerminal {
                role,
                offset: self.plexer_axis(x, y),
                bits,
                output: matches!(role, Output(_) | Group | EnableOut),
            })
        };
        match kind {
            Plexer::Multiplexer | Plexer::Demultiplexer => {
                let mux = kind == Plexer::Multiplexer;
                let sign = if mux { -1 } else { 1 };
                add(if mux { Output(0) } else { Input(0) }, 0, 0, bits);
                add(
                    Select,
                    sign * 20,
                    self.plexer_side() * height,
                    self.plexing.select_bits.clamp(1, 5),
                );
                if self.plexing.enable {
                    add(Enable, sign * 10, self.plexer_side() * height, 1);
                }
                for i in 0..n {
                    add(
                        if mux { Input(i) } else { Output(i) },
                        sign * depth,
                        if n == 2 {
                            -10 + i32::from(i) * 20
                        } else {
                            -half + i32::from(i) * 10
                        },
                        bits,
                    );
                }
            }
            Plexer::Decoder => {
                add(Select, 0, 0, self.plexing.select_bits.clamp(1, 5));
                if self.plexing.enable {
                    add(Enable, -10, 0, 1);
                }
                let start = if self.plexer_side() > 0 {
                    if n == 2 { -30 } else { -i32::from(n) * 10 }
                } else if n == 2 {
                    10
                } else {
                    0
                };
                for i in 0..n {
                    add(
                        Output(i),
                        if n == 2 { 10 } else { 20 },
                        start + i32::from(i) * if n == 2 { 20 } else { 10 },
                        1,
                    );
                }
            }
            Plexer::PriorityEncoder => {
                add(Output(0), 0, 0, self.plexing.select_bits.clamp(1, 5));
                add(Group, 0, 10, 1);
                add(EnableOut, -20, -half, 1);
                add(Enable, -20, half + 10, 1);
                for i in 0..n {
                    add(Input(i), -40, 10 - half + i32::from(i) * 10, 1);
                }
            }
        }
        result
    }
    /// Local polygon for drawing and hit testing; terminal coordinates remain exact.
    pub fn plexer_body(&self) -> Vec<Point> {
        let Kind::Plexer(kind) = self.kind else {
            return vec![];
        };
        let n = i32::from(self.plexer_count());
        let h = if n == 2 { 20 } else { n * 5 + 10 };
        let depth = if n == 2 { 30 } else { 40 };
        let inset = if n == 2 { 10 } else { 20 };
        let points = match kind {
            Plexer::Multiplexer => vec![(-depth, -h), (-depth, h), (0, h - inset), (0, inset - h)],
            Plexer::Demultiplexer => vec![(0, inset - h), (0, h - inset), (depth, h), (depth, -h)],
            Plexer::Decoder => {
                let start = if self.plexer_side() > 0 {
                    if n == 2 { -40 } else { -n * 10 - 10 }
                } else if n == 2 {
                    0
                } else {
                    -10
                };
                let end = start + if n == 2 { 40 } else { (n + 2) * 10 };
                let d = if n == 2 { 10 } else { 20 };
                vec![
                    (-20, start + inset),
                    (-20, end - inset),
                    (d, end),
                    (d, start),
                ]
            }
            Plexer::PriorityEncoder => vec![
                (-40, -n * 5),
                (-40, n * 5 + 10),
                (0, n * 5 + 10),
                (0, -n * 5),
            ],
        };
        points
            .into_iter()
            .map(|(x, y)| self.plexer_axis(x, y))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circuit;
    fn terminal(c: &Component, role: PlexerRole) -> Point {
        c.position
            + c.facing.transform(
                c.plexer_terminals()
                    .into_iter()
                    .find(|p| p.role == role)
                    .unwrap()
                    .offset,
            )
    }
    #[test]
    fn observed_default_and_rotated_numbering_and_control_sides() {
        use PlexerRole::*;
        let mut mux = Component::new(Kind::Plexer(Plexer::Multiplexer), Point::ZERO);
        assert_eq!(
            mux.ports(),
            [(0, 0), (-20, 20), (-30, -10), (-30, 10)].map(|(x, y)| Point::new(x, y))
        );
        mux.plexing.select_bits = 2;
        mux.plexing.enable = true;
        for (facing, input, select, enable) in [
            (Facing::East, (-40, -20), (-20, 20), (-10, 20)),
            (Facing::South, (-20, -40), (-20, -20), (-20, -10)),
            (Facing::West, (40, -20), (20, 20), (10, 20)),
            (Facing::North, (-20, 40), (-20, 20), (-20, 10)),
        ] {
            mux.facing = facing;
            assert_eq!(terminal(&mux, Input(0)), Point::new(input.0, input.1));
            assert_eq!(terminal(&mux, Select), Point::new(select.0, select.1));
            assert_eq!(terminal(&mux, Enable), Point::new(enable.0, enable.1));
        }
        let mut dec = Component::new(Kind::Plexer(Plexer::Decoder), Point::ZERO);
        assert_eq!(terminal(&dec, Output(0)), Point::new(10, -30));
        dec.plexing.select_bits = 2;
        dec.facing = Facing::South;
        assert_eq!(terminal(&dec, Output(0)), Point::new(0, 20));
        assert_eq!(terminal(&dec, Output(3)), Point::new(30, 20));
        dec.plexing.top_right = true;
        assert_eq!(terminal(&dec, Output(0)), Point::new(-40, 20));
        let enc = Component::new(Kind::Plexer(Plexer::PriorityEncoder), Point::ZERO);
        assert_eq!(terminal(&enc, Input(0)), Point::new(-40, -30));
        assert_eq!(terminal(&enc, Enable), Point::new(-20, 50));
        assert_eq!(terminal(&enc, EnableOut), Point::new(-20, -40));
        assert_eq!(terminal(&enc, Group), Point::new(0, 10));
    }
    #[test]
    fn edits_preserve_semantic_connections_when_control_and_data_counts_change() {
        use PlexerRole::*;
        let mut circuit = Circuit::default();
        let mut c = Component::new(Kind::Plexer(Plexer::Multiplexer), Point::new(300, 300));
        let data = terminal(&c, Input(1));
        let sel = terminal(&c, Select);
        let id = circuit.add_component(c.clone());
        circuit.add_wire(data, Point::new(100, data.y));
        circuit.add_wire(sel, Point::new(sel.x, 500));
        c.id = id;
        c.plexing.enable = true;
        c.plexing.select_bits = 3;
        c.plexing.top_right = true;
        let new_data = terminal(&c, Input(1));
        let new_select = terminal(&c, Select);
        let en = terminal(&c, Enable);
        circuit.replace_component(c);
        assert!(
            circuit
                .wires
                .iter()
                .any(|w| w.a == new_data || w.b == new_data)
        );
        assert!(
            circuit
                .wires
                .iter()
                .any(|w| w.a == new_select || w.b == new_select)
        );
        assert!(!circuit.wires.iter().any(|w| w.a == en || w.b == en));
    }
}

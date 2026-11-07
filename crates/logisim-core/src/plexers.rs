//! Terminal roles and geometry observed from the running reference.
use crate::{Component, Facing, Kind, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plexer {
    Multiplexer,
    Demultiplexer,
    Decoder,
    PriorityEncoder,
    BitSelector,
    DisplayDecoder,
}
impl Plexer {
    pub const ALL: [Self; 6] = [
        Self::Multiplexer,
        Self::Demultiplexer,
        Self::Decoder,
        Self::PriorityEncoder,
        Self::BitSelector,
        Self::DisplayDecoder,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Multiplexer => "Multiplexer",
            Self::Demultiplexer => "Demultiplexer",
            Self::Decoder => "Decoder",
            Self::PriorityEncoder => "Priority Encoder",
            Self::BitSelector => "Bit Selector",
            Self::DisplayDecoder => "7-Segment Display Decoder",
        }
    }
    pub fn has_data_bits(self) -> bool {
        matches!(
            self,
            Self::Multiplexer | Self::Demultiplexer | Self::BitSelector
        )
    }
    pub fn file_name(self) -> &'static str {
        match self {
            Self::BitSelector => "BitSelector",
            Self::DisplayDecoder => "DisplayDecoder",
            _ => self.name(),
        }
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
    pub group_bits: u8,
    pub multibit: bool,
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
            group_bits: 1,
            multibit: true,
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
    LampTest,
    Blanking,
    ZeroBlanking,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlexerTerminal {
    pub role: PlexerRole,
    pub offset: Point,
    pub bits: u8,
    pub output: bool,
}
impl Component {
    pub fn bit_selector_select_bits(&self) -> u8 {
        let groups = self
            .bits
            .clamp(1, 32)
            .div_ceil(self.plexing.group_bits.clamp(1, 32));
        (8 - (groups - 1).leading_zeros() as u8).max(1)
    }
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
            Plexer::BitSelector => {
                add(Output(0), 0, 0, self.plexing.group_bits.clamp(1, 32));
                add(Input(0), -30, 0, bits);
                add(
                    Select,
                    -10,
                    self.plexer_side() * 10,
                    self.bit_selector_select_bits(),
                );
            }
            Plexer::DisplayDecoder => {
                for i in 0..7 {
                    add(Output(i), 0, -30 + i32::from(i) * 10, 1);
                }
                if self.plexing.multibit {
                    add(Input(0), -40, 0, 4);
                } else {
                    for i in 0..4 {
                        add(Input(i), -40, -30 + i32::from(i) * 20, 1);
                    }
                }
                add(Enable, -20, -40, 1);
                add(LampTest, -30, 40, 1);
                add(Blanking, -20, 40, 1);
                add(ZeroBlanking, -10, 40, 1);
            }
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
            Plexer::BitSelector => vec![(-30, -15), (-30, 15), (0, 5), (0, -5)],
            Plexer::DisplayDecoder => vec![(-40, -40), (-40, 40), (0, 40), (0, -40)],
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

#[cfg(test)]
mod extra_tests {
    use super::*;
    use crate::Circuit;
    #[test]
    fn selector_group_widths_and_screen_sides_match_runtime_connections() {
        let mut c = Component::new(Kind::Plexer(Plexer::BitSelector), Point::ZERO);
        assert_eq!(c.bits, 8);
        for (width, group, select) in [
            (1, 1, 1),
            (8, 1, 3),
            (8, 3, 2),
            (5, 3, 1),
            (8, 32, 1),
            (32, 1, 5),
            (32, 7, 3),
            (32, 32, 1),
        ] {
            c.bits = width;
            c.plexing.group_bits = group;
            assert_eq!(c.bit_selector_select_bits(), select);
            assert_eq!(c.port_width(0), Some(group));
            assert_eq!(c.port_width(1), Some(width));
            assert_eq!(c.port_width(2), Some(select));
            for (facing, data, bottom, top) in [
                (Facing::East, (-30, 0), (-10, 10), (-10, -10)),
                (Facing::South, (0, -30), (-10, -10), (10, -10)),
                (Facing::West, (30, 0), (10, 10), (10, -10)),
                (Facing::North, (0, 30), (-10, 10), (10, 10)),
            ] {
                c.facing = facing;
                for (tr, s) in [(false, bottom), (true, top)] {
                    c.plexing.top_right = tr;
                    assert_eq!(
                        c.ports(),
                        vec![
                            Point::ZERO,
                            Point::new(data.0, data.1),
                            Point::new(s.0, s.1)
                        ]
                    );
                }
            }
        }
    }
    #[test]
    fn display_screen_order_and_input_format_edits_keep_original_wires() {
        use PlexerRole::*;
        let mut c = Component::new(Kind::Plexer(Plexer::DisplayDecoder), Point::ZERO);
        for (facing, first, data, en, lt, bi, rbi) in [
            (
                Facing::East,
                (0, -30),
                (-40, 0),
                (-20, -40),
                (-30, 40),
                (-20, 40),
                (-10, 40),
            ),
            (
                Facing::South,
                (-30, 0),
                (0, -40),
                (-40, -20),
                (40, -30),
                (40, -20),
                (40, -10),
            ),
            (
                Facing::West,
                (0, -30),
                (40, 0),
                (20, -40),
                (30, 40),
                (20, 40),
                (10, 40),
            ),
            (
                Facing::North,
                (-30, 0),
                (0, 40),
                (-40, 20),
                (40, 30),
                (40, 20),
                (40, 10),
            ),
        ] {
            c.facing = facing;
            for (role, want) in [
                (Output(0), first),
                (Input(0), data),
                (Enable, en),
                (LampTest, lt),
                (Blanking, bi),
                (ZeroBlanking, rbi),
            ] {
                let p = c
                    .plexer_terminals()
                    .into_iter()
                    .find(|p| p.role == role)
                    .unwrap();
                assert_eq!(facing.transform(p.offset), Point::new(want.0, want.1));
                assert_eq!(p.bits, if role == Input(0) { 4 } else { 1 });
            }
            c.plexing.multibit = false;
            let ps = c.ports();
            assert_eq!(ps.len(), 15);
            for i in 0..4 {
                assert_eq!(c.port_width(7 + i), Some(1));
            }
            let first_input = match facing {
                Facing::East => Point::new(-40, -30),
                Facing::West => Point::new(40, -30),
                Facing::South => Point::new(-30, -40),
                Facing::North => Point::new(-30, 40),
            };
            assert_eq!(ps[7], first_input);
            c.plexing.multibit = true;
        }
        c.facing = Facing::East;
        c.position = Point::new(200, 200);
        let mut circuit = Circuit::default();
        let id = circuit.add_component(c);
        circuit.add_wire(Point::new(100, 200), Point::new(160, 200));
        circuit.add_wire(Point::new(180, 100), Point::new(180, 160));
        let wires = circuit.wires.clone();
        let mut changed = circuit.component(id).unwrap().clone();
        changed.plexing.multibit = false;
        circuit.replace_component(changed);
        assert_eq!(circuit.wires, wires);
    }
}

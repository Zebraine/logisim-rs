//! Placement and drawing geometry consume the logical terminal interface.
use super::{Plexer, PlexerRole, PlexerTerminal};
use crate::{Component, Facing, Kind, Point};

impl Component {
    pub fn bit_selector_select_bits(&self) -> u8 {
        super::logic::selector_bits(self.bits, self.plexing.group_bits)
    }
    pub fn plexer_count(&self) -> u8 {
        super::logic::count(self.plexing.select_bits)
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
        let Some(logic) = self.plexer_logic() else {
            return vec![];
        };
        logic
            .ports()
            .map(|port| {
                let (x, y) = self.plexer_position(logic.kind, port.role);
                PlexerTerminal {
                    role: port.role,
                    bits: port.bits,
                    output: port.role.is_output(),
                    offset: self.plexer_axis(x, y),
                }
            })
            .collect()
    }

    fn plexer_position(&self, kind: Plexer, role: PlexerRole) -> (i32, i32) {
        use PlexerRole::*;
        let n = self.plexer_count();
        let half = i32::from(n) * 5;
        match kind {
            Plexer::BitSelector => match role {
                Output(0) => (0, 0),
                Input(0) => (-30, 0),
                Select => (-10, self.plexer_side() * 10),
                _ => unreachable!("bit selector terminal"),
            },
            Plexer::DisplayDecoder => match role {
                Output(i) => (0, -30 + i32::from(i) * 10),
                Input(i) => (
                    -40,
                    if self.plexing.multibit {
                        0
                    } else {
                        -30 + i32::from(i) * 20
                    },
                ),
                Enable => (-20, -40),
                LampTest => (-30, 40),
                Blanking => (-20, 40),
                ZeroBlanking => (-10, 40),
                _ => unreachable!("display decoder terminal"),
            },
            Plexer::Multiplexer | Plexer::Demultiplexer => {
                let mux = kind == Plexer::Multiplexer;
                let sign = if mux { -1 } else { 1 };
                let height = if n == 2 { 20 } else { half };
                match role {
                    Output(0) if mux => (0, 0),
                    Input(0) if !mux => (0, 0),
                    Select => (sign * 20, self.plexer_side() * height),
                    Enable => (sign * 10, self.plexer_side() * height),
                    Input(i) | Output(i) => (
                        sign * if n == 2 { 30 } else { 40 },
                        if n == 2 {
                            -10 + i32::from(i) * 20
                        } else {
                            -half + i32::from(i) * 10
                        },
                    ),
                    _ => unreachable!("mux/demux terminal"),
                }
            }
            Plexer::Decoder => match role {
                Select => (0, 0),
                Enable => (-10, 0),
                Output(i) => {
                    let start = if self.plexer_side() > 0 {
                        if n == 2 { -30 } else { -i32::from(n) * 10 }
                    } else if n == 2 {
                        10
                    } else {
                        0
                    };
                    (
                        if n == 2 { 10 } else { 20 },
                        start + i32::from(i) * if n == 2 { 20 } else { 10 },
                    )
                }
                _ => unreachable!("decoder terminal"),
            },
            Plexer::PriorityEncoder => match role {
                Output(0) => (0, 0),
                Group => (0, 10),
                EnableOut => (-20, -half),
                Enable => (-20, half + 10),
                Input(i) => (-40, 10 - half + i32::from(i) * 10),
                _ => unreachable!("priority encoder terminal"),
            },
        }
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

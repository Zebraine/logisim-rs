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
mod tests;

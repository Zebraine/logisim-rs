//! Logical terminal order and behavior, independent of placement and drawing.
use super::{Plexer, PlexerRole};
use crate::{Component, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlexerLogic {
    pub kind: Plexer,
    pub bits: u8,
    pub select_bits: u8,
    pub enable: bool,
    pub disabled_zero: bool,
    pub three_state: bool,
    pub no_input_zero: bool,
    pub group_bits: u8,
    pub multibit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlexerPort {
    pub role: PlexerRole,
    pub bits: u8,
}

impl PlexerRole {
    pub fn is_output(self) -> bool {
        matches!(self, Self::Output(_) | Self::Group | Self::EnableOut)
    }
}

pub(super) fn count(select_bits: u8) -> u8 {
    1 << select_bits.clamp(1, 5)
}
pub(super) fn selector_bits(bits: u8, group_bits: u8) -> u8 {
    let groups = bits.clamp(1, 32).div_ceil(group_bits.clamp(1, 32));
    (8 - (groups - 1).leading_zeros() as u8).max(1)
}

impl PlexerLogic {
    pub fn count(&self) -> u8 {
        count(self.select_bits)
    }

    pub fn selector_bits(&self) -> u8 {
        selector_bits(self.bits, self.group_bits)
    }

    pub fn port_count(&self) -> usize {
        match self.kind {
            Plexer::BitSelector => 3,
            Plexer::DisplayDecoder => {
                if self.multibit {
                    12
                } else {
                    15
                }
            }
            Plexer::Multiplexer | Plexer::Demultiplexer => {
                2 + usize::from(self.enable) + usize::from(self.count())
            }
            Plexer::Decoder => 1 + usize::from(self.enable) + usize::from(self.count()),
            Plexer::PriorityEncoder => 4 + usize::from(self.count()),
        }
    }

    pub fn port(&self, index: usize) -> Option<PlexerPort> {
        use PlexerRole::*;
        if index >= self.port_count() {
            return None;
        }
        let bits = self.bits.clamp(1, 32);
        let select = self.select_bits.clamp(1, 5);
        let (role, bits) = match self.kind {
            Plexer::BitSelector => match index {
                0 => (Output(0), self.group_bits.clamp(1, 32)),
                1 => (Input(0), bits),
                _ => (Select, self.selector_bits()),
            },
            Plexer::DisplayDecoder => {
                let controls = if self.multibit { 8 } else { 11 };
                if index < 7 {
                    (Output(index as u8), 1)
                } else if index < controls {
                    (Input((index - 7) as u8), if self.multibit { 4 } else { 1 })
                } else {
                    (
                        [Enable, LampTest, Blanking, ZeroBlanking][index - controls],
                        1,
                    )
                }
            }
            Plexer::Multiplexer | Plexer::Demultiplexer => {
                let mux = self.kind == Plexer::Multiplexer;
                match index {
                    0 => (if mux { Output(0) } else { Input(0) }, bits),
                    1 => (Select, select),
                    2 if self.enable => (Enable, 1),
                    _ => {
                        let i = (index - 2 - usize::from(self.enable)) as u8;
                        (if mux { Input(i) } else { Output(i) }, bits)
                    }
                }
            }
            Plexer::Decoder => match index {
                0 => (Select, select),
                1 if self.enable => (Enable, 1),
                _ => (Output((index - 1 - usize::from(self.enable)) as u8), 1),
            },
            Plexer::PriorityEncoder => match index {
                0 => (Output(0), select),
                1 => (Group, 1),
                2 => (EnableOut, 1),
                3 => (Enable, 1),
                _ => (Input((index - 4) as u8), 1),
            },
        };
        Some(PlexerPort { role, bits })
    }

    pub fn ports(&self) -> impl ExactSizeIterator<Item = PlexerPort> + '_ {
        (0..self.port_count()).map(|i| self.port(i).expect("index inside interface"))
    }

    pub fn port_index(&self, role: PlexerRole) -> Option<usize> {
        use PlexerRole::*;
        let index = match (self.kind, role) {
            (Plexer::BitSelector, Output(0)) => 0,
            (Plexer::BitSelector, Input(0)) => 1,
            (Plexer::BitSelector, Select) => 2,
            (Plexer::DisplayDecoder, Output(i)) if i < 7 => usize::from(i),
            (Plexer::DisplayDecoder, Input(i)) if i < if self.multibit { 1 } else { 4 } => {
                7 + usize::from(i)
            }
            (Plexer::DisplayDecoder, Enable | LampTest | Blanking | ZeroBlanking) => {
                (if self.multibit { 8 } else { 11 })
                    + match role {
                        Enable => 0,
                        LampTest => 1,
                        Blanking => 2,
                        _ => 3,
                    }
            }
            (Plexer::Multiplexer, Output(0)) | (Plexer::Demultiplexer, Input(0)) => 0,
            (Plexer::Multiplexer | Plexer::Demultiplexer, Select) => 1,
            (Plexer::Multiplexer | Plexer::Demultiplexer, Enable) if self.enable => 2,
            (Plexer::Multiplexer, Input(i)) | (Plexer::Demultiplexer, Output(i))
                if i < self.count() =>
            {
                2 + usize::from(self.enable) + usize::from(i)
            }
            (Plexer::Decoder, Select) => 0,
            (Plexer::Decoder, Enable) if self.enable => 1,
            (Plexer::Decoder, Output(i)) if i < self.count() => {
                1 + usize::from(self.enable) + usize::from(i)
            }
            (Plexer::PriorityEncoder, Output(0)) => 0,
            (Plexer::PriorityEncoder, Group) => 1,
            (Plexer::PriorityEncoder, EnableOut) => 2,
            (Plexer::PriorityEncoder, Enable) => 3,
            (Plexer::PriorityEncoder, Input(i)) if i < self.count() => 4 + usize::from(i),
            _ => return None,
        };
        Some(index)
    }
}

impl Component {
    pub fn plexer_logic(&self) -> Option<PlexerLogic> {
        let Kind::Plexer(kind) = self.kind else {
            return None;
        };
        Some(PlexerLogic {
            kind,
            bits: self.bits.clamp(1, 32),
            select_bits: self.plexing.select_bits,
            enable: self.plexing.enable,
            disabled_zero: self.plexing.disabled_zero,
            three_state: self.plexing.three_state,
            no_input_zero: self.plexing.no_input_zero,
            group_bits: self.plexing.group_bits,
            multibit: self.plexing.multibit,
        })
    }
}

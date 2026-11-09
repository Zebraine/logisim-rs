//! Terminal roles and geometry observed from the running reference.
use crate::Point;
mod geometry;
mod logic;
pub use logic::{PlexerLogic, PlexerPort};

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

#[cfg(test)]
mod tests;

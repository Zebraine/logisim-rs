//! Persistent memory attributes and interfaces, independent of instance storage.
mod contents;
mod geometry;
mod logic;
use crate::{Kind, Point};
pub use contents::MemoryContents;
pub use logic::{MemoryKind, MemoryLogic, MemoryPort};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MemoryInterface {
    #[default]
    SharedSync,
    SharedAsync,
    Separate,
}
impl MemoryInterface {
    pub const ALL: [Self; 3] = [Self::SharedSync, Self::SharedAsync, Self::Separate];
    pub fn name(self) -> &'static str {
        match self {
            Self::SharedSync => "Shared data port · synchronous",
            Self::SharedAsync => "Shared data port · asynchronous",
            Self::Separate => "Separate read and write ports",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    pub address_bits: u8,
    pub interface: MemoryInterface,
    pub select_high: bool,
    pub simple: bool,
    pub contents: MemoryContents,
}
impl Default for Memory {
    fn default() -> Self {
        Self {
            address_bits: 8,
            interface: MemoryInterface::SharedSync,
            select_high: false,
            simple: true,
            contents: MemoryContents::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryRole {
    Data,
    Address,
    Input,
    Select,
    Clock,
    Load,
    Store,
    Clear,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryTerminal {
    pub role: MemoryRole,
    pub offset: Point,
    pub bits: u8,
}
impl Kind {
    pub fn is_memory(self) -> bool {
        matches!(self, Self::Rom | Self::Ram)
    }
}
#[cfg(test)]
mod tests;

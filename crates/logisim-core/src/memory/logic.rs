//! Electrical configuration and terminal order without document images or drawing.
use super::{MemoryInterface, MemoryRole};
use crate::{Component, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryKind {
    Rom,
    Ram {
        interface: MemoryInterface,
        simple: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryLogic {
    pub kind: MemoryKind,
    pub bits: u8,
    pub address_bits: u8,
    pub select_high: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryPort {
    pub role: MemoryRole,
    pub bits: u8,
}

impl MemoryLogic {
    pub fn capacity(&self) -> u32 {
        1 << self.address_bits.clamp(2, 24)
    }
    pub fn separate(&self) -> bool {
        matches!(
            self.kind,
            MemoryKind::Ram {
                interface: MemoryInterface::Separate,
                ..
            }
        )
    }
    pub fn asynchronous(&self) -> bool {
        matches!(
            self.kind,
            MemoryKind::Ram {
                interface: MemoryInterface::SharedAsync,
                ..
            }
        )
    }
    pub fn ports(&self) -> impl Iterator<Item = MemoryPort> {
        use MemoryRole::*;
        let (ram, simple) = match self.kind {
            MemoryKind::Rom => (false, true),
            MemoryKind::Ram { simple, .. } => (true, simple),
        };
        let separate = self.separate();
        [
            Some(Data),
            Some(Address),
            separate.then_some(Input),
            (!separate || !simple).then_some(Select),
            (ram && !self.asynchronous()).then_some(Clock),
            (ram && !separate).then_some(Load),
            (separate && !simple).then_some(Store),
            (ram && !simple).then_some(Clear),
        ]
        .into_iter()
        .flatten()
        .map(|role| MemoryPort {
            role,
            bits: match role {
                Data | Input => self.bits.clamp(1, 32),
                Address => self.address_bits.clamp(2, 24),
                _ => 1,
            },
        })
    }
    pub fn port(&self, index: usize) -> Option<MemoryPort> {
        self.ports().nth(index)
    }
    pub fn port_index(&self, role: MemoryRole) -> Option<usize> {
        self.ports().position(|p| p.role == role)
    }
}

impl Component {
    pub fn memory_logic(&self) -> Option<MemoryLogic> {
        Some(MemoryLogic {
            kind: match self.kind {
                Kind::Rom => MemoryKind::Rom,
                Kind::Ram => MemoryKind::Ram {
                    interface: self.memory.interface,
                    simple: self.memory.simple,
                },
                _ => return None,
            },
            bits: self.bits.clamp(1, 32),
            address_bits: self.memory.address_bits.clamp(2, 24),
            select_high: self.memory.select_high,
        })
    }
    pub fn memory_capacity(&self) -> u32 {
        1 << self.memory.address_bits.clamp(2, 24)
    }
    pub fn memory_port(&self, role: MemoryRole) -> Option<usize> {
        self.memory_logic()?.port_index(role)
    }
}

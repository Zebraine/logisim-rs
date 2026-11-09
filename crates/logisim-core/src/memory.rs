//! Memory geometry and editable ROM data, independent of runtime storage.
use crate::{Component, Kind, Point};
use std::{collections::BTreeMap, sync::Arc};

/// Piecewise constant words. Zero is implicit; long repeated runs use two boundaries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryContents {
    changes: Arc<BTreeMap<u32, u32>>,
}
impl MemoryContents {
    pub fn get(&self, address: u32) -> u32 {
        self.changes
            .range(..=address)
            .next_back()
            .map_or(0, |(_, v)| *v)
    }
    pub fn boundaries(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.changes.iter().map(|(a, v)| (*a, *v))
    }
    pub fn set(&mut self, address: u32, value: u32) {
        if address < (1 << 24) {
            self.fill(address, address + 1, value);
        }
    }
    pub fn fill(&mut self, start: u32, end: u32, value: u32) {
        if start >= end || end > (1 << 24) {
            return;
        }
        let before = if start == 0 { 0 } else { self.get(start - 1) };
        let after = self.get(end);
        let map = Arc::make_mut(&mut self.changes);
        let removed: Vec<_> = map.range(start..=end).map(|(a, _)| *a).collect();
        for a in removed {
            map.remove(&a);
        }
        if before != value {
            map.insert(start, value);
        }
        if value != after {
            map.insert(end, after);
        }
    }
    pub fn resize(&mut self, address_bits: u8, data_bits: u8) {
        let limit = 1 << address_bits.clamp(2, 24);
        let mask = u32::MAX >> (32 - data_bits.clamp(1, 32));
        let mut previous = 0;
        let mut changes = BTreeMap::new();
        for (address, value) in self.boundaries().take_while(|(a, _)| *a < limit) {
            let value = value & mask;
            if value != previous {
                changes.insert(address, value);
                previous = value;
            }
        }
        if previous != 0 {
            changes.insert(limit, 0);
        }
        self.changes = Arc::new(changes);
    }
    pub fn clear(&mut self) {
        self.changes = Arc::default();
    }
}
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
impl Component {
    pub fn memory_capacity(&self) -> u32 {
        1 << self.memory.address_bits.clamp(2, 24)
    }
    pub fn memory_terminals(&self) -> Vec<MemoryTerminal> {
        if !self.kind.is_memory() {
            return vec![];
        }
        use MemoryRole::*;
        let mut p = vec![
            (Data, 0, 0, self.bits.clamp(1, 32)),
            (Address, -140, 0, self.memory.address_bits.clamp(2, 24)),
        ];
        let separate = self.memory.interface == MemoryInterface::Separate;
        if self.kind == Kind::Rom {
            p.push((Select, -90, 40, 1));
        } else {
            if separate {
                p.push((Input, -140, 20, self.bits.clamp(1, 32)));
            }
            if !separate || !self.memory.simple {
                p.push((Select, -90, 40, 1));
            }
            if self.memory.interface != MemoryInterface::SharedAsync {
                p.push((Clock, -70, 40, 1));
            }
            if !separate {
                p.push((Load, -50, 40, 1));
            }
            if separate && !self.memory.simple {
                p.push((Store, -110, 40, 1));
            }
            if !self.memory.simple {
                p.push((Clear, -30, 40, 1));
            }
        }
        p.into_iter()
            .map(|(role, x, y, bits)| MemoryTerminal {
                role,
                offset: Point::new(x, y),
                bits,
            })
            .collect()
    }
    pub fn memory_port(&self, role: MemoryRole) -> Option<usize> {
        self.memory_terminals().iter().position(|p| p.role == role)
    }
}

#[cfg(test)]
mod tests;

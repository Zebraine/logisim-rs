//! Compressed memory images shared by documents and instance storage.
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

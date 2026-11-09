//! Instance RAM storage and memory kernels; no document or drawing dependencies.
use crate::{Bit, Value};
use logisim_core::{MemoryContents, MemoryLogic, MemoryRole};

#[derive(Clone, Debug)]
pub(super) struct State {
    contents: MemoryContents,
    clock: Bit,
    address_bits: u8,
    data_bits: u8,
}
impl State {
    pub fn new(c: &MemoryLogic) -> Self {
        Self {
            contents: MemoryContents::default(),
            clock: Bit::Unknown,
            address_bits: c.address_bits,
            data_bits: c.bits,
        }
    }
    pub fn adapt(mut self, c: &MemoryLogic) -> Self {
        if self.address_bits != c.address_bits || self.data_bits != c.bits {
            self.contents.resize(c.address_bits, c.bits);
            self.address_bits = c.address_bits;
            self.data_bits = c.bits;
        }
        self
    }
    pub fn contents(&self) -> &MemoryContents {
        &self.contents
    }
    pub fn fill(&mut self, start: u32, end: u32, value: u32) -> bool {
        if start >= end || end > (1 << self.address_bits.clamp(2, 24)) {
            return false;
        }
        let mask = u32::MAX >> (32 - self.data_bits.clamp(1, 32));
        self.contents.fill(start, end, value & mask);
        true
    }
    pub fn clear(&mut self) {
        self.contents.clear();
    }
    pub fn replace_contents(&mut self, mut contents: MemoryContents) {
        contents.resize(self.address_bits, self.data_bits);
        self.contents = contents;
    }
    pub fn evaluate(&mut self, c: &MemoryLogic, ports: &[Value]) -> Value {
        self.sample(c, ports);
        self.read(c, ports)
    }
    /// Commit a state sample without exposing new outputs to other components.
    /// Clock history is retained even when no word changes.
    pub fn sample(&mut self, c: &MemoryLogic, ports: &[Value]) -> bool {
        use MemoryRole::*;
        let get = |role| c.port_index(role).map(|i| ports[i]);
        let bit = |role, default| get(role).map_or(default, |v| v.bit(0));
        let clock = bit(Clock, Bit::Unknown);
        let edge = self.clock == Bit::Zero && clock == Bit::One;
        self.clock = clock;
        let separate = c.separate();
        let writing = if separate {
            bit(Store, Bit::One) != Bit::Zero
        } else {
            !reading(c, ports)
        };
        let address = ports[1].number().filter(|a| *a < c.capacity());
        if bit(Clear, Bit::Zero) == Bit::One {
            let changed = self.contents.boundaries().next().is_some();
            self.contents.clear();
            return changed;
        } else if selected(c, ports)
            && writing
            && (edge || c.asynchronous())
            && let Some(address) = address
        {
            let data = get(if separate { Input } else { Data }).expect("memory data port");
            let mask = u32::MAX >> (32 - c.bits.clamp(1, 32));
            let value = data.number().unwrap_or(u32::MAX) & mask;
            if self.contents.get(address) != value {
                self.contents.set(address, value);
                return true;
            }
        }
        false
    }
    /// Pure asynchronous read of the committed image; never consumes an edge.
    pub fn read(&self, c: &MemoryLogic, ports: &[Value]) -> Value {
        if selected(c, ports)
            && reading(c, ports)
            && let Some(address) = ports[1].number().filter(|a| *a < c.capacity())
        {
            Value::known(c.bits, self.contents.get(address))
        } else {
            Value::filled(c.bits, Bit::Unknown)
        }
    }
}
fn selected(c: &MemoryLogic, ports: &[Value]) -> bool {
    c.port_index(MemoryRole::Select)
        .map_or(Bit::Unknown, |i| ports[i].bit(0))
        != if c.select_high { Bit::Zero } else { Bit::One }
}
fn reading(c: &MemoryLogic, ports: &[Value]) -> bool {
    c.separate()
        || c.port_index(MemoryRole::Load)
            .map_or(Bit::Unknown, |i| ports[i].bit(0))
            != Bit::Zero
}
pub(super) fn rom(c: &MemoryLogic, contents: &MemoryContents, ports: &[Value]) -> Value {
    let disabled = ports[2].bit(0) == if c.select_high { Bit::Zero } else { Bit::One };
    if !disabled && let Some(address) = ports[1].number().filter(|a| *a < c.capacity()) {
        Value::known(c.bits, contents.get(address))
    } else {
        Value::filled(c.bits, Bit::Unknown)
    }
}

#[cfg(test)]
mod tests;

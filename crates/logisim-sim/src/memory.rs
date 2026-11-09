use crate::{Bit, Value};
use logisim_core::{Component, MemoryContents, MemoryInterface, MemoryRole};

#[derive(Clone, Debug)]
pub(super) struct State {
    pub contents: MemoryContents,
    clock: Bit,
    address_bits: u8,
    data_bits: u8,
}
impl State {
    pub fn new(c: &Component) -> Self {
        Self {
            contents: MemoryContents::default(),
            clock: Bit::Unknown,
            address_bits: c.memory.address_bits,
            data_bits: c.bits,
        }
    }
    pub fn adapt(mut self, c: &Component) -> Self {
        if self.address_bits != c.memory.address_bits || self.data_bits != c.bits {
            self.contents.resize(c.memory.address_bits, c.bits);
            self.address_bits = c.memory.address_bits;
            self.data_bits = c.bits;
        }
        self
    }
    pub fn evaluate(&mut self, c: &Component, ports: &[Value]) -> Value {
        self.sample(c, ports);
        self.read(c, ports)
    }
    /// Commit a state sample without exposing new outputs to other components.
    /// Clock history is retained even when no word changes.
    pub fn sample(&mut self, c: &Component, ports: &[Value]) -> bool {
        use MemoryRole::*;
        let get = |role| c.memory_port(role).map(|i| ports[i]);
        let bit = |role, default| get(role).map_or(default, |v| v.bit(0));
        let clock = bit(Clock, Bit::Unknown);
        let edge = self.clock == Bit::Zero && clock == Bit::One;
        self.clock = clock;
        let separate = c.memory.interface == MemoryInterface::Separate;
        let writing = if separate {
            bit(Store, Bit::One) != Bit::Zero
        } else {
            !reading(c, ports)
        };
        let address = ports[1].number().filter(|a| *a < c.memory_capacity());
        if bit(Clear, Bit::Zero) == Bit::One {
            let changed = self.contents.boundaries().next().is_some();
            self.contents.clear();
            return changed;
        } else if selected(c, ports)
            && writing
            && (edge || c.memory.interface == MemoryInterface::SharedAsync)
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
    pub fn read(&self, c: &Component, ports: &[Value]) -> Value {
        if selected(c, ports)
            && reading(c, ports)
            && let Some(address) = ports[1].number().filter(|a| *a < c.memory_capacity())
        {
            Value::known(c.bits, self.contents.get(address))
        } else {
            Value::filled(c.bits, Bit::Unknown)
        }
    }
}
fn selected(c: &Component, ports: &[Value]) -> bool {
    c.memory_port(MemoryRole::Select)
        .map_or(Bit::Unknown, |i| ports[i].bit(0))
        != if c.memory.select_high {
            Bit::Zero
        } else {
            Bit::One
        }
}
fn reading(c: &Component, ports: &[Value]) -> bool {
    c.memory.interface == MemoryInterface::Separate
        || c.memory_port(MemoryRole::Load)
            .map_or(Bit::Unknown, |i| ports[i].bit(0))
            != Bit::Zero
}
pub(super) fn rom(c: &Component, ports: &[Value]) -> Value {
    let disabled = ports[2].bit(0)
        == if c.memory.select_high {
            Bit::Zero
        } else {
            Bit::One
        };
    if !disabled && let Some(address) = ports[1].number().filter(|a| *a < c.memory_capacity()) {
        Value::known(c.bits, c.memory.contents.get(address))
    } else {
        Value::filled(c.bits, Bit::Unknown)
    }
}

#[cfg(test)]
mod tests;

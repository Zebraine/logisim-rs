//! Instance-local signal cache. Topology edits and reset construct a fresh cache.
use super::{Network, Port, merge};
use crate::{Bit, Value, work::WorkSet};
use std::collections::BTreeMap;
#[cfg(test)]
#[path = "resolution_tests.rs"]
mod tests;

#[derive(Default)]
pub(crate) struct Resolution {
    drivers: Vec<Option<Value>>,
    bits: Vec<Bit>,
    dirty_bits: WorkSet,
    dirty_nets: WorkSet,
}
impl Resolution {
    pub fn new(network: &Network, drives: &BTreeMap<Port, Value>) -> Self {
        let drivers: Vec<_> = network
            .drivers
            .iter()
            .map(|p| drives.get(p).copied())
            .collect();
        let bits = (0..network.bit_drivers.len())
            .map(|bit| resolve_bit(network, &drivers, bit))
            .collect();
        Self {
            drivers,
            bits,
            dirty_bits: WorkSet::new(network.bit_drivers.len()),
            dirty_nets: WorkSet::new(network.nets.len()),
        }
    }
    pub fn values(&self, network: &Network) -> Vec<Value> {
        (0..network.nets.len())
            .map(|n| self.net_value(network, n))
            .collect()
    }
    fn net_value(&self, network: &Network, net: usize) -> Value {
        let net = &network.nets[net];
        if net.mismatch {
            Value::filled(net.width, Bit::Unknown)
        } else {
            Value::from_bits(net.bits.iter().map(|bit| self.bits[*bit]))
        }
    }
    /// Apply the entire driver batch before resolving any bit, including shared buses.
    /// `None` removes a driver; an all-X value releases it while retaining its width.
    pub fn update(
        &mut self,
        network: &Network,
        updates: impl IntoIterator<Item = (Port, Option<Value>)>,
        values: &mut [Value],
        changed: &mut WorkSet,
    ) {
        changed.clear();
        self.dirty_bits.clear();
        self.dirty_nets.clear();
        for (port, value) in updates {
            let Some(&driver) = network.driver_indices.get(&port) else {
                continue;
            };
            if self.drivers[driver] == value {
                continue;
            }
            self.drivers[driver] = value;
            for bit in &network.driver_bits[driver] {
                self.dirty_bits.insert(*bit);
            }
        }
        for &bit in &self.dirty_bits.items {
            let value = resolve_bit(network, &self.drivers, bit);
            if self.bits[bit] != value {
                self.bits[bit] = value;
                for net in &network.bit_nets[bit] {
                    self.dirty_nets.insert(*net);
                }
            }
        }
        for &net in &self.dirty_nets.items {
            let value = self.net_value(network, net);
            if values[net] != value {
                values[net] = value;
                changed.insert(net);
            }
        }
    }
}
fn resolve_bit(network: &Network, values: &[Option<Value>], bit: usize) -> Bit {
    let driven = network.bit_drivers[bit]
        .iter()
        .filter_map(|(driver, bit)| values[*driver].map(|v| v.bit(*bit)))
        .fold(Bit::Unknown, merge);
    if driven == Bit::Unknown {
        network.bit_pulls[bit]
    } else {
        driven
    }
}

impl Network {
    pub(super) fn index_dependencies(&mut self) {
        self.driver_indices = self
            .drivers
            .iter()
            .copied()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect();
        self.driver_bits = vec![vec![]; self.drivers.len()];
        self.bit_nets = vec![vec![]; self.bit_drivers.len()];
        for (bit, drivers) in self.bit_drivers.iter().enumerate() {
            for (driver, _) in drivers {
                self.driver_bits[*driver].push(bit);
            }
        }
        for bits in &mut self.driver_bits {
            bits.dedup();
        }
        for (index, net) in self.nets.iter().enumerate().filter(|(_, n)| !n.mismatch) {
            for bit in &net.bits {
                self.bit_nets[*bit].push(index);
            }
        }
        for nets in &mut self.bit_nets {
            nets.dedup();
        }
    }
}

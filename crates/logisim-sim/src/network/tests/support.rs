use super::super::*;

// Full-resolution oracle retained for differential tests.
impl Network {
    pub fn resolve(&self, drives: &BTreeMap<Port, crate::Value>) -> Vec<crate::Value> {
        use crate::{Bit, Value};
        // Resolve each bus driver once. Splitter aliases and individual bits share
        // its compiled index, avoiding a map lookup for every driven bit.
        let values: Vec<_> = self.drivers.iter().map(|id| drives.get(id)).collect();
        let bits: Vec<_> = self
            .bit_drivers
            .iter()
            .zip(&self.bit_pulls)
            .map(|(drivers, pull)| {
                let driven = drivers
                    .iter()
                    .filter_map(|(index, bit)| values[*index].map(|v| v.bit(*bit)))
                    .fold(Bit::Unknown, merge);
                if driven == Bit::Unknown {
                    *pull
                } else {
                    driven
                }
            })
            .collect();
        self.nets
            .iter()
            .map(|net| {
                if net.mismatch {
                    Value::filled(net.width, Bit::Unknown)
                } else {
                    Value::from_bits(net.bits.iter().map(|i| bits[*i]))
                }
            })
            .collect()
    }
}

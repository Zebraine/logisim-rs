//! Read-only dependencies from the same canonical bits used by signal resolution.
use super::{Network, Port};
use crate::Bit;

impl Network {
    /// Destination bit, source driver port, and source bit, including splitter aliases.
    /// Multiple sources must remain distinct even when they belong to one component.
    pub(crate) fn port_sources(&self, port: Port) -> impl Iterator<Item = (u8, Port, u8)> + '_ {
        self.nets[self.ports[&port]]
            .bits
            .iter()
            .enumerate()
            .flat_map(move |(bit, group)| {
                self.bit_drivers[*group]
                    .iter()
                    .map(move |(driver, source)| (bit as u8, self.drivers[*driver], *source))
            })
    }

    pub(crate) fn port_has_pull(&self, port: Port) -> bool {
        let net = &self.nets[self.ports[&port]];
        !net.pulls.is_empty()
            || net
                .bits
                .iter()
                .any(|bit| self.bit_pulls[*bit] != Bit::Unknown)
    }
}

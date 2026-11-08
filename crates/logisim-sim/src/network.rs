use logisim_core::{Circuit, Id, Kind, WireIndex};
use std::collections::{BTreeMap, BTreeSet};
mod dependencies;
mod resolution;
pub(crate) use resolution::Resolution;

#[cfg(test)]
mod topology_tests;

pub(crate) type Port = (Id, usize);

#[derive(Default)]
pub(crate) struct Network {
    pub nets: Vec<Net>,
    pub ports: BTreeMap<Port, usize>,
    pub wires: BTreeMap<Id, usize>,
    drivers: Vec<Port>,
    bit_drivers: Vec<Vec<(usize, u8)>>,
    bit_pulls: Vec<crate::Bit>,
    driver_indices: BTreeMap<Port, usize>,
    driver_bits: Vec<Vec<usize>>,
    bit_nets: Vec<Vec<usize>>,
}
#[derive(Default)]
pub(crate) struct Net {
    pub ports: Vec<Port>,
    pub wires: Vec<Id>,
    pub width: u8,
    pub mismatch: bool,
    drivers: Vec<usize>,
    bits: Vec<usize>,
    pulls: Vec<crate::Bit>,
}
impl Network {
    pub fn compile(circuit: &Circuit) -> Self {
        let ports: Vec<_> = circuit
            .components
            .iter()
            .flat_map(|c| {
                c.ports()
                    .into_iter()
                    .enumerate()
                    .map(move |(i, p)| (c, i, p))
            })
            .collect();
        let offset = circuit.wires.len();
        let mut groups = Groups((0..offset + ports.len()).collect());
        let index = WireIndex::new(&circuit.wires);
        let mut positions = BTreeMap::new();
        // Query each connection coordinate once. Repeated endpoints and co-located
        // ports reuse its representative, without materializing pairwise wire edges.
        let mut attach = |point, node, groups: &mut Groups| {
            if let Some(previous) = positions.insert(point, node) {
                groups.join(node, previous);
            } else {
                index.visit(point, |wire, _| groups.join(node, wire));
            }
        };
        for (i, wire) in circuit.wires.iter().enumerate() {
            for point in [wire.a, wire.b] {
                attach(point, i, &mut groups);
            }
        }
        let mut tunnels = BTreeMap::new();
        for (i, (c, _, point)) in ports.iter().enumerate() {
            let node = offset + i;
            attach(*point, node, &mut groups);
            if c.kind == Kind::Tunnel
                && !c.label.is_empty()
                && let Some(previous) = tunnels.insert(&c.label, node)
            {
                groups.join(node, previous);
            }
        }
        let mut network = Self::default();
        let mut roots = BTreeMap::new();
        let mut widths: Vec<BTreeSet<u8>> = vec![];
        for i in 0..offset + ports.len() {
            let root = groups.root(i);
            let net = *roots.entry(root).or_insert_with(|| {
                widths.push(BTreeSet::new());
                network.nets.push(Net::default());
                network.nets.len() - 1
            });
            if i < offset {
                let id = circuit.wires[i].id;
                network.wires.insert(id, net);
                network.nets[net].wires.push(id);
            } else {
                let (c, index, _) = ports[i - offset];
                network.ports.insert((c.id, index), net);
                network.nets[net].ports.push((c.id, index));
                if let Some(width) = c.port_width(index).filter(|w| *w > 0) {
                    widths[net].insert(width);
                }
                if c.kind == Kind::PullResistor {
                    network.nets[net]
                        .pulls
                        .push(crate::bus::pull_bit(c.bus.pull));
                }
                if c.drives_port(index) {
                    network.nets[net].drivers.push(network.drivers.len());
                    network.drivers.push((c.id, index));
                }
            }
        }
        for (net, widths) in network.nets.iter_mut().zip(widths) {
            net.width = widths.last().copied().unwrap_or(0);
            net.mismatch = widths.len() > 1;
        }
        network.connect_bits(circuit);
        network.index_dependencies();
        network
    }
    fn connect_bits(&mut self, circuit: &Circuit) {
        let mut count = 0;
        for net in &mut self.nets {
            net.bits = (count..count + usize::from(net.width)).collect();
            count += usize::from(net.width);
        }
        let mut groups = Groups((0..count).collect());
        for c in circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Splitter)
        {
            let common = &self.nets[self.ports[&(c.id, 0)]];
            if common.mismatch {
                continue;
            }
            for branch in 0..c.splitter.fanout.clamp(1, 32) {
                let end = &self.nets[self.ports[&(c.id, usize::from(branch) + 1)]];
                if end.mismatch {
                    continue;
                }
                for (bit, combined) in c
                    .splitter
                    .branch_bits(c.bits, branch)
                    .into_iter()
                    .enumerate()
                {
                    groups.join(common.bits[usize::from(combined)], end.bits[bit]);
                }
            }
        }
        self.bit_drivers = vec![vec![]; count];
        self.bit_pulls = vec![crate::Bit::Unknown; count];
        for net in &mut self.nets {
            for (bit, node) in net.bits.iter_mut().enumerate() {
                *node = groups.root(*node);
                if !net.mismatch {
                    self.bit_drivers[*node].extend(net.drivers.iter().map(|id| (*id, bit as u8)));
                    for pull in &net.pulls {
                        self.bit_pulls[*node] = merge(self.bit_pulls[*node], *pull);
                    }
                }
            }
        }
    }
    // Full-resolution oracle retained for differential tests.
    #[cfg(test)]
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
    pub fn connected(&self, port: Port) -> bool {
        self.ports.get(&port).is_some_and(|i| {
            let net = &self.nets[*i];
            net.ports.len() > 1 || !net.wires.is_empty()
        })
    }
}
struct Groups(Vec<usize>);
impl Groups {
    fn root(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
    fn join(&mut self, a: usize, b: usize) {
        let a = self.root(a);
        let b = self.root(b);
        self.0[a] = b;
    }
}

pub(crate) fn merge(a: crate::Bit, b: crate::Bit) -> crate::Bit {
    use crate::Bit;
    match (a, b) {
        (Bit::Unknown, b) | (b, Bit::Unknown) => b,
        (a, b) if a == b => a,
        _ => Bit::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bit, Value};
    use logisim_core::{Component, Point, Pull};

    #[test]
    fn bus_drivers_resolve_each_bit_and_release_without_stale_values() {
        use Bit::{Error as E, One as H, Unknown as X, Zero as L};
        let states = [L, H, X, E];
        // Two strong drivers with a weak pull-up, including conflicts and release.
        let expected = [[L, E, L, E], [E, H, H, E], [L, H, H, E], [E, E, E, E]];
        for width in [1, 8, 32] {
            let mut circuit = Circuit::default();
            let mut pin = Component::new(Kind::Pin, Point::ZERO);
            pin.bits = width;
            let a = (circuit.add_component(pin.clone()), 0);
            let b = (circuit.add_component(pin), 0);
            let mut pull = Component::new(Kind::PullResistor, Point::ZERO);
            pull.bus.pull = Pull::One;
            circuit.add_component(pull);
            let network = Network::compile(&circuit);
            let net = network.ports[&a];
            let mut drives = BTreeMap::new();
            for i in 0..4 {
                for j in 0..4 {
                    // Mixed states within a bus detect incorrect reuse of a bit.
                    let left =
                        Value::from_bits((0..width).map(|bit| states[(i + usize::from(bit)) % 4]));
                    let right =
                        Value::from_bits((0..width).map(|bit| states[(j + usize::from(bit)) % 4]));
                    drives.insert(a, left);
                    drives.insert(b, right);
                    let want = Value::from_bits((0..width).map(|bit| {
                        expected[(i + usize::from(bit)) % 4][(j + usize::from(bit)) % 4]
                    }));
                    assert_eq!(network.resolve(&drives)[net], want);
                    drives.remove(&a);
                    let want = Value::from_bits(
                        (0..width).map(|bit| expected[2][(j + usize::from(bit)) % 4]),
                    );
                    assert_eq!(network.resolve(&drives)[net], want);
                    drives.clear();
                    assert_eq!(network.resolve(&drives)[net], Value::filled(width, H));
                }
            }
        }
    }
}

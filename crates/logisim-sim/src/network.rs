use logisim_core::{Circuit, Id, Kind, WireIndex};
use std::collections::{BTreeMap, BTreeSet};
mod dependencies;
mod resolution;
pub(crate) use resolution::Resolution;

#[cfg(test)]
mod tests;

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

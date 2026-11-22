use logisim_core::{Circuit, ConnectionGroups, Id, Kind};
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
    pub driver_indices: BTreeMap<Port, usize>,
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
        let mut groups = ConnectionGroups::from_geometry(
            &circuit.wires,
            ports.iter().map(|(c, _, point)| {
                (
                    *point,
                    (c.kind == Kind::Tunnel && !c.label.is_empty()).then_some(c.label.as_str()),
                )
            }),
        );
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
        let mut groups = ConnectionGroups::new(count);
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
pub(crate) fn merge(a: crate::Bit, b: crate::Bit) -> crate::Bit {
    use crate::Bit;
    match (a, b) {
        (Bit::Unknown, b) | (b, Bit::Unknown) => b,
        (a, b) if a == b => a,
        _ => Bit::Error,
    }
}

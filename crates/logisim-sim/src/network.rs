use logisim_core::{Circuit, Id, Kind};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Port = (Id, usize);

#[derive(Default)]
pub(crate) struct Network {
    pub nets: Vec<Net>,
    pub ports: BTreeMap<Port, usize>,
    pub wires: BTreeMap<Id, usize>,
}
#[derive(Default)]
pub(crate) struct Net {
    pub ports: Vec<Port>,
    pub wires: Vec<Id>,
    pub width: u8,
    pub mismatch: bool,
    pub drivers: Vec<Id>,
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
        for (a, wire) in circuit.wires.iter().enumerate() {
            for (b, other) in circuit.wires[..a].iter().enumerate() {
                if wire.touches(*other) {
                    groups.join(a, b);
                }
            }
        }
        let mut positions = BTreeMap::new();
        let mut tunnels = BTreeMap::new();
        for (i, (c, _, point)) in ports.iter().enumerate() {
            let node = offset + i;
            for (j, wire) in circuit.wires.iter().enumerate() {
                if wire.contains(*point) {
                    groups.join(node, j);
                }
            }
            if let Some(previous) = positions.insert(*point, node) {
                groups.join(node, previous);
            }
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
                widths[net].insert(c.bits.clamp(1, 32));
                if index == 0
                    && (matches!(c.kind, Kind::Gate(_)) || (c.kind == Kind::Pin && !c.output))
                {
                    network.nets[net].drivers.push(c.id);
                }
            }
        }
        for (net, widths) in network.nets.iter_mut().zip(widths) {
            net.width = widths.last().copied().unwrap_or(1);
            net.mismatch = widths.len() > 1;
        }
        network
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

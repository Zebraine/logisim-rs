//! Document-derived scheduling metadata. Values and instance state are never cached here.
#[cfg(test)]
mod tests;
use crate::{Bit, Value, network::Network, work::WorkSet};
use logisim_core::{Circuit, Component, Kind};

#[derive(Default)]
pub(crate) struct Plan {
    pub single: Vec<Entry>,
    pub multiple: Vec<Entry>,
    pub controls: Vec<Entry>,
    pub storage: Vec<Entry>,
    pub memories: Vec<Entry>,
    pub plugins: Vec<Entry>,
    pub matrices: Vec<Entry>,
    pub terminals: Vec<Entry>,
    pub clocks: Vec<usize>,
    pub has_clocks: bool,
}

pub(crate) struct Entry {
    pub component: usize,
    ports: Vec<Reader>,
    per_batch: bool,
}
struct Reader {
    net: usize,
    mismatch_width: Option<u8>,
    connected: bool,
}
impl Entry {
    fn new(component: usize, c: &Component, network: &Network) -> Self {
        Self {
            component,
            // Level-triggered state can change on repeated evaluations without a
            // port change (e.g. T/JK feedback masked by another bus driver).
            per_batch: c.kind.is_sequential()
                && matches!(
                    c.sequential.trigger,
                    logisim_core::Trigger::High | logisim_core::Trigger::Low
                ),
            ports: c
                .ports()
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    let net = network.ports[&(c.id, index)];
                    Reader {
                        net,
                        mismatch_width: network.nets[net]
                            .mismatch
                            .then(|| c.port_width(index).unwrap_or(network.nets[net].width)),
                        connected: network.connected((c.id, index)),
                    }
                })
                .collect(),
        }
    }
    pub fn affected(&self, changed: Option<&WorkSet>) -> bool {
        self.per_batch
            || changed.is_none_or(|set| {
                // Include output/bidirectional readers: held decoders, RAM and plugin
                // definitions may sample those ports as well as ordinary inputs.
                self.ports
                    .iter()
                    .any(|p| p.mismatch_width.is_none() && set.contains(p.net))
            })
    }
    pub fn port(&self, index: usize, values: &[Value]) -> Value {
        let reader = &self.ports[index];
        reader.mismatch_width.map_or_else(
            || values[reader.net],
            |width| Value::filled(width, Bit::Unknown),
        )
    }
    pub fn optional_port(&self, index: usize, values: &[Value]) -> Option<Value> {
        self.ports.get(index).map(|_| self.port(index, values))
    }
    pub fn connected(&self, index: usize) -> bool {
        self.ports[index].connected
    }
    pub fn sample(&self, values: &[Value]) -> Vec<Value> {
        self.values(values).collect()
    }
    pub fn values<'a>(&'a self, values: &'a [Value]) -> impl Iterator<Item = Value> + 'a {
        (0..self.ports.len()).map(move |index| self.port(index, values))
    }
}
impl Plan {
    pub fn compile(circuit: &Circuit, network: &Network) -> Self {
        let mut plan = Self::default();
        for (index, c) in circuit.components.iter().enumerate() {
            if c.kind == Kind::Clock {
                plan.clocks.push(index);
            }
            plan.has_clocks |=
                c.kind == Kind::Clock || (c.kind == Kind::DotMatrix && c.io.matrix_persistence > 0);
            let group = match c.kind {
                Kind::Gate(_)
                | Kind::Parity(_)
                | Kind::ControlledBuffer
                | Kind::ControlledInverter
                | Kind::Transistor
                | Kind::TransmissionGate
                | Kind::PlaRom
                | Kind::Constant
                | Kind::Power
                | Kind::Ground
                | Kind::BitExtender => &mut plan.single,
                Kind::Plexer(_) | Kind::Arithmetic(_) => &mut plan.multiple,
                Kind::Switch | Kind::DipSwitch | Kind::Joystick | Kind::Slider => {
                    &mut plan.controls
                }
                Kind::Plugin => &mut plan.plugins,
                Kind::DotMatrix => &mut plan.matrices,
                kind if kind.is_sequential() => &mut plan.storage,
                kind if kind.is_memory() => &mut plan.memories,
                kind if kind.is_terminal() => &mut plan.terminals,
                _ => continue,
            };
            group.push(Entry::new(index, c, network));
        }
        plan
    }
}

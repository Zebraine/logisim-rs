//! Pure RAM reads and simultaneous write samples in the opt-in stable scheduler.
use super::*;
use logisim_core::MemoryRole;

pub(super) struct Plan {
    read_ports: Vec<usize>,
    inputs: Option<Vec<Value>>,
}
impl Plan {
    pub fn compile(
        sim: &Simulator,
        dependencies: &mut BTreeSet<(Port, Port)>,
    ) -> BTreeMap<Id, Self> {
        sim.circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Ram)
            .map(|c| {
                // The shared data port is a write input, never a read dependency.
                // Excluding it allows ordinary bidirectional buses and stored feedback.
                let read_ports: Vec<_> =
                    [MemoryRole::Address, MemoryRole::Select, MemoryRole::Load]
                        .into_iter()
                        .filter_map(|role| c.memory_port(role))
                        .collect();
                for &port in &read_ports {
                    for (_, source, _) in sim.network.port_sources((c.id, port)) {
                        dependencies.insert((source, (c.id, 0)));
                    }
                }
                (
                    c.id,
                    Self {
                        read_ports,
                        inputs: None,
                    },
                )
            })
            .collect()
    }
    pub fn invalidate(&mut self) {
        self.inputs = None;
    }
}
impl Simulator {
    /// Queue pure read outputs. All memories in this pass see committed networks.
    pub(crate) fn stable_memory_outputs(&mut self) -> usize {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        let mut drives = vec![];
        for entry in &self.plan.memories {
            let c = &self.circuit.components[entry.component];
            let Some(read) = plan.memories.get_mut(&c.id) else {
                continue;
            };
            if read.inputs.as_ref().is_some_and(|inputs| {
                inputs
                    .iter()
                    .zip(&read.read_ports)
                    .all(|(&value, &port)| value == entry.port(port, &self.values))
            }) {
                continue;
            }
            let ports = entry.sample(&self.values);
            let value =
                self.memories[&c.id].read(&c.memory_logic().expect("RAM configuration"), &ports);
            read.inputs = Some(read.read_ports.iter().map(|&port| ports[port]).collect());
            drives.push(((c.id, 0), value));
        }
        let evaluated = drives.len();
        for (port, value) in drives {
            self.drive_port(port, value);
        }
        evaluated
    }
    /// Sample writes only after the whole design has settled, and without committing
    /// any output. A hidden word change must still schedule another pure read pass.
    pub(crate) fn stable_sample_memories(&mut self, changes: Option<&WorkSet>) -> (usize, bool) {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        let mut sampled = 0;
        let mut changed = false;
        for entry in &self.plan.memories {
            let c = &self.circuit.components[entry.component];
            let Some(read) = plan.memories.get_mut(&c.id) else {
                continue;
            };
            if !entry.affected(changes) {
                continue;
            }
            sampled += 1;
            if self.memories.get_mut(&c.id).unwrap().sample(
                &c.memory_logic().expect("RAM configuration"),
                &entry.sample(&self.values),
            ) {
                read.invalidate();
                changed = true;
            }
        }
        (sampled, changed)
    }
}

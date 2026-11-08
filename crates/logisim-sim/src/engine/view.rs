//! Read-only signal, memory and diagnostic projections.
use crate::{Bit, Diagnostic, Problem, Simulator, Value};
use logisim_core::Id;

impl Simulator {
    /// Visit each resolved net once without exposing persistent network indices.
    pub fn signal_nets(&self) -> impl Iterator<Item = crate::SignalNet<'_>> {
        self.network
            .nets
            .iter()
            .zip(&self.values)
            .map(|(net, &value)| crate::SignalNet {
                value,
                width_mismatch: net.mismatch,
                wires: &net.wires,
                ports: &net.ports,
            })
    }

    pub fn memory_contents(&self, id: Id) -> Option<&logisim_core::MemoryContents> {
        self.memories.get(&id).map(|s| &s.contents)
    }
    pub fn stored_value(&self, id: Id) -> Option<Value> {
        self.storage
            .get(&id)
            .filter(|_| {
                self.circuit
                    .component(id)
                    .is_some_and(|c| c.kind.is_storage())
            })
            .map(|s| s.value)
    }
    pub fn wire_value(&self, id: Id) -> Option<Value> {
        self.network.wires.get(&id).map(|i| self.values[*i])
    }
    pub fn wire_mismatch(&self, id: Id) -> bool {
        self.network
            .wires
            .get(&id)
            .is_some_and(|i| self.network.nets[*i].mismatch)
    }
    pub fn port_mismatch(&self, id: Id, index: usize) -> bool {
        self.network
            .ports
            .get(&(id, index))
            .is_some_and(|i| self.network.nets[*i].mismatch)
    }
    /// Last committed component drive, before merging other drivers or weak pulls.
    /// Pending drives are not exposed; undriven/non-output ports return None.
    /// Use this snapshot for expression boundary inputs, not `port_value`.
    pub fn drive_value(&self, id: Id, index: usize) -> Option<Value> {
        self.drives.get(&(id, index)).copied()
    }
    pub fn port_value(&self, id: Id, index: usize) -> Option<Value> {
        let net = *self.network.ports.get(&(id, index))?;
        Some(if self.network.nets[net].mismatch {
            // Only mismatched nets need the individual port's declared width.
            // Normal reads use the resolved bus directly, without scanning the circuit.
            let width = self
                .circuit
                .component(id)?
                .port_width(index)
                .unwrap_or(self.network.nets[net].width);
            Value::filled(width, Bit::Unknown)
        } else {
            self.values[net]
        })
    }
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut result: Vec<_> = self
            .network
            .nets
            .iter()
            .zip(&self.values)
            .filter_map(|(net, value)| {
                let problem = if net.mismatch {
                    Problem::WidthMismatch
                } else if value.contains(Bit::Error) {
                    Problem::ErrorSignal
                } else {
                    return None;
                };
                let mut components: Vec<_> = net.ports.iter().map(|(id, _)| *id).collect();
                components.sort_unstable();
                components.dedup();
                Some(Diagnostic {
                    problem,
                    components,
                    wires: net.wires.clone(),
                })
            })
            .collect();
        if self.limited {
            result.push(Diagnostic {
                problem: Problem::PropagationLimit,
                components: self.pending.keys().map(|(id, _)| *id).collect(),
                wires: vec![],
            });
        }
        result
    }
}

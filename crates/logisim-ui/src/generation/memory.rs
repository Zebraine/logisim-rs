//! Generated expressions belong to this open project, never to document/history data.
use logisim_core::{
    Circuit, CircuitDefinition, Id, MAX_CIRCUITS, MAX_PROJECT_ITEMS, generation::Analysis,
};
use std::collections::VecDeque;

struct Entry {
    id: Id,
    circuit: Circuit,
    expressions: Analysis,
}
impl Entry {
    fn items(&self) -> usize {
        self.circuit.components.len() + self.circuit.wires.len()
    }
}

#[derive(Default)]
pub(crate) struct Memory {
    entries: VecDeque<Entry>,
}
impl Memory {
    pub fn recall(&self, definition: &CircuitDefinition) -> Option<Analysis> {
        self.entries
            .iter()
            .find(|e| e.id == definition.id && e.circuit == definition.circuit)
            .map(|e| e.expressions.clone())
    }

    pub fn remember(&mut self, definition: &CircuitDefinition, expressions: Analysis) {
        self.entries.retain(|e| e.id != definition.id);
        self.entries.push_back(Entry {
            id: definition.id,
            circuit: definition.circuit.clone(),
            expressions,
        });
        // Old versions can outlive edits/deletions so undo may make them valid
        // again. Bound their total retained geometry by the project budget.
        let mut items: usize = self.entries.iter().map(Entry::items).sum();
        while self.entries.len() > MAX_CIRCUITS || items > MAX_PROJECT_ITEMS {
            items -= self.entries.pop_front().unwrap().items();
        }
    }
}

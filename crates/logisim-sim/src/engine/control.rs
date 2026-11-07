//! Interactive mutations of instance state; no document editing or wall clock.
use crate::{Simulator, Value, pins};
use logisim_core::{Id, Kind};

impl Simulator {
    pub fn input(&self, id: Id) -> Option<Value> {
        self.inputs.get(&id).copied()
    }
    pub fn set_input(&mut self, id: Id, number: u32) {
        if let Some(value) = self.input(id) {
            self.set_input_value(id, Value::known(value.width(), number));
        }
    }
    pub(crate) fn set_input_value(&mut self, id: Id, value: Value) {
        let Some(input) = self.inputs.get_mut(&id) else {
            return;
        };
        if *input == value {
            return;
        }
        *input = value;
        self.limited = false;
        self.progress.restart();
        if self
            .circuit
            .component(id)
            .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch))
        {
            self.schedule_controls();
        } else {
            self.schedule(id, value);
        }
    }
    pub(crate) fn external_input(&mut self, id: Id, value: Value) {
        if let Some(input) = self.inputs.get_mut(&id)
            && *input != value
        {
            // A three-state pin initially receiving all X retains X, even with a pull.
            // Pulls apply when an external value changes the pin's current state.
            let value = pins::pulled(self.circuit.component(id).expect("interface pin"), value);
            *input = value;
            self.schedule(id, value);
        }
    }
    pub fn set_memory_word(&mut self, id: Id, address: u32, value: u32) {
        if let Some(end) = address.checked_add(1) {
            self.fill_memory(id, address, end, value);
        }
    }
    pub fn fill_memory(&mut self, id: Id, start: u32, end: u32, value: u32) {
        let Some(c) = self.circuit.component(id) else {
            return;
        };
        if start >= end || end > c.memory_capacity() {
            return;
        }
        let mask = u32::MAX >> (32 - c.bits.clamp(1, 32));
        let Some(state) = self.memories.get_mut(&id) else {
            return;
        };
        state.contents.fill(start, end, value & mask);
        self.limited = false;
        self.progress.restart();
        self.schedule_components(false);
    }
    pub fn clear_memory(&mut self, id: Id) {
        let Some(state) = self.memories.get_mut(&id) else {
            return;
        };
        state.contents.clear();
        self.limited = false;
        self.progress.restart();
        self.schedule_components(false);
    }
    /// Replace a RAM image atomically, then schedule one propagation pass.
    pub fn replace_memory(&mut self, id: Id, mut contents: logisim_core::MemoryContents) -> bool {
        let Some(c) = self.circuit.component(id) else {
            return false;
        };
        let Some(state) = self.memories.get_mut(&id) else {
            return false;
        };
        contents.resize(c.memory.address_bits, c.bits);
        state.contents = contents;
        self.limited = false;
        self.progress.restart();
        self.schedule_components(false);
        true
    }
    /// Tick controls are useful for clock sources and for display persistence.
    pub fn has_clocks(&self) -> bool {
        self.circuit.components.iter().any(|c| {
            c.kind == Kind::Clock || (c.kind == Kind::DotMatrix && c.io.matrix_persistence > 0)
        })
    }
    /// Queue a tick only on a settled network, so separate edges cannot collapse into one batch.
    pub fn tick(&mut self) -> bool {
        if !self.pending.is_empty() || self.limited {
            return false;
        }
        self.tick_clocks();
        self.has_clocks()
    }
    pub(crate) fn tick_clocks(&mut self) {
        self.display_ticks = self.display_ticks.saturating_add(1);
        self.update_displays();
        let clocks: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Clock)
            .cloned()
            .collect();
        for c in clocks {
            let state = self.storage.get_mut(&c.id).unwrap();
            state.tick(&c);
            let value = state.value;
            self.drive_port((c.id, 0), value);
        }
    }
    pub fn poke_clock(&mut self, id: Id) {
        if self
            .circuit
            .component(id)
            .is_some_and(|c| c.kind == Kind::Clock)
        {
            let state = self.storage.get_mut(&id).unwrap();
            state.toggle();
            let value = state.value;
            self.drive_port((id, 0), value);
            self.limited = false;
            self.progress.restart();
        }
    }
    pub fn toggle_stored(&mut self, id: Id) {
        if self
            .circuit
            .component(id)
            .is_some_and(|c| matches!(c.kind, Kind::FlipFlop(_) | Kind::Latch(_)))
        {
            self.storage.get_mut(&id).unwrap().toggle();
            // Apply asserted asynchronous controls and output scheduling through the same path.
            self.schedule_storage();
            self.limited = false;
            self.progress.restart();
        }
    }
}

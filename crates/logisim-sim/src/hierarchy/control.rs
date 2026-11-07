//! Instance-path command forwarding and parent-driven input protection.
use super::*;

impl ProjectSimulator {
    pub fn set_button(&mut self, path: &[Id], id: Id, pressed: bool) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_button(id, pressed);
        }
    }
    pub fn type_character(&mut self, path: &[Id], id: Id, value: u16) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.type_character(id, value);
        }
    }
    pub fn set_joystick(&mut self, path: &[Id], id: Id, offset: logisim_core::Point) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_joystick(id, offset);
        }
    }
    pub fn release_joysticks(&mut self) {
        for state in self.states.values_mut() {
            state.sim.release_joysticks();
        }
        for root in self.cached_roots.values_mut() {
            for state in root.states.values_mut() {
                state.sim.release_joysticks();
            }
        }
    }
    pub fn release_buttons(&mut self) {
        for state in self.states.values_mut() {
            state.sim.release_buttons();
        }
        for root in self.cached_roots.values_mut() {
            for state in root.states.values_mut() {
                state.sim.release_buttons();
            }
        }
    }
    pub fn set_memory_word(&mut self, path: &[Id], id: Id, address: u32, value: u32) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_memory_word(id, address, value);
        }
    }
    pub fn set_register_file_word(&mut self, path: &[Id], id: Id, address: u8, value: u32) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_register_file_word(id, address, value);
        }
    }
    pub fn fill_memory(&mut self, path: &[Id], id: Id, start: u32, end: u32, value: u32) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.fill_memory(id, start, end, value);
        }
    }
    pub fn clear_memory(&mut self, path: &[Id], id: Id) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.clear_memory(id);
        }
    }
    pub fn replace_memory(
        &mut self,
        path: &[Id],
        id: Id,
        contents: logisim_core::MemoryContents,
    ) -> bool {
        let replaced = self
            .states
            .get_mut(path)
            .is_some_and(|state| state.sim.replace_memory(id, contents));
        if replaced {
            self.progress.restart();
        }
        replaced
    }
    pub fn poke_clock(&mut self, path: &[Id], id: Id) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.poke_clock(id);
            self.progress.restart();
        }
    }
    pub fn toggle_stored(&mut self, path: &[Id], id: Id) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.toggle_stored(id);
            self.progress.restart();
        }
    }
    // Nested interface pins are parent-driven; local switches remain interactive.
    pub fn set_input(&mut self, id: Id, value: u32) {
        self.set_input_at(&[], id, value);
    }
    pub fn toggle_input(&mut self, id: Id, bit: u8) {
        self.toggle_input_at(&[], id, bit);
    }
    pub fn set_input_unknown_at(&mut self, path: &[Id], id: Id) {
        if path.is_empty()
            && let Some(state) = self.states.get_mut(path)
        {
            state.sim.set_input_unknown(id);
            self.progress.restart();
        }
    }
    pub fn set_input_at(&mut self, path: &[Id], id: Id, value: u32) {
        if let Some(state) = self.states.get_mut(path)
            && (path.is_empty()
                || state
                    .sim
                    .circuit
                    .component(id)
                    .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch)))
        {
            state.sim.set_input(id, value);
            self.progress.restart();
        }
    }
    pub fn toggle_input_at(&mut self, path: &[Id], id: Id, bit: u8) {
        if let Some(state) = self.states.get_mut(path)
            && (path.is_empty()
                || state
                    .sim
                    .circuit
                    .component(id)
                    .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch)))
        {
            state.sim.toggle_input(id, bit);
            self.progress.restart();
        }
    }
    pub fn input(&self, id: Id) -> Option<Value> {
        self.view(&[])?.input(id)
    }
}

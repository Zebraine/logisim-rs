//! File operations share this boundary with the asynchronous desktop service.
use logisim_core::{Id, MemoryContents};
use logisim_plugin::Registry;

pub(crate) trait Host {
    fn new_document(&mut self);
    fn set_plugins(&mut self, plugins: Registry);
    fn replace_memory(&mut self, path: &[Id], id: Id, contents: MemoryContents) -> bool;
}

#[cfg(test)]
impl Host for logisim_sim::ProjectSimulator {
    fn new_document(&mut self) {
        *self = Self::default();
    }
    fn set_plugins(&mut self, plugins: Registry) {
        self.set_plugins(plugins);
    }
    fn replace_memory(&mut self, path: &[Id], id: Id, contents: MemoryContents) -> bool {
        self.replace_memory(path, id, contents)
    }
}

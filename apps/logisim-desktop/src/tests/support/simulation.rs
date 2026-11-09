use crate::simulation::Host;
use logisim_core::{Id, MemoryContents};
use logisim_plugin::Registry;

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

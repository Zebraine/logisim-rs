//! File operations share this boundary with the asynchronous desktop service.
use logisim_core::{Id, MemoryContents};
use logisim_plugin::Registry;

pub(crate) trait Host {
    fn new_document(&mut self);
    fn set_plugins(&mut self, plugins: Registry);
    fn replace_memory(&mut self, path: &[Id], id: Id, contents: MemoryContents) -> bool;
}

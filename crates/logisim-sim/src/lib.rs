//! Bounded signal propagation and sequential state, independent of the GUI.
mod engine;
mod gates;
mod hierarchy;
mod progress;
pub use hierarchy::ProjectSimulator;
mod arithmetic;
#[cfg(test)]
mod arithmetic_tests;
mod bus;
mod displays;
mod io;
mod joystick;
mod terminal;
#[cfg(test)]
mod terminal_tests;
pub use terminal::TtyScreen;
mod latches;
mod memory;
#[cfg(test)]
mod memory_tests;
mod network;
mod parity;
mod pins;
mod pla;
#[cfg(test)]
mod plexer_tests;
mod plexers;
mod plugins;
mod random;
#[cfg(test)]
mod random_tests;
mod register_file;
mod sequential;
#[cfg(test)]
mod sequential_extra_tests;
#[cfg(test)]
mod sequential_tests;
#[cfg(test)]
mod tests;
mod transistors;
mod ttl;
mod value;

pub use logisim_core::{Bit, Value};
use logisim_core::{Circuit, Id, Point};
use network::Network;
use std::collections::BTreeMap;

const PROPAGATION_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    WidthMismatch,
    ErrorSignal,
    PropagationLimit,
}
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub problem: Problem,
    pub components: Vec<Id>,
    pub wires: Vec<Id>,
}

pub struct Simulator {
    plugins: logisim_plugin::Registry,
    plugin_states: BTreeMap<Id, plugins::State>,
    circuit: Circuit,
    network: Network,
    inputs: BTreeMap<Id, Value>,
    joysticks: BTreeMap<Id, Point>,
    terminals: BTreeMap<Id, terminal::State>,
    matrices: BTreeMap<Id, displays::MatrixState>,
    display_ticks: u64,
    storage: BTreeMap<Id, sequential::State>,
    memories: BTreeMap<Id, memory::State>,
    register_files: BTreeMap<Id, register_file::State>,
    drives: BTreeMap<network::Port, Value>,
    pending: BTreeMap<network::Port, Value>,
    values: Vec<Value>,
    enabled: bool,
    limited: bool,
    progress: progress::Propagation,
}
impl Default for Simulator {
    fn default() -> Self {
        Self {
            plugins: Default::default(),
            plugin_states: BTreeMap::new(),
            circuit: Circuit::default(),
            network: Network::default(),
            inputs: BTreeMap::new(),
            joysticks: BTreeMap::new(),
            terminals: BTreeMap::new(),
            matrices: BTreeMap::new(),
            display_ticks: 0,
            storage: BTreeMap::new(),
            memories: BTreeMap::new(),
            register_files: BTreeMap::new(),
            drives: BTreeMap::new(),
            pending: BTreeMap::new(),
            values: vec![],
            enabled: true,
            limited: false,
            progress: progress::Propagation::default(),
        }
    }
}

//! Bounded signal propagation and sequential state, independent of the GUI.
pub mod combinational;
mod engine;
mod execution;
mod gates;
#[cfg(test)]
mod generation_tests;
mod hierarchy;
mod progress;
pub mod stable;
mod work;
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

/// A borrowed, instance-local resolved net. Membership is valid only for this read.
/// On width mismatch, ports must use `Simulator::port_value` for their declared width.
pub struct SignalNet<'a> {
    pub value: Value,
    pub width_mismatch: bool,
    pub wires: &'a [logisim_core::Id],
    pub ports: &'a [(logisim_core::Id, usize)],
}
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
    resolution: network::Resolution,
    changed_nets: work::WorkSet,
    initial_batch: bool,
    plan: execution::Plan,
    stable_plan: Option<Result<stable::Plan, stable::Failure>>,
    inputs: BTreeMap<Id, Value>,
    joysticks: BTreeMap<Id, Point>,
    terminals: BTreeMap<Id, terminal::State>,
    matrices: BTreeMap<Id, displays::MatrixState>,
    display_ticks: u64,
    storage: BTreeMap<Id, sequential::State>,
    memories: BTreeMap<Id, memory::State>,
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
            resolution: network::Resolution::default(),
            changed_nets: work::WorkSet::default(),
            initial_batch: false,
            plan: execution::Plan::default(),
            stable_plan: None,
            inputs: BTreeMap::new(),
            joysticks: BTreeMap::new(),
            terminals: BTreeMap::new(),
            matrices: BTreeMap::new(),
            display_ticks: 0,
            storage: BTreeMap::new(),
            memories: BTreeMap::new(),
            drives: BTreeMap::new(),
            pending: BTreeMap::new(),
            values: vec![],
            enabled: true,
            limited: false,
            progress: progress::Propagation::default(),
        }
    }
}

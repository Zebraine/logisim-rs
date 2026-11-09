//! Structural discovery for expression compilation, independent of signal values.
//!
//! Regions describe pure acyclic functions of their boundary inputs. They do not
//! certify timing-safe substitution into an event-driven parent. Discovery never
//! changes propagation, samples state, or assumes that a subcircuit is stateless.
mod expression;
pub(crate) mod graph;
pub use expression::{CompileStats, CompiledCircuit, EvaluationError, EvaluationStats, Evaluator};
#[cfg(test)]
mod tests;

use crate::{Bit, Simulator, network::Network};
use logisim_core::{Circuit, Component, Id, Kind, Plexer};
use std::collections::{BTreeMap, BTreeSet};

pub type Port = (Id, usize);

/// A bit in a component's output drive, before any network resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Source {
    pub port: Port,
    pub bit: u8,
}

/// Canonical strong driver bits and the weak value used only when all release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputBit {
    pub sources: Vec<Source>,
    /// Unknown means no effective pull; conflicting pulls resolve to Error.
    pub pull: Bit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub port: Port,
    /// Physically absent gate inputs differ from connected, undriven inputs.
    pub connected: bool,
    /// Low bit first. Empty sources with no pull are floating, not Boolean zero.
    pub bits: Vec<InputBit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub component: Id,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Port>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// Topological order; shared nodes appear once, irrespective of output count.
    pub nodes: Vec<Node>,
    /// Distinct external drive bits. Floating inputs are recorded on each node.
    pub inputs: Vec<Source>,
    /// Drive bits read outside this region, including probes and display devices.
    /// Internal drives remain available through nodes for a future debug view.
    pub outputs: Vec<Source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryReason {
    ExternalInput,
    State,
    Interaction,
    Plugin,
    Hierarchy,
    UnsupportedComponent,
    WidthMismatch,
    /// A conservative component dependency cycle, not a diagnosis of oscillation.
    Feedback,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Boundary {
    pub component: Id,
    pub reason: BoundaryReason,
    /// Some for an electrical barrier, None for a component-level boundary.
    pub port: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Analysis {
    pub regions: Vec<Region>,
    pub boundaries: Vec<Boundary>,
}

impl Simulator {
    /// Inspect this instance's current circuit without changing it or running it.
    /// Reuses its compiled topology; no analysis work is added to tick/frame paths.
    pub fn combinational_analysis(&self) -> Analysis {
        Analysis::compile(&self.circuit, &self.network)
    }
}

impl Analysis {
    /// Discover regions within one circuit definition. No file or runtime state needed.
    /// Nested definitions can be analyzed separately; instances are explicit boundaries.
    pub fn new(circuit: &Circuit) -> Self {
        Self::compile(circuit, &Network::compile(circuit))
    }

    fn compile(circuit: &Circuit, network: &Network) -> Self {
        let mut analysis = Self::default();
        let mut nodes = BTreeMap::new();
        let mut readers: BTreeMap<Source, BTreeSet<Id>> = BTreeMap::new();
        for c in &circuit.components {
            // Wires, tunnels and splitters are aliases, not expression operations.
            if matches!(c.kind, Kind::Text | Kind::Tunnel | Kind::Splitter) {
                continue;
            }
            for index in 0..c.ports().len() {
                if !c.drives_port(index) {
                    for (_, port, bit) in network.port_sources((c.id, index)) {
                        readers
                            .entry(Source { port, bit })
                            .or_default()
                            .insert(c.id);
                    }
                }
            }
            match classify(c) {
                Classification::Pure => {
                    if let Some(node) = node(c, network, &mut analysis.boundaries) {
                        nodes.insert(c.id, node);
                    }
                }
                Classification::Boundary(reason) => analysis.boundaries.push(Boundary {
                    component: c.id,
                    reason,
                    port: None,
                }),
                Classification::Passive => {}
            }
        }
        let nodes: Vec<_> = nodes.into_values().collect();
        let indices: BTreeMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.component, i))
            .collect();
        let mut edges = vec![vec![]; nodes.len()];
        for (target, node) in nodes.iter().enumerate() {
            for input in &node.inputs {
                for source in input.bits.iter().flat_map(|bit| &bit.sources) {
                    if let Some(&origin) = indices.get(&source.port.0) {
                        edges[origin].push(target);
                    }
                }
            }
        }
        for edges in &mut edges {
            edges.sort_unstable();
            edges.dedup();
        }
        let (feedback, regions) = graph::partition(&edges);
        for index in feedback {
            analysis.boundaries.push(Boundary {
                component: nodes[index].component,
                reason: BoundaryReason::Feedback,
                port: None,
            });
        }
        for region in regions {
            let members: BTreeSet<_> = region.iter().map(|i| nodes[*i].component).collect();
            let mut inputs = BTreeSet::new();
            let mut outputs = BTreeSet::new();
            for &index in &region {
                let node = &nodes[index];
                inputs.extend(
                    node.inputs
                        .iter()
                        .flat_map(|input| input.bits.iter().flat_map(|bit| &bit.sources))
                        .filter(|source| !members.contains(&source.port.0))
                        .copied(),
                );
                for &port in &node.outputs {
                    for bit in 0..network.nets[network.ports[&port]].width {
                        let source = Source { port, bit };
                        if readers
                            .get(&source)
                            .is_some_and(|ids| ids.iter().any(|id| !members.contains(id)))
                        {
                            outputs.insert(source);
                        }
                    }
                }
            }
            analysis.regions.push(Region {
                nodes: region.iter().map(|i| nodes[*i].clone()).collect(),
                inputs: inputs.into_iter().collect(),
                outputs: outputs.into_iter().collect(),
            });
        }
        analysis.boundaries.sort_unstable();
        analysis
    }
}

fn node(c: &Component, network: &Network, barriers: &mut Vec<Boundary>) -> Option<Node> {
    let before = barriers.len();
    let mut node = Node {
        component: c.id,
        inputs: vec![],
        outputs: vec![],
    };
    for index in 0..c.ports().len() {
        let port = (c.id, index);
        let net = &network.nets[network.ports[&port]];
        let mut bits = vec![BTreeSet::new(); usize::from(net.width)];
        for (bit, port, source) in network.port_sources(port) {
            bits[usize::from(bit)].insert(Source { port, bit: source });
        }
        if net.mismatch {
            barriers.push(Boundary {
                component: c.id,
                reason: BoundaryReason::WidthMismatch,
                port: Some(index),
            });
        }
        if c.drives_port(index) {
            node.outputs.push(port);
        } else {
            node.inputs.push(Input {
                port,
                connected: network.connected(port),
                bits: bits
                    .into_iter()
                    .zip(network.port_pulls(port))
                    .map(|(sources, pull)| InputBit {
                        sources: sources.into_iter().collect(),
                        pull,
                    })
                    .collect(),
            });
        }
    }
    (barriers.len() == before).then_some(node)
}

pub(crate) enum Classification {
    Pure,
    Passive,
    Boundary(BoundaryReason),
}

pub(crate) fn classify(c: &Component) -> Classification {
    use BoundaryReason as B;
    use Classification::{Boundary, Passive, Pure};
    match c.kind {
        Kind::Gate(_)
        | Kind::Parity(_)
        | Kind::Arithmetic(_)
        | Kind::Plexer(
            Plexer::Multiplexer
            | Plexer::Demultiplexer
            | Plexer::Decoder
            | Plexer::PriorityEncoder
            | Plexer::BitSelector,
        )
        | Kind::Constant
        | Kind::Power
        | Kind::Ground
        | Kind::ControlledBuffer
        | Kind::ControlledInverter
        | Kind::BitExtender
        | Kind::Rom
        | Kind::PlaRom => Pure,
        Kind::Pin if !c.output => Boundary(B::ExternalInput),
        Kind::Clock
        | Kind::FlipFlop(_)
        | Kind::Latch(_)
        | Kind::Register
        | Kind::Counter
        | Kind::ShiftRegister
        | Kind::Random
        | Kind::Ram
        | Kind::Keyboard
        | Kind::Tty
        | Kind::DotMatrix
        | Kind::Plexer(Plexer::DisplayDecoder) => Boundary(B::State),
        Kind::Button | Kind::Switch | Kind::DipSwitch | Kind::Slider | Kind::Joystick => {
            Boundary(B::Interaction)
        }
        Kind::Plugin => Boundary(B::Plugin),
        Kind::Subcircuit(_) => Boundary(B::Hierarchy),
        Kind::Ttl(_) | Kind::Transistor | Kind::TransmissionGate => {
            Boundary(B::UnsupportedComponent)
        }
        Kind::Pin
        | Kind::PullResistor
        | Kind::Tunnel
        | Kind::Text
        | Kind::Probe
        | Kind::Splitter
        | Kind::Led
        | Kind::SevenSegment
        | Kind::HexDisplay
        | Kind::RgbLed => Passive,
    }
}

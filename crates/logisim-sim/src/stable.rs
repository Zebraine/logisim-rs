//! Opt-in stable-state execution. Propagation debugging continues to use `step`.
//!
//! A stimulus is fully settled before every simultaneous state sample. Combinational
//! path lengths therefore cannot change that sample. This is a different timing
//! model from observing intermediate propagation batches, not a claim that those
//! batches are equivalent. Unsupported circuits are rejected before advancing them.
#[cfg(test)]
mod bus_tests;
mod memory;
#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod tests;

use crate::combinational::{BoundaryReason, CompileStats, Evaluator, Port};
use crate::work::WorkSet;
use crate::{Bit, PROPAGATION_LIMIT, Simulator, Value};
use logisim_core::{Id, Kind, MemoryInterface, Trigger};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    UnsupportedComponent(Id),
    LevelTriggered(Id),
    Topology {
        component: Id,
        reason: BoundaryReason,
    },
    HierarchyRequiresProject(Id),
    InvalidProject(String),
    Plugin {
        component: Id,
        message: String,
    },
    PropagationLimit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub path: Vec<Id>,
    pub reason: Failure,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stable evaluation at {:?}: {:?}", self.path, self.reason)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub instances: usize,
    pub compiled_nodes: usize,
    pub folded_nodes: usize,
    /// Whole-design combinational passes, including hierarchy bridge convergence.
    pub combinational_passes: usize,
    /// Simultaneous whole-design state samples, not individual component evaluations.
    pub state_samples: usize,
    /// Expression instructions actually evaluated, excluding folded nodes.
    pub evaluated_instructions: usize,
    /// Individual state components sampled across all stable boundaries.
    pub sampled_components: usize,
    /// Actual Wasm output calls; unchanged declared inputs and state are cached.
    pub evaluated_plugins: usize,
    /// Actual Wasm state calls, also included in sampled_components.
    pub sampled_plugins: usize,
    /// Pure RAM reads actually evaluated, excluding unchanged inputs and contents.
    pub evaluated_memories: usize,
}
impl Report {
    pub(crate) fn include(&mut self, stats: CompileStats) {
        self.instances += 1;
        self.compiled_nodes += stats.nodes;
        self.folded_nodes += stats.folded_nodes;
    }
}

pub(crate) struct Plan {
    evaluator: Evaluator,
    inputs: BTreeMap<Port, u8>,
    stats: CompileStats,
    pub dependencies: BTreeSet<(Port, Port)>,
    pub plugins: BTreeMap<Id, crate::plugins::stable::Plan>,
    memories: BTreeMap<Id, memory::Plan>,
    materialized: bool,
    state_changes: WorkSet,
    sample_all: bool,
}
impl Plan {
    fn compile(sim: &Simulator) -> Result<Self, Failure> {
        let compiled = sim.compile_combinational();
        let analysis = compiled.analysis();
        for b in &analysis.boundaries {
            if matches!(
                b.reason,
                BoundaryReason::Feedback | BoundaryReason::WidthMismatch
            ) {
                return Err(Failure::Topology {
                    component: b.component,
                    reason: b.reason,
                });
            }
        }
        let nodes: BTreeSet<_> = analysis
            .regions
            .iter()
            .flat_map(|r| &r.nodes)
            .map(|n| n.component)
            .collect();
        for c in &sim.circuit.components {
            if nodes.contains(&c.id) {
                continue;
            }
            match c.kind {
                Kind::Pin
                | Kind::Button
                | Kind::Clock
                | Kind::Plugin
                | Kind::Subcircuit(_)
                | Kind::Text
                | Kind::Tunnel
                | Kind::Splitter
                | Kind::PullResistor
                | Kind::Probe
                | Kind::Led
                | Kind::RgbLed
                | Kind::SevenSegment
                | Kind::HexDisplay => {}
                Kind::Ram if c.memory.interface != MemoryInterface::SharedAsync => {}
                Kind::FlipFlop(_)
                | Kind::Register
                | Kind::Counter
                | Kind::ShiftRegister
                | Kind::Random => {
                    if matches!(c.sequential.trigger, Trigger::High | Trigger::Low) {
                        return Err(Failure::LevelTriggered(c.id));
                    }
                }
                _ => return Err(Failure::UnsupportedComponent(c.id)),
            }
        }
        // State-only and interface-only connections may never touch an expression
        // node. Width errors still reject the whole design before any state changes.
        for (&port, &net) in &sim.network.ports {
            if sim.network.nets[net].mismatch {
                return Err(Failure::Topology {
                    component: port.0,
                    reason: BoundaryReason::WidthMismatch,
                });
            }
        }
        let components: BTreeMap<_, _> = sim.circuit.components.iter().map(|c| (c.id, c)).collect();
        let inputs = analysis
            .regions
            .iter()
            .flat_map(|r| &r.inputs)
            .map(|s| {
                (
                    s.port,
                    components[&s.port.0]
                        .port_width(s.port.1)
                        .expect("drive width"),
                )
            })
            .collect();
        let mut dependencies = analysis
            .regions
            .iter()
            .flat_map(|r| &r.nodes)
            .flat_map(|node| {
                node.inputs
                    .iter()
                    .flat_map(|input| input.bits.iter().flat_map(|bit| &bit.sources))
                    .flat_map(|source| {
                        node.outputs
                            .iter()
                            .map(move |&output| (source.port, output))
                    })
            })
            .collect();
        let plugins = crate::plugins::stable::Plan::compile(sim, &mut dependencies)?;
        let memories = memory::Plan::compile(sim, &mut dependencies);
        check_feedback(&dependencies)?;
        Ok(Self {
            evaluator: compiled.evaluator(),
            inputs,
            stats: compiled.stats(),
            dependencies,
            plugins,
            memories,
            materialized: false,
            state_changes: WorkSet::new(sim.network.nets.len()),
            sample_all: true,
        })
    }
}

/// RAM reads and plugin declarations can close cycles through expression boundaries. Check raw
/// driver ports, keeping state-only inputs out of the combinational dependency graph.
fn check_feedback(dependencies: &BTreeSet<(Port, Port)>) -> Result<(), Failure> {
    let ports: Vec<_> = dependencies
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let indices: BTreeMap<_, _> = ports.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    let mut edges = vec![vec![]; ports.len()];
    for &(from, to) in dependencies {
        edges[indices[&from]].push(indices[&to]);
    }
    let (feedback, _) = crate::combinational::graph::partition(&edges);
    if let Some(&i) = feedback.first() {
        return Err(Failure::Topology {
            component: ports[i].0,
            reason: BoundaryReason::Feedback,
        });
    }
    Ok(())
}

impl Simulator {
    pub(crate) fn invalidate_stable_values(&mut self) {
        if let Some(Ok(plan)) = &mut self.stable_plan {
            plan.materialized = false;
            plan.sample_all = true;
            for plugin in plan.plugins.values_mut() {
                plugin.invalidate();
            }
            for memory in plan.memories.values_mut() {
                memory.invalidate();
            }
        }
    }
    pub(crate) fn prepare_stable(&mut self) -> Result<CompileStats, Failure> {
        if let Some(failure) = self.stable_plugin_error() {
            return Err(failure);
        }
        if self.stable_plan.is_none() {
            self.stable_plan = Some(Plan::compile(self));
        }
        match self.stable_plan.as_ref().unwrap() {
            Ok(plan) => Ok(plan.stats),
            Err(reason) => Err(reason.clone()),
        }
    }

    /// One expression pass, without any state evaluation. All node drives are
    /// materialized so ordinary wire/port views and subsequent `step` remain valid.
    pub(crate) fn stable_combinational_pass(&mut self) -> usize {
        self.stable_commit();
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        let evaluated = plan
            .evaluator
            .evaluate_cached(|port| {
                // Uninitialized drivers are floating. Unlike the public evaluator's
                // caller, the runtime knows every port's declared width at compile time.
                Some(
                    self.drives
                        .get(&port)
                        .copied()
                        .unwrap_or_else(|| Value::filled(plan.inputs[&port], Bit::Unknown)),
                )
            })
            .expect("runtime drive widths match compiled topology");
        if !evaluated && plan.materialized {
            return 0;
        }
        let mut materialize = |(port, value)| {
            if self.drives.get(&port) == Some(&value) {
                self.pending.remove(&port);
            } else {
                self.pending.insert(port, value);
            }
        };
        if plan.materialized {
            plan.evaluator.changed_values().for_each(&mut materialize);
        } else {
            plan.evaluator.values().for_each(&mut materialize);
        }
        let evaluated = plan.evaluator.stats().unwrap().instructions;
        plan.materialized = true;
        self.stable_commit();
        self.initial_batch = false;
        evaluated
    }

    pub(crate) fn stable_commit(&mut self) {
        if self.commit_pending() {
            let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
            // A hierarchy closure can commit several batches before sampling.
            // Keep their union; the final batch alone can omit a clock or reset.
            if !self.plan.storage.is_empty() || !plan.memories.is_empty() {
                for &net in &self.changed_nets.items {
                    plan.state_changes.insert(net);
                }
            }
            // The stable coordinator owns the convergence limit, including bridges.
            self.progress.record_batch(false);
        }
    }

    pub(crate) fn stable_sample_state(&mut self) -> (usize, bool) {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        if !plan.sample_all && plan.state_changes.items.is_empty() {
            return (0, false);
        }
        let mut changes = std::mem::take(&mut plan.state_changes);
        let all = std::mem::replace(&mut plan.sample_all, false);
        let sampled = self.schedule_storage((!all).then_some(&changes));
        let (memories, changed) = self.stable_sample_memories((!all).then_some(&changes));
        changes.clear();
        self.stable_plan
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap()
            .state_changes = changes;
        (sampled + memories, changed)
    }

    pub(crate) fn stable_limit(&mut self) {
        self.limited = true;
        self.enabled = false;
    }

    /// Explicitly settle one stimulus with four-state expressions and simultaneous
    /// state sampling. Like `step`, works while paused and never advances clocks.
    /// Reuses a document-derived plan, invalidated by electrical edits. It is not
    /// selected by `advance` or the desktop. Unsupported contents leave pending
    /// drives and state untouched; callers can continue with the propagation engine.
    /// A convergence failure retains the last bounded state and sets `limited`.
    pub fn settle_compiled(&mut self) -> Result<Report, Error> {
        let run = |sim: &mut Self| -> Result<Report, Failure> {
            if let Some(c) = sim
                .circuit
                .components
                .iter()
                .find(|c| matches!(c.kind, Kind::Subcircuit(_)))
            {
                return Err(Failure::HierarchyRequiresProject(c.id));
            }
            let mut report = Report::default();
            report.include(sim.prepare_stable()?);
            if sim.limited {
                return Err(Failure::PropagationLimit);
            }
            for _ in 0..PROPAGATION_LIMIT {
                report.evaluated_instructions += sim.stable_combinational_pass();
                report.evaluated_plugins += sim.stable_plugin_outputs()?;
                report.evaluated_memories += sim.stable_memory_outputs();
                report.combinational_passes += 1;
                if sim.pending.is_empty() {
                    let sampled = sim
                        .stage_stable_plugins()
                        .inspect_err(|_| sim.discard_stable_plugin_samples())?;
                    report.sampled_plugins += sampled;
                    let (builtins, memory_changed) = sim.stable_sample_state();
                    report.sampled_components += sampled + builtins;
                    report.state_samples += 1;
                    let changed = sim.commit_stable_plugins();
                    if sim.pending.is_empty() && !changed && !memory_changed {
                        return Ok(report);
                    }
                }
            }
            sim.stable_limit();
            Err(Failure::PropagationLimit)
        };
        run(self).map_err(|reason| Error {
            path: vec![],
            reason,
        })
    }
}

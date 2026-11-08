//! Opt-in stable-state execution. Propagation debugging continues to use `step`.
//!
//! A stimulus is fully settled before every simultaneous state sample. Combinational
//! path lengths therefore cannot change that sample. This is a different timing
//! model from observing intermediate propagation batches, not a claim that those
//! batches are equivalent. Unsupported circuits are rejected before advancing them.
#[cfg(test)]
mod tests;

use crate::combinational::{BoundaryReason, CompileStats, Evaluator, Port};
use crate::{Bit, PROPAGATION_LIMIT, Simulator, Value};
use logisim_core::{Id, Kind, Trigger};
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
    materialized: bool,
}
impl Plan {
    fn compile(sim: &Simulator) -> Result<Self, Failure> {
        let compiled = sim.compile_combinational();
        let analysis = compiled.analysis();
        for b in &analysis.boundaries {
            if matches!(
                b.reason,
                BoundaryReason::Feedback
                    | BoundaryReason::WidthMismatch
                    | BoundaryReason::MultipleDrivers
                    | BoundaryReason::WeakPull
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
                | Kind::Subcircuit(_)
                | Kind::Text
                | Kind::Tunnel
                | Kind::Splitter
                | Kind::Probe
                | Kind::Led
                | Kind::RgbLed
                | Kind::SevenSegment
                | Kind::HexDisplay => {}
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
        // node. Check those too, using canonical splitter bits rather than net drivers.
        for (&port, &net) in &sim.network.ports {
            let reason = if sim.network.nets[net].mismatch {
                Some(BoundaryReason::WidthMismatch)
            } else if sim.network.port_has_pull(port) {
                Some(BoundaryReason::WeakPull)
            } else {
                let mut seen = [false; 32];
                sim.network
                    .port_sources(port)
                    .any(|(bit, _, _)| std::mem::replace(&mut seen[usize::from(bit)], true))
                    .then_some(BoundaryReason::MultipleDrivers)
            };
            if let Some(reason) = reason {
                return Err(Failure::Topology {
                    component: port.0,
                    reason,
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
        let dependencies = analysis
            .regions
            .iter()
            .flat_map(|r| &r.nodes)
            .flat_map(|node| {
                node.inputs
                    .iter()
                    .flat_map(|input| input.bits.iter().flatten())
                    .flat_map(|source| {
                        node.outputs
                            .iter()
                            .map(move |&output| (source.port, output))
                    })
            })
            .collect();
        Ok(Self {
            evaluator: compiled.evaluator(),
            inputs,
            stats: compiled.stats(),
            dependencies,
            materialized: false,
        })
    }
}

impl Simulator {
    pub(crate) fn invalidate_stable_values(&mut self) {
        if let Some(Ok(plan)) = &mut self.stable_plan {
            plan.materialized = false;
        }
    }
    pub(crate) fn prepare_stable(&mut self) -> Result<CompileStats, Failure> {
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
    pub(crate) fn stable_combinational_pass(&mut self) {
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
            return;
        }
        for (port, value) in plan.evaluator.values() {
            if self.drives.get(&port) == Some(&value) {
                self.pending.remove(&port);
            } else {
                self.pending.insert(port, value);
            }
        }
        plan.materialized = true;
        self.stable_commit();
        self.initial_batch = false;
    }

    fn stable_commit(&mut self) {
        if self.commit_pending() {
            // The stable coordinator owns the convergence limit, including bridges.
            self.progress.record_batch(false);
        }
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
                sim.stable_combinational_pass();
                report.combinational_passes += 1;
                // Pure regions are acyclic, and all other drivers are state/input
                // boundaries, so one local expression pass completes this closure.
                sim.schedule_storage(None);
                report.state_samples += 1;
                if sim.pending.is_empty() {
                    return Ok(report);
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

use super::*;
use crate::combinational::BoundaryReason;
use crate::stable::{Error, Failure, Report};

impl ProjectSimulator {
    /// Validate the complete hierarchy before any pending drive or state changes.
    /// A rejected design can safely continue with the event engine.
    pub fn prepare_compiled(&mut self) -> Result<Report, Error> {
        if let Some(error) = &self.error {
            return Err(Error {
                path: vec![],
                reason: Failure::InvalidProject(error.clone()),
            });
        }
        let mut report = Report::default();
        for (path, instance) in &mut self.states {
            report.include(instance.sim.prepare_stable().map_err(|reason| Error {
                path: path.clone(),
                reason,
            })?);
        }
        if self.stable_checked.is_none() {
            self.stable_checked = Some(self.check_stable_bridges());
        }
        self.stable_checked.as_ref().unwrap().clone()?;
        if self.limited || self.states.values().any(|s| s.sim.limited()) {
            return Err(Error {
                path: vec![],
                reason: Failure::PropagationLimit,
            });
        }
        Ok(report)
    }

    /// Settle a stimulus completely, using simultaneous whole-design state samples.
    pub fn settle_compiled(&mut self) -> Result<Report, Error> {
        let mut report = self.prepare_compiled()?;
        loop {
            self.compiled_pass(&mut report)?;
            if !self.stable_active {
                return Ok(report);
            }
        }
    }

    /// Execute one cooperative stable-state batch, without advancing clocks.
    /// `pending_count` includes unfinished state closure even with no queued drives.
    /// Repeated calls retain the convergence budget; `step` switches back to events.
    pub fn step_compiled(&mut self) -> Result<Report, Error> {
        let mut report = self.prepare_compiled()?;
        self.compiled_pass(&mut report)?;
        Ok(report)
    }

    fn compiled_pass(&mut self, report: &mut Report) -> Result<(), Error> {
        if !self.stable_active {
            self.progress.restart();
            self.stable_active = true;
        }
        for (path, instance) in &mut self.states {
            report.evaluated_instructions += instance.sim.stable_combinational_pass();
            report.evaluated_plugins +=
                instance
                    .sim
                    .stable_plugin_outputs()
                    .map_err(|reason| Error {
                        path: path.clone(),
                        reason,
                    })?;
            report.evaluated_memories += instance.sim.stable_memory_outputs();
        }
        report.combinational_passes += 1;
        self.refresh_bridges();
        if self.pending_drives() == 0 {
            // Stage every plugin first. A Wasm failure must not partially commit
            // this whole-design state sample, including built-in registers.
            let mut error = None;
            for (path, instance) in &mut self.states {
                match instance.sim.stage_stable_plugins() {
                    Ok(sampled) => {
                        report.sampled_plugins += sampled;
                        report.sampled_components += sampled;
                    }
                    Err(reason) => {
                        error = Some(Error {
                            path: path.clone(),
                            reason,
                        });
                        break;
                    }
                }
            }
            if let Some(error) = error {
                for instance in self.states.values_mut() {
                    instance.sim.discard_stable_plugin_samples();
                }
                return Err(error);
            }
            // None of these calls commits outputs. Tree traversal order cannot
            // make a later register observe an earlier register's next state.
            let mut state_changed = false;
            for instance in self.states.values_mut() {
                let (sampled, changed) = instance.sim.stable_sample_state();
                report.sampled_components += sampled;
                state_changed |= changed;
                state_changed |= instance.sim.commit_stable_plugins();
            }
            report.state_samples += 1;
            if self.pending_drives() == 0 && !state_changed {
                self.progress.record_batch(false);
                self.stable_active = false;
                return Ok(());
            }
        }
        if !self.progress.record_batch(true) {
            return Ok(());
        }
        self.limited = true;
        self.enabled = false;
        for instance in self.states.values_mut() {
            instance.sim.stable_limit();
        }
        Err(Error {
            path: vec![],
            reason: Failure::PropagationLimit,
        })
    }

    pub(super) fn resume_event(&mut self) {
        if !std::mem::take(&mut self.stable_active) {
            return;
        }
        // A compiled pass may already have committed a clock without sampling it,
        // or sampled plugin/RAM state without scheduling its new outputs. Requeue
        // current drives (preserving newer pending values) and evaluate all readers
        // in the next event batch, even if those drives do not change network values.
        for instance in self.states.values_mut() {
            let sim = &mut instance.sim;
            for (&port, &value) in &sim.drives {
                sim.pending.entry(port).or_insert(value);
            }
            sim.initial_batch = true;
            sim.invalidate_stable_values();
        }
        self.progress.restart();
    }

    /// Detect cycles crossing definition boundaries. State outputs have no incoming
    /// edges, so legitimate register feedback does not become combinational feedback.
    fn check_stable_bridges(&self) -> Result<(), Error> {
        if self.bridges.is_empty() {
            return Ok(());
        }
        let mut indices = BTreeMap::new();
        for (path, instance) in &self.states {
            for c in &instance.sim.circuit.components {
                for i in 0..c.ports().len() {
                    if c.drives_port(i) {
                        let next = indices.len();
                        indices.insert((path.clone(), (c.id, i)), next);
                    }
                }
            }
        }
        let mut edges = vec![vec![]; indices.len()];
        for (path, instance) in &self.states {
            let plan = instance.sim.stable_plan.as_ref().unwrap().as_ref().unwrap();
            for &(source, target) in &plan.dependencies {
                let from = indices[&(path.clone(), source)];
                edges[from].push(indices[&(path.clone(), target)]);
            }
        }
        for bridge in &self.bridges {
            let (source_path, source_port, target) = if bridge.output {
                (
                    &bridge.child,
                    (bridge.pin, 0),
                    (bridge.parent.clone(), (bridge.instance, bridge.index)),
                )
            } else {
                (
                    &bridge.parent,
                    (bridge.instance, bridge.index),
                    (bridge.child.clone(), (bridge.pin, 0)),
                )
            };
            for (_, source, _) in self.states[source_path]
                .sim
                .network
                .port_sources(source_port)
            {
                let from = indices[&(source_path.clone(), source)];
                edges[from].push(indices[&target]);
            }
        }
        for edges in &mut edges {
            edges.sort_unstable();
            edges.dedup();
        }
        let (feedback, _) = crate::combinational::graph::partition(&edges);
        if let Some(&index) = feedback.first() {
            let ((path, port), _) = indices.iter().find(|(_, i)| **i == index).unwrap();
            return Err(Error {
                path: path.clone(),
                reason: Failure::Topology {
                    component: port.0,
                    reason: BoundaryReason::Feedback,
                },
            });
        }
        Ok(())
    }
}

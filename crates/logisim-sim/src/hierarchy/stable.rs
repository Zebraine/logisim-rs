use super::*;
use crate::combinational::BoundaryReason;
use crate::stable::{Error, Failure, Report};

impl ProjectSimulator {
    /// Opt-in stable-state execution across the complete expanded hierarchy.
    /// Preflight every instance before committing any pending drive. Unsupported
    /// designs can still use `step`/`advance`; they are never partially accelerated.
    /// Every bridge settles before any state samples, and every instance samples
    /// before any new state output is committed. See `Simulator::settle_compiled`.
    pub fn settle_compiled(&mut self) -> Result<Report, Error> {
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
        self.progress.restart();
        for _ in 0..crate::PROPAGATION_LIMIT {
            for instance in self.states.values_mut() {
                report.evaluated_instructions += instance.sim.stable_combinational_pass();
            }
            report.combinational_passes += 1;
            self.refresh_bridges();
            if self.pending_count() == 0 {
                // None of these calls commits outputs. Tree traversal order cannot
                // make a later register observe an earlier register's next state.
                for instance in self.states.values_mut() {
                    report.sampled_components += instance.sim.stable_sample_storage();
                }
                report.state_samples += 1;
                if self.pending_count() == 0 {
                    self.progress.record_batch(false);
                    return Ok(report);
                }
            }
            if self.progress.record_batch(true) {
                break;
            }
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

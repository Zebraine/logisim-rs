//! ABI capabilities and phase caches; the scheduler remains library-independent.
#[cfg(test)]
mod tests;
use super::*;
use crate::combinational::Port;
use crate::stable::Failure;
use logisim_plugin::StableInstance;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Plan {
    vm: StableInstance,
    output_ports: Vec<usize>,
    sample_ports: Option<Vec<usize>>,
    output_inputs: Option<Vec<api::Signal>>,
    sample_inputs: Option<Vec<api::Signal>>,
    next_state: Option<api::StateTransition>,
}
impl Plan {
    pub fn compile(
        sim: &Simulator,
        dependencies: &mut BTreeSet<(Port, Port)>,
    ) -> Result<BTreeMap<Id, Self>, Failure> {
        let mut plans = BTreeMap::new();
        for c in sim
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Plugin)
        {
            let definition = c
                .plugin
                .as_ref()
                .ok_or(Failure::UnsupportedComponent(c.id))?;
            let state = sim
                .plugin_states
                .get(&c.id)
                .ok_or(Failure::UnsupportedComponent(c.id))?;
            if let Some(message) = &state.error {
                return Err(failure(c.id, message.clone()));
            }
            let vm = sim
                .plugins
                .instantiate_stable(definition)
                .map_err(|message| failure(c.id, message))?
                .ok_or(Failure::UnsupportedComponent(c.id))?;
            let contract = vm.contract();
            contract
                .validate_state(&state.state)
                .map_err(|message| failure(c.id, message))?;
            for output in &contract.outputs {
                for &input in &output.inputs {
                    for (_, source, _) in sim.network.port_sources((c.id, usize::from(input))) {
                        dependencies.insert((source, (c.id, usize::from(output.port))));
                    }
                }
            }
            let output_ports = contract
                .outputs
                .iter()
                .flat_map(|o| &o.inputs)
                .chain(&contract.graphics_inputs)
                .map(|&i| usize::from(i))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let sample_ports = contract
                .state_inputs
                .as_ref()
                .map(|ports| ports.iter().map(|&i| usize::from(i)).collect());
            plans.insert(
                c.id,
                Self {
                    vm,
                    output_ports,
                    sample_ports,
                    output_inputs: None,
                    sample_inputs: None,
                    next_state: None,
                },
            );
        }
        Ok(plans)
    }
    pub fn invalidate(&mut self) {
        self.output_inputs = None;
        self.sample_inputs = None;
        self.next_state = None;
    }
}
fn failure(component: Id, message: String) -> Failure {
    Failure::Plugin { component, message }
}
fn unchanged(
    cached: &Option<Vec<api::Signal>>,
    ports: &[usize],
    entry: &crate::execution::Entry,
    values: &[Value],
) -> bool {
    cached.as_ref().is_some_and(|old| {
        old.iter()
            .zip(ports)
            .all(|(value, &port)| *value == api::Signal::from(entry.port(port, values)))
    })
}

impl Simulator {
    pub(crate) fn stable_plugin_error(&self) -> Option<Failure> {
        self.plugin_errors()
            .next()
            .map(|(id, message)| failure(id, message.into()))
    }
    /// Resolve pure outputs against committed explicit state. Drives remain pending
    /// so later plugins in this pass cannot observe earlier plugins' new outputs.
    pub(crate) fn stable_plugin_outputs(&mut self) -> Result<usize, Failure> {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        let mut evaluated = 0;
        let mut drives = vec![];
        let mut fault = None;
        for entry in &self.plan.plugins {
            let id = self.circuit.components[entry.component].id;
            let phase = plan.plugins.get_mut(&id).unwrap();
            let state = self.plugin_states.get_mut(&id).unwrap();
            if unchanged(
                &phase.output_inputs,
                &phase.output_ports,
                entry,
                &self.values,
            ) {
                continue;
            }
            let inputs: Vec<_> = entry.values(&self.values).map(Into::into).collect();
            evaluated += 1;
            match phase.vm.outputs(&inputs, &state.state) {
                Ok(update) => {
                    state.outputs = update.outputs;
                    state.graphics = update.graphics;
                    state.event_dirty = true;
                    phase.output_inputs =
                        Some(phase.output_ports.iter().map(|&i| inputs[i]).collect());
                }
                Err(message) => {
                    state.fail(message.clone());
                    fault = Some(failure(id, message));
                }
            }
            drives.extend(state.drives().map(|(i, v)| ((id, i), v)));
            if fault.is_some() {
                break;
            }
        }
        for (port, value) in drives {
            self.drive_port(port, value);
        }
        if let Some(fault) = fault {
            // Expose the fault immediately, even when no further propagation is run.
            self.stable_commit();
            return Err(fault);
        }
        Ok(evaluated)
    }
    /// Stage all plugin state transitions without changing any explicit state or drive.
    /// The project coordinator calls this on every instance before built-in sampling.
    pub(crate) fn stage_stable_plugins(&mut self) -> Result<usize, Failure> {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        let mut sampled = 0;
        let mut fault = None;
        for entry in &self.plan.plugins {
            let id = self.circuit.components[entry.component].id;
            let phase = plan.plugins.get_mut(&id).unwrap();
            let Some(ports) = &phase.sample_ports else {
                continue;
            };
            if unchanged(&phase.sample_inputs, ports, entry, &self.values) {
                continue;
            }
            let state = self.plugin_states.get_mut(&id).unwrap();
            let inputs: Vec<_> = entry.values(&self.values).map(Into::into).collect();
            sampled += 1;
            match phase.vm.transition(&inputs, &state.state) {
                Ok(next) => {
                    phase.next_state = (next.state != state.state).then_some(next);
                    phase.sample_inputs = Some(ports.iter().map(|&i| inputs[i]).collect());
                }
                Err(message) => {
                    state.fail(message.clone());
                    fault = Some((
                        failure(id, message),
                        state
                            .drives()
                            .map(|(i, v)| ((id, i), v))
                            .collect::<Vec<_>>(),
                    ));
                    break;
                }
            }
        }
        if let Some((error, drives)) = fault {
            for (port, value) in drives {
                self.drive_port(port, value);
            }
            self.stable_commit();
            return Err(error);
        }
        Ok(sampled)
    }
    pub(crate) fn discard_stable_plugin_samples(&mut self) {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        for phase in plan.plugins.values_mut() {
            if phase.next_state.take().is_some() {
                phase.sample_inputs = None;
            }
        }
    }
    pub(crate) fn commit_stable_plugins(&mut self) -> bool {
        let plan = self.stable_plan.as_mut().unwrap().as_mut().unwrap();
        let mut changed = false;
        for (&id, phase) in &mut plan.plugins {
            if let Some(next) = phase.next_state.take() {
                let state = self.plugin_states.get_mut(&id).unwrap();
                state.state = next.state;
                state.event_dirty = true;
                if next.outputs_changed {
                    phase.output_inputs = None;
                }
                if !next.fixed_point {
                    phase.sample_inputs = None;
                }
                // History-only fixed points still publish state and dirty the event
                // cache, but cannot require another combinational/sample closure.
                changed |= next.outputs_changed || !next.fixed_point;
            }
        }
        changed
    }
}

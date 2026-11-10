use super::*;
#[cfg(test)]
mod tests;
use logisim_plugin_api::{
    Event, LEGACY_ABI_VERSION, STABLE_ABI_VERSION, Signal, StableConfig, StableContract,
    StableEvaluation, StableOutputs, StateTransition, frame,
};

impl Plugin {
    /// Query in a disposable VM before constructing a distinct execution VM.
    /// ABI 1 modules never receive the new messages. None is an explicit opt-out,
    /// while malformed declarations/traps are errors, never silent fallbacks.
    pub fn instantiate_stable(&self, component: &Component) -> Result<Option<StableInstance>> {
        self.check_component(component)?;
        if self.manifest.abi == LEGACY_ABI_VERSION {
            return Ok(None);
        }
        let response =
            self.instantiate(component)?
                .call(&Request::StableContract(StableConfig {
                    tool: component.tool.id.clone(),
                    properties: component.properties.clone(),
                }))?;
        let Response::StableContract(contract) = response else {
            return Err("Plugin did not return a stable-state contract.".into());
        };
        let Some(contract) = contract else {
            return Ok(None);
        };
        contract.validate(&component.tool)?;
        let mut instance = self.instantiate(component)?;
        instance.bind_frame(component)?;
        Ok(Some(StableInstance {
            plugin: self.clone(),
            component: component.clone(),
            contract,
            instance,
            fault: None,
        }))
    }
}

/// A capability-checked VM bound to one immutable component definition and properties.
/// The caller owns explicit state and is responsible for committing a whole sample
/// batch atomically. These methods never commit it. Faults persist until reset in a
/// fresh VM; callers must publish Error drives and discard runtime graphics on failure.
pub struct StableInstance {
    plugin: Plugin,
    component: Component,
    contract: StableContract,
    instance: Instance,
    fault: Option<String>,
}
impl StableInstance {
    /// Whether this VM negotiated the optional direct linear-memory transport.
    pub fn uses_linear_memory(&self) -> bool {
        self.instance.frame.is_some()
    }
    pub fn contract(&self) -> &StableContract {
        &self.contract
    }
    pub fn reset(&mut self, inputs: &[Signal]) -> Result<Update> {
        let result = (|| {
            self.instance = self.plugin.instantiate(&self.component)?;
            self.instance.bind_frame(&self.component)?;
            let update = self.instance.evaluate(
                &self.component,
                Evaluation {
                    tool: self.component.tool.id.clone(),
                    properties: self.component.properties.clone(),
                    inputs: inputs.to_vec(),
                    state: vec![],
                    event: Event::Reset,
                },
            )?;
            self.contract.validate_state(&update.state)?;
            Ok(update)
        })();
        self.fault = None;
        self.finish(result)
    }
    pub fn outputs(&mut self, inputs: &[Signal], state: &[u8]) -> Result<StableOutputs> {
        let result = (|| {
            self.validate(inputs, state)?;
            if self.instance.frame.is_some() {
                let frame::Output::Outputs(update) =
                    self.instance
                        .call_frame(frame::Operation::Outputs, inputs, state)?
                else {
                    return Err("Plugin did not return stable-state outputs.".into());
                };
                self.component.tool.validate_stable_outputs(&update)?;
                return Ok(update);
            }
            let request = self.request(inputs, state);
            let Response::StableOutputs(update) =
                self.instance.call(&Request::StableOutputs(request))?
            else {
                return Err("Plugin did not return stable-state outputs.".into());
            };
            self.component.tool.validate_stable_outputs(&update)?;
            Ok(update)
        })();
        self.finish(result)
    }
    pub fn sample(&mut self, inputs: &[Signal], state: &[u8]) -> Result<Vec<u8>> {
        self.transition(inputs, state).map(|next| next.state)
    }
    /// Sample without committing. ABI 2 results conservatively invalidate both phase
    /// caches; only ABI 3 receives the request for explicit transition promises.
    pub fn transition(&mut self, inputs: &[Signal], state: &[u8]) -> Result<StateTransition> {
        let result = (|| {
            self.validate(inputs, state)?;
            if self.contract.state_inputs.is_none() {
                return Ok(StateTransition {
                    state: vec![],
                    outputs_changed: false,
                    fixed_point: true,
                });
            }
            if self.instance.frame.is_some() {
                let frame::Output::Transition(next) =
                    self.instance
                        .call_frame(frame::Operation::Transition, inputs, state)?
                else {
                    return Err("Plugin did not return a stable-state transition.".into());
                };
                self.contract.validate_state(&next.state)?;
                return Ok(next);
            }
            let request = self.request(inputs, state);
            let next = if self.plugin.manifest.abi == STABLE_ABI_VERSION {
                let Response::StableState(next) =
                    self.instance.call(&Request::StableSample(request))?
                else {
                    return Err("Plugin did not return a stable-state sample.".into());
                };
                StateTransition::conservative(next)
            } else {
                let Response::StableTransition(next) =
                    self.instance.call(&Request::StableTransition(request))?
                else {
                    return Err("Plugin did not return a stable-state transition.".into());
                };
                next
            };
            self.contract.validate_state(&next.state)?;
            Ok(next)
        })();
        self.finish(result)
    }
    fn validate(&self, inputs: &[Signal], state: &[u8]) -> Result<()> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        self.contract.validate_state(state)?;
        self.component.tool.validate_stable_values(
            &self.component.tool.id,
            &self.component.properties,
            inputs,
            state,
        )
    }
    fn request(&self, inputs: &[Signal], state: &[u8]) -> StableEvaluation {
        StableEvaluation {
            tool: self.component.tool.id.clone(),
            properties: self.component.properties.clone(),
            inputs: inputs.to_vec(),
            state: state.to_vec(),
        }
    }
    fn finish<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            self.fault = Some(error.clone());
        }
        result
    }
}

impl StableInstance {
    pub fn raster(
        &mut self,
        state: &[u8],
        previous: Option<&Arc<logisim_plugin_api::raster::Image>>,
    ) -> Result<Option<Arc<logisim_plugin_api::raster::Image>>> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        let result = self.instance.raster(&self.component, state, previous);
        self.finish(result)
    }
}

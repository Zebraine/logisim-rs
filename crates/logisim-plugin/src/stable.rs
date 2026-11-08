use super::*;
#[cfg(test)]
mod tests;
use logisim_plugin_api::{
    Event, LEGACY_ABI_VERSION, Signal, StableConfig, StableContract, StableEvaluation,
    StableOutputs,
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
        Ok(Some(StableInstance {
            plugin: self.clone(),
            component: component.clone(),
            contract,
            instance: self.instantiate(component)?,
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
    pub fn contract(&self) -> &StableContract {
        &self.contract
    }
    pub fn reset(&mut self, inputs: &[Signal]) -> Result<Update> {
        let result = (|| {
            self.instance = self.plugin.instantiate(&self.component)?;
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
            let request = self.request(inputs, state)?;
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
        let result = (|| {
            let request = self.request(inputs, state)?;
            if self.contract.state_inputs.is_none() {
                return Ok(vec![]);
            }
            let Response::StableState(next) =
                self.instance.call(&Request::StableSample(request))?
            else {
                return Err("Plugin did not return a stable-state sample.".into());
            };
            self.contract.validate_state(&next)?;
            Ok(next)
        })();
        self.finish(result)
    }
    fn request(&self, inputs: &[Signal], state: &[u8]) -> Result<StableEvaluation> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        self.contract.validate_state(state)?;
        let request = StableEvaluation {
            tool: self.component.tool.id.clone(),
            properties: self.component.properties.clone(),
            inputs: inputs.to_vec(),
            state: state.to_vec(),
        };
        self.component.tool.validate_stable_evaluation(&request)?;
        Ok(request)
    }
    fn finish<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            self.fault = Some(error.clone());
        }
        result
    }
}

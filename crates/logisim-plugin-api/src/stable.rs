use super::*;
use std::collections::BTreeSet;
#[cfg(test)]
mod tests;

/// ABI 2 capability query, evaluated in a disposable VM for these exact properties.
/// Return `StableContract(None)` when unsupported, including unsupported modes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableConfig {
    pub tool: String,
    pub properties: Properties,
}

/// An explicit opt-in to settled combinational evaluation followed by state sampling.
///
/// Every request is a pure function of properties, inputs and explicit state: hidden
/// VM memory, call count and query order must not affect results. Outputs computes
/// current drives/graphics without advancing state. Sample computes next state without
/// publishing drives. The host must settle ALL combinational paths/bridges, snapshot
/// ALL instances' inputs, sample ALL states, then publish the resulting outputs.
/// Initialize using ordinary Reset in a fresh VM; state has the same encoding in both
/// execution modes. Actions use ordinary Evaluate. Never switch modes mid-propagation.
///
/// Port indices refer to Tool.ports. Each output must appear exactly once. Dependency
/// lists contain unique INPUT ports and must conservatively include every input which
/// can affect that result for any explicit state. State itself is an implicit dependency
/// of every output/graphic. `state_inputs: None` means stateless (empty opaque state);
/// `Some` declares the inputs affecting next state, including clock/history controls.
/// These declarations are promises, not proofs; hosts validate shape/bounds but cannot
/// prove arbitrary Wasm is pure or that the declared dependencies are complete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableContract {
    pub outputs: Vec<OutputDependency>,
    pub graphics_inputs: Vec<u8>,
    pub state_inputs: Option<Vec<u8>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputDependency {
    pub port: u8,
    pub inputs: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableEvaluation {
    pub tool: String,
    pub properties: Properties,
    pub inputs: Vec<Signal>,
    pub state: Vec<u8>,
}
/// No state field: output evaluation cannot replace the host's committed state.
/// As with Update, outputs has one entry per port; input-port entries are ignored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableOutputs {
    pub outputs: Vec<Signal>,
    pub graphics: Vec<Shape>,
}

impl StableContract {
    pub fn validate(&self, tool: &Tool) -> Result<(), String> {
        tool.validate()?;
        let inputs = |ports: &[u8]| {
            ports.len() <= tool.ports.len()
                && ports.iter().collect::<BTreeSet<_>>().len() == ports.len()
                && ports
                    .iter()
                    .all(|&i| tool.ports.get(usize::from(i)).is_some_and(|p| !p.output))
        };
        let output_count = tool.ports.iter().filter(|p| p.output).count();
        let mut seen = BTreeSet::new();
        if self.outputs.len() != output_count
            || self.outputs.iter().any(|d| {
                !seen.insert(d.port)
                    || !tool
                        .ports
                        .get(usize::from(d.port))
                        .is_some_and(|p| p.output)
                    || !inputs(&d.inputs)
            })
            || !inputs(&self.graphics_inputs)
            || self.state_inputs.as_ref().is_some_and(|p| !inputs(p))
        {
            return Err("Invalid plugin stable-state dependencies.".into());
        }
        Ok(())
    }
    pub fn validate_state(&self, state: &[u8]) -> Result<(), String> {
        if state.len() > MAX_STATE || (self.state_inputs.is_none() && !state.is_empty()) {
            return Err("Invalid plugin stable-state size.".into());
        }
        Ok(())
    }
}
impl Tool {
    pub fn validate_stable_config(&self, config: &StableConfig) -> Result<(), String> {
        if config.tool != self.id {
            return Err("Invalid plugin stable-state tool.".into());
        }
        self.validate_properties(&config.properties)
    }
    pub fn validate_stable_evaluation(&self, request: &StableEvaluation) -> Result<(), String> {
        self.validate_request(
            &request.tool,
            &request.properties,
            &request.inputs,
            &request.state,
        )
    }
    pub fn validate_stable_outputs(&self, update: &StableOutputs) -> Result<(), String> {
        self.validate_outputs(&update.outputs, &update.graphics)
    }
}

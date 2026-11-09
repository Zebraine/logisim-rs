//! Shared protocol handling for pure combinational libraries without runtime actions.
//! Callbacks receive validated tools, properties and signals. All state is empty;
//! dependency declarations must include every input used by outputs or graphics.
use crate::api::*;

pub struct Library {
    pub manifest: fn() -> &'static Manifest,
    pub outputs: fn(&str, &Properties, &[Signal]) -> StableOutputs,
    pub dependencies: fn(&Tool) -> StableContract,
}
impl Library {
    fn tool(&self, id: &str) -> Result<&'static Tool, String> {
        (self.manifest)()
            .tools
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| "Unknown combinational tool".into())
    }
    fn empty(state: &[u8]) -> Result<(), String> {
        if state.is_empty() {
            Ok(())
        } else {
            Err("Component is stateless".into())
        }
    }
    pub fn contract(&self, config: StableConfig) -> Result<Option<StableContract>, String> {
        let tool = self.tool(&config.tool)?;
        tool.validate_stable_config(&config)?;
        let contract = (self.dependencies)(tool);
        contract.validate(tool)?;
        if contract.state_inputs.is_some() {
            return Err("Combinational contract must be stateless".into());
        }
        Ok(Some(contract))
    }
    fn validate(
        &self,
        id: &str,
        properties: &Properties,
        inputs: &[Signal],
        state: &[u8],
    ) -> Result<(), String> {
        self.tool(id)?
            .validate_stable_values(id, properties, inputs, state)?;
        Self::empty(state)
    }
    fn transition() -> StateTransition {
        StateTransition {
            state: vec![],
            outputs_changed: false,
            fixed_point: true,
        }
    }
    pub fn frame(
        &self,
        config: &StableConfig,
        operation: frame::Operation,
        inputs: &[Signal],
        state: &[u8],
    ) -> Result<frame::Output, String> {
        self.validate(&config.tool, &config.properties, inputs, state)?;
        Ok(match operation {
            frame::Operation::Outputs => {
                frame::Output::Outputs((self.outputs)(&config.tool, &config.properties, inputs))
            }
            frame::Operation::Transition => frame::Output::Transition(Self::transition()),
        })
    }
    pub fn handle(&self, request: Request) -> Response {
        self.respond(request).unwrap_or_else(Response::Error)
    }
    fn respond(&self, request: Request) -> Result<Response, String> {
        Ok(match request {
            Request::Describe => Response::Manifest((self.manifest)().clone()),
            Request::Evaluate(r) => {
                self.tool(&r.tool)?.validate_evaluation(&r)?;
                Self::empty(&r.state)?;
                if matches!(r.event, Event::Action { .. }) {
                    return Err("Combinational components do not support actions".into());
                }
                let result = (self.outputs)(&r.tool, &r.properties, &r.inputs);
                Response::Update(Update {
                    outputs: result.outputs,
                    graphics: result.graphics,
                    state: vec![],
                })
            }
            Request::StableContract(c) => Response::StableContract(self.contract(c)?),
            Request::StableOutputs(r) => {
                self.validate(&r.tool, &r.properties, &r.inputs, &r.state)?;
                Response::StableOutputs((self.outputs)(&r.tool, &r.properties, &r.inputs))
            }
            Request::StableSample(r) => {
                self.validate(&r.tool, &r.properties, &r.inputs, &r.state)?;
                Response::StableState(vec![])
            }
            Request::StableTransition(r) => {
                self.validate(&r.tool, &r.properties, &r.inputs, &r.state)?;
                Response::StableTransition(Self::transition())
            }
        })
    }
}

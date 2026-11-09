//! Standalone TTL package: observed pin maps, four-state behavior and vector drawings.
mod graphics;
mod presentation;
pub use presentation::metadata as ui_metadata;
mod logic;
#[cfg(feature = "host-check")]
pub mod migrate;
pub mod model;
mod stable;
pub use stable::frame as handle_stable_frame;
#[cfg(test)]
mod tests;
use logisim_core::Value;
use logisim_plugin_api::*;
logisim_plugin_sdk::export_plugin!(
    handle,
    stable = handle_stable_frame,
    presentation = ui_metadata
);

pub fn handle(request: Request) -> Response {
    match request {
        Request::Describe => Response::Manifest(graphics::manifest().clone()),
        Request::Evaluate(request) => evaluate(request)
            .map(Response::Update)
            .unwrap_or_else(Response::Error),
        Request::StableContract(config) => stable::contract(config)
            .map(Response::StableContract)
            .unwrap_or_else(Response::Error),
        Request::StableOutputs(request) => stable::outputs(
            &request.tool,
            &request.properties,
            &request.inputs,
            &request.state,
        )
        .map(Response::StableOutputs)
        .unwrap_or_else(Response::Error),
        Request::StableSample(request) => stable::transition(
            &request.tool,
            &request.properties,
            &request.inputs,
            &request.state,
        )
        .map(|s| Response::StableState(s.state))
        .unwrap_or_else(Response::Error),
        Request::StableTransition(request) => stable::transition(
            &request.tool,
            &request.properties,
            &request.inputs,
            &request.state,
        )
        .map(Response::StableTransition)
        .unwrap_or_else(Response::Error),
    }
}
fn tool(id: &str) -> Result<&'static Tool, String> {
    graphics::manifest()
        .tools
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "Unknown TTL tool".into())
}
fn outputs(id: &str, properties: &Properties, inputs: &[Signal]) -> StableOutputs {
    let chip = model::Chip::parse(id).expect("validated tool");
    let values: Vec<_> = inputs.iter().copied().map(Value::from).collect();
    let mut outputs = inputs.to_vec();
    for (i, v) in logic::evaluate(chip, &values) {
        outputs[i] = v.into();
    }
    StableOutputs {
        outputs,
        graphics: if properties["internal"] == "true" {
            graphics::drawing(chip, true)
        } else {
            vec![]
        },
    }
}
fn evaluate(request: Evaluation) -> Result<Update, String> {
    tool(&request.tool)?.validate_evaluation(&request)?;
    if !request.state.is_empty() {
        return Err("TTL components are stateless".into());
    }
    let out = outputs(&request.tool, &request.properties, &request.inputs);
    Ok(Update {
        outputs: out.outputs,
        graphics: out.graphics,
        state: vec![],
    })
}

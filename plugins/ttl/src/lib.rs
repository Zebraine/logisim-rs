//! Standalone TTL package: observed pin maps, four-state behavior and vector drawings.
mod graphics;
mod presentation;
pub use presentation::metadata as ui_metadata;
mod logic;
#[cfg(feature = "host-check")]
pub mod migrate;
pub mod model;
mod stable;
pub fn handle_stable_frame(
    config: &StableConfig,
    operation: frame::Operation,
    inputs: &[Signal],
    state: &[u8],
) -> Result<frame::Output, String> {
    LIBRARY.frame(config, operation, inputs, state)
}
#[cfg(test)]
mod tests;
use logisim_core::Value;
use logisim_plugin_api::*;
logisim_plugin_sdk::export_plugin!(
    handle,
    stable = handle_stable_frame,
    presentation = ui_metadata
);

const LIBRARY: logisim_plugin_sdk::stateless::Library = logisim_plugin_sdk::stateless::Library {
    manifest: graphics::manifest,
    outputs,
    dependencies: stable::dependencies,
};
pub fn handle(request: Request) -> Response {
    LIBRARY.handle(request)
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

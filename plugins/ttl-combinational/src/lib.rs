//! Second TTL batch: observed combinational chips, independent of the basic gate module.
mod graphics;
mod logic;
pub mod model;
mod presentation;
mod stable;
#[cfg(test)]
mod tests;
#[cfg(any(test, feature = "host-check"))]
pub mod validation;
use logisim_core::Value;
use logisim_plugin_api::*;
pub use presentation::metadata as ui_metadata;

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
pub fn handle_stable_frame(
    config: &StableConfig,
    operation: frame::Operation,
    inputs: &[Signal],
    state: &[u8],
) -> Result<frame::Output, String> {
    LIBRARY.frame(config, operation, inputs, state)
}
fn outputs(id: &str, properties: &Properties, inputs: &[Signal]) -> StableOutputs {
    let chip = model::Chip::parse(id).expect("validated tool");
    let values: Vec<_> = inputs.iter().copied().map(Value::from).collect();
    let mut outputs = inputs.to_vec();
    for (pin, value) in chip
        .kind
        .outputs()
        .iter()
        .zip(logic::evaluate(chip, &values))
    {
        outputs[chip.index(*pin)] = Value::filled(1, value).into();
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

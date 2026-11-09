use crate::{model::Chip, tool};
use logisim_plugin_api::*;
pub fn contract(config: StableConfig) -> Result<Option<StableContract>, String> {
    let tool = tool(&config.tool)?;
    tool.validate_stable_config(&config)?;
    let chip = Chip::parse(&config.tool).expect("validated tool");
    Ok(Some(StableContract {
        outputs: chip
            .kind
            .gates()
            .iter()
            .map(|&[a, b, o]| {
                let mut inputs = vec![chip.index(a) as u8];
                if a != b {
                    inputs.push(chip.index(b) as u8);
                }
                if chip.power {
                    inputs.extend([chip.index(7) as u8, chip.index(14) as u8]);
                }
                OutputDependency {
                    port: chip.index(o) as u8,
                    inputs,
                }
            })
            .collect(),
        graphics_inputs: vec![],
        state_inputs: None,
    }))
}
fn validate(
    id: &str,
    properties: &Properties,
    inputs: &[Signal],
    state: &[u8],
) -> Result<(), String> {
    tool(id)?.validate_stable_values(id, properties, inputs, state)?;
    if !state.is_empty() {
        return Err("TTL components are stateless".into());
    }
    Ok(())
}
pub fn outputs(
    id: &str,
    properties: &Properties,
    inputs: &[Signal],
    state: &[u8],
) -> Result<StableOutputs, String> {
    validate(id, properties, inputs, state)?;
    Ok(crate::outputs(id, properties, inputs))
}
pub fn transition(
    id: &str,
    properties: &Properties,
    inputs: &[Signal],
    state: &[u8],
) -> Result<StateTransition, String> {
    validate(id, properties, inputs, state)?;
    Ok(StateTransition {
        state: vec![],
        outputs_changed: false,
        fixed_point: true,
    })
}
pub fn frame(
    config: &StableConfig,
    operation: frame::Operation,
    inputs: &[Signal],
    state: &[u8],
) -> Result<frame::Output, String> {
    match operation {
        frame::Operation::Outputs => {
            outputs(&config.tool, &config.properties, inputs, state).map(frame::Output::Outputs)
        }
        frame::Operation::Transition => transition(&config.tool, &config.properties, inputs, state)
            .map(frame::Output::Transition),
    }
}

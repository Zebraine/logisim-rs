use crate::{graphics, register_state};
use logisim_core::Value;
use logisim_plugin_api::*;
use std::sync::OnceLock;

pub fn contract(config: StableConfig) -> Result<Option<StableContract>, String> {
    let tool = graphics::manifest()
        .tools
        .iter()
        .find(|t| t.id == config.tool)
        .ok_or("Unknown tool")?;
    tool.validate_stable_config(&config)?;
    Ok(describe(&config.tool, &config.properties).cloned())
}
fn describe(tool: &str, properties: &Properties) -> Option<&'static StableContract> {
    if tool == "riscv-probe" {
        static PROBE: OnceLock<StableContract> = OnceLock::new();
        return Some(PROBE.get_or_init(|| StableContract {
            outputs: vec![],
            graphics_inputs: vec![0],
            state_inputs: None,
        }));
    }
    if !matches!(properties["trigger"].as_str(), "rising" | "falling") {
        return None;
    }
    static REGISTER: OnceLock<StableContract> = OnceLock::new();
    Some(REGISTER.get_or_init(|| StableContract {
        outputs: vec![
            OutputDependency {
                port: 0,
                inputs: vec![6],
            },
            OutputDependency {
                port: 1,
                inputs: vec![7],
            },
        ],
        graphics_inputs: vec![],
        state_inputs: Some(vec![2, 3, 4, 5]),
    }))
}
fn validate(
    tool_id: &str,
    properties: &Properties,
    inputs: &[Signal],
    state: &[u8],
) -> Result<(), String> {
    let tool = graphics::manifest()
        .tools
        .iter()
        .find(|t| t.id == tool_id)
        .ok_or("Unknown tool")?;
    tool.validate_stable_values(tool_id, properties, inputs, state)?;
    let contract = describe(tool_id, properties)
        .ok_or("This configuration does not support stable-state execution.")?;
    contract.validate_state(state)
}
pub fn frame(
    config: &StableConfig,
    operation: frame::Operation,
    inputs: &[Signal],
    state: &[u8],
) -> Result<frame::Output, String> {
    match operation {
        frame::Operation::Outputs => {
            outputs_borrowed(&config.tool, &config.properties, inputs, state)
                .map(frame::Output::Outputs)
        }
        frame::Operation::Transition => {
            transition_borrowed(&config.tool, &config.properties, inputs, state)
                .map(frame::Output::Transition)
        }
    }
}
pub fn outputs(request: StableEvaluation) -> Result<StableOutputs, String> {
    outputs_borrowed(
        &request.tool,
        &request.properties,
        &request.inputs,
        &request.state,
    )
}
fn outputs_borrowed(
    tool: &str,
    properties: &Properties,
    inputs: &[Signal],
    state: &[u8],
) -> Result<StableOutputs, String> {
    validate(tool, properties, inputs, state)?;
    if tool == "riscv-probe" {
        return Ok(StableOutputs {
            graphics: graphics::probe(Value::from(inputs[0])),
            outputs: inputs.to_vec(),
        });
    }
    let file = register_state::decode(state)?;
    let ports = std::array::from_fn(|i| Value::from(inputs[i]));
    let mut outputs = inputs.to_vec();
    for (i, value) in file.outputs(&ports).into_iter().enumerate() {
        outputs[i] = Signal::from(value);
    }
    Ok(StableOutputs {
        outputs,
        graphics: graphics::register_words(file.words()),
    })
}
pub fn sample(request: StableEvaluation) -> Result<Vec<u8>, String> {
    transition(request).map(|next| next.state)
}
pub fn transition(request: StableEvaluation) -> Result<StateTransition, String> {
    transition_borrowed(
        &request.tool,
        &request.properties,
        &request.inputs,
        &request.state,
    )
}
fn transition_borrowed(
    tool: &str,
    properties: &Properties,
    inputs: &[Signal],
    state: &[u8],
) -> Result<StateTransition, String> {
    validate(tool, properties, inputs, state)?;
    if tool == "riscv-probe" {
        return Ok(StateTransition {
            state: vec![],
            outputs_changed: false,
            fixed_point: true,
        });
    }
    let mut file = register_state::decode(state)?;
    let before = *file.words();
    let ports = std::array::from_fn(|i| Value::from(inputs[i]));
    file.sample(register_state::trigger(properties), &ports);
    Ok(StateTransition {
        state: register_state::encode(&file),
        outputs_changed: before != *file.words(),
        fixed_point: true,
    })
}

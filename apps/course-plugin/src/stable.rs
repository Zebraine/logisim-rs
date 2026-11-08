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
fn validate(request: &StableEvaluation) -> Result<(), String> {
    let tool = graphics::manifest()
        .tools
        .iter()
        .find(|t| t.id == request.tool)
        .ok_or("Unknown tool")?;
    tool.validate_stable_evaluation(request)?;
    let contract = describe(&request.tool, &request.properties)
        .ok_or("This configuration does not support stable-state execution.")?;
    contract.validate_state(&request.state)
}
pub fn outputs(request: StableEvaluation) -> Result<StableOutputs, String> {
    validate(&request)?;
    if request.tool == "riscv-probe" {
        return Ok(StableOutputs {
            graphics: graphics::probe(Value::from(request.inputs[0])),
            outputs: request.inputs,
        });
    }
    let file = register_state::decode(&request.state)?;
    let ports = std::array::from_fn(|i| Value::from(request.inputs[i]));
    let mut outputs = request.inputs;
    for (i, value) in file.outputs(&ports).into_iter().enumerate() {
        outputs[i] = Signal::from(value);
    }
    Ok(StableOutputs {
        outputs,
        graphics: graphics::register_words(file.words()),
    })
}
pub fn sample(request: StableEvaluation) -> Result<Vec<u8>, String> {
    validate(&request)?;
    if request.tool == "riscv-probe" {
        return Ok(vec![]);
    }
    let mut file = register_state::decode(&request.state)?;
    let ports = std::array::from_fn(|i| Value::from(request.inputs[i]));
    file.sample(register_state::trigger(&request.properties), &ports);
    Ok(register_state::encode(&file))
}

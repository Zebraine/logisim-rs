//! Independent third TTL batch. Explicit instance state is shared by both protocols.
mod graphics;
pub mod model;
mod state;
#[cfg(test)]
mod tests;
#[cfg(any(test, feature = "host-check"))]
pub mod validation;
use logisim_core::Value;
use logisim_plugin_api::presentation::Presentation;
use logisim_plugin_api::*;
use model::{Chip, OUTPUTS};
use state::State;

logisim_plugin_sdk::export_plugin!(
    handle,
    stable = handle_stable_frame,
    presentation = ui_metadata,
    reconfiguration = reconfiguration_policy
);

pub fn ui_metadata() -> Presentation {
    Presentation {
        translations: std::collections::BTreeMap::from([(
            "zh-CN".into(),
            serde_json::from_str(include_str!("../locales/zh-CN.json"))
                .expect("embedded translations"),
        )]),
        ..Presentation::default()
    }
}
fn tool(id: &str) -> Result<&'static Tool, String> {
    graphics::manifest()
        .tools
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "Unknown sequential tool".into())
}
fn contract(config: StableConfig) -> Result<Option<StableContract>, String> {
    let tool = tool(&config.tool)?;
    tool.validate_stable_config(&config)?;
    let chip = Chip::parse(&config.tool).expect("validated tool");
    let power = if chip.power {
        vec![chip.index(8) as u8, chip.index(16) as u8]
    } else {
        vec![]
    };
    Ok(Some(StableContract {
        outputs: OUTPUTS
            .map(|pin| OutputDependency {
                port: chip.index(pin) as u8,
                inputs: power.clone(),
            })
            .to_vec(),
        graphics_inputs: vec![],
        state_inputs: Some(
            tool.ports
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.output)
                .map(|(i, _)| i as u8)
                .collect(),
        ),
    }))
}
fn outputs(chip: Chip, properties: &Properties, inputs: &[Signal], state: &State) -> StableOutputs {
    let mut values = inputs.to_vec();
    for (pin, value) in OUTPUTS.into_iter().zip(state.outputs(chip, inputs)) {
        values[chip.index(pin)] = Value::filled(1, value).into();
    }
    StableOutputs {
        outputs: values,
        graphics: if properties["internal"] == "true" {
            graphics::drawing(Some(state))
        } else {
            vec![]
        },
    }
}
pub fn handle_stable_frame(
    config: &StableConfig,
    op: frame::Operation,
    inputs: &[Signal],
    bytes: &[u8],
) -> Result<frame::Output, String> {
    tool(&config.tool)?.validate_stable_values(&config.tool, &config.properties, inputs, bytes)?;
    let chip = Chip::parse(&config.tool).expect("validated tool");
    let mut state = State::decode(bytes)?;
    Ok(match op {
        frame::Operation::Outputs => {
            frame::Output::Outputs(outputs(chip, &config.properties, inputs, &state))
        }
        frame::Operation::Transition => {
            let before = state.bits;
            state.sample(chip, inputs);
            frame::Output::Transition(StateTransition {
                state: state.encode(),
                outputs_changed: before != state.bits,
                fixed_point: true,
            })
        }
    })
}
fn stable(request: StableEvaluation, op: frame::Operation) -> Result<frame::Output, String> {
    handle_stable_frame(
        &StableConfig {
            tool: request.tool,
            properties: request.properties,
        },
        op,
        &request.inputs,
        &request.state,
    )
}
pub fn handle(request: Request) -> Response {
    respond(request).unwrap_or_else(Response::Error)
}
fn respond(request: Request) -> Result<Response, String> {
    Ok(match request {
        Request::Describe => Response::Manifest(graphics::manifest().clone()),
        Request::StableContract(c) => Response::StableContract(contract(c)?),
        Request::StableOutputs(r) => {
            let frame::Output::Outputs(out) = stable(r, frame::Operation::Outputs)? else {
                unreachable!()
            };
            Response::StableOutputs(out)
        }
        Request::StableSample(r) => {
            let frame::Output::Transition(out) = stable(r, frame::Operation::Transition)? else {
                unreachable!()
            };
            Response::StableState(out.state)
        }
        Request::StableTransition(r) => {
            let frame::Output::Transition(out) = stable(r, frame::Operation::Transition)? else {
                unreachable!()
            };
            Response::StableTransition(out)
        }
        Request::Evaluate(r) => {
            tool(&r.tool)?.validate_evaluation(&r)?;
            if matches!(r.event, Event::Action { .. }) {
                return Err("74165 has no runtime actions".into());
            }
            let chip = Chip::parse(&r.tool).expect("validated tool");
            let mut state = if matches!(r.event, Event::Reset) {
                State::default()
            } else {
                State::decode(&r.state)?
            };
            state.sample(chip, &r.inputs);
            let out = outputs(chip, &r.properties, &r.inputs, &state);
            Response::Update(Update {
                outputs: out.outputs,
                graphics: out.graphics,
                state: state.encode(),
            })
        }
    })
}

pub fn reconfiguration_policy() -> logisim_plugin_api::reconfiguration::Policy {
    logisim_plugin_api::reconfiguration::Policy {
        preserve: ["74165", "74165-powered"]
            .into_iter()
            .map(|tool| (tool.into(), vec!["internal".into()]))
            .collect(),
    }
}

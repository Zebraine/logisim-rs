//! General keyboard controller: eight mapped channels and a clock-consumed event FIFO.
mod model;
mod state;
#[cfg(test)]
mod tests;
use logisim_plugin_api::*;
use state::State;

logisim_plugin_sdk::export_plugin!(
    handle,
    stable = stable_frame,
    presentation = presentation,
    interaction = interaction
);

pub fn interaction() -> interaction::Interaction {
    interaction::Interaction {
        keyboards: vec![interaction::Keyboard {
            tool: "keyboard".into(),
            action: "key".into(),
            keys: interaction::Key::ALL.to_vec(),
        }],
    }
}
pub fn presentation() -> presentation::Presentation {
    presentation::Presentation {
        translations: [(
            "zh-CN".into(),
            serde_json::from_str(include_str!("../locales/zh-CN.json"))
                .expect("embedded translations"),
        )]
        .into(),
        ..Default::default()
    }
}
fn tool() -> &'static Tool {
    &model::manifest().tools[0]
}
fn capacity(p: &Properties) -> usize {
    p["capacity"].parse().expect("validated capacity")
}
fn outputs(p: &Properties, state: &State) -> StableOutputs {
    let front = state.queue.front().copied().unwrap_or(0);
    let values = [
        0,
        0,
        0,
        state.mask(p),
        u32::from(front & 7),
        u32::from(front >> 3),
        u32::from(!state.queue.is_empty()),
        u32::from(state.overflow),
        state.queue.len() as u32,
    ];
    StableOutputs {
        outputs: tool()
            .ports
            .iter()
            .zip(values)
            .map(|(p, value)| Signal {
                width: p.width,
                value,
                unknown: 0,
                error: 0,
            })
            .collect(),
        graphics: model::drawing(p, state),
    }
}
pub fn stable_frame(
    config: &StableConfig,
    op: frame::Operation,
    inputs: &[Signal],
    bytes: &[u8],
) -> Result<frame::Output, String> {
    tool().validate_stable_values(&config.tool, &config.properties, inputs, bytes)?;
    let mut state = State::decode(bytes, capacity(&config.properties))?;
    Ok(match op {
        frame::Operation::Outputs => frame::Output::Outputs(outputs(&config.properties, &state)),
        frame::Operation::Transition => {
            let before = (state.held, state.queue.clone(), state.overflow);
            state.sample(&config.properties, inputs);
            let changed = before != (state.held, state.queue.clone(), state.overflow);
            frame::Output::Transition(StateTransition {
                state: state.encode(),
                outputs_changed: changed,
                fixed_point: true,
            })
        }
    })
}
fn stable(r: StableEvaluation, op: frame::Operation) -> Result<frame::Output, String> {
    stable_frame(
        &StableConfig {
            tool: r.tool,
            properties: r.properties,
        },
        op,
        &r.inputs,
        &r.state,
    )
}
pub fn handle(request: Request) -> Response {
    respond(request).unwrap_or_else(Response::Error)
}
fn respond(request: Request) -> Result<Response, String> {
    Ok(match request {
        Request::Describe => Response::Manifest(model::manifest().clone()),
        Request::StableContract(c) => {
            tool().validate_stable_config(&c)?;
            Response::StableContract(Some(StableContract {
                outputs: (3..9)
                    .map(|port| OutputDependency {
                        port,
                        inputs: vec![],
                    })
                    .collect(),
                graphics_inputs: vec![],
                state_inputs: Some(vec![0, 1, 2]),
            }))
        }
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
            tool().validate_evaluation(&r)?;
            let capacity = capacity(&r.properties);
            let mut state = if matches!(r.event, Event::Reset) {
                State::default()
            } else {
                State::decode(&r.state, capacity)?
            };
            state.sample(&r.properties, &r.inputs);
            if let Event::Action { argument, .. } = r.event {
                let event: interaction::InputEvent =
                    serde_json::from_str(&argument).map_err(|e| e.to_string())?;
                if state::bit(r.inputs[2]) != 1 {
                    state.input(&r.properties, capacity, event);
                }
            }
            let out = outputs(&r.properties, &state);
            Response::Update(Update {
                outputs: out.outputs,
                state: state.encode(),
                graphics: out.graphics,
            })
        }
    })
}

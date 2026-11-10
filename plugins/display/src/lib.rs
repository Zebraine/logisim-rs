//! Clocked pixel memory with explicit whole-frame presentation. No host-specific logic.
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
    raster = raster
);
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
fn outputs(tool: &Tool) -> StableOutputs {
    StableOutputs {
        outputs: tool
            .ports
            .iter()
            .map(|p| Signal {
                width: p.width,
                value: 0,
                unknown: 0,
                error: 0,
            })
            .collect(),
        graphics: vec![],
    }
}
pub fn raster(
    config: &StableConfig,
    bytes: &[u8],
    previous: Option<u64>,
) -> Result<raster::Output, String> {
    let (tool, side) = model::tool(&config.tool)?;
    tool.validate_stable_config(config)?;
    let state = State::decode(bytes, side)?;
    if previous == Some(state.revision) {
        return Ok(raster::Output::Unchanged(state.revision));
    }
    let pixels = state
        .front
        .iter()
        .flat_map(|v| {
            if config.properties["palette"] == "Grayscale" {
                [*v, *v, *v, 255]
            } else {
                [
                    ((u32::from(v >> 5) * 255) / 7) as u8,
                    ((u32::from((v >> 2) & 7) * 255) / 7) as u8,
                    (v & 3) * 85,
                    255,
                ]
            }
        })
        .collect();
    Ok(raster::Output::Image(raster::Image {
        revision: state.revision,
        width: side as u32,
        height: side as u32,
        bounds: [[50, 0], [50 + side as i32 * 8, side as i32 * 8]],
        pixels,
    }))
}
pub fn stable_frame(
    config: &StableConfig,
    op: frame::Operation,
    inputs: &[Signal],
    bytes: &[u8],
) -> Result<frame::Output, String> {
    let (tool, side) = model::tool(&config.tool)?;
    tool.validate_stable_values(&config.tool, &config.properties, inputs, bytes)?;
    let mut state = State::decode(bytes, side)?;
    Ok(match op {
        frame::Operation::Outputs => frame::Output::Outputs(outputs(tool)),
        frame::Operation::Transition => {
            let before = state.revision;
            state.sample(inputs, side)?;
            frame::Output::Transition(StateTransition {
                outputs_changed: before != state.revision,
                state: state.encode(),
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
        Request::StableContract(config) => {
            model::tool(&config.tool)?
                .0
                .validate_stable_config(&config)?;
            Response::StableContract(Some(StableContract {
                outputs: vec![],
                graphics_inputs: vec![],
                state_inputs: Some((0..7).collect()),
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
            let (tool, side) = model::tool(&r.tool)?;
            tool.validate_evaluation(&r)?;
            let state = if matches!(r.event, Event::Reset) {
                let mut s = State::new(side);
                s.clock = state::known(r.inputs[0]).unwrap_or(2) as u8;
                s
            } else {
                let mut s = State::decode(&r.state, side)?;
                s.sample(&r.inputs, side)?;
                s
            };
            // Only explicit state crosses calls. No hidden framebuffer or render counter.
            let out = outputs(tool);
            Response::Update(Update {
                outputs: out.outputs,
                graphics: out.graphics,
                state: state.encode(),
            })
        }
    })
}

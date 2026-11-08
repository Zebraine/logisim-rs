//! Example dynamic library: geometry, ports, editable properties, actions and behavior.
mod graphics;
#[cfg(feature = "host-check")]
pub mod migrate;
mod register_state;
mod stable;
#[cfg(test)]
mod tests;
use logisim_core::Value;
use logisim_course::RegisterFile;
use logisim_plugin_api::*;
logisim_plugin_sdk::export_plugin!(handle);

pub fn handle(request: Request) -> Response {
    match request {
        Request::Describe => Response::Manifest(graphics::manifest().clone()),
        Request::StableContract(config) => stable::contract(config)
            .map(Response::StableContract)
            .unwrap_or_else(Response::Error),
        Request::StableOutputs(request) => stable::outputs(request)
            .map(Response::StableOutputs)
            .unwrap_or_else(Response::Error),
        Request::StableSample(request) => stable::sample(request)
            .map(Response::StableState)
            .unwrap_or_else(Response::Error),
        Request::Evaluate(request) => match evaluate(request) {
            Ok(update) => Response::Update(update),
            Err(e) => Response::Error(e),
        },
    }
}
fn evaluate(request: Evaluation) -> Result<Update, String> {
    let manifest = graphics::manifest();
    let tool = manifest
        .tools
        .iter()
        .find(|t| t.id == request.tool)
        .ok_or("Unknown tool")?;
    tool.validate_evaluation(&request)?;
    let inputs: Vec<_> = request.inputs.iter().copied().map(Value::from).collect();
    let mut update = Update {
        outputs: request.inputs.clone(),
        state: vec![],
        graphics: vec![],
    };
    if request.tool == "riscv-probe" {
        update.graphics = graphics::probe(inputs[0]);
        return Ok(update);
    }
    let mut file = if matches!(request.event, Event::Reset) {
        RegisterFile::default()
    } else {
        register_state::decode(&request.state)?
    };
    if let Event::Action { id, argument } = &request.event {
        if id != "write" {
            return Err("Unknown action".into());
        }
        let fields: Vec<_> = argument.split_whitespace().collect();
        if fields.len() != 2 {
            return Err("Enter a decimal address and a hexadecimal word.".into());
        }
        let address = fields[0].parse::<u8>().map_err(|e| e.to_string())?;
        let word = u32::from_str_radix(fields[1].trim_start_matches("0x"), 16)
            .map_err(|e| e.to_string())?;
        if !file.poke(address, word) {
            return Err("Write address must be 1–31.".into());
        }
    }
    let outputs = file.evaluate(
        register_state::trigger(&request.properties),
        &inputs
            .try_into()
            .map_err(|_| "Invalid register-file ports")?,
    );
    for (i, v) in outputs.into_iter().enumerate() {
        update.outputs[i] = Signal::from(v);
    }
    update.state = register_state::encode(&file);
    update.graphics = graphics::register_words(file.words());
    Ok(update)
}

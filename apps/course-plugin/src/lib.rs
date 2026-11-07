//! Example dynamic library: geometry, ports, editable properties, actions and behavior.
mod graphics;
#[cfg(test)]
mod tests;
use logisim_core::{Bit, Trigger, Value};
use logisim_course::{RegisterFile, decode_instruction};
use logisim_plugin_api::*;
logisim_plugin_sdk::export_plugin!(handle);

pub fn handle(request: Request) -> Response {
    match request {
        Request::Describe => Response::Manifest(graphics::manifest()),
        Request::Evaluate(request) => match evaluate(request) {
            Ok(update) => Response::Update(update),
            Err(e) => Response::Error(e),
        },
    }
}
fn value(s: Signal) -> Value {
    Value::from_bits((0..s.width).map(|i| {
        let mask = 1 << i;
        if s.error & mask != 0 {
            Bit::Error
        } else if s.unknown & mask != 0 {
            Bit::Unknown
        } else if s.value & mask != 0 {
            Bit::One
        } else {
            Bit::Zero
        }
    }))
}
fn signal(v: Value) -> Signal {
    let mut s = Signal {
        width: v.width(),
        value: 0,
        unknown: 0,
        error: 0,
    };
    for i in 0..v.width() {
        match v.bit(i) {
            Bit::One => s.value |= 1 << i,
            Bit::Unknown => s.unknown |= 1 << i,
            Bit::Error => s.error |= 1 << i,
            Bit::Zero => {}
        }
    }
    s
}
fn evaluate(request: Evaluation) -> Result<Update, String> {
    let manifest = graphics::manifest();
    let tool = manifest
        .tools
        .iter()
        .find(|t| t.id == request.tool)
        .ok_or("Unknown tool")?;
    tool.validate_properties(&request.properties)?;
    if request.inputs.len() != tool.ports.len() {
        return Err("Invalid input count".into());
    }
    for (input, port) in request.inputs.iter().zip(&tool.ports) {
        input.validate()?;
        if input.width != port.width {
            return Err("Invalid input width".into());
        }
    }
    let inputs: Vec<_> = request.inputs.iter().copied().map(value).collect();
    let mut update = Update {
        outputs: request.inputs.clone(),
        state: vec![],
        graphics: vec![],
    };
    if request.tool == "riscv-probe" {
        if let Some(word) = inputs[0].number() {
            update.graphics.push(graphics::text(
                [-60, -20],
                decode_instruction(word),
                9,
                if word == 0 {
                    [200, 45, 55, 255]
                } else {
                    [37, 99, 235, 255]
                },
            ));
        }
        return Ok(update);
    }
    let mut file = RegisterFile::default();
    if !matches!(request.event, Event::Reset) && !request.state.is_empty() {
        let saved: Vec<Signal> =
            serde_json::from_slice(&request.state).map_err(|e| e.to_string())?;
        if saved.len() != 33 || saved[32].width != 1 {
            return Err("Invalid register-file state".into());
        }
        for v in &saved {
            v.validate()?;
        }
        let words = std::array::from_fn(|i| value(saved[i]));
        file = RegisterFile::restore(words, value(saved[32]).bit(0))?;
    }
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
    let trigger = match request.properties["trigger"].as_str() {
        "falling" => Trigger::Falling,
        "high" => Trigger::High,
        "low" => Trigger::Low,
        _ => Trigger::Rising,
    };
    let outputs = file.evaluate(
        trigger,
        &inputs
            .try_into()
            .map_err(|_| "Invalid register-file ports")?,
    );
    for (i, v) in outputs.into_iter().enumerate() {
        update.outputs[i] = signal(v);
    }
    let (words, clock) = file.snapshot();
    let mut saved: Vec<_> = words.into_iter().map(signal).collect();
    saved.push(signal(Value::filled(1, clock)));
    update.state = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
    for (i, word) in file.words().iter().enumerate() {
        let text = word.number().map_or_else(
            || {
                (0..8)
                    .rev()
                    .map(|n| {
                        let bits = (0..4).map(|b| word.bit(n * 4 + b)).collect::<Vec<_>>();
                        if bits.contains(&Bit::Error) {
                            'E'
                        } else if bits.contains(&Bit::Unknown) {
                            'X'
                        } else {
                            char::from_digit(
                                bits.iter().enumerate().fold(0, |v, (i, b)| {
                                    v | if *b == Bit::One { 1 << i } else { 0 }
                                }),
                                16,
                            )
                            .unwrap()
                        }
                    })
                    .collect()
            },
            |v| format!("{v:08X}"),
        );
        update.graphics.push(graphics::text(
            [-126 + 76 * (i / 16) as i32, -84 + 10 * (i % 16) as i32],
            text,
            7,
            [51, 65, 85, 255],
        ));
    }
    Ok(update)
}

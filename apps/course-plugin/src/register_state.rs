use logisim_core::{Trigger, Value};
use logisim_course::RegisterFile;
use logisim_plugin_api::{Properties, Signal};

pub fn decode(state: &[u8]) -> Result<RegisterFile, String> {
    if state.is_empty() {
        return Ok(RegisterFile::default());
    }
    let saved: Vec<Signal> = serde_json::from_slice(state).map_err(|e| e.to_string())?;
    if saved.len() != 33 || saved[32].width != 1 {
        return Err("Invalid register-file state".into());
    }
    for v in &saved {
        v.validate()?;
    }
    RegisterFile::restore(
        std::array::from_fn(|i| Value::from(saved[i])),
        Value::from(saved[32]).bit(0),
    )
}
pub fn encode(file: &RegisterFile) -> Result<Vec<u8>, String> {
    let (words, clock) = file.snapshot();
    let mut saved: Vec<_> = words.into_iter().map(Signal::from).collect();
    saved.push(Signal::from(Value::filled(1, clock)));
    serde_json::to_vec(&saved).map_err(|e| e.to_string())
}
pub fn trigger(properties: &Properties) -> Trigger {
    match properties["trigger"].as_str() {
        "falling" => Trigger::Falling,
        "high" => Trigger::High,
        "low" => Trigger::Low,
        _ => Trigger::Rising,
    }
}

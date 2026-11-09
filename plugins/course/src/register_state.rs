use logisim_core::{Bit, Trigger, Value};
use logisim_course::RegisterFile;
use logisim_plugin_api::{Properties, Signal};

// Private, versioned wire format, independent of Rust enum/layout details:
// LRF + version, one clock byte, then 32 words, each eight bytes, LSB first.
// Each pair of bits encodes 0 / 1 / unknown / error as 0 / 1 / 2 / 3.
const HEADER: &[u8; 4] = b"LRF\x01";
const LENGTH: usize = 5 + 32 * 8;
const BITS: [Bit; 4] = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error];

pub fn decode(state: &[u8]) -> Result<RegisterFile, String> {
    if state.is_empty() {
        return Ok(RegisterFile::default());
    }
    if state.starts_with(b"LRF") {
        if state.len() != LENGTH || &state[..4] != HEADER || state[4] > 3 {
            return Err("Invalid register-file state encoding".into());
        }
        let words = std::array::from_fn(|word| {
            Value::from_bits((0..32).map(|bit| {
                BITS[usize::from((state[5 + word * 8 + bit / 4] >> (2 * (bit % 4))) & 3)]
            }))
        });
        return RegisterFile::restore(words, BITS[usize::from(state[4])]);
    }
    // Existing event/stable snapshots remain readable; new writes are compact.
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
pub fn encode(file: &RegisterFile) -> Vec<u8> {
    let (words, clock) = file.snapshot();
    let mut saved = Vec::with_capacity(LENGTH);
    saved.extend_from_slice(HEADER);
    saved.push(code(clock));
    for word in words {
        for byte in 0..8 {
            let mut packed = 0;
            for bit in 0..4 {
                packed |= code(word.bit(byte * 4 + bit)) << (2 * bit);
            }
            saved.push(packed);
        }
    }
    saved
}
fn code(bit: Bit) -> u8 {
    match bit {
        Bit::Zero => 0,
        Bit::One => 1,
        Bit::Unknown => 2,
        Bit::Error => 3,
    }
}
pub fn trigger(properties: &Properties) -> Trigger {
    match properties["trigger"].as_str() {
        "falling" => Trigger::Falling,
        "high" => Trigger::High,
        "low" => Trigger::Low,
        _ => Trigger::Rising,
    }
}

#[cfg(test)]
mod tests;

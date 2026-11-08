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
mod tests {
    use super::*;

    #[test]
    fn compact_and_legacy_states_preserve_all_words_bits_and_clock_states() {
        for clock in BITS {
            let words = std::array::from_fn(|word| {
                if word == 0 {
                    Value::known(32, 0)
                } else {
                    Value::from_bits((0..32).map(|bit| BITS[(word + bit / 4 + bit) % 4]))
                }
            });
            let file = RegisterFile::restore(words, clock).unwrap();
            let packed = encode(&file);
            assert_eq!(packed.len(), LENGTH);
            assert_eq!(decode(&packed).unwrap().snapshot(), (words, clock));
            assert_eq!(encode(&decode(&packed).unwrap()), packed);
            let mut legacy: Vec<Signal> = words.into_iter().map(Into::into).collect();
            legacy.push(Value::filled(1, clock).into());
            let legacy = serde_json::to_vec(&legacy).unwrap();
            assert_eq!(decode(&legacy).unwrap().snapshot(), (words, clock));
            assert_eq!(encode(&decode(&legacy).unwrap()), packed);
        }
        assert_eq!(
            decode(&[]).unwrap().snapshot(),
            RegisterFile::default().snapshot()
        );
        let mut file = RegisterFile::default();
        assert!(file.poke(31, 0x8000_0001));
        let packed = encode(&file);
        assert_eq!(&packed[253..], &[1, 0, 0, 0, 0, 0, 0, 64]);
        assert_eq!(
            decode(&packed).unwrap().words()[31].number(),
            Some(0x8000_0001)
        );
    }

    #[test]
    fn malformed_states_and_invalid_zero_register_are_rejected() {
        let state = encode(&RegisterFile::default());
        for len in 1..state.len() {
            assert!(decode(&state[..len]).is_err());
        }
        let mut invalid = state.clone();
        invalid.push(0);
        assert!(decode(&invalid).is_err());
        for (index, value) in [(0, b'X'), (3, 2), (4, 4), (5, 1), (12, 128)] {
            let mut invalid = state.clone();
            invalid[index] = value;
            assert!(decode(&invalid).is_err());
        }
        for invalid in [b"[]".as_slice(), b"null", b"garbage"] {
            assert!(decode(invalid).is_err());
        }
        let mut legacy = vec![Signal::from(Value::known(32, 0)); 32];
        legacy.push(Value::known(1, 0).into());
        for (index, signal) in [
            (0, Value::known(32, 1).into()),
            (1, Value::known(31, 0).into()),
            (32, Value::known(2, 0).into()),
            (
                1,
                Signal {
                    width: 32,
                    value: 1,
                    unknown: 1,
                    error: 0,
                },
            ),
        ] {
            let mut invalid = legacy.clone();
            invalid[index] = signal;
            assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
    }
}

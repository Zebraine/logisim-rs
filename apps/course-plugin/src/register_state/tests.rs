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

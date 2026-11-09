//! Deterministic regression vectors and fingerprints captured from the running reference.
//! Never included in the Wasm module. Local raw observations retain each individual result.
use crate::model::{Chip, Kind};
use logisim_core::{Bit, Value};
use logisim_plugin_api::Signal;
use std::collections::BTreeSet;
pub fn expected(kind: Kind) -> (u64, u64) {
    match kind {
        Kind::Decoder7447 => (0xe8561a73eaa07399, 0x34a2b1c6b3233c89),
        Kind::Comparator7485 => (0x86757efea5112ec5, 0x2c47913d0ee625b3),
        Kind::Buffer74125 => (0x06b4ac626b99ba55, 0x8ef4f764a539d01d),
        Kind::Adder74283 => (0x8e265520a4d18205, 0xee4ab57a894e0383),
    }
}
pub fn binary(kind: Kind) -> Vec<String> {
    let bits = kind.inputs().len();
    (0..1usize << bits)
        .map(|n| format!("{n:0bits$b}"))
        .collect()
}
pub fn four(kind: Kind) -> Vec<String> {
    let n = kind.inputs().len();
    let mut cases = BTreeSet::new();
    for seed in [
        "0".repeat(n),
        "1".repeat(n),
        "01".repeat(n)[..n].into(),
        "10".repeat(n)[..n].into(),
    ] {
        for i in 0..n {
            for bit in ["X", "E"] {
                let mut value = seed.clone();
                value.replace_range(i..i + 1, bit);
                cases.insert(value);
            }
        }
    }
    let prefixes: &[&str] = match kind {
        Kind::Decoder7447 => &["0000", "1010"],
        Kind::Comparator7485 => &["00000000", "11111111", "10101010"],
        _ => &[],
    };
    for prefix in prefixes {
        for a in "01XE".chars() {
            for b in "01XE".chars() {
                for c in "01XE".chars() {
                    cases.insert(format!("{prefix}{a}{b}{c}"));
                }
            }
        }
    }
    if kind == Kind::Buffer74125 {
        for a in "01XE".chars() {
            for b in "01XE".chars() {
                cases.insert(format!("{a}{b}").repeat(4));
            }
        }
    }
    cases.into_iter().collect()
}
pub fn decode(c: char) -> Bit {
    match c {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        'E' => Bit::Error,
        _ => panic!("invalid case bit"),
    }
}
pub fn signals(chip: Chip, values: &str) -> Vec<Signal> {
    assert_eq!(values.len(), chip.kind.inputs().len());
    let mut inputs = vec![Value::known(1, 0).into(); chip.pins().len()];
    for (pin, value) in chip.kind.inputs().iter().zip(values.chars()) {
        inputs[chip.index(*pin)] = Value::filled(1, decode(value)).into();
    }
    if chip.power {
        inputs[chip.index(chip.kind.count())] = Value::known(1, 1).into();
    }
    inputs
}
pub const HASH_START: u64 = 0xcbf29ce484222325;
pub fn fingerprint(hash: &mut u64, chip: Chip, outputs: &[Signal]) {
    for pin in chip.kind.outputs() {
        let byte = match Value::from(outputs[chip.index(*pin)]).bit(0) {
            Bit::Zero => b'0',
            Bit::One => b'1',
            Bit::Unknown => b'X',
            Bit::Error => b'E',
        };
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
}

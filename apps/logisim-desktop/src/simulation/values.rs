//! Four-state masks and numeric text shared by input, probe and port presentation.
use logisim_core::Radix;
use logisim_sim::{Bit, Value};
use logisim_ui::{InputValue, SignalTone, SignalView};

pub(super) fn input_value(value: Value) -> InputValue {
    let mut input = InputValue {
        bits: value.width(),
        ..Default::default()
    };
    for bit in 0..value.width() {
        match value.bit(bit) {
            Bit::One => input.value |= 1 << bit,
            Bit::Unknown => input.unknown |= 1 << bit,
            Bit::Error => input.error |= 1 << bit,
            Bit::Zero => {}
        }
    }
    input
}

pub(super) fn signal(value: Value, mismatch: bool) -> SignalView {
    let tone = if mismatch {
        SignalTone::WidthMismatch
    } else if value.contains(Bit::Error) {
        SignalTone::Error
    } else if value.width() == 0 || value.contains(Bit::Unknown) {
        SignalTone::Unknown
    } else if value.width() > 1 {
        SignalTone::Bus
    } else if value.number() == Some(1) {
        SignalTone::High
    } else {
        SignalTone::Low
    };
    let text = if value.width() == 0 {
        "—".into()
    } else if let Some(number) = value.number() {
        InputValue {
            bits: value.width(),
            value: number,
            ..Default::default()
        }
        .text()
    } else if value.contains(Bit::Error) {
        "E".into()
    } else {
        "X".into()
    };
    SignalView {
        text,
        binary: value.binary(),
        tone,
    }
}

pub(super) fn probe_text(value: Value, radix: Radix) -> String {
    if value.width() == 0 {
        return "—".into();
    }
    if radix == Radix::Binary {
        return value.binary();
    }
    if let Some(n) = value.number() {
        return match radix {
            Radix::Octal => format!(
                "{n:0width$o}",
                width = usize::from(value.width().div_ceil(3))
            ),
            Radix::Hex => format!(
                "{n:0width$X}",
                width = usize::from(value.width().div_ceil(4))
            ),
            Radix::Signed => {
                let signed = if n & (1 << (value.width() - 1)) != 0 {
                    i64::from(n) - (1_i64 << value.width())
                } else {
                    i64::from(n)
                };
                signed.to_string()
            }
            Radix::Unsigned => n.to_string(),
            Radix::Binary => unreachable!(),
        };
    }
    if matches!(radix, Radix::Octal | Radix::Hex) {
        let size = if radix == Radix::Octal { 3 } else { 4 };
        return (0..value.width().div_ceil(size))
            .rev()
            .map(|group| {
                let bits: Vec<_> = (group * size..((group + 1) * size).min(value.width()))
                    .map(|i| value.bit(i))
                    .collect();
                if bits.contains(&Bit::Error) {
                    'E'
                } else if bits.contains(&Bit::Unknown) {
                    'X'
                } else {
                    let n = bits
                        .iter()
                        .enumerate()
                        .fold(0, |n, (i, b)| n | (u32::from(*b == Bit::One) << i));
                    char::from_digit(n, if radix == Radix::Octal { 8 } else { 16 })
                        .unwrap()
                        .to_ascii_uppercase()
                }
            })
            .collect();
    }
    if value.contains(Bit::Error) {
        "E".into()
    } else {
        "X".into()
    }
}

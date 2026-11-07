use crate::{Bit, Value};
use logisim_core::{Component, Kind, Plexer, PlexerRole};

pub(crate) fn evaluate(c: &Component, values: &[Value]) -> Vec<(usize, Value)> {
    use PlexerRole::*;
    let Kind::Plexer(kind) = c.kind else {
        return vec![];
    };
    let terminals = c.plexer_terminals();
    let read = |role| {
        terminals
            .iter()
            .position(|p| p.role == role)
            .map(|i| values[i])
    };
    let enable = read(Enable).map_or(Bit::One, |v| v.bit(0));
    let selected = read(Select);
    if kind == Plexer::BitSelector {
        let width = c.plexing.group_bits.clamp(1, 32);
        let input = read(Input(0)).expect("selector data");
        let value = selected.and_then(Value::number).map_or_else(
            || Value::filled(width, Bit::Unknown),
            |selection| {
                let base = selection * u32::from(width);
                Value::from_bits((0..width).map(|i| {
                    let bit = base + u32::from(i);
                    if bit < u32::from(input.width()) {
                        input.bit(bit as u8)
                    } else {
                        Bit::Zero
                    }
                }))
            },
        );
        return vec![(0, value)];
    }
    if kind == Plexer::DisplayDecoder {
        // EN=0 holds the previous drivers, including across data/control changes.
        if enable == Bit::Zero {
            return vec![];
        }
        let data = if c.plexing.multibit {
            read(Input(0)).and_then(Value::number)
        } else {
            (0..4).try_fold(0_u32, |value, i| {
                match read(Input(i)).expect("decoder bit").bit(0) {
                    Bit::Unknown => None,
                    Bit::One => Some(value | (1 << i)),
                    Bit::Zero | Bit::Error => Some(value),
                }
            })
        };
        let low = |role| read(role).is_some_and(|v| v.bit(0) == Bit::Zero);
        let segments = if low(Blanking) {
            Some(0)
        } else if low(LampTest) {
            Some(0x7f)
        } else if data == Some(0) && low(ZeroBlanking) {
            Some(0)
        } else {
            // Observed 0..15 table; notably 9 omits segment 3.
            const TABLE: [u8; 16] = [
                0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x67, 0x77, 0x7c, 0x39, 0x5e,
                0x79, 0x71,
            ];
            data.map(|d| TABLE[d as usize])
        };
        return (0..7)
            .map(|i| {
                (
                    i,
                    segments.map_or_else(
                        || Value::filled(1, Bit::Unknown),
                        |v| Value::known(1, u32::from((v >> i) & 1)),
                    ),
                )
            })
            .collect();
    }
    let highest = (0..c.plexer_count())
        .rev()
        .find(|i| read(Input(*i)).is_some_and(|v| v.bit(0) == Bit::One));
    terminals
        .iter()
        .enumerate()
        .filter(|(_, p)| p.output)
        .map(|(index, p)| {
            let fill = |bit| Value::filled(p.bits, bit);
            let zero = || Value::known(p.bits, 0);
            let value = if kind == Plexer::PriorityEncoder {
                if enable == Bit::Zero {
                    if p.role == Output(0) && !c.plexing.disabled_zero {
                        fill(Bit::Unknown)
                    } else {
                        zero()
                    }
                } else {
                    match p.role {
                        Group => Value::known(1, u32::from(highest.is_some())),
                        EnableOut => Value::known(1, u32::from(highest.is_none())),
                        _ => highest.map_or_else(
                            || {
                                if c.plexing.no_input_zero {
                                    zero()
                                } else {
                                    fill(Bit::Unknown)
                                }
                            },
                            |i| Value::known(p.bits, u32::from(i)),
                        ),
                    }
                }
            } else if enable == Bit::Zero {
                if c.plexing.disabled_zero {
                    zero()
                } else {
                    fill(Bit::Unknown)
                }
            } else if enable == Bit::Error || selected.is_some_and(|v| v.contains(Bit::Error)) {
                fill(Bit::Error)
            } else if let Some(selection) = selected.and_then(Value::number) {
                match kind {
                    Plexer::Multiplexer => {
                        read(Input(selection as u8)).expect("selected mux input")
                    }
                    Plexer::Demultiplexer | Plexer::Decoder => {
                        if p.role == Output(selection as u8) {
                            if kind == Plexer::Decoder {
                                Value::known(1, 1)
                            } else {
                                read(Input(0)).expect("demux data")
                            }
                        } else if c.plexing.three_state {
                            fill(Bit::Unknown)
                        } else {
                            zero()
                        }
                    }
                    Plexer::PriorityEncoder | Plexer::BitSelector | Plexer::DisplayDecoder => {
                        unreachable!()
                    }
                }
            } else {
                fill(Bit::Unknown)
            };
            (index, value)
        })
        .collect()
}

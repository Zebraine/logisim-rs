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
                    Plexer::PriorityEncoder => unreachable!(),
                }
            } else {
                fill(Bit::Unknown)
            };
            (index, value)
        })
        .collect()
}

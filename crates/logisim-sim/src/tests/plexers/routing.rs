use crate::*;
use PlexerRole::*;
use logisim_core::{Component, Facing, Kind, Plexer, PlexerRole, Point};

use super::support::*;

#[test]
fn observed_routing_enable_and_unknown_selector_matrix() {
    for kind in [Plexer::Multiplexer, Plexer::Demultiplexer, Plexer::Decoder] {
        for zero in [false, true] {
            for tri in [false, true] {
                let mut c = Component::new(Kind::Plexer(kind), Point::ZERO);
                c.bits = if kind.has_data_bits() { 4 } else { 1 };
                c.plexing.enable = true;
                c.plexing.disabled_zero = zero;
                c.plexing.three_state = tri;
                let (c, id, inputs) = fixture(c);
                let mut s = Simulator::default();
                s.synchronize(&c);
                settle(&mut s);
                if kind != Plexer::Decoder {
                    drive(
                        &mut s,
                        &inputs,
                        Input(0),
                        Value::known(4, if kind == Plexer::Multiplexer { 5 } else { 10 }),
                    );
                }
                if kind == Plexer::Multiplexer {
                    drive(&mut s, &inputs, Input(1), Value::known(4, 10));
                }
                for (enable, selector, expected) in [
                    (Bit::One, Bit::Zero, Some(0)),
                    (Bit::One, Bit::One, Some(1)),
                    (Bit::Unknown, Bit::One, Some(1)),
                    (Bit::One, Bit::Unknown, None),
                    (Bit::One, Bit::Error, None),
                    (Bit::Error, Bit::Unknown, None),
                    (Bit::Zero, Bit::Error, None),
                    (Bit::Zero, Bit::One, None),
                ] {
                    drive(&mut s, &inputs, Enable, Value::filled(1, enable));
                    drive(&mut s, &inputs, Select, Value::filled(1, selector));
                    settle(&mut s);
                    let width = if kind == Plexer::Decoder { 1 } else { 4 };
                    for i in 0..if kind == Plexer::Multiplexer { 1 } else { 2 } {
                        let want = if enable == Bit::Zero {
                            Value::filled(width, if zero { Bit::Zero } else { Bit::Unknown })
                        } else if enable == Bit::Error || selector == Bit::Error {
                            Value::filled(width, Bit::Error)
                        } else if let Some(select) = expected {
                            if kind == Plexer::Multiplexer {
                                Value::known(width, if select == 0 { 5 } else { 10 })
                            } else if i == select {
                                Value::known(width, if kind == Plexer::Decoder { 1 } else { 10 })
                            } else {
                                Value::filled(width, if tri { Bit::Unknown } else { Bit::Zero })
                            }
                        } else {
                            Value::filled(width, Bit::Unknown)
                        };
                        assert_eq!(
                            output(&s, id, Output(i)),
                            want,
                            "{kind:?}, en {enable:?}, sel {selector:?}, output {i}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn priority_encoder_cascade_ignores_undefined_data_and_has_distinct_disabled_and_empty_states() {
    for disabled_zero in [false, true] {
        for no_input_zero in [false, true] {
            let mut c = Component::new(Kind::Plexer(Plexer::PriorityEncoder), Point::ZERO);
            c.plexing.select_bits = 2;
            c.plexing.disabled_zero = disabled_zero;
            c.plexing.no_input_zero = no_input_zero;
            let (c, id, inputs) = fixture(c);
            let mut s = Simulator::default();
            s.synchronize(&c);
            settle(&mut s);
            for (data, highest) in [
                ([Bit::Zero, Bit::One, Bit::Zero, Bit::One], Some(3)),
                ([Bit::One, Bit::Error, Bit::Unknown, Bit::Zero], Some(0)),
                ([Bit::Zero; 4], None),
                ([Bit::Error; 4], None),
                ([Bit::Unknown; 4], None),
            ] {
                for (i, bit) in data.into_iter().enumerate() {
                    drive(&mut s, &inputs, Input(i as u8), Value::filled(1, bit));
                }
                for enable in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
                    drive(&mut s, &inputs, Enable, Value::filled(1, enable));
                    settle(&mut s);
                    let enabled = enable != Bit::Zero;
                    assert_eq!(
                        output(&s, id, Group),
                        Value::known(1, u32::from(enabled && highest.is_some()))
                    );
                    assert_eq!(
                        output(&s, id, EnableOut),
                        Value::known(1, u32::from(enabled && highest.is_none()))
                    );
                    let want = if !enabled {
                        Value::filled(
                            2,
                            if disabled_zero {
                                Bit::Zero
                            } else {
                                Bit::Unknown
                            },
                        )
                    } else {
                        highest.map_or_else(
                            || {
                                Value::filled(
                                    2,
                                    if no_input_zero {
                                        Bit::Zero
                                    } else {
                                        Bit::Unknown
                                    },
                                )
                            },
                            |i| Value::known(2, i),
                        )
                    };
                    assert_eq!(output(&s, id, Output(0)), want);
                }
            }
        }
    }
}

#[test]
fn selectors_cover_every_input_and_output_on_all_orientations_and_bus_widths() {
    for kind in [
        Plexer::Multiplexer,
        Plexer::Demultiplexer,
        Plexer::Decoder,
        Plexer::PriorityEncoder,
    ] {
        for bits in 1..=5 {
            for facing in Facing::ALL {
                let mut c = Component::new(Kind::Plexer(kind), Point::ZERO);
                c.plexing.select_bits = bits;
                c.facing = facing;
                c.bits = if kind.has_data_bits() { 32 } else { 1 };
                let (c, id, inputs) = fixture(c);
                let mut s = Simulator::default();
                s.synchronize(&c);
                settle(&mut s);
                if kind == Plexer::PriorityEncoder {
                    drive(&mut s, &inputs, Enable, Value::known(1, 1));
                }
                for i in 0..(1 << bits) {
                    match kind {
                        Plexer::Multiplexer => {
                            for j in 0..(1 << bits) {
                                drive(&mut s, &inputs, Input(j), Value::known(32, 1 << j));
                            }
                            drive(&mut s, &inputs, Select, Value::known(bits, i.into()));
                        }
                        Plexer::Demultiplexer => {
                            drive(&mut s, &inputs, Input(0), Value::known(32, 0xdeadbeef));
                            drive(&mut s, &inputs, Select, Value::known(bits, i.into()));
                        }
                        Plexer::Decoder => {
                            drive(&mut s, &inputs, Select, Value::known(bits, i.into()))
                        }
                        Plexer::BitSelector | Plexer::DisplayDecoder => unreachable!(),
                        Plexer::PriorityEncoder => {
                            drive(&mut s, &inputs, Input(i), Value::known(1, 1))
                        }
                    }
                    settle(&mut s);
                    let (role, want) = match kind {
                        Plexer::Multiplexer => (Output(0), Value::known(32, 1 << i)),
                        Plexer::Demultiplexer => (Output(i), Value::known(32, 0xdeadbeef)),
                        Plexer::Decoder => (Output(i), Value::known(1, 1)),
                        Plexer::PriorityEncoder => (Output(0), Value::known(bits, i.into())),
                        Plexer::BitSelector | Plexer::DisplayDecoder => unreachable!(),
                    };
                    assert_eq!(output(&s, id, role), want);
                }
            }
        }
    }
}

#[test]
fn changing_only_output_policy_recompiles_and_disabled_outputs_release_shared_nets() {
    let mut c = Component::new(Kind::Plexer(Plexer::Multiplexer), Point::ZERO);
    c.plexing.enable = true;
    let (mut circuit, id, _) = fixture(c);
    let at = circuit.component(id).unwrap().ports()[0];
    let pull = Component::new(Kind::Constant, at);
    circuit.add_component(pull);
    let mut s = Simulator::default();
    s.synchronize(&circuit);
    settle(&mut s);
    assert_eq!(output(&s, id, Output(0)), Value::known(1, 1));
    circuit
        .components
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .plexing
        .disabled_zero = true;
    assert!(s.synchronize(&circuit));
    settle(&mut s);
    assert_eq!(output(&s, id, Output(0)), Value::filled(1, Bit::Error));
}

#[test]
fn mixed_selector_error_dominates_unknown_and_selected_data_preserves_unknown_or_error() {
    for kind in [Plexer::Multiplexer, Plexer::Demultiplexer, Plexer::Decoder] {
        let mut c = Component::new(Kind::Plexer(kind), Point::ZERO);
        c.plexing.select_bits = 2;
        c.bits = if kind.has_data_bits() { 4 } else { 1 };
        let (c, id, inputs) = fixture(c);
        let mut s = Simulator::default();
        s.synchronize(&c);
        settle(&mut s);
        for (select, want) in [
            (
                Value::from_bits([Bit::Error, Bit::Unknown].into_iter()),
                Bit::Error,
            ),
            (
                Value::from_bits([Bit::One, Bit::Unknown].into_iter()),
                Bit::Unknown,
            ),
        ] {
            drive(&mut s, &inputs, Select, select);
            settle(&mut s);
            assert_eq!(
                output(&s, id, Output(0)),
                Value::filled(if kind == Plexer::Decoder { 1 } else { 4 }, want)
            );
            if kind != Plexer::Multiplexer {
                assert_eq!(
                    output(&s, id, Output(1)),
                    Value::filled(if kind == Plexer::Decoder { 1 } else { 4 }, want)
                );
            }
        }
        if kind.has_data_bits() {
            drive(&mut s, &inputs, Select, Value::known(2, 0));
            for data in [Bit::Unknown, Bit::Error] {
                drive(&mut s, &inputs, Input(0), Value::filled(4, data));
                settle(&mut s);
                assert_eq!(output(&s, id, Output(0)), Value::filled(4, data));
                if kind == Plexer::Demultiplexer {
                    assert_eq!(output(&s, id, Output(1)), Value::known(4, 0));
                }
            }
        }
    }
}

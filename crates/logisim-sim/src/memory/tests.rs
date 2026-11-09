use super::*;
use logisim_core::{Component, Kind, MemoryInterface, Point};
const FOUR: [Bit; 4] = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error];
fn fixture(interface: MemoryInterface, simple: bool) -> (MemoryLogic, Vec<Value>) {
    let mut c = Component::new(Kind::Ram, Point::ZERO);
    c.bits = 4;
    c.memory.address_bits = 2;
    c.memory.interface = interface;
    c.memory.simple = simple;
    let ports = c
        .memory_terminals()
        .iter()
        .map(|p| Value::known(p.bits, 0))
        .collect();
    (c.memory_logic().unwrap(), ports)
}
fn set(c: &MemoryLogic, p: &mut [Value], role: MemoryRole, value: Value) {
    if let Some(i) = c.port_index(role) {
        p[i] = value;
    }
}
#[test]
fn observed_select_store_load_and_four_state_data_matrix() {
    for interface in MemoryInterface::ALL {
        for simple in [false, true] {
            for high in [false, true] {
                for select in FOUR {
                    for control in FOUR {
                        for data in [
                            Value::known(4, 5),
                            Value::filled(4, Bit::Unknown),
                            Value::filled(4, Bit::Error),
                            Value::from_bits(
                                [Bit::One, Bit::Zero, Bit::Unknown, Bit::One].into_iter(),
                            ),
                        ] {
                            let (mut c, mut p) = fixture(interface, simple);
                            c.select_high = high;
                            set(&c, &mut p, MemoryRole::Select, Value::filled(1, select));
                            set(&c, &mut p, MemoryRole::Store, Value::filled(1, control));
                            set(&c, &mut p, MemoryRole::Load, Value::filled(1, control));
                            set(
                                &c,
                                &mut p,
                                if interface == MemoryInterface::Separate {
                                    MemoryRole::Input
                                } else {
                                    MemoryRole::Data
                                },
                                data,
                            );
                            let mut s = State::new(&c);
                            s.evaluate(&c, &p);
                            set(&c, &mut p, MemoryRole::Clock, Value::known(1, 1));
                            s.evaluate(&c, &p);
                            let separate = interface == MemoryInterface::Separate;
                            let selected = separate && simple
                                || select != if high { Bit::Zero } else { Bit::One };
                            let writing = if separate {
                                simple || control != Bit::Zero
                            } else {
                                control == Bit::Zero
                            };
                            let expected = if selected && writing {
                                data.number().unwrap_or(15)
                            } else {
                                0
                            };
                            assert_eq!(
                                s.contents.get(0),
                                expected,
                                "{interface:?} simple={simple} high={high} select={select:?} control={control:?} data={data:?}"
                            );
                            set(&c, &mut p, MemoryRole::Load, Value::known(1, 1));
                            assert_eq!(
                                s.evaluate(&c, &p),
                                if selected {
                                    Value::known(4, expected)
                                } else {
                                    Value::filled(4, Bit::Unknown)
                                }
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn unknown_addresses_clear_priority_startup_and_word_boundaries() {
    for interface in MemoryInterface::ALL {
        for bits in [1, 32] {
            let (mut c, mut p) = fixture(interface, false);
            c.bits = bits;
            c.address_bits = 24;
            p[0] = Value::known(bits, 0x8000_0001);
            p[1] = Value::known(24, 0xff_ffff);
            set(
                &c,
                &mut p,
                MemoryRole::Input,
                Value::known(bits, 0x8000_0001),
            );
            set(&c, &mut p, MemoryRole::Store, Value::known(1, 1));
            set(&c, &mut p, MemoryRole::Clock, Value::known(1, 1));
            let mut state = State::new(&c);
            state.evaluate(&c, &p);
            assert_eq!(
                state.contents.get(0xff_ffff),
                if interface == MemoryInterface::SharedAsync {
                    0x8000_0001 & (u32::MAX >> (32 - bits))
                } else {
                    0
                }
            );
            set(&c, &mut p, MemoryRole::Clock, Value::known(1, 0));
            state.evaluate(&c, &p);
            set(&c, &mut p, MemoryRole::Clock, Value::known(1, 1));
            state.evaluate(&c, &p);
            assert_eq!(
                state.contents.get(0xff_ffff),
                0x8000_0001 & (u32::MAX >> (32 - bits))
            );
            for bit in [Bit::Unknown, Bit::Error] {
                p[1] = Value::filled(24, bit);
                set(&c, &mut p, MemoryRole::Load, Value::known(1, 1));
                assert_eq!(state.evaluate(&c, &p), Value::filled(bits, Bit::Unknown));
                set(&c, &mut p, MemoryRole::Clear, Value::filled(1, bit));
                state.evaluate(&c, &p);
                assert_ne!(state.contents.get(0xff_ffff), 0);
            }
            set(&c, &mut p, MemoryRole::Select, Value::known(1, 1));
            set(&c, &mut p, MemoryRole::Clear, Value::known(1, 1));
            state.evaluate(&c, &p);
            assert_eq!(state.contents.boundaries().count(), 0);
        }
    }
}
#[test]
fn rom_read_select_and_invalid_address_match_runtime() {
    for simple in [false, true] {
        for high in [false, true] {
            for select in FOUR {
                let mut c = Component::new(Kind::Rom, Point::ZERO);
                c.bits = 32;
                c.memory.address_bits = 24;
                c.memory.simple = simple;
                c.memory.select_high = high;
                c.memory.contents.set(0xff_ffff, 0xdead_beef);
                let mut p = vec![
                    Value::filled(32, Bit::Unknown),
                    Value::known(24, 0xff_ffff),
                    Value::filled(1, select),
                ];
                assert_eq!(
                    rom(&c.memory_logic().unwrap(), &c.memory.contents, &p),
                    if select == if high { Bit::Zero } else { Bit::One } {
                        Value::filled(32, Bit::Unknown)
                    } else {
                        Value::known(32, 0xdead_beef)
                    }
                );
                for bit in [Bit::Unknown, Bit::Error] {
                    p[1] = Value::filled(24, bit);
                    assert_eq!(
                        rom(&c.memory_logic().unwrap(), &c.memory.contents, &p),
                        Value::filled(32, Bit::Unknown)
                    );
                }
            }
        }
    }
}

#[test]
fn ram_image_edits_and_dimension_changes_preserve_clock_history_and_isolation() {
    let (mut config, mut ports) = fixture(MemoryInterface::Separate, false);
    let mut state = State::new(&config);
    set(&config, &mut ports, MemoryRole::Store, Value::known(1, 1));
    set(&config, &mut ports, MemoryRole::Input, Value::known(4, 9));
    state.evaluate(&config, &ports);
    set(&config, &mut ports, MemoryRole::Clock, Value::known(1, 1));
    assert_eq!(state.evaluate(&config, &ports), Value::known(4, 9));
    let sibling = state.clone();

    assert!(!state.fill(0, 5, 7));
    assert!(!state.fill(3, 2, 7));
    assert!(!state.fill(4, 4, 7));
    assert_eq!(state.contents().get(0), 9);
    assert!(state.fill(0, 4, 0x12));
    assert_eq!(state.contents().get(0), 2);
    assert_eq!(state.contents().get(3), 2);
    assert_eq!(sibling.contents().get(0), 9);

    let mut image = MemoryContents::default();
    image.fill(0, 16, 0x1b);
    state.replace_contents(image);
    assert_eq!(state.contents().get(0), 11);
    assert_eq!(state.contents().get(4), 0);
    // Neither image replacement nor a configuration edit creates a rising edge.
    assert_eq!(state.evaluate(&config, &ports), Value::known(4, 11));
    for bits in [1, 32] {
        config.bits = bits;
        state = state.adapt(&config);
        assert_eq!(state.evaluate(&config, &ports), Value::known(bits, 1));
    }
    config.address_bits = 4;
    state = state.adapt(&config);
    assert_eq!(state.contents().get(4), 0);
    assert!(state.fill(8, 16, 5));
    for address_bits in [2, 4] {
        config.address_bits = address_bits;
        state = state.adapt(&config);
        assert_eq!(state.contents().get(8), 0);
    }
    set(&config, &mut ports, MemoryRole::Input, Value::known(32, 7));
    set(&config, &mut ports, MemoryRole::Clock, Value::known(1, 0));
    assert_eq!(state.evaluate(&config, &ports), Value::known(32, 1));
    set(&config, &mut ports, MemoryRole::Clock, Value::known(1, 1));
    assert_eq!(state.evaluate(&config, &ports), Value::known(32, 7));
    state.clear();
    assert_eq!(state.evaluate(&config, &ports), Value::known(32, 0));
    assert_eq!(sibling.contents().get(0), 9);
}

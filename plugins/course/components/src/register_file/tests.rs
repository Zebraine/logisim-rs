use super::*;
#[test]
fn public_poke_keeps_zero_immutable_and_instances_independent() {
    let mut a = RegisterFile::default();
    let b = RegisterFile::default();
    for address in [0, 32, u8::MAX] {
        assert!(!a.poke(address, u32::MAX));
    }
    assert!(a.poke(31, u32::MAX));
    assert_eq!(a.words()[0].number(), Some(0));
    assert_eq!(a.words()[31].number(), Some(u32::MAX));
    assert!(b.words().iter().all(|v| v.number() == Some(0)));
}
fn ports() -> [Value; 8] {
    [
        Value::known(32, 0),
        Value::known(32, 0),
        Value::known(32, 0xa5a55a5a),
        Value::known(1, 0),
        Value::known(1, 1),
        Value::known(5, 1),
        Value::known(5, 1),
        Value::known(5, 0),
    ]
}
#[test]
fn all_triggers_initial_clock_transparency_and_dual_read() {
    for trigger in Trigger::ALL {
        let mut s = RegisterFile::default();
        let mut p = ports();
        assert_eq!(
            s.evaluate(trigger, &p)[0].number(),
            Some(if trigger == Trigger::Low {
                0xa5a55a5a
            } else {
                0
            })
        );
        p[3] = Value::known(1, 1);
        s.evaluate(trigger, &p);
        assert_eq!(
            s.words[1].number(),
            Some(if trigger == Trigger::Falling {
                0
            } else {
                0xa5a55a5a
            })
        );
        p[2] = Value::known(32, 42);
        s.evaluate(trigger, &p);
        assert_eq!(
            s.words[1].number(),
            Some(if trigger == Trigger::High {
                42
            } else if trigger == Trigger::Falling {
                0
            } else {
                0xa5a55a5a
            })
        );
        p[3] = Value::known(1, 0);
        let out = s.evaluate(trigger, &p);
        assert_eq!(out[1].number(), Some(0));
        assert_eq!(
            out[0].number(),
            Some(if trigger == Trigger::Rising {
                0xa5a55a5a
            } else {
                42
            })
        );
    }
    for (trigger, clock) in [(Trigger::Rising, 1), (Trigger::Falling, 0)] {
        let mut s = RegisterFile::default();
        let mut p = ports();
        p[3] = Value::known(1, clock);
        assert_eq!(s.evaluate(trigger, &p)[0].number(), Some(0));
    }
}
#[test]
fn four_state_controls_addresses_and_data() {
    for we in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
        let mut s = RegisterFile::default();
        let mut p = ports();
        p[3] = Value::known(1, 1);
        p[4] = Value::filled(1, we);
        assert_eq!(
            s.evaluate(Trigger::High, &p)[0].number(),
            Some(if we == Bit::Zero { 0 } else { 0xa5a55a5a })
        );
    }
    for bad in [Bit::Unknown, Bit::Error] {
        let mut s = RegisterFile::default();
        let mut p = ports();
        p[3] = Value::filled(1, bad);
        assert_eq!(s.evaluate(Trigger::High, &p)[0].number(), Some(0));
        p[3] = Value::known(1, 1);
        p[2] = Value::filled(32, bad);
        assert_eq!(s.evaluate(Trigger::High, &p)[0], p[2]);
        p[2] = Value::known(32, 7);
        p[5] = Value::filled(5, bad);
        s.evaluate(Trigger::High, &p);
        assert_eq!(s.words[0].number(), Some(0));
        assert!(
            s.words[1..]
                .iter()
                .all(|v| *v == Value::filled(32, Bit::Unknown))
        );
        p[4] = Value::known(1, 0);
        p[6] = Value::filled(5, bad);
        assert_eq!(
            s.evaluate(Trigger::High, &p)[0],
            Value::filled(32, Bit::Unknown)
        );
    }
    let mut s = RegisterFile::default();
    let mut p = ports();
    p[3] = Value::known(1, 1);
    p[5] = Value::known(5, 0);
    p[6] = p[5];
    assert_eq!(s.evaluate(Trigger::High, &p)[0].number(), Some(0));
    p[5] = Value::known(5, 31);
    p[6] = p[5];
    p[7] = p[5];
    p[2] = Value::from_bits((0..32).map(|i| if i == 0 { Bit::Unknown } else { Bit::Zero }));
    assert_eq!(s.evaluate(Trigger::High, &p), [p[2]; 2]);
}

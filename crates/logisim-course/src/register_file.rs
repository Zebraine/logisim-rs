//! Observed 32 × 32 register file; caller supplies all ports sampled in one batch.
use logisim_core::{Bit, Trigger, Value};

pub struct RegisterFile {
    words: [Value; 32],
    clock: Bit,
}
impl Default for RegisterFile {
    fn default() -> Self {
        Self {
            words: [Value::known(32, 0); 32],
            clock: Bit::Unknown,
        }
    }
}
impl RegisterFile {
    /// Opaque-state transports may encode these values without serializing a simulator.
    pub fn snapshot(&self) -> ([Value; 32], Bit) {
        (self.words, self.clock)
    }
    pub fn restore(words: [Value; 32], clock: Bit) -> Result<Self, String> {
        if words.iter().any(|v| v.width() != 32) || words[0] != Value::known(32, 0) {
            return Err("Invalid register-file snapshot.".into());
        }
        Ok(Self { words, clock })
    }
    pub fn words(&self) -> &[Value; 32] {
        &self.words
    }

    /// Runtime-only edit. Register zero is immutable.
    pub fn poke(&mut self, address: u8, value: u32) -> bool {
        if !(1..32).contains(&address) {
            return false;
        }
        self.words[usize::from(address)] = Value::known(32, value);
        true
    }

    fn read(&self, address: Value) -> Value {
        address
            .number()
            .and_then(|i| self.words.get(i as usize).copied())
            .unwrap_or_else(|| Value::filled(32, Bit::Unknown))
    }
    /// Ports follow core's fixed contract: A/B/W (32), CLK/WE (1), rW/rA/rB (5).
    /// A/B inputs are ignored; the returned values drive those two output ports.
    pub fn evaluate(&mut self, trigger: Trigger, ports: &[Value; 8]) -> [Value; 2] {
        let clock = ports[3].bit(0);
        let write = match trigger {
            Trigger::Rising => self.clock == Bit::Zero && clock == Bit::One,
            Trigger::Falling => self.clock == Bit::One && clock == Bit::Zero,
            Trigger::High => clock == Bit::One,
            Trigger::Low => clock == Bit::Zero,
        };
        self.clock = clock;
        if write && ports[4].bit(0) != Bit::Zero {
            match ports[5].number() {
                Some(1..=31) => self.words[ports[5].number().unwrap() as usize] = ports[2],
                Some(_) => {}
                None => self.words[1..].fill(Value::filled(32, Bit::Unknown)),
            }
        }
        [self.read(ports[6]), self.read(ports[7])]
    }
}
#[cfg(test)]
mod tests {
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
}

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
        self.sample(trigger, ports);
        self.outputs(ports)
    }
    /// Read the committed words without consuming a clock edge or changing state.
    pub fn outputs(&self, ports: &[Value; 8]) -> [Value; 2] {
        [self.read(ports[6]), self.read(ports[7])]
    }
    /// Sample next state only. Callers must capture all instances' inputs before
    /// publishing outputs from any updated instance.
    pub fn sample(&mut self, trigger: Trigger, ports: &[Value; 8]) {
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
    }
}
#[cfg(test)]
mod tests;

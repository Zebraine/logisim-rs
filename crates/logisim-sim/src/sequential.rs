//! State transitions sample one resolved network batch before any new outputs are committed.
use crate::{Bit, Value};
use logisim_core::{Component, CounterOverflow, FlipFlop, Kind, Trigger};

#[derive(Clone, Debug)]
pub(super) struct State {
    kind: Kind,
    pub value: Value,
    clock: Bit,
    phase: u64,
    random: Option<crate::random::State>,
}
impl State {
    pub fn new(c: &Component) -> Self {
        let random =
            (c.kind == Kind::Random).then(|| crate::random::State::new(c.sequential.random_seed));
        Self {
            random,
            kind: c.kind,
            value: if let Some(state) = &random {
                state.value(c.bits)
            } else {
                Value::known(
                    if matches!(c.kind, Kind::Register | Kind::Counter | Kind::ShiftRegister) {
                        c.bits
                    } else {
                        1
                    },
                    0,
                )
            },
            clock: Bit::Unknown,
            phase: 0,
        }
    }
    pub fn adapt(mut self, c: &Component) -> Option<Self> {
        if self.kind != c.kind {
            return None;
        }
        if let Some(random) = &self.random {
            self.value = random.value(c.bits);
            return Some(self);
        }
        let width = if matches!(c.kind, Kind::Register | Kind::Counter | Kind::ShiftRegister) {
            c.bits.clamp(1, 32)
        } else {
            1
        };
        if self.value.width() != width {
            if !matches!(c.kind, Kind::Counter | Kind::ShiftRegister) {
                return None;
            }
            self.value = Value::from_bits((0..width).map(|i| {
                if i < self.value.width() {
                    self.value.bit(i)
                } else {
                    Bit::Zero
                }
            }));
        }
        Some(self)
    }
    pub fn tick(&mut self, c: &Component) {
        let low = u64::from(c.sequential.low_ticks.max(1));
        let period = low + u64::from(c.sequential.high_ticks.max(1));
        self.phase = (self.phase + 1) % period;
        if self.phase == 0 || self.phase == low {
            self.toggle();
        }
    }
    pub fn toggle(&mut self) {
        self.value = Value::known(1, u32::from(self.value.number() != Some(1)));
    }
    fn counter_down(c: &Component, ports: &[Value]) -> bool {
        if c.sequential.counter_old {
            ports[3].bit(0) == Bit::One && ports[4].bit(0) != Bit::Zero
        } else {
            ports[4].bit(0) == Bit::One
        }
    }
    pub fn carry(&self, c: &Component, ports: &[Value]) -> Value {
        let goal = if Self::counter_down(c, ports) {
            0
        } else {
            c.sequential.counter_max
        };
        Value::known(1, u32::from(self.value.number() == Some(goal)))
    }
    fn counter(&mut self, c: &Component, ports: &[Value], triggered: bool) -> Value {
        let max = c.sequential.counter_max;
        let load = || {
            let data = ports[1].number().unwrap_or(0);
            if data > max { data & max } else { data }
        };
        if ports[7].bit(0) == Bit::One {
            self.value = Value::known(c.bits, 0);
        } else if ports[6].bit(0) == Bit::One {
            self.value = Value::filled(c.bits, Bit::One);
        } else if triggered {
            let loading = ports[3].bit(0) == Bit::One
                && (!c.sequential.counter_old || ports[4].bit(0) == Bit::Zero);
            let enabled = if c.sequential.counter_old {
                ports[4].bit(0) != Bit::Zero
            } else {
                ports[8].bit(0) != Bit::Zero
            };
            if loading {
                self.value = Value::known(c.bits, load());
            } else if enabled {
                let old = self.value.number().unwrap_or(0);
                let down = Self::counter_down(c, ports);
                let next = if down {
                    old.wrapping_sub(1)
                } else {
                    old.wrapping_add(1)
                };
                let overflow = old == if down { 0 } else { max };
                self.value = Value::known(
                    c.bits,
                    if overflow {
                        match c.sequential.counter_overflow {
                            CounterOverflow::Wrap => {
                                if down {
                                    max
                                } else {
                                    0
                                }
                            }
                            CounterOverflow::Stay => old,
                            CounterOverflow::Continue => next,
                            CounterOverflow::Load => load(),
                        }
                    } else {
                        next
                    },
                );
            }
        }
        self.value
    }
    pub fn evaluate(&mut self, c: &Component, ports: &[Value]) -> Value {
        if c.kind == Kind::Clock {
            return self.value;
        }
        if matches!(c.kind, Kind::Latch(_)) {
            return crate::latches::evaluate(&mut self.value, c, ports);
        }
        let clock = ports[2].bit(0);
        let triggered = match c.sequential.trigger {
            Trigger::Rising => self.clock == Bit::Zero && clock == Bit::One,
            Trigger::Falling => self.clock == Bit::One && clock == Bit::Zero,
            Trigger::High => clock == Bit::One,
            Trigger::Low => clock == Bit::Zero,
        };
        self.clock = clock;
        if let Some(random) = &mut self.random {
            self.value = random.evaluate(c, ports, triggered);
            return self.value;
        }
        if c.kind == Kind::Counter {
            return self.counter(c, ports, triggered);
        }
        if c.kind == Kind::ShiftRegister {
            if ports[4].bit(0) == Bit::One {
                self.value = Value::known(c.bits, 0);
            } else if triggered {
                let old = self.value;
                self.value = match ports[3].number() {
                    Some(1) => Value::from_bits((0..c.bits).map(|i| {
                        if i + 1 == c.bits {
                            ports[1].bit(i)
                        } else {
                            old.bit(i + 1)
                        }
                    })),
                    Some(2) => Value::from_bits((0..c.bits).map(|i| {
                        if i == 0 {
                            ports[1].bit(0)
                        } else {
                            old.bit(i - 1)
                        }
                    })),
                    Some(3) => ports[1],
                    _ => old,
                };
            }
            return self.value;
        }
        if c.kind == Kind::Register
            && ports[4].bit(0)
                == if c.sequential.select_high {
                    Bit::Zero
                } else {
                    Bit::One
                }
        {
            return Value::filled(c.bits, Bit::Unknown);
        }
        let active = if matches!(c.kind, Kind::FlipFlop(_)) && c.sequential.negate_controls {
            Bit::Zero
        } else {
            Bit::One
        };
        let clear = ports[ports.len() - 1].bit(0) == active;
        let preset = ports[ports.len() - 2].bit(0) == active;
        if clear {
            self.value = Value::known(self.value.width(), 0);
        } else if preset {
            self.value = Value::filled(self.value.width(), Bit::One);
        } else if triggered {
            match c.kind {
                Kind::Register if ports[3].bit(0) != Bit::Zero && ports[1].number().is_some() => {
                    self.value = ports[1]
                }
                Kind::FlipFlop(ff) => {
                    if let Some(a) = ports[3].number() {
                        let old = self.value.number().unwrap_or(0);
                        let next = match ff {
                            FlipFlop::D => a,
                            FlipFlop::T => old ^ a,
                            FlipFlop::Jk | FlipFlop::Sr => match (a, ports[4].number()) {
                                (0, Some(1)) => 0,
                                (1, Some(0)) => 1,
                                (1, Some(1)) if ff == FlipFlop::Jk => old ^ 1,
                                _ => old,
                            },
                        };
                        self.value = Value::known(1, next);
                    }
                }
                _ => {}
            }
        }
        self.value
    }
}

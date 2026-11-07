//! Recurrence fitted to runtime output samples; only the observable 44 state bits are retained.
use crate::{Bit, Value};
use logisim_core::Component;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug)]
pub(super) struct State {
    sequence: u64,
    output: u32,
    clearing: bool,
}
impl State {
    pub fn new(seed: i32) -> Self {
        let sequence = if seed == 0 {
            // Automatic seeds are intentionally local to this run, not document state.
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64);
            time.wrapping_add(
                NEXT.fetch_add(1, Ordering::Relaxed)
                    .wrapping_mul(0x9e3779b9),
            )
        } else {
            i64::from(seed) as u64
        } & ((1 << 44) - 1);
        Self {
            sequence,
            output: sequence as u32,
            clearing: false,
        }
    }
    pub fn value(&self, bits: u8) -> Value {
        Value::known(bits, self.output)
    }
    pub fn evaluate(&mut self, c: &Component, ports: &[Value], triggered: bool) -> Value {
        let clear = ports[3].bit(0) == Bit::One;
        if clear {
            // Reseed once per assertion. This bounds automatic-seed propagation while
            // reset is held; a new assertion or simulation reset chooses a fresh seed.
            if !self.clearing {
                *self = Self::new(c.sequential.random_seed);
            }
        } else if triggered && ports[1].bit(0) != Bit::Zero {
            self.sequence =
                self.sequence.wrapping_mul(25_214_903_917).wrapping_add(11) & ((1 << 44) - 1);
            self.output = (self.sequence >> 12) as u32;
        }
        self.clearing = clear;
        self.value(c.bits)
    }
}

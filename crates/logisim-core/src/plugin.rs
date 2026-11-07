//! Dynamic components contain document data only; execution belongs to the host.
use crate::{Bit, Component, Kind, Point, Value};
pub use logisim_plugin_api as api;
impl Component {
    pub fn from_plugin(plugin: api::Component, position: Point) -> Self {
        let mut c = Self::new(Kind::Plugin, position);
        c.plugin = Some(plugin);
        c
    }
}
impl From<Value> for api::Signal {
    fn from(value: Value) -> Self {
        let mut s = Self {
            width: value.width(),
            value: 0,
            unknown: 0,
            error: 0,
        };
        for i in 0..value.width() {
            match value.bit(i) {
                Bit::One => s.value |= 1 << i,
                Bit::Unknown => s.unknown |= 1 << i,
                Bit::Error => s.error |= 1 << i,
                Bit::Zero => {}
            }
        }
        s
    }
}
impl From<api::Signal> for Value {
    fn from(s: api::Signal) -> Self {
        Self::from_bits((0..s.width.min(32)).map(|i| {
            let mask = 1 << i;
            if s.error & mask != 0 {
                Bit::Error
            } else if s.unknown & mask != 0 {
                Bit::Unknown
            } else if s.value & mask != 0 {
                Bit::One
            } else {
                Bit::Zero
            }
        }))
    }
}

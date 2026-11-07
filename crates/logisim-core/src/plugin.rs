//! Dynamic components contain document data only; execution belongs to the host.
use crate::{Bit, Component, Kind, Point, Value};
pub use logisim_plugin_api as api;
/// Document dependency, resolved by the desktop relative to the circuit's directory.
/// `fallback` normally retains the absolute location for files moved separately.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSource {
    pub plugin: String,
    pub version: String,
    pub path: String,
    #[serde(default)]
    pub fallback: String,
}
impl PluginSource {
    pub fn validate(&self) -> Result<(), String> {
        let identity = |s: &str| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        };
        if !identity(&self.plugin)
            || !identity(&self.version)
            || [&self.path, &self.fallback]
                .iter()
                .any(|p| p.len() > 4096 || p.contains('\0'))
        {
            return Err("Invalid plugin source.".into());
        }
        Ok(())
    }
}
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

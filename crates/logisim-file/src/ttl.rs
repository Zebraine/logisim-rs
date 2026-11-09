use crate::{Error, Result};
use logisim_core::{Component, Kind};
use std::collections::BTreeMap;

pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "facing" | "VccGndPorts" | "ShowInternalStructure" | "label" | "labelfont" | "labelcolor"
    )
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !matches!(c.kind, Kind::Ttl(_)) {
        return Ok(());
    }
    for (key, value) in [
        ("VccGndPorts", &mut c.ttl.power_pins),
        ("ShowInternalStructure", &mut c.ttl.internal),
    ] {
        *value = match a.get(key).map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(Error(format!("Invalid {key}"))),
        };
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    if matches!(c.kind, Kind::Ttl(_)) {
        if c.bits != 1 {
            return Err(Error("TTL pins must be one bit".into()));
        }
        crate::xml::attribute(out, "VccGndPorts", c.ttl.power_pins)?;
        crate::xml::attribute(out, "ShowInternalStructure", c.ttl.internal)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

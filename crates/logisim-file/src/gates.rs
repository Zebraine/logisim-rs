use crate::{Error, Result};
use logisim_core::{Component, Gate, Kind};
use std::collections::BTreeMap;

pub(crate) fn index(key: &str) -> Option<u8> {
    let suffix = key.strip_prefix("negate")?;
    let i = suffix.parse::<u8>().ok()?;
    (i < Gate::MAX_INPUTS && suffix == i.to_string()).then_some(i)
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    let Kind::Gate(gate) = c.kind else {
        return Ok(());
    };
    if matches!(gate, Gate::Xor | Gate::Xnor) {
        c.gate_options.xor_odd = match a.get("xor").map(String::as_str) {
            None => false,
            Some("odd") => true,
            _ => return Err(Error("Unsupported XOR mode".into())),
        };
    }
    for (key, value) in a {
        if let Some(i) = index(key) {
            if i >= c.input_count() {
                return Err(Error("Inversion refers to an inactive gate input".into()));
            }
            match value.as_str() {
                "false" => {}
                "true" => c.gate_options.negated |= 1 << i,
                _ => return Err(Error("Invalid gate input inversion".into())),
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute;
    let Kind::Gate(gate) = c.kind else {
        return Ok(());
    };
    if matches!(gate, Gate::Xor | Gate::Xnor) && c.gate_options.xor_odd {
        attribute(out, "xor", "odd")?;
    }
    if !gate.unary() {
        for i in 0..c.input_count() {
            if c.gate_negated(i) {
                attribute(out, &format!("negate{i}"), true)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

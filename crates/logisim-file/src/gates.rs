use crate::{Error, Result};
use logisim_core::{Component, Gate, GateSize, Kind};
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
    c.gate_options.inputs = crate::attributes::number(a, "inputs", 2, 2..=Gate::MAX_INPUTS)?;
    c.gate_options.size = match (
        gate,
        crate::attributes::value(a, "size", if gate.unary() { "20" } else { "30" }),
    ) {
        (Gate::Not | Gate::Buffer, "20") => GateSize::Narrow,
        (_, "30") if !gate.unary() => GateSize::Narrow,
        (Gate::Not, "30") => GateSize::Wide,
        (_, "50") if !gate.unary() => GateSize::Medium,
        (_, "70") if !gate.unary() => GateSize::Wide,
        _ => return Err(Error("Unsupported gate size".into())),
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
    if !(2..=Gate::MAX_INPUTS).contains(&c.gate_options.inputs) {
        return Err(Error("Invalid gate input count".into()));
    }
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

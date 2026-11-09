use crate::{Error, Result};
use logisim_core::{Component, GateSize, Kind, ParityOutput};
use std::collections::BTreeMap;

fn negated_index(key: &str) -> Option<u8> {
    let suffix = key.strip_prefix("negate")?;
    let n = suffix.parse::<u8>().ok()?;
    (n < 32 && suffix == n.to_string()).then_some(n)
}
pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "facing" | "width" | "size" | "inputs" | "out" | "label" | "labelfont" | "labelcolor"
    ) || negated_index(key).is_some()
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !matches!(c.kind, Kind::Parity(_)) {
        return Ok(());
    }
    c.inputs = crate::attributes::number(a, "inputs", 2, 2..=32)?;
    c.gate_size = match a.get("size").map(String::as_str) {
        None | Some("30") => GateSize::Narrow,
        Some("50") => GateSize::Medium,
        Some("70") => GateSize::Wide,
        _ => return Err(Error("Unsupported parity gate size".into())),
    };
    c.parity.output = match a.get("out").map(String::as_str) {
        None => ParityOutput::Normal,
        Some("0Z") => ParityOutput::ZeroFloating,
        Some("Z1") => ParityOutput::FloatingOne,
        _ => return Err(Error("Unsupported parity output mode".into())),
    };
    for (key, value) in a {
        if let Some(i) = negated_index(key) {
            if i >= c.inputs {
                return Err(Error("Inversion refers to an inactive parity input".into()));
            }
            match value.as_str() {
                "false" => {}
                "true" => c.parity.negated |= 1 << i,
                _ => return Err(Error("Invalid parity input inversion".into())),
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute as a;
    if !matches!(c.kind, Kind::Parity(_)) {
        return Ok(());
    }
    a(out, "size", c.gate_width())?;
    a(out, "inputs", c.inputs)?;
    match c.parity.output {
        ParityOutput::Normal => {}
        ParityOutput::ZeroFloating => a(out, "out", "0Z")?,
        ParityOutput::FloatingOne => a(out, "out", "Z1")?,
    }
    for i in 0..c.input_count() {
        if c.parity_negated(i) {
            a(out, &format!("negate{i}"), true)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

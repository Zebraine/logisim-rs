use crate::{Error, Result};
use logisim_core::{Component, Kind, TransistorType};
use std::collections::BTreeMap;

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    matches!(key, "facing" | "width" | "gate") || (kind == Kind::Transistor && key == "type")
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !c.kind.is_transistor() {
        return Ok(());
    }
    c.transistor.gate_bottom_right = match a.get("gate").map(String::as_str) {
        None | Some("tl") => false,
        Some("br") => true,
        _ => return Err(Error("Unsupported transistor gate location".into())),
    };
    if c.kind == Kind::Transistor {
        c.transistor.transistor_type = match a.get("type").map(String::as_str) {
            None | Some("p") => TransistorType::P,
            Some("n") => TransistorType::N,
            _ => return Err(Error("Unsupported transistor type".into())),
        };
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    use crate::xml::attribute as a;
    if !c.kind.is_transistor() {
        return Ok(());
    }
    a(
        out,
        "gate",
        if c.transistor.gate_bottom_right {
            "br"
        } else {
            "tl"
        },
    )?;
    if c.kind == Kind::Transistor {
        a(
            out,
            "type",
            match c.transistor.transistor_type {
                TransistorType::P => "p",
                TransistorType::N => "n",
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

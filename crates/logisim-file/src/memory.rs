use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind, MemoryContents, MemoryInterface};
use std::{collections::BTreeMap, fmt::Write};

pub(crate) fn allowed(kind: Kind, key: &str) -> bool {
    matches!(
        key,
        "addrWidth" | "dataWidth" | "label" | "labelfont" | "labelcolor" | "Select" | "simpleMode"
    ) || (kind == Kind::Rom && key == "contents")
        || (kind == Kind::Ram && key == "bus")
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if !c.kind.is_memory() {
        return Ok(());
    }
    c.bits = crate::attributes::number(a, "dataWidth", 8, 1..=32)?;
    c.memory.address_bits = crate::attributes::number(a, "addrWidth", 8, 2..=24)?;
    c.memory.select_high = match a.get("Select").map(String::as_str) {
        None | Some("low") => false,
        Some("high") => true,
        _ => return Err(Error("Unsupported memory select polarity".into())),
    };
    c.memory.simple = match a.get("simpleMode").map(String::as_str) {
        None | Some("true") => true,
        Some("false") => false,
        _ => return Err(Error("Invalid memory simplified mode".into())),
    };
    c.memory.interface = match a.get("bus").map(String::as_str) {
        None => MemoryInterface::SharedSync,
        Some("asynch") => MemoryInterface::SharedAsync,
        Some("separate") => MemoryInterface::Separate,
        _ => return Err(Error("Unsupported memory data interface".into())),
    };
    if let Some(contents) = a.get("contents") {
        c.memory.contents = parse_contents(contents, c.memory.address_bits, c.bits)?;
    }
    Ok(())
}
fn parse_contents(text: &str, address_bits: u8, data_bits: u8) -> Result<MemoryContents> {
    let invalid = || Error("Invalid ROM contents header, word or run length".into());
    let mut tokens = text.split_whitespace();
    if tokens.next() != Some("addr/data:")
        || tokens.next().and_then(|v| v.parse::<u8>().ok()) != Some(address_bits)
        || tokens.next().and_then(|v| v.parse::<u8>().ok()) != Some(data_bits)
    {
        return Err(invalid());
    }
    let mut contents = MemoryContents::default();
    let mut address = 0u32;
    let mask = u32::MAX >> (32 - data_bits);
    for token in tokens {
        let (count, word) = if let Some((n, word)) = token.split_once('*') {
            (
                n.parse::<u32>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(invalid)?,
                word,
            )
        } else {
            (1, token)
        };
        if word.is_empty() || !word.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        let value = u32::from_str_radix(word, 16)
            .ok()
            .filter(|v| *v <= mask)
            .ok_or_else(invalid)?;
        let end = address
            .checked_add(count)
            .filter(|e| *e <= 1 << address_bits)
            .ok_or_else(invalid)?;
        if value != 0 {
            contents.fill(address, end, value);
        }
        address = end;
    }
    if address == 0 {
        return Err(invalid());
    }
    Ok(contents)
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    if !c.kind.is_memory() {
        return Ok(());
    }
    if c.facing != Facing::East || !(2..=24).contains(&c.memory.address_bits) {
        return Err(Error(
            "Unsupported memory orientation or address width".into(),
        ));
    }
    use crate::xml::attribute as a;
    a(out, "addrWidth", c.memory.address_bits)?;
    a(out, "dataWidth", c.bits)?;
    if c.memory.select_high {
        a(out, "Select", "high")?;
    }
    if !c.memory.simple {
        a(out, "simpleMode", false)?;
    }
    if c.kind == Kind::Ram {
        if c.memory.contents.boundaries().next().is_some() {
            return Err(Error(
                "RAM runtime contents cannot be saved in a circuit".into(),
            ));
        }
        match c.memory.interface {
            MemoryInterface::SharedSync => {}
            MemoryInterface::SharedAsync => a(out, "bus", "asynch")?,
            MemoryInterface::Separate => a(out, "bus", "separate")?,
        }
    } else {
        if c.memory.interface != MemoryInterface::SharedSync {
            return Err(Error("ROM cannot have a RAM data interface".into()));
        }
        let mut cursor = 0;
        let mut previous = 0;
        let mask = u32::MAX >> (32 - c.bits);
        let mut data = String::new();
        for (address, value) in c.memory.contents.boundaries() {
            if address > c.memory_capacity() || value > mask {
                return Err(Error("ROM contents exceed memory dimensions".into()));
            }
            let count = address - cursor;
            if count > 0 {
                if !data.is_empty() {
                    data.push(' ');
                }
                if count > 1 {
                    write!(data, "{count}*").unwrap();
                }
                write!(data, "{previous:x}").unwrap();
            }
            cursor = address;
            previous = value;
        }
        if previous != 0 {
            return Err(Error("ROM contents exceed memory capacity".into()));
        }
        if data.is_empty() {
            data.push('0');
        }
        writeln!(
            out,
            "      <a name=\"contents\">addr/data: {} {}\n{data}\n</a>",
            c.memory.address_bits, c.bits
        )
        .unwrap();
    }
    Ok(())
}

#[cfg(test)]
mod tests;

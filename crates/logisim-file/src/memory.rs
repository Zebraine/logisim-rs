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
    c.bits = crate::read::number(a, "dataWidth", 8, 1..=32)?;
    c.memory.address_bits = crate::read::number(a, "addrWidth", 8, 2..=24)?;
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
    use crate::write::attribute as a;
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
mod tests {
    use super::*;
    use crate::{Document, parse, serialize};
    use logisim_core::{Circuit, Point};
    fn xml(attributes: &str, kind: &str) -> String {
        format!(
            r##"<project source="2.16.2.2.jar" version="1.0"><lib name="m" desc="#Memory"/><main name="main"/><circuit name="main"><comp lib="m" name="{kind}" loc="(200,100)">{attributes}</comp></circuit></project>"##
        )
    }
    #[test]
    fn runtime_defaults_rle_and_memory_interfaces_roundtrip() {
        for kind in [Kind::Rom, Kind::Ram] {
            let imported = parse(&xml("", kind.name())).unwrap();
            assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
            let c = &imported.document.main_circuit().circuit.components[0];
            assert_eq!(
                (
                    c.bits,
                    c.memory.address_bits,
                    c.memory.simple,
                    c.memory.select_high
                ),
                (8, 8, true, false)
            );
            for interface in MemoryInterface::ALL {
                for simple in [false, true] {
                    for high in [false, true] {
                        if kind == Kind::Rom && interface != MemoryInterface::SharedSync {
                            continue;
                        }
                        let mut c = Component::new(kind, Point::new(200, 100));
                        c.bits = 32;
                        c.memory.address_bits = 24;
                        c.memory.interface = interface;
                        c.memory.simple = simple;
                        c.memory.select_high = high;
                        c.label = "程序 & 数据".into();
                        if kind == Kind::Rom {
                            c.memory.contents.fill(0, 4, 0xabcdef01);
                            c.memory.contents.set(0xff_ffff, 0xdead_beef);
                        }
                        let mut circuit = Circuit::default();
                        let id = circuit.add_component(c);
                        let doc = Document::single("main", circuit);
                        let text = serialize(&doc).unwrap();
                        if kind == Kind::Rom {
                            assert!(text.contains("4*abcdef01 16777211*0 deadbeef"));
                        } else {
                            assert!(!text.contains("contents"));
                        }
                        let imported = parse(&text).unwrap();
                        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                        let before = doc.main_circuit().circuit.component(id).unwrap();
                        let mut after =
                            imported.document.main_circuit().circuit.components[0].clone();
                        after.id = id;
                        assert_eq!(before, &after);
                        assert_eq!(serialize(&imported.document).unwrap(), text);
                    }
                }
            }
        }
        let contents = parse_contents(
            "addr/data: 24 32\n80000001 ffffffff 12345678 16777212*0 deadbeef",
            24,
            32,
        )
        .unwrap();
        assert_eq!(
            (
                contents.get(0),
                contents.get(1),
                contents.get(2),
                contents.get(3),
                contents.get(0xff_ffff)
            ),
            (0x80000001, u32::MAX, 0x12345678, 0, 0xdeadbeef)
        );
        assert_eq!(
            parse_contents("addr/data: 24 32\n16777216*ffffffff", 24, 32)
                .unwrap()
                .boundaries()
                .count(),
            2
        );
    }
    #[test]
    fn invalid_or_unimplemented_memory_data_is_omitted_and_protected() {
        for (kind, attrs) in [
            ("ROM", r#"<a name="addrWidth" val="1"/>"#),
            ("RAM", r#"<a name="dataWidth" val="33"/>"#),
            ("RAM", r#"<a name="bus" val="unknown"/>"#),
            ("RAM", r#"<a name="trigger" val="falling"/>"#),
            (
                "RAM",
                r#"<a name="contents">addr/data: 8 8
ff</a>"#,
            ),
            (
                "ROM",
                r#"<a name="contents">addr/data: 9 8
ff</a>"#,
            ),
            (
                "ROM",
                r#"<a name="contents">addr/data: 8 8
257*0</a>"#,
            ),
            (
                "ROM",
                r#"<a name="contents">addr/data: 8 8
0*ff</a>"#,
            ),
            (
                "ROM",
                r#"<a name="contents">addr/data: 8 8
100</a>"#,
            ),
            (
                "ROM",
                r#"<a name="contents">addr/data: 8 8
</a>"#,
            ),
            (
                "ROM",
                r#"<a name="contents">addr/data: 8 8
+1</a>"#,
            ),
        ] {
            let imported = parse(&xml(attrs, kind)).unwrap();
            assert!(!imported.warnings.is_empty(), "{attrs}");
            assert!(
                imported
                    .document
                    .main_circuit()
                    .circuit
                    .components
                    .is_empty()
            );
        }
        let mut c = Component::new(Kind::Ram, Point::ZERO);
        c.memory.contents.set(0, 1);
        let mut circuit = Circuit::default();
        circuit.add_component(c);
        assert!(serialize(&Document::single("main", circuit)).is_err());
    }
}

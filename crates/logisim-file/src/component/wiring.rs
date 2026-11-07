//! Wiring-family file attributes.
use crate::{
    Error, Result,
    attributes::{Attributes, number, value},
};
use logisim_core::{Component, Facing, Kind};

pub(super) fn read(c: &mut Component, a: &Attributes) -> Result<()> {
    use logisim_core::{Extension, Radix, SplitAppearance};
    match c.kind {
        Kind::Constant => {
            let text = value(a, "value", "0x1");
            c.constant = if let Some(hex) = text.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).ok()
            } else if text == "0"
                || (!text.starts_with('0') && text.bytes().all(|b| b.is_ascii_digit()))
            {
                text.parse::<u32>().ok()
            } else {
                None
            }
            .ok_or_else(|| Error("Invalid constant value".into()))?;
            if c.bits < 32 {
                c.constant &= (1 << c.bits) - 1;
            }
        }
        Kind::Probe => {
            if let Some(width) = a.get("width") {
                if width != "16" {
                    return Err(Error("Unsupported probe width attribute".into()));
                }
                // The reference infers the bus width and drops this legacy attribute on save.
                c.bits = 1;
            }
            c.radix = match value(a, "radix", "2") {
                "2" => Radix::Binary,
                "8" => Radix::Octal,
                "10signed" => Radix::Signed,
                "10unsigned" => Radix::Unsigned,
                "16" => Radix::Hex,
                _ => return Err(Error("Unsupported probe radix".into())),
            };
            c.label_facing = match value(a, "labelloc", "north") {
                "north" => Facing::North,
                "south" => Facing::South,
                "east" => Facing::East,
                "west" => Facing::West,
                _ => return Err(Error("Invalid probe label location".into())),
            };
        }
        Kind::BitExtender => {
            c.input_bits = number(a, "in_width", 8, 1..=32)?;
            c.bits = number(a, "out_width", 16, 1..=32)?;
            c.extension = match value(a, "type", "zero") {
                "zero" => Extension::Zero,
                "one" => Extension::One,
                "sign" => Extension::Sign,
                "input" => Extension::Input,
                _ => return Err(Error("Unsupported extension type".into())),
            };
        }
        Kind::Splitter => {
            c.bits = number(a, "incoming", 4, 1..=32)?;
            c.splitter.fanout = number(a, "fanout", 4, 1..=32)?;
            c.splitter.appearance = match value(a, "appear", "left") {
                "left" => SplitAppearance::Left,
                "right" => SplitAppearance::Right,
                "center" => SplitAppearance::Center,
                "legacy" => SplitAppearance::Legacy,
                _ => return Err(Error("Unsupported splitter appearance".into())),
            };
            let descending = match value(a, "order", "ascending") {
                "ascending" => false,
                "descending" => true,
                _ => return Err(Error("Unsupported splitter distribution".into())),
            };
            c.splitter.distribute(c.bits, descending);
            // Runtime loading differs from the GUI's distribute action for omitted bitN values.
            for i in 0..c.bits.min(c.splitter.fanout) {
                c.splitter.mapping[usize::from(i)] = Some(i);
            }
            for (key, val) in a.iter().filter(|(k, _)| k.starts_with("bit")) {
                let bit: usize = key[3..]
                    .parse()
                    .map_err(|_| Error("Invalid splitter bit".into()))?;
                if bit >= usize::from(c.bits) {
                    return Err(Error("Splitter mapping exceeds incoming width".into()));
                }
                c.splitter.mapping[bit] = if val == "none" {
                    None
                } else {
                    Some(
                        val.parse::<u8>()
                            .ok()
                            .filter(|n| *n < c.splitter.fanout)
                            .ok_or_else(|| Error("Invalid splitter branch".into()))?,
                    )
                };
            }
        }
        _ => {}
    }
    Ok(())
}

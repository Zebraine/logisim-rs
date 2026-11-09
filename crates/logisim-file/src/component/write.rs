//! Serialize one supported component; family-specific rules remain in their codecs.
use crate::{
    Document, Error, Result, valid_point,
    xml::{attribute, escape},
};
use logisim_core::{Component, Gate, HorizontalAlign, Kind, Radix, SplitAppearance, VerticalAlign};
use std::fmt::Write;

pub(crate) fn write(out: &mut String, c: &Component, document: &Document) -> Result<()> {
    if !valid_point(c.position)
        || !(1..=32).contains(&c.bits)
        || c.font.size == 0
        || c.label.len() > 65_536
        || (c.kind != Kind::Text && c.color_alpha != 255)
        || (c.kind != Kind::Button && c.label_center)
    {
        return Err(Error(format!("Invalid {} attributes.", c.kind.name())));
    }
    if c.kind == Kind::Plugin {
        return crate::plugin::write(out, c);
    }
    let lib = match c.kind {
        Kind::Ttl(_) => 7,
        Kind::Plexer(_) => 4,
        Kind::Arithmetic(_) => 5,
        Kind::Button
        | Kind::Switch
        | Kind::DipSwitch
        | Kind::Slider
        | Kind::Joystick
        | Kind::Keyboard
        | Kind::Tty
        | Kind::Led
        | Kind::SevenSegment
        | Kind::RgbLed
        | Kind::DotMatrix
        | Kind::HexDisplay => 6,
        Kind::Gate(_) | Kind::Parity(_) | Kind::ControlledBuffer | Kind::ControlledInverter => 1,
        Kind::Text => 2,
        Kind::Register
        | Kind::FlipFlop(_)
        | Kind::Latch(_)
        | Kind::Counter
        | Kind::Random
        | Kind::ShiftRegister
        | Kind::Rom
        | Kind::PlaRom
        | Kind::Ram => 3,
        _ => 0,
    };
    if let Kind::Subcircuit(id) = c.kind {
        let name = escape(
            &document
                .circuit(id)
                .ok_or_else(|| Error("Missing subcircuit".into()))?
                .name,
        )?;
        writeln!(
            out,
            "    <comp loc=\"({},{})\" name=\"{name}\">",
            c.position.x, c.position.y
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "    <comp lib=\"{lib}\" loc=\"({},{})\" name=\"{}\">",
            c.position.x,
            c.position.y,
            if let Kind::Arithmetic(a) = c.kind {
                a.file_name()
            } else if let Kind::Plexer(p) = c.kind {
                p.file_name()
            } else if c.kind == Kind::PlaRom {
                "PlaRom"
            } else if c.kind == Kind::RgbLed {
                "RGBLed"
            } else if c.kind == Kind::DotMatrix {
                "DotMatrix"
            } else if c.kind == Kind::DipSwitch {
                "DipSwitch"
            } else if c.kind == Kind::Random {
                "Random"
            } else if c.kind == Kind::ShiftRegister {
                "Universal Register"
            } else {
                c.kind.name()
            }
        )
        .unwrap();
    }
    if c.kind.rotatable() {
        attribute(out, "facing", c.facing.name().to_ascii_lowercase())?;
    }
    if !matches!(
        c.kind,
        Kind::Text
            | Kind::Ttl(_)
            | Kind::Probe
            | Kind::PullResistor
            | Kind::Splitter
            | Kind::BitExtender
            | Kind::Subcircuit(_)
            | Kind::Clock
            | Kind::FlipFlop(_)
            | Kind::Latch(_)
            | Kind::ShiftRegister
            | Kind::Button
            | Kind::DipSwitch
            | Kind::Joystick
            | Kind::Keyboard
            | Kind::Tty
            | Kind::Led
            | Kind::SevenSegment
            | Kind::RgbLed
            | Kind::DotMatrix
            | Kind::HexDisplay
            | Kind::Rom
            | Kind::PlaRom
            | Kind::Ram
            | Kind::Plexer(
                logisim_core::Plexer::Decoder
                    | logisim_core::Plexer::PriorityEncoder
                    | logisim_core::Plexer::DisplayDecoder
            )
    ) {
        attribute(out, "width", c.bits)?;
    }
    crate::ttl::write(out, c)?;
    crate::latches::write(out, c)?;
    crate::parity::write(out, c)?;
    crate::gates::write(out, c)?;
    crate::bus::write(out, c)?;
    crate::transistors::write(out, c)?;
    crate::sequential::write(out, c)?;
    crate::memory::write(out, c)?;
    crate::pla::write(out, c)?;
    crate::io::write(out, c)?;
    crate::plexers::write(out, c)?;
    crate::arithmetic::write(out, c)?;
    if let Kind::Gate(g) = c.kind {
        if g != Gate::Buffer {
            attribute(out, "size", c.gate_width())?;
        }
        if !g.unary() {
            attribute(out, "inputs", c.gate_options.inputs)?;
        }
    }
    if c.kind == Kind::Pin {
        attribute(out, "output", c.output)?;
        attribute(out, "tristate", c.pin.three_state)?;
        attribute(
            out,
            "pull",
            match c.pin.pull {
                logisim_core::PinPull::None => "none",
                logisim_core::PinPull::Up => "up",
                logisim_core::PinPull::Down => "down",
            },
        )?;
        attribute(out, "labelloc", c.label_facing.name().to_ascii_lowercase())?;
    }
    match c.kind {
        Kind::Constant => {
            if c.bits < 32 && c.constant >= (1 << c.bits) {
                return Err(Error("Constant value exceeds width".into()));
            }
            attribute(out, "value", format!("0x{:x}", c.constant))?;
        }
        Kind::Subcircuit(_) => {
            attribute(out, "labelloc", c.label_facing.name().to_ascii_lowercase())?;
        }
        Kind::Probe => {
            attribute(
                out,
                "radix",
                match c.radix {
                    Radix::Binary => "2",
                    Radix::Octal => "8",
                    Radix::Signed => "10signed",
                    Radix::Unsigned => "10unsigned",
                    Radix::Hex => "16",
                },
            )?;
            attribute(out, "labelloc", c.label_facing.name().to_ascii_lowercase())?;
        }
        Kind::BitExtender => {
            if !(1..=32).contains(&c.input_bits) {
                return Err(Error("Invalid extender input width".into()));
            }
            attribute(out, "in_width", c.input_bits)?;
            attribute(out, "out_width", c.bits)?;
            attribute(out, "type", c.extension.name().to_ascii_lowercase())?;
        }
        Kind::Splitter => {
            if !(1..=32).contains(&c.splitter.fanout) {
                return Err(Error("Invalid splitter fanout".into()));
            }
            attribute(out, "fanout", c.splitter.fanout)?;
            attribute(out, "incoming", c.bits)?;
            if c.splitter.descending {
                attribute(out, "order", "descending")?;
            }
            attribute(
                out,
                "appear",
                match c.splitter.appearance {
                    SplitAppearance::Left => "left",
                    SplitAppearance::Right => "right",
                    SplitAppearance::Center => "center",
                    SplitAppearance::Legacy => "legacy",
                },
            )?;
            for bit in 0..c.bits {
                let branch = c.splitter.mapping[usize::from(bit)];
                if branch.is_some_and(|b| b >= c.splitter.fanout) {
                    return Err(Error("Invalid splitter mapping".into()));
                }
                attribute(
                    out,
                    &format!("bit{bit}"),
                    branch.map_or_else(|| "none".into(), |b| b.to_string()),
                )?;
            }
        }
        _ => {}
    }
    if !c.kind.has_label() {
        out.push_str("    </comp>\n");
        return Ok(());
    }
    attribute(
        out,
        if c.kind == Kind::Text {
            "text"
        } else {
            "label"
        },
        &c.label,
    )?;
    attribute(
        out,
        if c.kind == Kind::Text {
            "font"
        } else {
            "labelfont"
        },
        crate::fonts::serialize(&c.font)?,
    )?;
    if c.kind != Kind::Tunnel {
        attribute(
            out,
            if c.kind == Kind::Text {
                "color"
            } else {
                "labelcolor"
            },
            crate::io::color_text([c.color[0], c.color[1], c.color[2], c.color_alpha]),
        )?;
    }
    if c.kind == Kind::Text {
        attribute(
            out,
            "halign",
            match c.text_horizontal {
                HorizontalAlign::Left => "left",
                HorizontalAlign::Center => "center",
                HorizontalAlign::Right => "right",
            },
        )?;
        attribute(
            out,
            "valign",
            match c.text_vertical {
                VerticalAlign::Top => "top",
                VerticalAlign::Center => "center",
                VerticalAlign::Baseline => "base",
                VerticalAlign::Bottom => "bottom",
            },
        )?;
    }
    out.push_str("    </comp>\n");
    Ok(())
}

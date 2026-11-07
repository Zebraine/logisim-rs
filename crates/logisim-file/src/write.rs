use super::{Document, Error, MAX_BYTES, MAX_CIRCUIT_ITEMS, MAX_ITEMS, Result, valid_point};
use logisim_core::{Gate, HorizontalAlign, Kind, Radix, SplitAppearance, VerticalAlign};
use std::fmt::Write;

pub(crate) fn escape(s: &str) -> Result<String> {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if c >= '\u{20}' && c != '\u{fffe}' && c != '\u{ffff}' => out.push(c),
            _ => {
                return Err(Error(
                    "Text contains a character XML cannot represent.".into(),
                ));
            }
        }
    }
    Ok(out)
}
pub(crate) fn attribute(out: &mut String, name: &str, value: impl std::fmt::Display) -> Result<()> {
    writeln!(
        out,
        "      <a name=\"{name}\" val=\"{}\"/>",
        escape(&value.to_string())?
    )
    .unwrap();
    Ok(())
}
pub fn serialize(document: &Document) -> Result<String> {
    document.validate().map_err(Error)?;
    if document
        .circuits
        .iter()
        .map(|c| c.circuit.components.len() + c.circuit.wires.len())
        .sum::<usize>()
        > MAX_ITEMS
    {
        return Err(Error(format!("Project exceeds {MAX_ITEMS} items.")));
    }
    for definition in &document.circuits {
        if definition.circuit.components.len() + definition.circuit.wires.len() > MAX_CIRCUIT_ITEMS
        {
            return Err(Error(format!(
                "Circuit '{}' exceeds {MAX_CIRCUIT_ITEMS} items.",
                definition.name
            )));
        }
    }
    let name = escape(&document.main_circuit().name)?;
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- Written by logisim-rs; source declares the compatible Logisim-ITA format. -->\n<project source=\"2.16.2.2.jar\" version=\"1.0\">\n  <lib desc=\"#Wiring\" name=\"0\"/>\n  <lib desc=\"#Gates\" name=\"1\"/>\n  <lib desc=\"#Base\" name=\"2\"/>\n  <lib desc=\"#Memory\" name=\"3\"/>\n  <lib desc=\"#Plexers\" name=\"4\"/>\n  <lib desc=\"#Arithmetic\" name=\"5\"/>\n  <lib desc=\"#I/O\" name=\"6\"/>\n  <lib desc=\"#TTL\" name=\"7\"/>\n",
    );
    let external_libraries = crate::external::libraries(document, &mut out)?;
    writeln!(out, "  <main name=\"{name}\"/>").unwrap();
    out.push_str("  <options>\n    <a name=\"gateUndefined\" val=\"ignore\"/>\n    <a name=\"simlimit\" val=\"1000\"/>\n    <a name=\"simrand\" val=\"0\"/>\n  </options>\n  <mappings>\n    <tool lib=\"2\" map=\"Button2\" name=\"Poke Tool\"/>\n    <tool lib=\"2\" map=\"Button3\" name=\"Menu Tool\"/>\n    <tool lib=\"2\" map=\"Ctrl Button1\" name=\"Menu Tool\"/>\n  </mappings>\n  <toolbar>\n    <tool lib=\"2\" name=\"Poke Tool\"/>\n    <tool lib=\"2\" name=\"Edit Tool\"/>\n    <sep/>\n    <tool lib=\"2\" name=\"Text Tool\"/>\n    <tool lib=\"0\" name=\"Pin\"><a name=\"tristate\" val=\"false\"/></tool>\n    <tool lib=\"0\" name=\"Pin\"><a name=\"output\" val=\"true\"/><a name=\"facing\" val=\"west\"/></tool>\n    <tool lib=\"1\" name=\"NOT Gate\"/>\n    <tool lib=\"1\" name=\"AND Gate\"/>\n    <tool lib=\"1\" name=\"OR Gate\"/>\n  </toolbar>\n");
    for definition in &document.circuits {
        let circuit = &definition.circuit;
        let name = escape(&definition.name)?;
        writeln!(out, "  <circuit name=\"{name}\">").unwrap();
        for w in &circuit.wires {
            if !valid_point(w.a)
                || !valid_point(w.b)
                || w.a == w.b
                || (w.a.x != w.b.x && w.a.y != w.b.y)
            {
                return Err(Error("Invalid wire coordinates.".into()));
            }
            writeln!(
                out,
                "    <wire from=\"({},{})\" to=\"({},{})\"/>",
                w.a.x, w.a.y, w.b.x, w.b.y
            )
            .unwrap();
        }
        for c in &circuit.components {
            if !valid_point(c.position)
                || !(1..=32).contains(&c.bits)
                || !(2..=if matches!(c.kind, Kind::Parity(_)) {
                    32
                } else {
                    8
                })
                    .contains(&c.inputs)
                || c.font_size == 0
                || c.label.len() > 65_536
            {
                return Err(Error(format!("Invalid {} attributes.", c.kind.name())));
            }
            if c.kind.is_external() {
                crate::external::write(&mut out, c, external_libraries[&c.external_library])?;
                continue;
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
                Kind::Gate(_)
                | Kind::Parity(_)
                | Kind::ControlledBuffer
                | Kind::ControlledInverter => 1,
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
                attribute(&mut out, "facing", c.facing.name().to_ascii_lowercase())?;
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
                attribute(&mut out, "width", c.bits)?;
            }
            crate::ttl::write(&mut out, c)?;
            crate::latches::write(&mut out, c)?;
            crate::parity::write(&mut out, c)?;
            crate::bus::write(&mut out, c)?;
            crate::transistors::write(&mut out, c)?;
            crate::sequential::write(&mut out, c)?;
            crate::memory::write(&mut out, c)?;
            crate::pla::write(&mut out, c)?;
            crate::io::write(&mut out, c)?;
            crate::plexers::write(&mut out, c)?;
            crate::arithmetic::write(&mut out, c)?;
            if let Kind::Gate(g) = c.kind {
                if g != Gate::Buffer {
                    attribute(&mut out, "size", c.gate_width())?;
                }
                if !g.unary() {
                    attribute(&mut out, "inputs", c.inputs)?;
                }
            }
            if c.kind == Kind::Pin {
                attribute(&mut out, "output", c.output)?;
                attribute(&mut out, "tristate", false)?;
            }
            match c.kind {
                Kind::Constant => {
                    if c.bits < 32 && c.constant >= (1 << c.bits) {
                        return Err(Error("Constant value exceeds width".into()));
                    }
                    attribute(&mut out, "value", format!("0x{:x}", c.constant))?;
                }
                Kind::Subcircuit(_) => {
                    attribute(
                        &mut out,
                        "labelloc",
                        c.label_facing.name().to_ascii_lowercase(),
                    )?;
                }
                Kind::Probe => {
                    attribute(
                        &mut out,
                        "radix",
                        match c.radix {
                            Radix::Binary => "2",
                            Radix::Octal => "8",
                            Radix::Signed => "10signed",
                            Radix::Unsigned => "10unsigned",
                            Radix::Hex => "16",
                        },
                    )?;
                    attribute(
                        &mut out,
                        "labelloc",
                        c.label_facing.name().to_ascii_lowercase(),
                    )?;
                }
                Kind::BitExtender => {
                    if !(1..=32).contains(&c.input_bits) {
                        return Err(Error("Invalid extender input width".into()));
                    }
                    attribute(&mut out, "in_width", c.input_bits)?;
                    attribute(&mut out, "out_width", c.bits)?;
                    attribute(&mut out, "type", c.extension.name().to_ascii_lowercase())?;
                }
                Kind::Splitter => {
                    if !(1..=32).contains(&c.splitter.fanout) {
                        return Err(Error("Invalid splitter fanout".into()));
                    }
                    attribute(&mut out, "fanout", c.splitter.fanout)?;
                    attribute(&mut out, "incoming", c.bits)?;
                    if c.splitter.descending {
                        attribute(&mut out, "order", "descending")?;
                    }
                    attribute(
                        &mut out,
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
                            &mut out,
                            &format!("bit{bit}"),
                            branch.map_or_else(|| "none".into(), |b| b.to_string()),
                        )?;
                    }
                }
                _ => {}
            }
            if !c.kind.has_label() {
                out.push_str("    </comp>\n");
                continue;
            }
            attribute(
                &mut out,
                if c.kind == Kind::Text {
                    "text"
                } else {
                    "label"
                },
                &c.label,
            )?;
            attribute(
                &mut out,
                if c.kind == Kind::Text {
                    "font"
                } else {
                    "labelfont"
                },
                format!("Dialog plain {}", c.font_size),
            )?;
            if c.kind != Kind::Tunnel {
                attribute(
                    &mut out,
                    if c.kind == Kind::Text {
                        "color"
                    } else {
                        "labelcolor"
                    },
                    format!("#{:02x}{:02x}{:02x}", c.color[0], c.color[1], c.color[2]),
                )?;
            }
            if c.kind == Kind::Text {
                attribute(
                    &mut out,
                    "halign",
                    match c.text_horizontal {
                        HorizontalAlign::Left => "left",
                        HorizontalAlign::Center => "center",
                        HorizontalAlign::Right => "right",
                    },
                )?;
                attribute(
                    &mut out,
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
        }
        if let Some(appearance) = &circuit.appearance {
            crate::appearance::write(&mut out, appearance, circuit)?;
        }
        out.push_str("  </circuit>\n");
    }
    out.push_str("</project>\n");
    if out.len() > MAX_BYTES {
        return Err(Error("Serialized file exceeds 8 MiB.".into()));
    }
    Ok(out)
}

//! Identity and attribute dispatch for one component.
use crate::{
    Error, Result,
    attributes::{boolean, number, value},
    xml::{attr, attributes, point},
};
use logisim_core::{Component, Facing, Gate, HorizontalAlign, Kind, VerticalAlign};
use roxmltree::Node;
use std::collections::BTreeMap;

pub(crate) fn read(
    n: Node<'_, '_>,
    libraries: &BTreeMap<&str, &str>,
    names: &BTreeMap<String, logisim_core::Id>,
) -> Result<(Component, Vec<String>)> {
    let lib = n
        .attribute("lib")
        .and_then(|id| libraries.get(id))
        .copied()
        .unwrap_or("");
    let name = attr(n, "name")?;
    if lib == crate::plugin::DESCRIPTOR {
        return Ok((crate::plugin::read(n)?, vec![]));
    }
    let kind = match (lib, name) {
        ("", name) if n.attribute("lib").is_none() && names.contains_key(name) => {
            Kind::Subcircuit(names[name])
        }
        ("#TTL", name) => Kind::Ttl(
            logisim_core::Ttl::ALL
                .into_iter()
                .find(|t| t.name() == name)
                .ok_or_else(|| Error(format!("Unsupported TTL component: {name}")))?,
        ),
        ("#Arithmetic", name) => Kind::Arithmetic(
            logisim_core::Arithmetic::ALL
                .into_iter()
                .find(|a| a.file_name() == name)
                .ok_or_else(|| Error(format!("Unsupported arithmetic component: {name}")))?,
        ),
        ("#Plexers", name) => Kind::Plexer(
            logisim_core::Plexer::ALL
                .into_iter()
                .find(|p| p.file_name() == name)
                .ok_or_else(|| Error(format!("Unsupported plexer: {name}")))?,
        ),
        ("#Wiring", "Clock") => Kind::Clock,
        ("#Memory", "Register") => Kind::Register,
        ("#Memory", "Counter") => Kind::Counter,
        ("#Memory", "Random") => Kind::Random,
        ("#I/O", "Button") => Kind::Button,
        ("#I/O", "Switch") => Kind::Switch,
        ("#I/O", "DipSwitch") => Kind::DipSwitch,
        ("#I/O", "Slider") => Kind::Slider,
        ("#I/O", "Joystick") => Kind::Joystick,
        ("#I/O", "Keyboard") => Kind::Keyboard,
        ("#I/O", "TTY") => Kind::Tty,
        ("#I/O", "LED") => Kind::Led,
        ("#I/O", "7-Segment Display") => Kind::SevenSegment,
        ("#I/O", "Hex Digit Display") => Kind::HexDisplay,
        ("#I/O", "RGBLed") => Kind::RgbLed,
        ("#I/O", "DotMatrix") => Kind::DotMatrix,
        ("#Memory", "ROM") => Kind::Rom,
        ("#Memory", "PlaRom") => Kind::PlaRom,
        ("#Memory", "RAM") => Kind::Ram,
        ("#Memory", "Universal Register") => Kind::ShiftRegister,
        ("#Memory", "D Latch") => Kind::Latch(logisim_core::Latch::D),
        ("#Memory", "S-R Latch") => Kind::Latch(logisim_core::Latch::Sr),
        ("#Memory", name) => Kind::FlipFlop(
            logisim_core::FlipFlop::ALL
                .into_iter()
                .find(|ff| ff.name() == name)
                .ok_or_else(|| Error("Unsupported memory component".into()))?,
        ),
        ("#Wiring", "Pin") => Kind::Pin,
        ("#Wiring", "Tunnel") => Kind::Tunnel,
        ("#Wiring", "Constant") => Kind::Constant,
        ("#Wiring", "Power") => Kind::Power,
        ("#Wiring", "Ground") => Kind::Ground,
        ("#Wiring", "Probe") => Kind::Probe,
        ("#Wiring", "Splitter") => Kind::Splitter,
        ("#Wiring", "Bit Extender") => Kind::BitExtender,
        ("#Base", "Text") => Kind::Text,
        ("#Gates", "Controlled Buffer") => Kind::ControlledBuffer,
        ("#Gates", "Controlled Inverter") => Kind::ControlledInverter,
        ("#Wiring", "Pull Resistor") => Kind::PullResistor,
        ("#Wiring", "Transistor") => Kind::Transistor,
        ("#Wiring", "Transmission Gate") => Kind::TransmissionGate,
        ("#Gates", "Odd Parity") => Kind::Parity(logisim_core::Parity::Odd),
        ("#Gates", "Even Parity") => Kind::Parity(logisim_core::Parity::Even),
        ("#Gates", name) => Kind::Gate(
            Gate::ALL
                .into_iter()
                .find(|g| g.name() == name)
                .ok_or_else(|| Error("Unsupported gate".into()))?,
        ),
        _ => return Err(Error("Unsupported component or subcircuit".into())),
    };
    let mut c = Component::new(kind, point(attr(n, "loc")?)?);
    let a = attributes(n)?;
    for key in a.keys() {
        let allowed = match kind {
            Kind::Plugin => false,
            Kind::Ttl(_) => crate::ttl::allowed(key),
            Kind::Parity(_) => crate::parity::allowed(key),
            Kind::Latch(_) => crate::latches::allowed(key),
            Kind::Transistor | Kind::TransmissionGate => crate::transistors::allowed(kind, key),
            Kind::ControlledBuffer | Kind::ControlledInverter | Kind::PullResistor => {
                crate::bus::allowed(kind, key)
            }
            Kind::Plexer(p) => crate::plexers::allowed(p, key),
            Kind::Rom | Kind::Ram => crate::memory::allowed(kind, key),
            Kind::PlaRom => crate::pla::allowed(key),
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
            | Kind::HexDisplay => crate::io::allowed(kind, key),
            Kind::Arithmetic(a) => crate::arithmetic::allowed(a, key),
            Kind::Clock
            | Kind::Register
            | Kind::FlipFlop(_)
            | Kind::Counter
            | Kind::Random
            | Kind::ShiftRegister => crate::sequential::allowed(kind, key),
            Kind::Constant => matches!(key.as_str(), "facing" | "width" | "value"),
            Kind::Power | Kind::Ground => matches!(key.as_str(), "facing" | "width"),
            Kind::Probe | Kind::Subcircuit(_) => {
                matches!(
                    key.as_str(),
                    "facing" | "radix" | "label" | "labelloc" | "labelfont" | "labelcolor"
                ) || (kind == Kind::Probe && key == "width")
            }
            Kind::BitExtender => matches!(key.as_str(), "in_width" | "out_width" | "type"),
            Kind::Splitter => {
                matches!(
                    key.as_str(),
                    "facing" | "fanout" | "incoming" | "appear" | "order"
                ) || key
                    .strip_prefix("bit")
                    .and_then(|n| n.parse::<u8>().ok())
                    .is_some_and(|i| i < 32)
            }
            Kind::Text => matches!(
                key.as_str(),
                "text" | "font" | "color" | "halign" | "valign"
            ),
            Kind::Tunnel => matches!(key.as_str(), "facing" | "width" | "label" | "labelfont"),
            Kind::Pin => matches!(
                key.as_str(),
                "facing"
                    | "width"
                    | "label"
                    | "labelfont"
                    | "labelcolor"
                    | "output"
                    | "tristate"
                    | "pull"
                    | "labelloc"
            ),
            Kind::Gate(g) => {
                matches!(
                    key.as_str(),
                    "facing" | "width" | "label" | "labelfont" | "labelcolor"
                ) || (key == "size" && g != Gate::Buffer)
                    || (key == "inputs" && !g.unary())
                    || (!g.unary() && crate::gates::index(key).is_some())
                    || (key == "xor" && matches!(g, Gate::Xor | Gate::Xnor))
            }
        };
        if !allowed {
            return Err(Error(format!("Unsupported attribute {key}")));
        }
    }
    c.facing = match value(
        &a,
        "facing",
        match kind {
            Kind::Power => "north",
            Kind::Ground | Kind::PullResistor => "south",
            Kind::Led | Kind::RgbLed => "west",
            _ => "east",
        },
    ) {
        "east" => Facing::East,
        "south" => Facing::South,
        "west" => Facing::West,
        "north" => Facing::North,
        v => return Err(Error(format!("Invalid facing: {v}"))),
    };
    c.bits = number(
        &a,
        "width",
        if matches!(
            kind,
            Kind::Slider
                | Kind::Register
                | Kind::Counter
                | Kind::ShiftRegister
                | Kind::Random
                | Kind::Arithmetic(_)
                | Kind::Plexer(logisim_core::Plexer::BitSelector)
        ) {
            8
        } else {
            1
        },
        1..=32,
    )?;
    crate::ttl::read(&mut c, &a)?;
    crate::latches::read(&mut c, &a)?;
    crate::bus::read(&mut c, &a)?;
    crate::transistors::read(&mut c, &a)?;
    crate::sequential::read(&mut c, &a)?;
    crate::memory::read(&mut c, &a)?;
    crate::pla::read(&mut c, &a)?;
    crate::io::read(&mut c, &a)?;
    crate::plexers::read(&mut c, &a)?;
    crate::arithmetic::read(&mut c, &a)?;
    super::wiring::read(&mut c, &a)?;
    if matches!(
        kind,
        Kind::Subcircuit(_)
            | Kind::Pin
            | Kind::Clock
            | Kind::Button
            | Kind::Switch
            | Kind::DipSwitch
            | Kind::Slider
            | Kind::Led
            | Kind::RgbLed
    ) {
        c.label_facing = match value(
            &a,
            "labelloc",
            if kind == Kind::Clock { "west" } else { "north" },
        ) {
            "east" => Facing::East,
            "south" => Facing::South,
            "west" => Facing::West,
            "north" => Facing::North,
            "center" if kind == Kind::Button => {
                c.label_center = true;
                Facing::North
            }
            _ => return Err(Error("Invalid label location".into())),
        };
    }

    if !kind.has_label() {
        return Ok((c, vec![]));
    }
    if matches!(kind, Kind::Parity(_)) {
        crate::parity::read(&mut c, &a)?;
    }
    c.output = boolean(&a, "output", false)?;
    if kind == Kind::Pin {
        c.pin.three_state = boolean(&a, "tristate", true)?;
        c.pin.pull = match value(&a, "pull", "none") {
            "none" => logisim_core::PinPull::None,
            "up" => logisim_core::PinPull::Up,
            "down" => logisim_core::PinPull::Down,
            _ => return Err(Error("Invalid pin pull behavior".into())),
        };
    }
    crate::gates::read(&mut c, &a)?;
    c.label = value(
        &a,
        if kind == Kind::Text { "text" } else { "label" },
        if kind == Kind::Tunnel { "tunnel" } else { "" },
    )
    .to_owned();
    if c.label.len() > 65_536 {
        return Err(Error("Label exceeds 64 KiB.".into()));
    }
    c.font = crate::fonts::parse(value(
        &a,
        if kind == Kind::Text {
            "font"
        } else {
            "labelfont"
        },
        "Dialog plain 12",
    ))?;
    let color = value(
        &a,
        if kind == Kind::Text {
            "color"
        } else {
            "labelcolor"
        },
        "#000000",
    );
    let rgba = if kind == Kind::Text {
        crate::io::rgba(color)?
    } else {
        let rgb = color
            .strip_prefix('#')
            .filter(|s| s.len() == 6)
            .and_then(|s| u32::from_str_radix(s, 16).ok())
            .ok_or_else(|| Error("Invalid RGB color".into()))?;
        [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255]
    };
    c.color = [rgba[0], rgba[1], rgba[2]];
    c.color_alpha = rgba[3];
    if kind == Kind::Text {
        c.text_horizontal = match value(&a, "halign", "center") {
            "left" => HorizontalAlign::Left,
            "center" => HorizontalAlign::Center,
            "right" => HorizontalAlign::Right,
            _ => return Err(Error("Unsupported text alignment".into())),
        };
        c.text_vertical = match value(&a, "valign", "base") {
            "top" => VerticalAlign::Top,
            "center" => VerticalAlign::Center,
            "base" => VerticalAlign::Baseline,
            "bottom" => VerticalAlign::Bottom,
            _ => return Err(Error("Unsupported text alignment".into())),
        };
    }
    Ok((c, vec![]))
}

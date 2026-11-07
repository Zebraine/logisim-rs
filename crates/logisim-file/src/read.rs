use super::{Document, Error, Import, MAX_BYTES, MAX_ITEMS, Result, valid_point};
use logisim_core::{
    Circuit, Component, Facing, Gate, GateSize, HorizontalAlign, Kind, Point, VerticalAlign,
};
use roxmltree::Node;
use std::collections::BTreeMap;

type Attributes = BTreeMap<String, String>;
fn attr<'a>(n: Node<'a, '_>, name: &str) -> Result<&'a str> {
    n.attribute(name)
        .ok_or_else(|| Error(format!("<{}> is missing {name}", n.tag_name().name())))
}
fn attributes(n: Node<'_, '_>) -> Result<Attributes> {
    let mut result = BTreeMap::new();
    for a in n.children().filter(Node::is_element) {
        if !a.has_tag_name("a") || a.children().any(|n| n.is_element()) {
            return Err(Error(format!(
                "Unsupported <{}> content",
                a.tag_name().name()
            )));
        }
        let key = attr(a, "name")?;
        let value = a.attribute("val").map(str::to_owned).unwrap_or_else(|| {
            a.children()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect()
        });
        if result.insert(key.to_owned(), value).is_some() {
            return Err(Error(format!("Duplicate attribute: {key}")));
        }
    }
    Ok(result)
}
fn value<'a>(a: &'a Attributes, key: &str, default: &'a str) -> &'a str {
    a.get(key).map(String::as_str).unwrap_or(default)
}
pub(crate) fn number(
    a: &Attributes,
    key: &str,
    default: u8,
    range: std::ops::RangeInclusive<u8>,
) -> Result<u8> {
    let n = if let Some(s) = a.get(key) {
        s.parse::<u8>()
            .map_err(|_| Error(format!("Invalid {key}: {s}")))?
    } else {
        default
    };
    if !range.contains(&n) {
        return Err(Error(format!("Unsupported {key}: {n}")));
    }
    Ok(n)
}
fn boolean(a: &Attributes, key: &str, default: bool) -> Result<bool> {
    match a.get(key).map(String::as_str) {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(v) => Err(Error(format!("Invalid {key}: {v}"))),
    }
}
fn point(s: &str) -> Result<Point> {
    let parsed = s
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .and_then(|s| s.split_once(','))
        .and_then(|(x, y)| Some(Point::new(x.trim().parse().ok()?, y.trim().parse().ok()?)));
    parsed
        .filter(|p| valid_point(*p))
        .ok_or_else(|| Error(format!("Invalid or out-of-range coordinate: {s}")))
}
fn warn(warnings: &mut Vec<String>, message: String) {
    if warnings.len() < 200 {
        warnings.push(message);
    } else if warnings.len() == 200 {
        warnings.push("Further omissions are not listed.".into());
    }
}

pub fn parse(xml: &str) -> Result<Import> {
    if xml.len() > MAX_BYTES {
        return Err(Error("File exceeds the 8 MiB limit.".into()));
    }
    let xml = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
        },
    )
    .map_err(|e| Error(format!("Invalid XML: {e}")))?;
    let root = xml.root_element();
    if !root.has_tag_name("project") || root.attribute("version") != Some("1.0") {
        return Err(Error("Expected a .circ project with version=1.0.".into()));
    }
    let mut warnings = vec![];
    if root.attribute("source") != Some("2.16.2.2.jar") {
        warn(
            &mut warnings,
            "Unverified source version; port geometry follows Logisim-ITA 2.16.2.2.".into(),
        );
    }
    let mut libraries = BTreeMap::new();
    for lib in root.children().filter(|n| n.has_tag_name("lib")) {
        let id = attr(lib, "name")?;
        if libraries.insert(id, attr(lib, "desc")?).is_some() {
            return Err(Error(format!("Duplicate library: {id}")));
        }
    }
    let mains: Vec<_> = root.children().filter(|n| n.has_tag_name("main")).collect();
    if mains.len() != 1 {
        return Err(Error("Expected one main circuit declaration.".into()));
    }
    let name = attr(mains[0], "name")?;
    if name.is_empty() || name.len() > 1024 {
        return Err(Error("Invalid main circuit name.".into()));
    }
    let circuits: Vec<_> = root
        .children()
        .filter(|n| n.has_tag_name("circuit"))
        .collect();
    let selected: Vec<_> = circuits
        .iter()
        .filter(|n| n.attribute("name") == Some(name))
        .collect();
    if selected.len() != 1 {
        return Err(Error(format!("Missing or ambiguous main circuit: {name}")));
    }
    if circuits.is_empty() || circuits.len() > logisim_core::MAX_CIRCUITS {
        return Err(Error("Invalid number of circuits.".into()));
    }
    let mut document = Document::single(attr(circuits[0], "name")?, Circuit::default());
    for node in &circuits[1..] {
        document.add(attr(*node, "name")?.into()).map_err(Error)?;
    }
    document.main = document
        .named(name)
        .ok_or_else(|| Error("Missing main circuit".into()))?
        .id;
    let names: BTreeMap<_, _> = document
        .circuits
        .iter()
        .map(|c| (c.name.clone(), c.id))
        .collect();
    let mut blocked = std::collections::BTreeSet::new();
    for n in root.children().filter(Node::is_element) {
        match n.tag_name().name() {
            "lib" | "main" | "circuit" | "toolbar" | "mappings" => {}
            "options" => {
                for (key, val) in attributes(n)? {
                    if !matches!(
                        (key.as_str(), val.as_str()),
                        ("gateUndefined", "ignore") | ("simlimit", "1000") | ("simrand", "0")
                    ) {
                        warn(
                            &mut warnings,
                            format!("Project option {key}={val} is not supported."),
                        );
                    }
                }
            }
            tag => warn(&mut warnings, format!("Project <{tag}> was omitted.")),
        }
    }
    let mut total_items = 0;
    for selected in circuits {
        let name = attr(selected, "name")?;
        if selected.children().filter(Node::is_element).count() > MAX_ITEMS + 32 {
            return Err(Error(format!(
                "Circuit exceeds the {MAX_ITEMS} item limit."
            )));
        }
        let mut circuit = Circuit::default();
        for n in selected.children().filter(Node::is_element) {
            match n.tag_name().name() {
                "wire" => {
                    let a = point(attr(n, "from")?)?;
                    let b = point(attr(n, "to")?)?;
                    if a == b || (a.x != b.x && a.y != b.y) {
                        return Err(Error(
                            "Invalid wire: endpoints must form a nonzero orthogonal segment."
                                .into(),
                        ));
                    }
                    circuit.add_wire(a, b);
                }
                "comp" => match component(n, &libraries, &names, &blocked) {
                    Ok((c, notes)) => {
                        for note in notes {
                            warn(
                                &mut warnings,
                                format!(
                                    "{} at ({},{}): {note}",
                                    c.kind.name(),
                                    c.position.x,
                                    c.position.y
                                ),
                            );
                        }
                        circuit.add_component(c);
                    }
                    Err(e) => warn(
                        &mut warnings,
                        format!(
                            "Skipped {} at {}: {e}",
                            n.attribute("name").unwrap_or("component"),
                            n.attribute("loc").unwrap_or("?")
                        ),
                    ),
                },
                "appear" => {}
                "a" => {
                    let key = attr(n, "name")?;
                    let val = n.attribute("val").map(str::to_owned).unwrap_or_else(|| {
                        n.children()
                            .filter(Node::is_text)
                            .filter_map(|n| n.text())
                            .collect()
                    });
                    if !((key == "circuit" && val == name)
                        || matches!(
                            (key, val.as_str()),
                            ("clabel", "")
                                | ("clabelup", "east")
                                | ("clabelfont", "Dialog plain 12")
                                | ("clabelcolor", "#000000")
                        ))
                    {
                        warn(
                            &mut warnings,
                            format!("Circuit attribute {key} was omitted."),
                        );
                    }
                }
                tag => warn(&mut warnings, format!("Circuit <{tag}> was omitted.")),
            }
        }
        if circuit.components.len() + circuit.wires.len() > MAX_ITEMS {
            return Err(Error("Circuit has too many items.".into()));
        }
        total_items += circuit.components.len() + circuit.wires.len();
        if total_items > MAX_ITEMS {
            return Err(Error("Project has too many items.".into()));
        }
        // Unsupported input-pin attributes would change the default interface geometry.
        let original_pins = selected
            .children()
            .filter(|n| {
                n.has_tag_name("comp")
                    && n.attribute("name") == Some("Pin")
                    && n.attribute("lib").and_then(|id| libraries.get(id)).copied()
                        == Some("#Wiring")
            })
            .count();
        if original_pins
            != circuit
                .components
                .iter()
                .filter(|c| c.kind == Kind::Pin)
                .count()
        {
            blocked.insert(names[name]);
            warn(
                &mut warnings,
                format!(
                    "Circuit '{name}' has unsupported interface pins; its instances are omitted."
                ),
            );
        }
        let appearances: Vec<_> = selected
            .children()
            .filter(|n| n.has_tag_name("appear"))
            .collect();
        if !appearances.is_empty() {
            let parsed = if appearances.len() == 1 {
                crate::appearance::read(appearances[0], &circuit)
            } else {
                Err(Error("Duplicate appearance".into()))
            };
            match parsed {
                Ok(appearance) => circuit.appearance = Some(appearance),
                Err(error) => {
                    blocked.insert(names[name]);
                    warn(
                        &mut warnings,
                        format!(
                            "Circuit '{name}' has an unsupported custom appearance: {error}; its instances are omitted."
                        ),
                    );
                }
            }
        }
        document.circuit_mut(names[name]).unwrap().circuit = circuit;
    }
    for definition in &mut document.circuits {
        definition
            .circuit
            .components
            .retain(|c| !matches!(c.kind,Kind::Subcircuit(id) if blocked.contains(&id)));
    }
    document.validate().map_err(Error)?;
    document.refresh_interfaces(false);
    for definition in &mut document.circuits {
        definition.circuit.normalize_wires();
    }
    if document
        .circuits
        .iter()
        .map(|d| d.circuit.components.len() + d.circuit.wires.len())
        .sum::<usize>()
        > MAX_ITEMS
    {
        return Err(Error(
            "Project exceeds item limit after preserving wire junctions.".into(),
        ));
    }
    Ok(Import { document, warnings })
}

fn component(
    n: Node<'_, '_>,
    libraries: &BTreeMap<&str, &str>,
    names: &BTreeMap<String, logisim_core::Id>,
    blocked: &std::collections::BTreeSet<logisim_core::Id>,
) -> Result<(Component, Vec<String>)> {
    let lib = n
        .attribute("lib")
        .and_then(|id| libraries.get(id))
        .copied()
        .unwrap_or("");
    let name = attr(n, "name")?;
    let kind = match (lib, name) {
        ("", name)
            if n.attribute("lib").is_none()
                && names.contains_key(name)
                && !blocked.contains(&names[name]) =>
        {
            Kind::Subcircuit(names[name])
        }
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
        ("#Memory", "Universal Register") => Kind::ShiftRegister,
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
            Kind::Plexer(p) => crate::plexers::allowed(p, key),
            Kind::Arithmetic(a) => crate::arithmetic::allowed(a, key),
            Kind::Clock
            | Kind::Register
            | Kind::FlipFlop(_)
            | Kind::Counter
            | Kind::ShiftRegister => crate::sequential::allowed(kind, key),
            Kind::Constant => matches!(key.as_str(), "facing" | "width" | "value"),
            Kind::Power | Kind::Ground => matches!(key.as_str(), "facing" | "width"),
            Kind::Probe | Kind::Subcircuit(_) => matches!(
                key.as_str(),
                "facing" | "radix" | "label" | "labelloc" | "labelfont" | "labelcolor"
            ),
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
                "facing" | "width" | "label" | "labelfont" | "labelcolor" | "output" | "tristate"
            ),
            Kind::Gate(g) => {
                matches!(
                    key.as_str(),
                    "facing" | "width" | "label" | "labelfont" | "labelcolor"
                ) || (key == "size" && g != Gate::Buffer)
                    || (key == "inputs" && !g.unary())
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
            Kind::Ground => "south",
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
            Kind::Register
                | Kind::Counter
                | Kind::ShiftRegister
                | Kind::Arithmetic(_)
                | Kind::Plexer(logisim_core::Plexer::BitSelector)
        ) {
            8
        } else {
            1
        },
        1..=32,
    )?;
    crate::sequential::read(&mut c, &a)?;
    crate::plexers::read(&mut c, &a)?;
    crate::arithmetic::read(&mut c, &a)?;
    read_wiring(&mut c, &a)?;
    if matches!(kind, Kind::Subcircuit(_) | Kind::Clock) {
        c.label_facing = match value(
            &a,
            "labelloc",
            if kind == Kind::Clock { "west" } else { "north" },
        ) {
            "east" => Facing::East,
            "south" => Facing::South,
            "west" => Facing::West,
            "north" => Facing::North,
            _ => return Err(Error("Invalid label location".into())),
        };
    }

    if !kind.has_label() {
        return Ok((c, vec![]));
    }
    c.inputs = number(&a, "inputs", 2, 2..=8)?;
    c.output = boolean(&a, "output", false)?;
    if kind == Kind::Pin && !c.output && boolean(&a, "tristate", true)? {
        return Err(Error(
            "Three-state inputs are not implemented (tristate must be false).".into(),
        ));
    }
    if let Kind::Gate(g) = kind {
        c.gate_size = match (g, value(&a, "size", if g.unary() { "20" } else { "30" })) {
            (Gate::Not | Gate::Buffer, "20") => GateSize::Narrow,
            (_, "30") if !g.unary() => GateSize::Narrow,
            (Gate::Not, "30") => GateSize::Wide,
            (_, "50") if !g.unary() => GateSize::Medium,
            (_, "70") if !g.unary() => GateSize::Wide,
            _ => return Err(Error("Unsupported gate size".into())),
        };
    }
    c.label = value(
        &a,
        if kind == Kind::Text { "text" } else { "label" },
        if kind == Kind::Tunnel { "tunnel" } else { "" },
    )
    .to_owned();
    if c.label.len() > 65_536 {
        return Err(Error("Label exceeds 64 KiB.".into()));
    }
    let mut notes = vec![];
    let font = value(
        &a,
        if kind == Kind::Text {
            "font"
        } else {
            "labelfont"
        },
        "Dialog plain 12",
    );
    let (family_style, size) = font
        .rsplit_once(' ')
        .ok_or_else(|| Error("Invalid font".into()))?;
    c.font_size = size
        .parse()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| Error("Unsupported font size".into()))?;
    if family_style != "Dialog plain" {
        notes.push(format!(
            "Font '{font}' is rendered and saved using the default plain font."
        ));
    }
    let color = value(
        &a,
        if kind == Kind::Text {
            "color"
        } else {
            "labelcolor"
        },
        "#000000",
    );
    let rgb = color
        .strip_prefix('#')
        .filter(|s| s.len() == 6)
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .ok_or_else(|| Error("Invalid RGB color".into()))?;
    c.color = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
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
    Ok((c, notes))
}

fn read_wiring(c: &mut Component, a: &Attributes) -> Result<()> {
    use logisim_core::{Extension, Radix, SplitAppearance};
    match c.kind {
        Kind::Constant => {
            let text = value(a, "value", "0x1");
            c.constant = text
                .strip_prefix("0x")
                .and_then(|v| u32::from_str_radix(v, 16).ok())
                .ok_or_else(|| Error("Invalid constant value".into()))?;
            if c.bits < 32 {
                c.constant &= (1 << c.bits) - 1;
            }
        }
        Kind::Probe => {
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

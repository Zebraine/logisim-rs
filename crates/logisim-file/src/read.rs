//! Project assembly, import budgets and omission handling.
use crate::{
    Document, Error, Import, MAX_BYTES, MAX_CIRCUIT_ITEMS, MAX_ITEMS, MAX_XML_NODES, Result,
    xml::{attr, attributes, point},
};
use logisim_core::{Circuit, Kind};
use roxmltree::Node;
use std::collections::BTreeMap;

fn warn(warnings: &mut crate::diagnostics::Warnings, message: String) {
    warnings.add(message);
}

pub fn parse(xml: &str) -> Result<Import> {
    if xml.len() > MAX_BYTES {
        return Err(Error("File exceeds the 8 MiB limit.".into()));
    }
    let xml = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
        },
    )
    .map_err(|e| match e {
        roxmltree::Error::NodesLimitReached => {
            Error(format!("XML exceeds the {MAX_XML_NODES} node limit."))
        }
        _ => Error(format!("Invalid XML: {e}")),
    })?;
    let root = xml.root_element();
    if !root.has_tag_name("project") || root.attribute("version") != Some("1.0") {
        return Err(Error("Expected a .circ project with version=1.0.".into()));
    }
    let mut warnings = crate::diagnostics::Warnings::default();
    if !matches!(
        root.attribute("source"),
        Some("2.16.2.2.jar" | "logisim-rs-plugin-v1")
    ) {
        warn(
            &mut warnings,
            "Unverified source version; port geometry follows Logisim-ITA 2.16.2.2.".into(),
        );
    }
    let mut libraries = BTreeMap::new();
    for lib in root.children().filter(|n| n.has_tag_name("lib")) {
        let id = attr(lib, "name")?;
        if attr(lib, "desc")?.starts_with("jar#") && lib.descendants().any(|a| a.has_tag_name("a"))
        {
            warn(
                &mut warnings,
                format!(
                    "Library {id}: palette tool settings were omitted; circuit instances use their own attributes."
                ),
            );
        }
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
    // Count raw items before any component parsing or topology work, including unsupported ones.
    let mut raw_items = 0;
    for circuit in &circuits {
        let items = circuit
            .children()
            .filter(|n| n.has_tag_name("comp") || n.has_tag_name("wire"))
            .count();
        if items > MAX_CIRCUIT_ITEMS {
            return Err(Error(format!(
                "Circuit '{}' exceeds {MAX_CIRCUIT_ITEMS} items.",
                attr(*circuit, "name")?
            )));
        }
        raw_items += items;
    }
    if raw_items > MAX_ITEMS {
        return Err(Error(format!(
            "Project exceeds {MAX_ITEMS} items ({raw_items} found)."
        )));
    }
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
    for selected in circuits {
        let name = attr(selected, "name")?;
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
                "comp" => match crate::component::read(n, &libraries, &names) {
                    Ok((c, notes)) => {
                        for note in notes {
                            warnings.component(
                                name,
                                c.kind.name(),
                                format!("({},{})", c.position.x, c.position.y),
                                &note,
                                false,
                            );
                        }
                        circuit.add_component(c);
                    }
                    Err(e) => warnings.component(
                        name,
                        n.attribute("name").unwrap_or("component"),
                        n.attribute("loc").unwrap_or("?").into(),
                        &e.0,
                        true,
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
                    if key == "circuit" && val == name {
                        continue;
                    }
                    if let Err(error) = crate::fonts::read_label(&mut circuit.label, key, &val) {
                        warn(&mut warnings, format!("Circuit '{name}': {error}"));
                    }
                }
                tag => warn(
                    &mut warnings,
                    format!("Circuit '{name}': <{tag}> was omitted."),
                ),
            }
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
    let definition_names: BTreeMap<_, _> = names
        .iter()
        .map(|(name, id)| (*id, name.as_str()))
        .collect();
    for definition in &mut document.circuits {
        definition.circuit.components.retain(|c| {
            if let Kind::Subcircuit(id) = c.kind
                && blocked.contains(&id)
            {
                warnings.component(
                    &definition.name,
                    definition_names[&id],
                    format!("({},{})", c.position.x, c.position.y),
                    "Target circuit has unsupported interface pins or appearance.",
                    true,
                );
                false
            } else {
                true
            }
        });
    }
    document.validate().map_err(Error)?;
    document.refresh_interfaces(false);
    for definition in &mut document.circuits {
        definition.circuit.normalize_wires();
        if definition.circuit.components.len() + definition.circuit.wires.len() > MAX_CIRCUIT_ITEMS
        {
            return Err(Error(format!(
                "Circuit '{}' exceeds {MAX_CIRCUIT_ITEMS} items after preserving wire junctions.",
                definition.name
            )));
        }
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
    Ok(Import {
        document,
        warnings: warnings.finish(),
    })
}

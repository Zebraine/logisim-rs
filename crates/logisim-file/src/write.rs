//! Project serialization and aggregate limits.
use crate::{
    Document, Error, MAX_BYTES, MAX_CIRCUIT_ITEMS, MAX_ITEMS, Result, valid_point, xml::escape,
};
use std::fmt::Write;

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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- Written by logisim-rs; source identifies the file format. -->\n<project source=\"2.16.2.2.jar\" version=\"1.0\">\n  <lib desc=\"#Wiring\" name=\"0\"/>\n  <lib desc=\"#Gates\" name=\"1\"/>\n  <lib desc=\"#Base\" name=\"2\"/>\n  <lib desc=\"#Memory\" name=\"3\"/>\n  <lib desc=\"#Plexers\" name=\"4\"/>\n  <lib desc=\"#Arithmetic\" name=\"5\"/>\n  <lib desc=\"#I/O\" name=\"6\"/>\n  <lib desc=\"#TTL\" name=\"7\"/>\n",
    );
    crate::plugin::declaration(document, &mut out)?;
    writeln!(out, "  <main name=\"{name}\"/>").unwrap();
    out.push_str("  <options>\n    <a name=\"gateUndefined\" val=\"ignore\"/>\n    <a name=\"simlimit\" val=\"1000\"/>\n    <a name=\"simrand\" val=\"0\"/>\n  </options>\n  <mappings>\n    <tool lib=\"2\" map=\"Button2\" name=\"Poke Tool\"/>\n    <tool lib=\"2\" map=\"Button3\" name=\"Menu Tool\"/>\n    <tool lib=\"2\" map=\"Ctrl Button1\" name=\"Menu Tool\"/>\n  </mappings>\n  <toolbar>\n    <tool lib=\"2\" name=\"Poke Tool\"/>\n    <tool lib=\"2\" name=\"Edit Tool\"/>\n    <sep/>\n    <tool lib=\"2\" name=\"Text Tool\"/>\n    <tool lib=\"0\" name=\"Pin\"><a name=\"tristate\" val=\"false\"/></tool>\n    <tool lib=\"0\" name=\"Pin\"><a name=\"output\" val=\"true\"/><a name=\"facing\" val=\"west\"/></tool>\n    <tool lib=\"1\" name=\"NOT Gate\"/>\n    <tool lib=\"1\" name=\"AND Gate\"/>\n    <tool lib=\"1\" name=\"OR Gate\"/>\n  </toolbar>\n");
    for definition in &document.circuits {
        let circuit = &definition.circuit;
        let name = escape(&definition.name)?;
        writeln!(out, "  <circuit name=\"{name}\">").unwrap();
        crate::fonts::write_label(&mut out, &circuit.label)?;
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
            crate::component::write(&mut out, c, document)?;
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

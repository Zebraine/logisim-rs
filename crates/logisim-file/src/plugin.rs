//! Explicit logisim-rs extension; never claim arbitrary plugins are Logisim-ITA built-ins.
use crate::{
    Error, Result,
    xml::{attr, attribute, attributes, point},
};
use logisim_core::{Component, Kind, plugin::api};
use std::fmt::Write;
pub(crate) const DESCRIPTOR: &str = "logisim-rs#plugin-v1";
pub(crate) fn read(n: roxmltree::Node<'_, '_>) -> Result<Component> {
    let attrs = attributes(n)?;
    if !attrs.contains_key("definition")
        || attrs.keys().any(|k| {
            !["definition", "facing", "label", "labelfont", "labelcolor"].contains(&k.as_str())
        })
    {
        return Err(Error("Invalid plugin component attributes.".into()));
    }
    let text = &attrs["definition"];
    if text.len() > api::MAX_MESSAGE {
        return Err(Error("Plugin definition exceeds message limit.".into()));
    }
    let definition: api::Component =
        serde_json::from_str(text).map_err(|e| Error(e.to_string()))?;
    definition.validate().map_err(Error)?;
    if attr(n, "name")? != definition.tool.id {
        return Err(Error("Plugin tool identity mismatch.".into()));
    }
    let mut c = Component::from_plugin(definition, point(attr(n, "loc")?)?);
    if let Some(facing) = attrs.get("facing") {
        c.facing = match facing.as_str() {
            "east" => logisim_core::Facing::East,
            "north" => logisim_core::Facing::North,
            "west" => logisim_core::Facing::West,
            "south" => logisim_core::Facing::South,
            _ => return Err(Error("Invalid plugin facing.".into())),
        };
    }
    if let Some(label) = attrs.get("label") {
        if label.len() > 65_536 {
            return Err(Error("Label exceeds 64 KiB.".into()));
        }
        c.label.clone_from(label);
    }
    if let Some(font) = attrs.get("labelfont") {
        c.font = crate::fonts::parse(font)?;
    }
    if let Some(color) = attrs.get("labelcolor") {
        let value = color
            .strip_prefix('#')
            .filter(|s| s.len() == 6)
            .and_then(|s| u32::from_str_radix(s, 16).ok())
            .ok_or_else(|| Error("Invalid RGB color".into()))?;
        c.color = [(value >> 16) as u8, (value >> 8) as u8, value as u8];
    }
    Ok(c)
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    let definition = c
        .plugin
        .as_ref()
        .ok_or_else(|| Error("Missing plugin definition.".into()))?;
    definition.validate().map_err(Error)?;
    let mut expected = Component::from_plugin(definition.clone(), c.position);
    expected.id = c.id;
    expected.facing = c.facing;
    expected.label.clone_from(&c.label);
    expected.font = c.font.clone();
    expected.color = c.color;
    if *c != expected {
        return Err(Error("Unsupported plugin component attributes.".into()));
    }
    let text = serde_json::to_string(definition).map_err(|e| Error(e.to_string()))?;
    if text.len() > api::MAX_MESSAGE {
        return Err(Error("Plugin definition exceeds message limit.".into()));
    }
    writeln!(
        out,
        "    <comp lib=\"rs-plugin-v1\" name=\"{}\" loc=\"({},{})\">",
        definition.tool.id, c.position.x, c.position.y
    )
    .unwrap();
    attribute(out, "definition", text)?;
    let default = Component::from_plugin(definition.clone(), c.position);
    if c.facing != default.facing {
        attribute(out, "facing", c.facing.name().to_ascii_lowercase())?;
    }
    if c.label != default.label {
        attribute(out, "label", &c.label)?;
    }
    if c.font != default.font {
        attribute(out, "labelfont", crate::fonts::serialize(&c.font)?)?;
    }
    if c.color != default.color {
        attribute(
            out,
            "labelcolor",
            crate::io::color_text([c.color[0], c.color[1], c.color[2], 255]),
        )?;
    }
    out.push_str("    </comp>\n");
    Ok(())
}
pub(crate) fn declaration(document: &crate::Document, out: &mut String) -> Result<()> {
    if !document.plugin_sources.is_empty()
        || document
            .circuits
            .iter()
            .any(|d| d.circuit.components.iter().any(|c| c.kind == Kind::Plugin))
    {
        *out = out.replace("source=\"2.16.2.2.jar\"", "source=\"logisim-rs-plugin-v1\"");
        out.push_str("  <!-- Plugin components require logisim-rs and matching Wasm modules. -->\n  <lib desc=\"logisim-rs#plugin-v1\" name=\"rs-plugin-v1\">\n");
        if !document.plugin_sources.is_empty() {
            let text = serde_json::to_string(&document.plugin_sources)
                .map_err(|e| Error(e.to_string()))?;
            if text.len() > api::MAX_MESSAGE {
                return Err(Error("Plugin sources exceed message limit.".into()));
            }
            attribute(out, "sources", text)?;
        }
        out.push_str("  </lib>\n");
    }
    Ok(())
}
pub(crate) fn sources(
    root: roxmltree::Node<'_, '_>,
) -> Result<Vec<logisim_core::plugin::PluginSource>> {
    let mut sources = vec![];
    for lib in root
        .children()
        .filter(|n| n.has_tag_name("lib") && n.attribute("desc") == Some(DESCRIPTOR))
    {
        let attrs = attributes(lib)?;
        if attrs.keys().any(|k| k != "sources") {
            return Err(Error("Unsupported plugin library metadata.".into()));
        }
        if let Some(text) = attrs.get("sources") {
            if text.len() > api::MAX_MESSAGE {
                return Err(Error("Plugin sources exceed message limit.".into()));
            }
            sources.extend(
                serde_json::from_str::<Vec<logisim_core::plugin::PluginSource>>(text)
                    .map_err(|e| Error(e.to_string()))?,
            );
        }
        if sources.len() > 64 {
            return Err(Error("Too many plugin sources.".into()));
        }
    }
    Ok(sources)
}

#[cfg(test)]
mod tests;

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
    if attrs.len() != 1 || !attrs.contains_key("definition") {
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
    Ok(Component::from_plugin(definition, point(attr(n, "loc")?)?))
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    let definition = c
        .plugin
        .as_ref()
        .ok_or_else(|| Error("Missing plugin definition.".into()))?;
    definition.validate().map_err(Error)?;
    let mut expected = Component::from_plugin(definition.clone(), c.position);
    expected.id = c.id;
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

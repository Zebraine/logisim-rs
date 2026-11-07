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
pub(crate) fn declaration(document: &crate::Document, out: &mut String) {
    if document
        .circuits
        .iter()
        .any(|d| d.circuit.components.iter().any(|c| c.kind == Kind::Plugin))
    {
        *out = out.replace("source=\"2.16.2.2.jar\"", "source=\"logisim-rs-plugin-v1\"");
        out.push_str("  <!-- Plugin components require logisim-rs and matching Wasm modules. -->\n  <lib desc=\"logisim-rs#plugin-v1\" name=\"rs-plugin-v1\"/>\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::{Point, Project};
    #[test]
    fn plugin_snapshot_roundtrip_and_invalid_schema_protection() {
        let definition = api::Component {
            plugin: "third-party.test".into(),
            version: "2.0".into(),
            tool: api::Tool {
                id: "widget".into(),
                name: "User <图形>".into(),
                bounds: [[-20, -20], [20, 20]],
                ports: vec![api::Port {
                    id: "out".into(),
                    label: "My & output".into(),
                    at: [20, 0],
                    width: 32,
                    output: true,
                }],
                graphics: vec![api::Shape::Text {
                    at: [-20, -20],
                    text: "A & B\n第二行".into(),
                    size: 10,
                    color: [1, 2, 3, 4],
                }],
                properties: vec![api::Property {
                    id: "count".into(),
                    label: "Rows".into(),
                    default: "2".into(),
                    kind: api::PropertyKind::Integer { min: 1, max: 10 },
                }],
                actions: vec![],
            },
            properties: [("count".into(), "3".into())].into(),
        };
        let mut p = Project::default();
        p.main_circuit_mut()
            .circuit
            .add_component(Component::from_plugin(
                definition.clone(),
                Point::new(100, 200),
            ));
        let xml = crate::serialize(&p).unwrap();
        assert!(xml.contains("source=\"logisim-rs-plugin-v1\""));
        let parsed = crate::parse(&xml).unwrap();
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(
            parsed.document.main_circuit().circuit.components[0]
                .plugin
                .as_ref(),
            Some(&definition)
        );
        assert_eq!(crate::serialize(&parsed.document).unwrap(), xml);
        // Missing/invalid metadata is reported, so the desktop protects the source.
        for broken in [
            xml.replace("&quot;width&quot;:32", "&quot;width&quot;:0"),
            xml.replace(
                "&quot;count&quot;:&quot;3&quot;",
                "&quot;count&quot;:&quot;999&quot;",
            ),
            xml.replace("name=\"widget\"", "name=\"different\""),
        ] {
            assert_ne!(xml, broken);
            let imported = crate::parse(&broken).unwrap();
            assert!(!imported.warnings.is_empty());
            assert!(
                imported
                    .document
                    .main_circuit()
                    .circuit
                    .components
                    .is_empty()
            );
        }
    }
}

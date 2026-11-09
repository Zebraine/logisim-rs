use super::*;
use logisim_core::{Point, Project};
#[test]
fn plugin_sources_roundtrip_and_reject_invalid_metadata() {
    let source = logisim_core::plugin::PluginSource {
        plugin: "org.example.tools".into(),
        version: "1.0".into(),
        path: "../插件/a & b.wasm".into(),
        fallback: "/opt/plugins/a & b.wasm".into(),
    };
    let mut p = Project::default();
    p.plugin_sources.push(source.clone());
    let xml = crate::serialize(&p).unwrap();
    let imported = crate::parse(&xml).unwrap();
    assert!(imported.warnings.is_empty());
    assert_eq!(imported.document.plugin_sources, vec![source.clone()]);
    assert_eq!(crate::serialize(&imported.document).unwrap(), xml);
    for broken in [
        xml.replace("&quot;fallback&quot;", "&quot;unknown&quot;"),
        xml.replace("org.example.tools", "invalid plugin identity"),
        xml.replace("name=\"sources\"", "name=\"unknown\""),
    ] {
        assert_ne!(xml, broken);
        assert!(crate::parse(&broken).is_err());
    }
    p.plugin_sources.push(source.clone());
    assert!(crate::serialize(&p).is_err());
    p.plugin_sources = (0..64)
        .map(|i| logisim_core::plugin::PluginSource {
            plugin: format!("plugin-{i}"),
            path: "p".repeat(4096),
            fallback: "f".repeat(4096),
            ..source.clone()
        })
        .collect();
    assert!(
        crate::serialize(&p)
            .unwrap_err()
            .0
            .contains("message limit")
    );
}
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
    // Geometry and label metadata belong to the host, not the plugin ABI snapshot.
    for facing in logisim_core::Facing::ALL {
        let c = &mut p.main_circuit_mut().circuit.components[0];
        c.facing = facing;
        c.label = "自定义 & label".into();
        c.font = logisim_core::ArtworkFont {
            family: "Dialog".into(),
            size: 16,
            bold: true,
            italic: true,
        };
        c.color = [18, 52, 86];
        let text = crate::serialize(&p).unwrap();
        let imported = crate::parse(&text).unwrap();
        assert!(imported.warnings.is_empty());
        assert_eq!(
            imported.document.main_circuit().circuit.components,
            p.main_circuit().circuit.components
        );
        assert_eq!(crate::serialize(&imported.document).unwrap(), text);
    }
    let labeled = crate::serialize(&p).unwrap();
    for broken in [
        labeled.replace(
            "name=\"facing\" val=\"north\"",
            "name=\"facing\" val=\"diagonal\"",
        ),
        labeled.replace("#123456", "#invalid"),
        labeled.replace("Dialog bolditalic 16", "Dialog plain 0"),
    ] {
        assert_ne!(labeled, broken);
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

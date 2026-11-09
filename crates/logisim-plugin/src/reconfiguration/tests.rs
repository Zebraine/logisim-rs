use super::*;
use logisim_plugin_api::{Property, PropertyKind, Tool, reconfiguration::Policy};
fn manifest() -> Manifest {
    Manifest {
        abi: 3,
        id: "policy.test".into(),
        version: "1".into(),
        name: "Policy".into(),
        tools: vec![Tool {
            id: "tool".into(),
            name: "Tool".into(),
            bounds: [[0, 0], [20, 20]],
            ports: vec![],
            graphics: vec![],
            actions: vec![],
            properties: ["display", "mode"]
                .into_iter()
                .map(|id| Property {
                    id: id.into(),
                    label: id.into(),
                    default: "false".into(),
                    kind: PropertyKind::Boolean,
                })
                .collect(),
        }],
    }
}
fn module(extension: &str, bytes: &[u8]) -> Vec<u8> {
    let encoded = |b: &[u8]| b.iter().map(|b| format!("\\{b:02x}")).collect::<String>();
    let manifest = serde_json::to_vec(&Response::Manifest(manifest())).unwrap();
    let packed = ((manifest.len() as u64) << 32) | 4096;
    wat::parse_str(format!(
        r#"(module (memory (export "memory") 1)
        (data (i32.const 4096) "{}") (data (i32.const 8192) "{}")
        (func (export "logisim_abi") (result i32) i32.const 3)
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64) i64.const {packed}) {extension})"#,
        encoded(&manifest),
        encoded(bytes)
    ))
    .unwrap()
}
#[test]
fn optional_policy_preserves_only_explicit_properties_of_identical_definitions() {
    let policy = Policy {
        preserve: [("tool".into(), vec!["display".into()])].into(),
    };
    policy.validate(&manifest()).unwrap();
    let bytes = serde_json::to_vec(&policy).unwrap();
    let packed = ((bytes.len() as u64) << 32) | 8192;
    let ext = format!(
        r#"(func (export "logisim_reconfiguration_version") (result i32) i32.const 1)
        (func (export "logisim_reconfiguration") (result i64) i64.const {packed})"#
    );
    let plugin = Plugin::load(&module(&ext, &bytes)).unwrap();
    let mut registry = Registry::default();
    registry.insert(plugin).unwrap();
    let old = manifest().components().remove(0);
    let mut new = old.clone();
    new.properties.insert("display".into(), "true".into());
    assert!(registry.preserves_state(&old, &new));
    assert!(registry.preserves_state(&new, &old));
    for bad in [
        {
            let mut c = new.clone();
            c.properties.insert("mode".into(), "true".into());
            c
        },
        {
            let mut c = new.clone();
            c.tool.name = "Changed".into();
            c
        },
        {
            let mut c = new.clone();
            c.version = "2".into();
            c
        },
        {
            let mut c = new.clone();
            c.plugin = "missing".into();
            c
        },
        {
            let mut c = new.clone();
            c.properties.remove("mode");
            c
        },
        {
            let mut c = new.clone();
            c.properties.insert("display".into(), "invalid".into());
            c
        },
    ] {
        assert!(!registry.preserves_state(&old, &bad));
    }
    for unknown in [
        "".to_owned(),
        ext.replace("i32.const 1)", "i32.const 99)")
            .replace(&format!("i64.const {packed}"), "unreachable"),
    ] {
        let plugin = Plugin::load(&module(&unknown, &bytes)).unwrap();
        let mut registry = Registry::default();
        registry.insert(plugin).unwrap();
        assert!(!registry.preserves_state(&old, &new));
    }
    for broken in [
        ext.replace(
            "(result i32) i32.const 1",
            "(param i32) (result i32) i32.const 1",
        ),
        ext.replace("logisim_reconfiguration\"", "missing\""),
        ext.replace(&format!("i64.const {packed}"), "unreachable"),
        ext.replace(
            &format!("i64.const {packed}"),
            "(loop $spin br $spin) i64.const 0",
        ),
        ext.replace(
            &format!("i64.const {packed}"),
            &format!("i64.const {}", ((MAX_MESSAGE as u64 + 1) << 32) | 8192),
        ),
        ext.replace(
            &format!("i64.const {packed}"),
            &format!("i64.const {}", ((bytes.len() as u64) << 32) | 65530),
        ),
    ] {
        assert!(Plugin::load(&module(&broken, &bytes)).is_err());
    }
    assert!(Plugin::load(&module(&ext, &vec![b'?'; bytes.len()])).is_err());
    for invalid in [
        Policy {
            preserve: [("missing".into(), vec!["display".into()])].into(),
        },
        Policy {
            preserve: [("tool".into(), vec!["missing".into()])].into(),
        },
        Policy {
            preserve: [("tool".into(), vec!["display".into(), "display".into()])].into(),
        },
        Policy {
            preserve: [("tool".into(), vec![])].into(),
        },
    ] {
        assert!(invalid.validate(&manifest()).is_err());
    }
    let mut unavailable = registry.clone();
    unavailable.unavailable(old.plugin.clone(), old.version.clone(), "missing".into());
    assert!(!unavailable.preserves_state(&old, &new));
}

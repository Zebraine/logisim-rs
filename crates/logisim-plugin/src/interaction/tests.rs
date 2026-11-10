use super::*;
use logisim_plugin_api::{
    Action, Property, PropertyKind, Tool,
    interaction::{Interaction, Key, Keyboard},
};
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
            actions: vec![Action {
                id: "input".into(),
                label: "Input".into(),
                argument: Some("Event".into()),
            }],
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
fn versioned_keyboard_metadata_is_optional_bounded_and_validated() {
    let value = Interaction {
        keyboards: vec![Keyboard {
            tool: "tool".into(),
            action: "input".into(),
            keys: vec![Key::A, Key::ArrowUp],
        }],
    };
    value.validate(&manifest()).unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    let packed = ((bytes.len() as u64) << 32) | 8192;
    let ext = format!(
        r#"(func (export "logisim_interaction_version") (result i32) i32.const 1)
        (func (export "logisim_interaction") (result i64) i64.const {packed})"#
    );
    let plugin = Plugin::load(&module(&ext, &bytes)).unwrap();
    assert_eq!(plugin.library().interaction, value);
    for absent in [
        String::new(),
        ext.replace("i32.const 1)", "i32.const 99)")
            .replace(&format!("i64.const {packed}"), "unreachable"),
    ] {
        assert!(
            Plugin::load(&module(&absent, &bytes))
                .unwrap()
                .library()
                .interaction
                .keyboards
                .is_empty()
        );
    }
    for broken in [
        ext.replace("logisim_interaction\"", "missing\""),
        ext.replace(
            "(result i32) i32.const 1",
            "(param i32) (result i32) i32.const 1",
        ),
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
    for case in 0..5 {
        let mut bad = value.clone();
        match case {
            0 => bad.keyboards[0].tool = "missing".into(),
            1 => bad.keyboards[0].action = "missing".into(),
            2 => bad.keyboards[0].keys.clear(),
            3 => bad.keyboards[0].keys.push(Key::A),
            _ => bad.keyboards.push(bad.keyboards[0].clone()),
        }
        assert!(bad.validate(&manifest()).is_err());
    }
    let mut m = manifest();
    m.tools[0].actions[0].argument = None;
    assert!(value.validate(&m).is_err());
}

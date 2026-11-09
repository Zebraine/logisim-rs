use super::*;
use logisim_plugin_api::{Action, Tool};
fn manifest() -> Manifest {
    Manifest {
        abi: 3,
        id: "ui.test".into(),
        version: "1".into(),
        name: "UI".into(),
        tools: vec![Tool {
            id: "tool".into(),
            name: "Tool".into(),
            bounds: [[0, 0], [20, 20]],
            ports: vec![],
            graphics: vec![],
            properties: vec![],
            actions: vec![Action {
                id: "write".into(),
                label: "Write".into(),
                argument: Some("Words".into()),
            }],
        }],
    }
}
fn module(extension: &str, bytes: &[u8]) -> Vec<u8> {
    let encoded = |b: &[u8]| b.iter().map(|b| format!("\\{b:02x}")).collect::<String>();
    let manifest = serde_json::to_vec(&Response::Manifest(manifest())).unwrap();
    let packed = ((manifest.len() as u64) << 32) | 4096;
    wat::parse_str(format!(
        r#"(module
        (memory (export "memory") 1)
        (data (i32.const 4096) "{}") (data (i32.const 8192) "{}")
        (func (export "logisim_abi") (result i32) i32.const 3)
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64) i64.const {packed})
        {extension})"#,
        encoded(&manifest),
        encoded(bytes)
    ))
    .unwrap()
}
#[test]
fn optional_metadata_is_bounded_and_does_not_change_saved_definitions() {
    let metadata = Presentation {
        translations: [("zh-CN".into(), [("Tool".into(), "工具".into())].into())].into(),
        imports: vec![presentation::MemoryImport {
            tool: "tool".into(),
            action: "write".into(),
            label: "Import".into(),
            address_bits: 5,
            data_bits: 32,
            argument_prefix: "image ".into(),
        }],
    };
    let bytes = serde_json::to_vec(&metadata).unwrap();
    let packed = ((bytes.len() as u64) << 32) | 8192;
    let ext = format!(
        r#"(func (export "logisim_ui_version") (result i32) i32.const 1) (func (export "logisim_ui") (result i64) i64.const {packed})"#
    );
    let plugin = Plugin::load(&module(&ext, &bytes)).unwrap();
    assert_eq!(plugin.library().presentation, metadata);
    plugin.check_component(&manifest().components()[0]).unwrap();
    for ext in [
        "",
        r#"(func (export "logisim_ui_version") (result i32) i32.const 999) (func (export "logisim_ui") (result i64) unreachable)"#,
    ] {
        assert_eq!(
            Plugin::load(&module(ext, &[]))
                .unwrap()
                .library()
                .presentation,
            Presentation::default()
        );
    }
    for ext in [
        r#"(func (export "logisim_ui_version") (param i32) (result i32) i32.const 1)"#.to_owned(),
        r#"(func (export "logisim_ui_version") (result i32) i32.const 1)"#.to_owned(),
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
        assert!(Plugin::load(&module(&ext, &bytes)).is_err());
    }
    let mut bad = metadata.clone();
    bad.imports[0].action = "missing".into();
    assert!(bad.validate(&manifest()).is_err());
    bad = metadata.clone();
    bad.imports[0].address_bits = 8;
    assert!(bad.validate(&manifest()).is_err());
    bad = metadata.clone();
    bad.translations
        .insert("bad locale".into(), Default::default());
    assert!(bad.validate(&manifest()).is_err());
    let malformed = vec![b'?'; bytes.len()];
    assert!(Plugin::load(&module(&ext, &malformed)).is_err());
}

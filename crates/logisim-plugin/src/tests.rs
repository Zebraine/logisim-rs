use super::*;
use logisim_plugin_api::*;
fn wasm(response: &Response, abi: u32, call: &str) -> Vec<u8> {
    let json = serde_json::to_vec(response).unwrap();
    let data = json
        .iter()
        .map(|b| format!("\\{b:02x}"))
        .collect::<String>();
    let packed = ((json.len() as u64) << 32) | 4096;
    wat::parse_str(format!(
        r#"(module
        (memory (export "memory") 1)
        (data (i32.const 4096) "{data}")
        (func (export "logisim_abi") (result i32) i32.const {abi})
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64) {call} i64.const {packed})
    )"#
    ))
    .unwrap()
}
fn manifest() -> Manifest {
    Manifest {
        abi: 1,
        id: "test.example".into(),
        version: "1.0".into(),
        name: "Test".into(),
        tools: vec![Tool {
            id: "custom-tool".into(),
            name: "Custom".into(),
            bounds: [[-20, -20], [20, 20]],
            ports: vec![],
            graphics: vec![],
            properties: vec![],
            actions: vec![],
        }],
    }
}
#[test]
fn handshake_definition_identity_duplicates_and_imports_are_checked() {
    let m = manifest();
    let bytes = wasm(&Response::Manifest(m.clone()), 1, "");
    let mut registry = Registry::default();
    registry.load(&bytes).unwrap();
    assert!(registry.load(&bytes).is_err());
    registry.instantiate(&m.components()[0]).unwrap();
    let mut wrong = m.components()[0].clone();
    wrong.tool.bounds[1][0] = 30;
    assert!(registry.instantiate(&wrong).is_err());
    wrong = m.components()[0].clone();
    wrong.version = "2".into();
    assert!(registry.instantiate(&wrong).is_err());
    assert!(Plugin::load(&wasm(&Response::Manifest(m), 2, "")).is_err());
    let imports = wat::parse_str(r#"(module (import "env" "run" (func)))"#).unwrap();
    assert!(Plugin::load(&imports).is_err());
}
#[test]
fn runaway_code_invalid_memory_and_oversized_messages_fail_boundedly() {
    let response = Response::Manifest(manifest());
    assert!(Plugin::load(&wasm(&response, 1, "(loop $forever br $forever)")).is_err());
    assert!(Plugin::load(&wasm(&response, 1, "unreachable")).is_err());
    let mut bytes = wasm(&response, 1, "");
    bytes[0] = 1;
    assert!(Plugin::load(&bytes).is_err());
    assert!(Plugin::load(&vec![0; MAX_MODULE + 1]).is_err());
    let invalid = wat::parse_str(
        r#"(module (memory (export "memory") 1)
        (func (export "logisim_abi") (result i32) i32.const 1)
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 65530)
        (func (export "logisim_call") (param i32) (result i64) i64.const 0))"#,
    )
    .unwrap();
    assert!(Plugin::load(&invalid).is_err());
    let bad = Response::Manifest(Manifest {
        abi: 9,
        ..manifest()
    });
    assert!(Plugin::load(&wasm(&bad, 1, "")).is_err());
}
#[test]
fn protocol_rejects_unknown_fields_and_noncanonical_signals() {
    assert!(serde_json::from_str::<Request>(r#"{"type":"describe","unexpected":true}"#).is_err());
    for s in [
        Signal {
            width: 0,
            value: 0,
            unknown: 0,
            error: 0,
        },
        Signal {
            width: 1,
            value: 2,
            unknown: 0,
            error: 0,
        },
        Signal {
            width: 1,
            value: 1,
            unknown: 1,
            error: 0,
        },
    ] {
        assert!(s.validate().is_err());
    }
    let mut m = manifest();
    m.tools[0].ports = vec![
        Port {
            id: "in".into(),
            label: "Input".into(),
            at: [0, 0],
            width: 1,
            output: false
        };
        2
    ];
    assert!(m.validate().is_err());
}

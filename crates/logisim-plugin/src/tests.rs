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

#[test]
fn evaluation_validates_actions_outputs_graphics_state_and_fuel() {
    // Distinguish the Describe message from Evaluate in a minimal standalone VM.
    // No native handler is substituted for Wasm execution in this test.
    fn module(manifest: &Manifest, response: &Response, execution: &str) -> Vec<u8> {
        let data = |response: &Response| {
            let json = serde_json::to_vec(response).unwrap();
            (
                json.len(),
                json.iter()
                    .map(|b| format!("\\{b:02x}"))
                    .collect::<String>(),
            )
        };
        let (mlen, m) = data(&Response::Manifest(manifest.clone()));
        let (rlen, r) = data(response);
        let describe_len = serde_json::to_vec(&Request::Describe).unwrap().len();
        let mp = ((mlen as u64) << 32) | 4096;
        let rp = ((rlen as u64) << 32) | 32768;
        wat::parse_str(format!(
            r#"(module (memory (export "memory") 8)
          (data (i32.const 4096) "{m}") (data (i32.const 32768) "{r}")
          (func (export "logisim_abi") (result i32) i32.const 1)
          (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
          (func (export "logisim_call") (param $n i32) (result i64)
            local.get $n i32.const {describe_len} i32.eq
            if (result i64) i64.const {mp}
            else {execution} i64.const {rp} end))"#
        ))
        .unwrap()
    }
    let mut m = manifest();
    m.tools[0].ports.push(Port {
        id: "out".into(),
        label: "Output".into(),
        at: [0, 0],
        width: 1,
        output: true,
    });
    let c = m.components().remove(0);
    let request = Evaluation {
        tool: c.tool.id.clone(),
        properties: c.properties.clone(),
        inputs: vec![Signal {
            width: 1,
            value: 0,
            unknown: 1,
            error: 0,
        }],
        state: vec![],
        event: Event::Reset,
    };
    let good = Update {
        outputs: vec![Signal {
            width: 1,
            value: 1,
            unknown: 0,
            error: 0,
        }],
        state: vec![42],
        graphics: vec![],
    };
    let p = Plugin::load(&module(&m, &Response::Update(good.clone()), "")).unwrap();
    let mut vm = p.instantiate(&c).unwrap();
    assert_eq!(vm.evaluate(&c, request.clone()).unwrap(), good);
    let mut invalid_action = request.clone();
    invalid_action.event = Event::Action {
        id: "undeclared".into(),
        argument: String::new(),
    };
    assert!(vm.evaluate(&c, invalid_action).is_err());
    let mut bad_width = good.clone();
    bad_width.outputs[0].width = 2;
    let mut bad_signal = good.clone();
    bad_signal.outputs[0].error = 1;
    let mut bad_state = good.clone();
    bad_state.state.resize(MAX_STATE + 1, 0);
    let mut bad_graphic = good.clone();
    bad_graphic.graphics.push(Shape::Line {
        a: [0, 0],
        b: [5000, 0],
        color: [0; 4],
    });
    for update in [
        bad_width,
        bad_signal,
        bad_state,
        bad_graphic,
        Update {
            outputs: vec![],
            ..good.clone()
        },
    ] {
        let p = Plugin::load(&module(&m, &Response::Update(update), "")).unwrap();
        assert!(
            p.instantiate(&c)
                .unwrap()
                .evaluate(&c, request.clone())
                .is_err()
        );
    }
    for execution in ["unreachable", "(loop $forever br $forever)"] {
        let p = Plugin::load(&module(&m, &Response::Update(good.clone()), execution)).unwrap();
        assert!(
            p.instantiate(&c)
                .unwrap()
                .evaluate(&c, request.clone())
                .is_err()
        );
    }
}

use super::*;
use logisim_plugin_api::*;
fn wasm(response: &Response, abi: u32, call: &str) -> Vec<u8> {
    let entry = match response {
        Response::Manifest(m) if abi == 4 => m
            .execution
            .map_or("logisim_abi4_both", |e| e.engines.entry()),
        _ => "logisim_abi",
    };
    wasm_entry(response, abi, entry, call)
}

fn wasm_entry(response: &Response, abi: u32, entry: &str, call: &str) -> Vec<u8> {
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
        (func (export "{entry}") (result i32) i32.const {abi})
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64) {call} i64.const {packed})
    )"#
    ))
    .unwrap()
}
fn manifest() -> Manifest {
    Manifest {
        execution: None,
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
fn execution_manifest(engines: AllowedEngines) -> Manifest {
    Manifest {
        abi: EXECUTION_ABI_VERSION,
        execution: Some(Execution::new(engines)),
        ..manifest()
    }
}

#[test]
fn both_runtime_choices_require_a_matching_static_and_manifest_contract() {
    for abi in 1..=3 {
        let legacy = Manifest { abi, ..manifest() };
        let bytes = wasm(&Response::Manifest(legacy.clone()), abi, "");
        for choice in [WasmEngine::Wasmi, WasmEngine::Wasmtime] {
            let p = Plugin::load_with_engine(&bytes, choice).unwrap();
            assert_eq!(p.engine(), WasmEngine::Wasmi);
            assert_eq!(p.manifest(), &legacy);
        }
    }
    for allowed in [
        AllowedEngines::Wasmi,
        AllowedEngines::Wasmtime,
        AllowedEngines::Both,
    ] {
        let m = execution_manifest(allowed);
        let bytes = wasm(&Response::Manifest(m.clone()), 4, "");
        for engine in [WasmEngine::Wasmi, WasmEngine::Wasmtime] {
            let loaded = Plugin::load_with_engine(&bytes, engine);
            match (allowed, engine) {
                (AllowedEngines::Wasmi, WasmEngine::Wasmtime)
                | (AllowedEngines::Wasmtime, WasmEngine::Wasmi) => {
                    assert!(loaded.err().unwrap().contains("does not permit"))
                }
                _ if !engine.available() => {
                    assert!(loaded.err().unwrap().contains("does not include"))
                }
                _ => {
                    let plugin = loaded.unwrap();
                    assert_eq!(plugin.engine(), engine);
                    assert_eq!(plugin.manifest(), &m);
                }
            }
        }
    }
    let m = Response::Manifest(execution_manifest(AllowedEngines::Wasmi));
    for (name, abi, expected) in [
        ("logisim_abi4_both", 4, "declaration differ"),
        ("logisim_abi4_both", 3, "Unsupported plugin ABI"),
        ("logisim_abi", 4, "Unsupported plugin ABI"),
        ("logisim_abi4_other", 4, "Unknown ABI 4"),
    ] {
        let err = Plugin::load(&wasm_entry(&m, abi, name, "")).err().unwrap();
        assert!(err.contains(expected), "{err}");
    }
    let ambiguous = wat::parse_str(
        r#"(module
        (func (export "logisim_abi") (export "logisim_abi4_both") (result i32) i32.const 3)
        (func $stop unreachable) (start $stop))"#,
    )
    .unwrap();
    assert!(
        Plugin::load_with_engine(&ambiguous, WasmEngine::Wasmtime)
            .err()
            .unwrap()
            .contains("one plugin ABI")
    );
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

#[test]
fn instance_enforces_memory_table_and_response_limits() {
    // A successful manifest proves rejection in the variants below is due to the
    // resource boundary rather than a missing export or bad handshake.
    let response = Response::Manifest(execution_manifest(AllowedEngines::Both));
    let json = serde_json::to_string(&response).unwrap();
    let data: String = json.bytes().map(|b| format!("\\{b:02x}")).collect();
    let module = |pages: u32, entries: u32, response_len: usize| {
        wat::parse_str(format!(
            r#"(module
            (memory (export "memory") {pages}) (table {entries} funcref)
            (data (i32.const 4096) "{data}")
            (func (export "logisim_abi4_both") (result i32) i32.const 4)
            (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
            (func (export "logisim_call") (param i32) (result i64) i64.const {}))"#,
            ((response_len as u64) << 32) | 4096
        ))
        .unwrap()
    };
    for engine in [WasmEngine::Wasmi, WasmEngine::Wasmtime]
        .into_iter()
        .filter(|e| e.available())
    {
        let load = |bytes: &[u8]| Plugin::load_with_engine(bytes, engine);
        assert!(load(&module(256, 10_000, json.len())).is_ok());
        assert!(load(&module(257, 10_000, json.len())).is_err());
        assert!(load(&module(256, 10_001, json.len())).is_err());
        assert_eq!(
            load(&module(1, 1, MAX_MESSAGE + 1)).err().as_deref(),
            Some("Plugin response exceeds message limit.")
        );
    }
}

#[cfg(feature = "engine-study")]
mod engine_contract;

#[test]
fn loading_pins_the_engine_per_module_and_implicit_load_stays_wasmi() {
    let manifest = execution_manifest(AllowedEngines::Both);
    let bytes = wasm(&Response::Manifest(manifest.clone()), 4, "");
    let default = Plugin::load(&bytes).unwrap();
    assert_eq!(default.engine(), WasmEngine::Wasmi);
    let explicit = Plugin::load_dependency_with_engine(
        &bytes,
        &manifest.id,
        &manifest.version,
        &manifest.components(),
        WasmEngine::Wasmtime,
    );
    if WasmEngine::Wasmtime.available() {
        let native = explicit.unwrap();
        assert_eq!(native.engine(), WasmEngine::Wasmtime);
        assert_eq!(native.manifest(), default.manifest());
        for module in [default, native] {
            module.instantiate(&manifest.components()[0]).unwrap();
        }
    } else {
        assert!(explicit.err().unwrap().contains("does not include"));
    }
}

#[cfg(all(target_os = "linux", feature = "wasmtime-cache"))]
mod module_cache;

#[test]
fn execution_metadata_is_strict_and_old_manifest_bytes_omit_it() {
    let legacy = serde_json::to_value(manifest()).unwrap();
    assert!(legacy.get("execution").is_none());
    let mut json = legacy.clone();
    json["abi"] = 4.into();
    assert!(
        serde_json::from_value::<Manifest>(json.clone())
            .unwrap()
            .validate()
            .is_err()
    );
    for bad in [
        serde_json::Value::Null,
        serde_json::json!({"fuel":"portable","engines":"both"}),
        serde_json::json!({"fuel":"engine-specific","engines":"auto"}),
        serde_json::json!({"fuel":"engine-specific","engines":"both","budget":1}),
    ] {
        json["execution"] = bad;
        assert!(serde_json::from_value::<Manifest>(json.clone()).is_err());
    }
    json["execution"] = serde_json::to_value(Execution::new(AllowedEngines::Both)).unwrap();
    assert_eq!(
        serde_json::from_value::<Manifest>(json.clone()).unwrap(),
        execution_manifest(AllowedEngines::Both)
    );
    for abi in 1..=3 {
        json["abi"] = abi.into();
        assert!(
            serde_json::from_value::<Manifest>(json.clone())
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

use super::*;
use logisim_core::{Component, Point, plugin::api};
use logisim_plugin::Plugin;

fn module(fail: bool) -> Plugin {
    let manifest = api::Manifest {
        abi: 3,
        id: "test.worker".into(),
        version: "1".into(),
        name: "Worker test".into(),
        tools: vec![api::Tool {
            id: "test".into(),
            name: "Test".into(),
            bounds: [[-10, -10], [10, 10]],
            ports: vec![api::Port {
                id: "out".into(),
                label: "Output".into(),
                at: [10, 0],
                width: 1,
                output: true,
            }],
            graphics: vec![],
            properties: vec![],
            actions: vec![],
        }],
    };
    let responses = [
        api::Response::Manifest(manifest),
        api::Response::StableContract(Some(api::StableContract {
            outputs: vec![api::OutputDependency {
                port: 0,
                inputs: vec![],
            }],
            graphics_inputs: vec![],
            state_inputs: Some(vec![]),
        })),
        api::Response::Update(api::Update {
            outputs: vec![logisim_sim::Value::known(1, 0).into()],
            state: vec![],
            graphics: vec![],
        }),
        api::Response::StableOutputs(api::StableOutputs {
            outputs: vec![logisim_sim::Value::known(1, 1).into()],
            graphics: vec![],
        }),
        if fail {
            api::Response::Error("sample failed".into())
        } else {
            api::Response::StableTransition(api::StateTransition {
                state: vec![1],
                outputs_changed: false,
                fixed_point: true,
            })
        },
    ];
    let mut data = String::new();
    let mut packed = vec![];
    for (i, response) in responses.into_iter().enumerate() {
        let bytes = serde_json::to_vec(&response).unwrap();
        let at = 32768 + i * 16384;
        let escaped: String = bytes.iter().map(|b| format!("\\{b:02x}")).collect();
        data.push_str(&format!("(data (i32.const {at}) \"{escaped}\")"));
        packed.push(((bytes.len() as u64) << 32) | at as u64);
    }
    let [manifest, contract, event, outputs, sample] = packed.as_slice() else {
        unreachable!()
    };
    let wasm = wat::parse_str(format!(
        r#"(module (memory (export "memory") 2) {data}
        (func (export "logisim_abi") (result i32) i32.const 3)
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64)
            i32.const 9 i32.load8_u i32.const 100 i32.eq
            if (result i64) i64.const {manifest}
            else i32.const 9 i32.load8_u i32.const 101 i32.eq
                if (result i64) i64.const {event}
                else i32.const 16 i32.load8_u i32.const 99 i32.eq
                    if (result i64) i64.const {contract}
                    else i32.const 16 i32.load8_u i32.const 111 i32.eq
                        if (result i64) i64.const {outputs} else i64.const {sample} end
                    end
                end
            end))"#
    ))
    .unwrap();
    Plugin::load(&wasm).unwrap()
}

#[test]
fn runtime_fault_stays_visible_without_event_fallback_and_module_replacement_recovers() {
    let plugin = module(true);
    let mut project = Project::default();
    let id = project
        .main_circuit_mut()
        .circuit
        .add_component(Component::from_plugin(
            plugin.manifest().components().remove(0),
            Point::ZERO,
        ));
    let mut plugins = Registry::default();
    plugins.insert(plugin).unwrap();
    let mut context = Context {
        revision: 1,
        root: project.main,
        circuit: project.main,
        project: Arc::new(project),
        path: vec![],
        language: Language::English,
        plugins,
    };
    let mut engine = Engine::default();
    engine.command(Command::Configure(context.clone()));
    for _ in 0..128 {
        engine.advance();
        if !engine.sim.enabled() {
            break;
        }
    }
    assert_eq!(engine.propagation, Propagation::Compiled);
    assert!(!engine.sim.enabled());
    assert!(engine.dirty);
    let view = engine.snapshot().unwrap().view;
    assert!(view.plugin_errors[&id].contains("sample failed"));
    assert_eq!(view.ports[&(id, 0)].binary, "E");
    engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
    engine.advance();
    assert!(!engine.sim.enabled());
    assert_eq!(engine.propagation, Propagation::Compiled);
    let mut plugins = Registry::default();
    plugins.insert(module(false)).unwrap();
    context.plugins = plugins;
    engine.command(Command::Configure(context));
    engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
    super::tests::finish(&mut engine);
    assert_eq!(engine.propagation, Propagation::Compiled);
    assert!(engine.snapshot().unwrap().view.plugin_errors.is_empty());
    assert_eq!(
        engine
            .sim
            .view(&[])
            .unwrap()
            .port_value(id, 0)
            .unwrap()
            .number(),
        Some(1)
    );
    engine.command(Command::Action(1, SimulationAction::Reset));
    super::tests::finish(&mut engine);
    assert_eq!(
        engine
            .sim
            .view(&[])
            .unwrap()
            .port_value(id, 0)
            .unwrap()
            .number(),
        Some(1)
    );
}

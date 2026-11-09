use logisim_core::{
    Component, Point, Project,
    plugin::{PluginSource, api},
};

pub(crate) fn fixture(id: &str, value: u32) -> (Project, Vec<u8>) {
    let manifest = api::Manifest {
        abi: 1,
        id: id.into(),
        version: "1.0".into(),
        name: "External test".into(),
        tools: vec![api::Tool {
            id: "tool".into(),
            name: "Plugin-only tool".into(),
            bounds: [[-20, -20], [20, 20]],
            ports: vec![api::Port {
                id: "out".into(),
                label: "Output".into(),
                at: [20, 0],
                width: 1,
                output: true,
            }],
            graphics: vec![api::Shape::Rect {
                min: [-20, -20],
                max: [20, 20],
                fill: [100, 200, 250, 255],
                stroke: [0, 0, 0, 255],
            }],
            properties: vec![],
            actions: vec![],
        }],
    };
    let mut p = Project::default();
    p.main_circuit_mut()
        .circuit
        .add_component(Component::from_plugin(
            manifest.components().remove(0),
            Point::new(100, 100),
        ));
    p.plugin_sources.push(PluginSource {
        plugin: id.into(),
        version: "1.0".into(),
        path: "missing.wasm".into(),
        fallback: String::new(),
    });
    let encode = |response: api::Response| {
        let json = serde_json::to_vec(&response).unwrap();
        (
            json.len(),
            json.iter()
                .map(|b| format!("\\{b:02x}"))
                .collect::<String>(),
        )
    };
    let (mlen, m) = encode(api::Response::Manifest(manifest));
    let (ulen, u) = encode(api::Response::Update(api::Update {
        outputs: vec![api::Signal {
            width: 1,
            value,
            unknown: 0,
            error: 0,
        }],
        state: vec![],
        graphics: vec![],
    }));
    let len = serde_json::to_vec(&api::Request::Describe).unwrap().len();
    let mp = (mlen as u64) << 32 | 4096;
    let up = (ulen as u64) << 32 | 32768;
    let bytes = wat::parse_str(format!(r#"(module (memory (export "memory") 1)
        (data (i32.const 4096) "{m}") (data (i32.const 32768) "{u}")
        (func (export "logisim_abi") (result i32) i32.const 1)
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param $n i32) (result i64)
            local.get $n i32.const {len} i32.eq if (result i64) i64.const {mp} else i64.const {up} end))"#)).unwrap();
    (p, bytes)
}

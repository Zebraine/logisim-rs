//! A real register-file with read port A wired to register 1.
use logisim_core::{
    Component, Kind, Point, Project,
    plugin::{PluginSource, api},
};
use std::{fs, path::Path};

pub fn plugin_tool(module: &str, tool: &str) -> (PluginSource, api::Component) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../plugins/{module}/{module}.wasm"));
    let plugin = logisim_plugin::Plugin::load(&fs::read(&path).unwrap()).unwrap();
    let manifest = plugin.manifest();
    let source = PluginSource {
        plugin: manifest.id.clone(),
        version: manifest.version.clone(),
        path: path.to_str().unwrap().into(),
        fallback: String::new(),
    };
    let definition = manifest
        .components()
        .into_iter()
        .find(|c| c.tool.id == tool)
        .unwrap();
    (source, definition)
}

pub fn register_file() -> Project {
    let (source, tool) = plugin_tool("course", "register-file");
    let mut project = Project::default();
    project.plugin_sources.push(source);
    let circuit = &mut project.main_circuit_mut().circuit;
    let rf = Component::from_plugin(tool, Point::new(320, 200));
    let ports = rf.ports();
    let id = circuit.add_component(rf);
    // Give both transports the same saved target in their independent sessions.
    circuit.agent_ids.items.insert(id, 0x42);
    let mut address = Component::new(Kind::Constant, ports[6]);
    address.bits = 5;
    address.constant = 1;
    circuit.add_component(address);
    circuit.add_component(Component::new(Kind::Probe, ports[0]));
    project
}

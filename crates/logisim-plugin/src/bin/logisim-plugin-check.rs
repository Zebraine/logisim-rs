//! Verify an SDK-built artifact against the actual host ABI, without opening the editor.
use logisim_plugin::{MAX_MODULE, Plugin};
use logisim_plugin_api::{Evaluation, Event, Signal};
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: logisim-plugin-check module.wasm")?;
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(MAX_MODULE as u64 + 1)
        .read_to_end(&mut bytes)?;
    let plugin = Plugin::load(&bytes)?;
    for component in plugin.manifest().components() {
        let inputs = component
            .tool
            .ports
            .iter()
            .map(|p| Signal {
                width: p.width,
                value: 0,
                unknown: 0,
                error: 0,
            })
            .collect();
        let request = Evaluation {
            tool: component.tool.id.clone(),
            properties: component.properties.clone(),
            inputs,
            state: vec![],
            event: Event::Reset,
        };
        let update = plugin
            .instantiate(&component)?
            .evaluate(&component, request)?;
        println!(
            "{}: {} ports, {} static + {} runtime graphics",
            component.tool.id,
            component.tool.ports.len(),
            component.tool.graphics.len(),
            update.graphics.len()
        );
    }
    println!(
        "{} {}: ABI verified",
        plugin.manifest().id,
        plugin.manifest().version
    );
    Ok(())
}

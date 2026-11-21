//! Verify SDK-built artifacts with the host loader, including packaged compatibility pairs.
use logisim_plugin::{MAX_MODULE, Plugin, WasmEngine};
use logisim_plugin_api::{AllowedEngines, Evaluation, Event, Execution, Signal};
use std::{ffi::OsStr, io::Read};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn load(path: &OsStr, requested: WasmEngine) -> Result<Plugin> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_MODULE as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(Plugin::load_with_engine(&bytes, requested)?)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [path] => verify(&load(path, WasmEngine::Wasmi)?),
        [option, old, current] if option == "--compare-abi3" => {
            let legacy = load(old, WasmEngine::Wasmtime)?;
            assert_eq!(legacy.engine(), WasmEngine::Wasmi);
            assert_eq!(legacy.manifest().abi, 3);
            assert_eq!(legacy.manifest().execution, None);
            let mut expected = legacy.manifest().clone();
            expected.abi = 4;
            expected.execution = Some(Execution::new(AllowedEngines::Both));
            let old_ui = legacy.library();
            for requested in [WasmEngine::Wasmi, WasmEngine::Wasmtime] {
                let plugin = load(current, requested)?;
                assert_eq!(plugin.engine(), requested);
                assert_eq!(plugin.manifest(), &expected);
                let ui = plugin.library();
                assert_eq!(ui.presentation, old_ui.presentation);
                assert_eq!(ui.interaction, old_ui.interaction);
                verify(&plugin)?;
            }
            println!(
                "{}: ABI 3 uses wasmi, ABI 4 verified on wasmi and Wasmtime",
                expected.id
            );
            Ok(())
        }
        _ => Err("Usage: logisim-plugin-check MODULE | --compare-abi3 LEGACY CURRENT".into()),
    }
}

fn verify(plugin: &Plugin) -> Result<()> {
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
        let mut vm = plugin.instantiate(&component)?;
        let update = vm.evaluate(&component, request)?;
        if let Some(image) = vm.raster(&component, &update.state, None)? {
            assert_eq!(
                vm.raster(&component, &update.state, Some(&image))?
                    .as_deref(),
                Some(image.as_ref())
            );
            println!(
                "{}: raster {}×{}",
                component.tool.id, image.width, image.height
            );
        }
        println!(
            "{}: {} ports, {} static + {} runtime graphics",
            component.tool.id,
            component.tool.ports.len(),
            component.tool.graphics.len(),
            update.graphics.len()
        );
        if let Some(instance) = plugin.instantiate_stable(&component)? {
            println!(
                "{}: stable transport = {}",
                component.tool.id,
                if instance.uses_linear_memory() {
                    "linear memory v1"
                } else {
                    "JSON"
                }
            );
        }
    }
    println!(
        "{} {}: ABI verified",
        plugin.manifest().id,
        plugin.manifest().version
    );
    Ok(())
}

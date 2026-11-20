//! Initialize the main circuit once, then expose only its immutable observations.
use crate::io;
use logisim_core::{
    Kind, Project,
    plugin::{PluginSource, api},
};
use logisim_plugin::{Plugin, Registry};
use logisim_sim::{Bit, Problem, ProjectSimulator, Value as Signal};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

#[derive(Default)]
struct Dependency<'a> {
    source: Option<&'a PluginSource>,
    components: Vec<&'a api::Component>,
}

fn load_dependencies(project: &Project, base: &Path) -> (Registry, Vec<Value>) {
    let mut dependencies = BTreeMap::new();
    for source in &project.plugin_sources {
        dependencies.insert(
            (source.plugin.as_str(), source.version.as_str()),
            Dependency {
                source: Some(source),
                components: Vec::new(),
            },
        );
    }
    for component in project
        .circuits
        .iter()
        .flat_map(|d| &d.circuit.components)
        .filter_map(|c| c.plugin.as_ref())
    {
        dependencies
            .entry((&component.plugin, &component.version))
            .or_default()
            .components
            .push(component);
    }
    let mut registry = Registry::default();
    let mut reports = Vec::new();
    for ((id, version), dep) in dependencies {
        let mut candidates = Vec::new();
        if let Some(source) = dep.source {
            for name in [&source.path, &source.fallback]
                .into_iter()
                .filter(|s| !s.is_empty())
            {
                candidates.push(Path::new(name));
            }
            candidates.sort_by_key(|p| p.is_absolute());
            candidates.dedup();
        }
        let mut errors = Vec::new();
        let mut found = None;
        for path in candidates {
            let path = base.join(path);
            let loaded = io::read(&path)
                .map_err(|e| e.message)
                .and_then(|bytes| {
                    Plugin::load_dependency(&bytes, id, version, dep.components.iter().copied())
                })
                .and_then(|p| registry.insert(p));
            match loaded {
                Ok(()) => {
                    found = Some(path);
                    break;
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        if let Some(path) = found {
            reports.push(json!({"id":id,"version":version,"status":"loaded","path":path}));
        } else {
            let error = if errors.is_empty() {
                "No plugin path is recorded.".to_owned()
            } else {
                errors.join("\n")
            };
            registry.unavailable(id.into(), version.into(), error.clone());
            reports.push(json!({"id":id,"version":version,"status":"missing","error":error}));
        }
    }
    (registry, reports)
}

fn value(v: Signal) -> Value {
    json!({"bits":v.width(),"binary":v.binary(),"number":v.number()})
}
fn tone(v: Option<Signal>, mismatch: bool) -> &'static str {
    if mismatch {
        return "width_mismatch";
    }
    match v {
        None => "unknown",
        Some(v) if v.contains(Bit::Error) => "error",
        Some(v) if v.width() == 0 || v.contains(Bit::Unknown) => "unknown",
        Some(v) if v.width() > 1 => "bus",
        Some(v) if v.number() == Some(1) => "high",
        Some(_) => "low",
    }
}

pub fn observe(project: &Project, path: &Path, session: &str) -> Value {
    let (registry, plugins) =
        load_dependencies(project, path.parent().expect("absolute project file"));
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry);
    sim.synchronize(project, project.main);
    let mut error = sim.error().map(str::to_owned);
    let mut fallback = None;
    let mut engine = "stable";
    if error.is_none() {
        match sim.prepare_compiled() {
            Ok(_) => {
                if let Err(e) = sim.settle_compiled() {
                    sim.set_enabled(false);
                    error = Some(e.to_string());
                }
            }
            Err(e) => {
                engine = "event";
                fallback = Some(e.to_string());
                while sim.step() {}
            }
        }
    }
    let mut components = Vec::new();
    let mut wires = Vec::new();
    let mut pins = Vec::new();
    let mut probes = Vec::new();
    let mut inputs = Vec::new();
    if let Some(root) = sim.view(&[]) {
        let circuit = &project.main_circuit().circuit;
        for c in &circuit.components {
            let mut ports = Vec::new();
            let mut drives = Vec::new();
            for index in 0..c.local_ports().len() {
                if let Some(v) = root.port_value(c.id, index) {
                    ports.push(json!({"index":index,"value":value(v)}));
                }
                if let Some(drive) = root.drive_value(c.id, index) {
                    drives.push(drive);
                }
            }
            let input = root.input(c.id);
            let observed = if matches!(c.kind, Kind::Pin | Kind::Probe) {
                root.port_value(c.id, 0)
            } else {
                root.stored_value(c.id)
                    .or(input)
                    .or(match drives.as_slice() {
                        [only] => Some(*only),
                        _ => None,
                    })
            };
            components.push(json!({"id":c.id,"kind":c.kind.name(),"label":c.label,"value":observed.map(value),"ports":ports}));
            let reading = || json!({"id":c.id,"label":c.label,"value":observed.map(value)});
            match c.kind {
                Kind::Pin => pins.push(reading()),
                Kind::Probe => probes.push(reading()),
                _ => {}
            }
            if let Some(v) = input {
                inputs.push(json!({"id":c.id,"label":c.label,"value":value(v)}));
            }
        }
        for w in &circuit.wires {
            let v = root.wire_value(w.id);
            let mismatch = root.wire_mismatch(w.id);
            wires.push(json!({"id":w.id,"value":v.map(value),"width_mismatch":mismatch,"tone":tone(v,mismatch)}));
        }
    }
    let mut diagnostics: Vec<_> = sim
        .diagnostics(&[])
        .into_iter()
        .map(|d| {
            let problem = match d.problem {
                Problem::WidthMismatch => "width_mismatch",
                Problem::ErrorSignal => "error_signal",
                Problem::PropagationLimit => "propagation_limit",
            };
            json!({"problem":problem,"components":d.components,"wires":d.wires})
        })
        .collect();
    for (path, id, message) in sim.plugin_failures() {
        diagnostics.push(json!({"problem":"plugin_error","instance_path":path,"components":[id],"wires":[],"message":message}));
    }
    json!({"session_id":session,"main":project.main,"enabled":sim.enabled(),"limited":sim.limited(),"steps":sim.steps(),"ticks":sim.ticks(),
        "error":error,"engine":engine,"fallback_reason":fallback,"components":components,"wires":wires,"pins":pins,"probes":probes,"inputs":inputs,"diagnostics":diagnostics,"plugins":plugins})
}

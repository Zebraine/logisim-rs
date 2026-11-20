//! Session-owned main-circuit simulation and current signal observations.
pub(crate) mod control;
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

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SimulationMode {
    #[default]
    Auto,
    Stable,
    Event,
}

enum Engine {
    Stable,
    Event { fallback: Option<String> },
}

pub(crate) struct Runtime {
    sim: ProjectSimulator,
    mode: SimulationMode,
    engine: Engine,
    plugins: Vec<Value>,
    failure: Option<String>,
    step_origin: u64,
}

impl Runtime {
    pub(crate) fn open(project: &Project, path: &Path, mode: SimulationMode) -> Self {
        let (registry, plugins) =
            load_dependencies(project, path.parent().expect("absolute project file"));
        let mut sim = ProjectSimulator::default();
        sim.set_plugins(registry);
        sim.synchronize(project, project.main);
        let mut runtime = Self {
            sim,
            mode,
            engine: Engine::Stable,
            plugins,
            failure: None,
            step_origin: 0,
        };
        runtime.initialize();
        runtime
    }

    fn initialize(&mut self) {
        self.failure = self.sim.error().map(str::to_owned);
        self.check_execution();
        if self.failure.is_some() {
            return;
        }
        self.engine = match self.mode {
            SimulationMode::Event => Engine::Event { fallback: None },
            _ => match self.sim.prepare_compiled() {
                Ok(_) => Engine::Stable,
                Err(e) => Engine::Event {
                    fallback: Some(e.to_string()),
                },
            },
        };
        self.propagate();
    }

    fn propagate(&mut self) {
        match self.engine {
            Engine::Stable => {
                if let Err(e) = self.sim.settle_compiled() {
                    self.failure = Some(e.to_string());
                }
            }
            Engine::Event { .. } => {
                while self.sim.step() {
                    if self.sim.runtime_plugin_failure().is_some() {
                        break;
                    }
                }
            }
        }
        self.check_execution();
    }

    fn check_execution(&mut self) {
        if self.failure.is_none() {
            self.failure = self
                .sim
                .runtime_plugin_failure()
                .map(str::to_owned)
                .or_else(|| {
                    self.sim
                        .limited()
                        .then(|| "Propagation limit reached.".into())
                });
        }
        if self.failure.is_some() {
            self.sim.set_enabled(false);
        }
    }

    pub(crate) fn state(&self, project: &Project, session: &str) -> Value {
        let sim = &self.sim;
        let (engine, fallback) = match &self.engine {
            Engine::Stable => ("stable", None),
            Engine::Event { fallback } => ("event", fallback.as_deref()),
        };
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
                        ports.push(json!({"index":index,"value":value(v),"tone":tone(Some(v),root.port_mismatch(c.id,index))}));
                    }
                    if let Some(drive) = root.drive_value(c.id, index) {
                        drives.push(drive);
                    }
                }
                let input = root.input(c.id);
                let observed = if matches!(c.kind, Kind::Pin | Kind::Probe | Kind::Led) {
                    root.port_value(c.id, 0)
                } else {
                    root.stored_value(c.id)
                        .or(input)
                        .or(match drives.as_slice() {
                            [only] => Some(*only),
                            _ => None,
                        })
                };
                let observed_tone = tone(
                    observed,
                    matches!(c.kind, Kind::Pin | Kind::Probe | Kind::Led)
                        && root.port_mismatch(c.id, 0),
                );
                components.push(json!({"id":c.id,"kind":c.kind.name(),"label":c.label,"value":observed.map(value),"tone":observed_tone,"ports":ports}));
                let reading = || json!({"id":c.id,"label":c.label,"value":observed.map(value),"tone":observed_tone});
                match c.kind {
                    Kind::Pin => pins.push(reading()),
                    Kind::Probe => probes.push(reading()),
                    _ => {}
                }
                if let Some(v) = input {
                    inputs.push(json!({"id":c.id,"label":c.label,"value":value(v),"tone":tone(Some(v),false)}));
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
        json!({"session_id":session,"main":project.main,"enabled":sim.enabled(),"limited":sim.limited(),"steps":sim.steps() - self.step_origin,"ticks":sim.ticks(),
        "pending_count":sim.pending_count(),"mode":self.mode,"error":self.failure,"engine":engine,"fallback_reason":fallback,"components":components,"wires":wires,"pins":pins,"probes":probes,"inputs":inputs,"diagnostics":diagnostics,"plugins":self.plugins})
    }
}

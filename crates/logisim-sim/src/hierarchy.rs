//! Isolated circuit instances linked by directional, four-state interface signals.
mod control;
mod stable;
#[cfg(test)]
mod tests;
use crate::{Diagnostic, Problem, Simulator, Value};
use logisim_core::{Id, Kind, Project};
use std::collections::BTreeMap;

type Path = Vec<Id>;
struct Instance {
    definition: Id,
    sim: Simulator,
}
struct CachedRoot {
    states: BTreeMap<Path, Instance>,
    ticks: u64,
}
struct Bridge {
    parent: Path,
    instance: Id,
    index: usize,
    child: Path,
    pin: Id,
    output: bool,
}

pub struct ProjectSimulator {
    plugins: logisim_plugin::Registry,
    project: Project,
    root: Id,
    states: BTreeMap<Path, Instance>,
    bridges: Vec<Bridge>,
    cached_roots: BTreeMap<Id, CachedRoot>,
    ticks: u64,
    enabled: bool,
    progress: crate::progress::Propagation,
    limited: bool,
    error: Option<String>,
    stable_checked: Option<Result<(), crate::stable::Error>>,
    stable_active: bool,
}
impl Default for ProjectSimulator {
    fn default() -> Self {
        Self {
            plugins: Default::default(),
            project: Project::default(),
            root: 0,
            states: BTreeMap::new(),
            bridges: vec![],
            cached_roots: BTreeMap::new(),
            ticks: 0,
            enabled: true,
            progress: crate::progress::Propagation::default(),
            limited: false,
            error: None,
            stable_checked: None,
            stable_active: false,
        }
    }
}
impl ProjectSimulator {
    pub fn plugin_failures(&self) -> Vec<(Vec<Id>, Id, String)> {
        self.states
            .iter()
            .flat_map(|(path, instance)| {
                instance
                    .sim
                    .plugin_errors()
                    .map(|(id, e)| (path.clone(), id, e.into()))
            })
            .collect()
    }
    pub fn set_plugins(&mut self, plugins: logisim_plugin::Registry) {
        if self.plugins.same(&plugins) {
            return;
        }
        self.resume_event();
        self.plugins = plugins;
        self.stable_checked = None;
        for instance in self.states.values_mut().chain(
            self.cached_roots
                .values_mut()
                .flat_map(|c| c.states.values_mut()),
        ) {
            instance.sim.set_plugins(self.plugins.clone());
        }
        self.progress.restart();
        self.limited = false;
    }
    pub fn plugin_action(&mut self, path: &[Id], id: Id, action: String, argument: String) {
        if let Some(instance) = self.states.get_mut(path) {
            instance.sim.plugin_action(id, action, argument);
            self.progress.restart();
            self.limited = false;
        }
    }
    pub fn synchronize(&mut self, project: &Project, root: Id) -> bool {
        if self.project == *project && self.root == root {
            return false;
        }
        let same_root = self.root == root;
        if !same_root {
            self.resume_event();
        }
        let mut old = if same_root {
            std::mem::take(&mut self.states)
        } else {
            if !self.states.is_empty() {
                self.cached_roots.insert(
                    self.root,
                    CachedRoot {
                        states: std::mem::take(&mut self.states),
                        ticks: self.ticks,
                    },
                );
            }
            let cached = self.cached_roots.remove(&root);
            self.ticks = cached.as_ref().map_or(0, |c| c.ticks);
            cached.map_or_else(BTreeMap::new, |c| c.states)
        };
        self.cached_roots
            .retain(|id, _| project.circuit(*id).is_some());
        self.project = project.clone();
        self.root = root;
        self.states.clear();
        self.bridges.clear();
        self.error = None;
        let compiled = (|| {
            project.validate()?;
            if project.circuit(root).is_none() {
                return Err("Missing simulation root.".into());
            }
            let mut stack = vec![(vec![], root)];
            let mut items = 0;
            let mut changed = !same_root;
            while let Some((path, definition)) = stack.pop() {
                if path.len() > 32 || self.states.len() >= 1024 {
                    return Err("Hierarchy exceeds 32 levels or 1024 instances.".into());
                }
                let circuit = &project
                    .circuit(definition)
                    .ok_or("Missing circuit")?
                    .circuit;
                items += circuit.components.len() + circuit.wires.len();
                if items > 65_536 {
                    return Err("Expanded hierarchy exceeds 65536 items.".into());
                }
                let mut instance = old
                    .remove(&path)
                    .filter(|s| s.definition == definition)
                    .unwrap_or_else(|| {
                        changed = true;
                        Instance {
                            definition,
                            sim: Simulator::default(),
                        }
                    });
                instance.sim.set_plugins(self.plugins.clone());
                changed |= instance.sim.synchronize(circuit);
                for c in &circuit.components {
                    if let Kind::Subcircuit(id) = c.kind {
                        let mut child = path.clone();
                        child.push(c.id);
                        stack.push((child.clone(), id));
                        for (index, port) in c.interface.ports.iter().enumerate() {
                            self.bridges.push(Bridge {
                                parent: path.clone(),
                                instance: c.id,
                                index,
                                child: child.clone(),
                                pin: port.pin,
                                output: port.output,
                            });
                        }
                    }
                }
                self.states.insert(path, instance);
            }
            changed |= !old.is_empty();
            Ok(changed)
        })();
        match compiled {
            Ok(changed) => {
                if changed {
                    self.stable_checked = None;
                    self.resume_event();
                    self.progress.reset();
                    self.limited = false;
                }
                self.refresh_bridges();
                changed
            }
            Err(e) => {
                self.stable_checked = None;
                self.stable_active = false;
                self.states.clear();
                self.bridges.clear();
                self.error = Some(e);
                true
            }
        }
    }
    fn refresh_bridges(&mut self) {
        // Gather before mutating: one deterministic boundary batch, independent of tree order.
        let values: Vec<_> = self
            .bridges
            .iter()
            .map(|b| {
                if b.output {
                    self.states[&b.child].sim.port_value(b.pin, 0)
                } else {
                    self.states[&b.parent].sim.port_value(b.instance, b.index)
                }
            })
            .collect();
        for (bridge, value) in self.bridges.iter().zip(values) {
            if let Some(value) = value {
                if bridge.output {
                    self.states
                        .get_mut(&bridge.parent)
                        .unwrap()
                        .sim
                        .drive_port((bridge.instance, bridge.index), value);
                } else {
                    self.states
                        .get_mut(&bridge.child)
                        .unwrap()
                        .sim
                        .external_input(bridge.pin, value);
                }
            }
        }
    }
    pub fn view(&self, path: &[Id]) -> Option<&Simulator> {
        self.states.get(path).map(|s| &s.sim)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn limited(&self) -> bool {
        self.limited
    }
    pub fn steps(&self) -> u64 {
        self.progress.steps()
    }
    pub fn pending_count(&self) -> usize {
        self.pending_drives() + usize::from(self.stable_active)
    }
    fn pending_drives(&self) -> usize {
        self.states.values().map(|s| s.sim.pending_count()).sum()
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.limited = false;
            self.progress.restart();
            for state in self.states.values_mut() {
                state.sim.set_enabled(true);
            }
        }
    }
    pub fn step(&mut self) -> bool {
        if self.limited || self.error.is_some() {
            return false;
        }
        self.resume_event();
        let mut advanced = false;
        for state in self.states.values_mut() {
            advanced |= state.sim.step();
        }
        self.refresh_bridges();
        if advanced {
            let exhausted = self.progress.record_batch(self.pending_count() != 0);
            if exhausted || self.states.values().any(|s| s.sim.limited()) {
                self.limited = true;
                self.enabled = false;
            }
        }
        advanced
    }
    pub fn advance(&mut self, budget: usize) {
        if self.enabled {
            for _ in 0..budget {
                if !self.step() {
                    break;
                }
            }
        }
    }
    pub fn reset(&mut self) {
        self.stable_active = false;
        self.cached_roots.clear();
        self.ticks = 0;
        for state in self.states.values_mut() {
            state.sim.reset();
        }
        self.progress.reset();
        self.limited = false;
        self.refresh_bridges();
    }
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn has_clocks(&self) -> bool {
        self.states.values().any(|s| s.sim.has_clocks())
    }
    pub fn tick(&mut self) -> bool {
        if self.limited || self.error.is_some() || self.pending_count() > 0 || !self.has_clocks() {
            return false;
        }
        for state in self.states.values_mut() {
            state.sim.tick_clocks();
        }
        self.ticks = self.ticks.saturating_add(1);
        true
    }
    pub fn diagnostics(&self, path: &[Id]) -> Vec<Diagnostic> {
        let mut out = vec![];
        for (child, state) in &self.states {
            if child == path {
                out.extend(state.sim.diagnostics());
            } else if child.starts_with(path)
                && let Some(instance) = child.get(path.len())
            {
                for issue in state.sim.diagnostics() {
                    out.push(Diagnostic {
                        problem: issue.problem,
                        components: vec![*instance],
                        wires: vec![],
                    });
                }
            }
        }
        if self.limited {
            out.push(Diagnostic {
                problem: Problem::PropagationLimit,
                components: vec![],
                wires: vec![],
            });
        }
        out
    }
}

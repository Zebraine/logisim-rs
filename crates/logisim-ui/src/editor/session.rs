//! Document working copy and the context restored by editing transactions.
use logisim_core::{Circuit, Id, Kind, Project};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Context {
    pub active: Id,
    pub root: Id,
    pub path: Vec<Id>,
    pub appearance_mode: bool,
}

/// A checkpoint contains its active circuit only in `project`. Preview gestures
/// read that circuit through `circuit()` instead of keeping a second copy.
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub project: Project,
    pub context: Context,
    pub selection: BTreeSet<Id>,
}

impl Snapshot {
    pub fn circuit(&self) -> &Circuit {
        &self.project.circuit(self.context.active).unwrap().circuit
    }
}

pub(crate) struct EditSession {
    pub project: Project,
    /// Independent working copy for gestures and grouped property edits. Publish
    /// it with `synchronize`; reload it after replacing project definitions.
    pub circuit: Circuit,
    pub context: Context,
    pub selection: BTreeSet<Id>,
}

impl Default for EditSession {
    fn default() -> Self {
        Self::new(Project::default())
    }
}

impl EditSession {
    pub fn new(project: Project) -> Self {
        Self {
            circuit: project.main_circuit().circuit.clone(),
            context: Context {
                active: project.main,
                root: project.main,
                path: vec![],
                appearance_mode: false,
            },
            project,
            selection: BTreeSet::new(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut project = self.project.clone();
        project.circuit_mut(self.context.active).unwrap().circuit = self.circuit.clone();
        Snapshot {
            project,
            context: self.context.clone(),
            selection: self.selection.clone(),
        }
    }

    pub fn restore(&mut self, mut snapshot: Snapshot) {
        snapshot.project.preserve_allocations(&self.project);
        self.context = snapshot.context;
        self.selection = snapshot.selection;
        self.replace_project(snapshot.project);
    }

    /// Install prepared definitions while keeping the current editing context.
    /// Refresh the working copy before it can overwrite the replacement.
    pub fn replace_project(&mut self, project: Project) {
        self.project = project;
        self.reload();
    }

    pub fn reload(&mut self) {
        self.circuit = self
            .project
            .circuit(self.context.active)
            .unwrap()
            .circuit
            .clone();
    }

    pub fn synchronize(&mut self) {
        let definition = self.project.circuit_mut(self.context.active).unwrap();
        if definition.circuit != self.circuit {
            definition.circuit = self.circuit.clone();
            self.project.refresh_interfaces(true);
            self.reload();
        }
    }

    pub fn normalize_wires_since(&mut self, before: &Snapshot) {
        for definition in &mut self.project.circuits {
            if before.project.circuit(definition.id).map(|d| &d.circuit)
                != Some(&definition.circuit)
            {
                let mapped = definition.circuit.normalize_wires();
                if definition.id == self.context.active {
                    self.selection = self
                        .selection
                        .iter()
                        .flat_map(|id| {
                            mapped.get(id).cloned().unwrap_or_else(|| {
                                if definition.circuit.component(*id).is_some() {
                                    BTreeSet::from([*id])
                                } else {
                                    BTreeSet::new()
                                }
                            })
                        })
                        .collect();
                }
            }
        }
        self.reload();
    }
    pub fn context_label(&self) -> String {
        let mut definition = self.context.root;
        let mut names = vec![
            self.project
                .circuit(definition)
                .map(|d| d.name.clone())
                .unwrap_or_default(),
        ];
        for id in &self.context.path {
            if let Some(c) = self
                .project
                .circuit(definition)
                .and_then(|d| d.circuit.component(*id))
                && let Kind::Subcircuit(target) = c.kind
            {
                names.push(if c.label.is_empty() {
                    format!("{} #{}", c.interface.name, c.id)
                } else {
                    c.label.clone()
                });
                definition = target;
            }
        }
        names.join(" / ")
    }
    pub fn switch_circuit(&mut self, id: Id) -> bool {
        if self.project.circuit(id).is_none() {
            return false;
        }
        self.context.active = id;
        self.context.root = id;
        self.context.path.clear();
        self.reload();
        self.selection.clear();
        true
    }
    pub fn enter_instance(&mut self, id: Id) -> bool {
        if let Some(c) = self.circuit.component(id)
            && let Kind::Subcircuit(target) = c.kind
            && self.project.circuit(target).is_some()
        {
            self.context.path.push(id);
            self.context.active = target;
            self.reload();
            self.selection.clear();
            true
        } else {
            false
        }
    }
    pub fn leave_instance(&mut self) -> bool {
        if self.context.path.pop().is_none() {
            return false;
        }
        let mut id = self.context.root;
        for instance in &self.context.path {
            if let Some(c) = self
                .project
                .circuit(id)
                .and_then(|d| d.circuit.component(*instance))
                && let Kind::Subcircuit(target) = c.kind
            {
                id = target;
            }
        }
        self.context.active = id;
        self.reload();
        self.selection.clear();
        true
    }
}

#[cfg(test)]
mod tests;

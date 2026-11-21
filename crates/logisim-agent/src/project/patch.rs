//! A batch operates on one disposable clone. Existing core edit policies own all ID allocation and routing.
use super::{
    Session, edit,
    instances::{Manifests, NewPlugin, PluginUpdate},
};
use crate::paths::Root;
use crate::{Error, Result};
use logisim_core::{Circuit, Component, Id, Kind, Point, Project};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectOp {
    AddComponent {
        circuit: Id,
        kind: String,
        #[serde(
            default,
            deserialize_with = "crate::supplied",
            skip_serializing_if = "Option::is_none"
        )]
        circuit_id: Option<Id>,
        position: [i32; 2],
        #[serde(default)]
        properties: Map<String, Value>,
    },
    AddPlugin(Box<NewPlugin>),
    UpdatePlugin(Box<PluginUpdate>),
    MoveComponent {
        circuit: Option<Id>,
        id: Id,
        position: [i32; 2],
    },
    UpdateComponent {
        circuit: Option<Id>,
        id: Id,
        properties: Map<String, Value>,
    },
    RemoveComponent {
        circuit: Option<Id>,
        id: Id,
    },
    AddWire {
        circuit: Id,
        from: [i32; 2],
        to: [i32; 2],
    },
    RemoveWire {
        circuit: Option<Id>,
        id: Id,
    },
    AddCircuit {
        name: String,
    },
    RenameCircuit {
        id: Id,
        name: String,
    },
    RemoveCircuit {
        id: Id,
    },
    SetMainCircuit {
        id: Id,
    },
}

fn invalid(message: impl ToString) -> Error {
    Error::new("invalid_edit", message)
}
pub(crate) fn indexed(mut error: Error, index: usize) -> Error {
    error.details["op_index"] = index.into();
    error
}
pub(super) fn point([x, y]: [i32; 2]) -> Result<Point> {
    use super::schema::COORD_LIMIT;
    if [x, y]
        .into_iter()
        .any(|n| !(-COORD_LIMIT..=COORD_LIMIT).contains(&n))
    {
        return Err(invalid("Coordinate outside the supported circuit bounds."));
    }
    Ok(Point::new(x, y))
}
fn new_component(
    project: &Project,
    owner: Id,
    name: &str,
    target: Option<Id>,
    at: Point,
) -> Result<Component> {
    let entry = super::catalog::placement(name)?;
    if entry.requires_circuit_id() {
        let target = target.ok_or_else(|| invalid("A subcircuit instance requires circuit_id."))?;
        if !project.can_instantiate(owner, target) {
            return Err(invalid(format!(
                "Cannot instantiate circuit {target} in {owner}: missing circuit or recursive reference."
            )));
        }
        return Ok(project
            .instance(target, at)
            .expect("validated target circuit"));
    }
    if target.is_some() {
        return Err(invalid("Only a subcircuit instance accepts circuit_id."));
    }
    Ok(Component::new(entry.kind, at))
}
fn circuit(project: &mut Project, id: Id) -> Result<&mut Circuit> {
    project
        .circuit_mut(id)
        .map(|d| &mut d.circuit)
        .ok_or_else(|| invalid(format!("Unknown circuit {id}.")))
}
fn owner(project: &Project, scope: Option<Id>, id: Id, wire: bool) -> Result<Id> {
    let mut matches = project
        .circuits
        .iter()
        .filter(|d| scope.is_none_or(|n| n == d.id))
        .filter(|d| {
            if wire {
                d.circuit.wires.iter().any(|w| w.id == id)
            } else {
                d.circuit.component(id).is_some()
            }
        });
    let first = matches.next().ok_or_else(|| {
        invalid(format!(
            "No such {} {id} in scope.",
            if wire { "wire" } else { "component" }
        ))
    })?;
    if matches.next().is_some() {
        return Err(Error::new(
            "ambiguous_id",
            format!("Specify circuit for item {id}."),
        ));
    }
    Ok(first.id)
}

fn apply_properties(
    c: &mut Component,
    properties: &Map<String, Value>,
    change: &mut Value,
) -> Result<()> {
    let old_dimensions = c.pla.dimensions();
    edit::apply(c, properties)?;
    if c.kind == Kind::PlaRom && c.pla.dimensions() != old_dimensions {
        super::pla::describe(&c.pla, &mut change["properties"]);
    }
    Ok(())
}

impl ProjectOp {
    fn apply(
        &self,
        project: &mut Project,
        manifests: &mut Manifests,
        root: Option<&Root>,
    ) -> Result<Value> {
        use ProjectOp::*;
        let mut change = serde_json::to_value(self).expect("JSON edit");
        let mut pin_edit = false;
        match self {
            AddComponent {
                circuit: cid,
                kind: name,
                circuit_id,
                position,
                properties,
            } => {
                let at = point(*position)?;
                let mut c = new_component(project, *cid, name, *circuit_id, at)?;
                apply_properties(&mut c, properties, &mut change)?;
                if c.position != at {
                    return Err(invalid("Conflicting component positions in add_component."));
                }
                logisim_file::validate_component(&c, project).map_err(invalid)?;
                pin_edit = c.kind == Kind::Pin;
                change["id"] = circuit(project, *cid)?.add_component(c).into();
            }
            AddPlugin(add) => {
                let c = add.build(project, manifests, root)?;
                logisim_file::validate_component(&c, project).map_err(invalid)?;
                change["id"] = circuit(project, add.circuit)?.add_component(c).into();
            }
            UpdatePlugin(update) => {
                let cid = owner(project, update.circuit, update.id, false)?;
                let old = project
                    .circuit(cid)
                    .expect("resolved circuit")
                    .circuit
                    .component(update.id)
                    .expect("resolved component");
                let c = update.build(old, project, manifests)?;
                logisim_file::validate_component(&c, project).map_err(invalid)?;
                circuit(project, cid)?.replace_component(c);
                change["circuit"] = cid.into();
            }
            MoveComponent {
                circuit: scope, id, ..
            }
            | UpdateComponent {
                circuit: scope,
                id,
                properties: _,
            }
            | RemoveComponent { circuit: scope, id } => {
                let cid = owner(project, *scope, *id, false)?;
                change["circuit"] = cid.into();
                let old = project
                    .circuit(cid)
                    .and_then(|d| d.circuit.component(*id))
                    .expect("resolved component");
                pin_edit = old.kind == Kind::Pin;
                match self {
                    MoveComponent { position, .. } => {
                        let delta = point(*position)? - old.position;
                        circuit(project, cid)?.move_selection(&BTreeSet::from([*id]), delta);
                    }
                    UpdateComponent { properties, .. } => {
                        let mut c = old.clone();
                        apply_properties(&mut c, properties, &mut change)?;
                        logisim_file::validate_component(&c, project).map_err(invalid)?;
                        let delta = c.position - old.position;
                        let circuit = circuit(project, cid)?;
                        if delta != Point::ZERO {
                            circuit.move_selection(&BTreeSet::from([*id]), delta);
                        }
                        circuit.replace_component(c);
                    }
                    RemoveComponent { .. } => circuit(project, cid)?.remove(&BTreeSet::from([*id])),
                    _ => unreachable!(),
                }
            }
            AddWire {
                circuit: cid,
                from,
                to,
            } => {
                let id = circuit(project, *cid)?
                    .add_wire(point(*from)?, point(*to)?)
                    .ok_or_else(|| invalid("A wire requires distinct orthogonal endpoints."))?;
                change["id"] = id.into();
            }
            RemoveWire { circuit: scope, id } => {
                let cid = owner(project, *scope, *id, true)?;
                change["circuit"] = cid.into();
                circuit(project, cid)?.remove(&BTreeSet::from([*id]));
            }
            AddCircuit { name } => {
                change["id"] = project.add(name.clone()).map_err(invalid)?.into()
            }
            RenameCircuit { id, name } => project.rename(*id, name.clone()).map_err(invalid)?,
            RemoveCircuit { id } => project.remove(*id).map_err(invalid)?,
            SetMainCircuit { id } => {
                circuit(project, *id)?;
                project.main = *id;
            }
        }
        if pin_edit {
            project.refresh_interfaces(true);
        }
        let mut total = 0;
        for def in &project.circuits {
            let count = def.circuit.components.len() + def.circuit.wires.len();
            if count > logisim_file::MAX_CIRCUIT_ITEMS {
                return Err(invalid("Circuit item limit exceeded."));
            }
            total += count;
        }
        if total > logisim_file::MAX_ITEMS {
            return Err(invalid("Project item limit exceeded."));
        }
        Ok(change)
    }
}

struct Preview {
    project: Project,
    changes: Vec<Value>,
    allocated: HashSet<u128>,
}

/// Normalize after the complete edit, as in the desktop editor. For a wire split at
/// terminals, return every surviving segment; for a merge, return the retained ID.
fn preview(session: &Session, ops: &[ProjectOp], root: Option<&Root>) -> Result<Preview> {
    let original = &session.project;
    let mut candidate = original.clone();
    let mut manifests = Manifests::new(&session.path);
    let mut changes = Vec::with_capacity(ops.len());
    for (index, op) in ops.iter().enumerate() {
        changes.push(
            op.apply(&mut candidate, &mut manifests, root)
                .map_err(|e| indexed(e, index))?,
        );
    }
    for def in &mut candidate.circuits {
        if original
            .circuit(def.id)
            .is_some_and(|old| old.circuit == def.circuit)
        {
            continue;
        }
        let mapped = def.circuit.normalize_wires();
        for (op, change) in ops.iter().zip(&mut changes) {
            if let ProjectOp::AddWire { circuit, .. } = op
                && *circuit == def.id
                && let Some(ids) = mapped.get(&change["id"].as_u64().expect("added wire ID"))
            {
                change["id"] = json!(ids.first());
                change["wire_ids"] = json!(ids);
            }
        }
    }
    let allocated = session.issued.fill_missing(&mut candidate)?;
    super::saveable(&candidate)?;
    Ok(Preview {
        project: candidate,
        changes,
        allocated,
    })
}
impl Session {
    pub fn patch(
        &mut self,
        id: &str,
        expected: Option<&str>,
        dry_run: bool,
        ops: &[ProjectOp],
        root: Option<&Root>,
    ) -> Result<Value> {
        self.check_editable(expected)?;
        let previewed = match preview(self, ops, root) {
            Ok(result) => result,
            Err(mut failure) if !ops.is_empty() && failure.details.get("op_index").is_none() => {
                // Aggregate codec or topology limits can surface only after normalization.
                // Locate a failing prefix without reserializing after every successful edit.
                let (mut good, mut bad) = (0, ops.len());
                while bad - good > 1 {
                    let mid = good + (bad - good) / 2;
                    match preview(self, &ops[..mid], root) {
                        Ok(_) => good = mid,
                        Err(e) => {
                            bad = mid;
                            failure = e;
                        }
                    }
                }
                return Err(indexed(failure, bad - 1));
            }
            Err(e) => return Err(e),
        };
        let changed = previewed.project != self.project;
        if !dry_run {
            self.issued.commit(previewed.allocated);
            self.project = previewed.project;
            if changed {
                self.simulation = None;
            }
        }
        Ok(
            json!({"session_id":id,"source_sha256":self.sha256,"dry_run":dry_run,"changed":changed,"changes":previewed.changes}),
        )
    }
}

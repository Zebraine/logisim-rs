//! A batch operates on one disposable clone. Existing core edit policies own all ID allocation and routing.
use super::{Session, edit};
use crate::{Error, Result};
use logisim_core::{Circuit, Component, Gate, Id, Kind, Parity, Point, Project};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectOp {
    AddComponent {
        circuit: Id,
        kind: String,
        position: [i32; 2],
        #[serde(default)]
        properties: Map<String, Value>,
    },
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
fn point([x, y]: [i32; 2]) -> Result<Point> {
    if [x, y]
        .into_iter()
        .any(|n| !(-1_000_000..=1_000_000).contains(&n))
    {
        return Err(invalid("Coordinate outside the supported circuit bounds."));
    }
    Ok(Point::new(x, y))
}
fn kind(name: &str) -> Result<Kind> {
    [Kind::Pin, Kind::Probe, Kind::Tunnel, Kind::Splitter]
        .into_iter()
        .chain(Gate::ALL.map(Kind::Gate))
        .chain(Parity::ALL.map(Kind::Parity))
        .find(|k| k.name().eq_ignore_ascii_case(name))
        .ok_or_else(|| edit::unsupported(name))
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

impl ProjectOp {
    fn apply(&self, project: &mut Project) -> Result<Value> {
        use ProjectOp::*;
        let mut change = serde_json::to_value(self).expect("JSON edit");
        let mut pin_edit = false;
        match self {
            AddComponent {
                circuit: cid,
                kind: name,
                position,
                properties,
            } => {
                let mut c = Component::new(kind(name)?, point(*position)?);
                edit::apply(&mut c, properties)?;
                logisim_file::validate_component(&c, project).map_err(invalid)?;
                pin_edit = c.kind == Kind::Pin;
                change["id"] = circuit(project, *cid)?.add_component(c).into();
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
                edit::require_supported(old.kind)?;
                pin_edit = old.kind == Kind::Pin;
                match self {
                    MoveComponent { position, .. } => {
                        let delta = point(*position)? - old.position;
                        circuit(project, cid)?.move_selection(&BTreeSet::from([*id]), delta);
                    }
                    UpdateComponent { properties, .. } => {
                        let mut c = old.clone();
                        edit::apply(&mut c, properties)?;
                        logisim_file::validate_component(&c, project).map_err(invalid)?;
                        circuit(project, cid)?.replace_component(c);
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

/// Normalize after the complete edit, as in the desktop editor. For a wire split at
/// terminals, return every surviving segment; for a merge, return the retained ID.
fn preview(original: &Project, ops: &[ProjectOp]) -> Result<(Project, Vec<Value>)> {
    let mut candidate = original.clone();
    let mut changes = Vec::with_capacity(ops.len());
    for (index, op) in ops.iter().enumerate() {
        changes.push(op.apply(&mut candidate).map_err(|e| indexed(e, index))?);
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
    super::saveable(&candidate)?;
    Ok((candidate, changes))
}
impl Session {
    pub fn patch(
        &mut self,
        id: &str,
        expected: Option<&str>,
        dry_run: bool,
        ops: &[ProjectOp],
    ) -> Result<Value> {
        self.check_editable(expected)?;
        let (candidate, changes) = match preview(&self.project, ops) {
            Ok(result) => result,
            Err(mut failure) if !ops.is_empty() && failure.details.get("op_index").is_none() => {
                // Aggregate codec or topology limits can surface only after normalization.
                // Locate a failing prefix without reserializing after every successful edit.
                let (mut good, mut bad) = (0, ops.len());
                while bad - good > 1 {
                    let mid = good + (bad - good) / 2;
                    match preview(&self.project, &ops[..mid]) {
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
        let changed = candidate != self.project;
        if !dry_run {
            self.project = candidate;
            if changed {
                self.simulation = None;
            }
        }
        Ok(
            json!({"session_id":id,"source_sha256":self.sha256,"dry_run":dry_run,"changed":changed,"changes":changes}),
        )
    }
}

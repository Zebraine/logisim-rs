//! Patch targets use either a circuit-local number or a project-wide typed document ID.
use crate::{Error, Result};
use logisim_core::{AgentIdKind, Id, Project};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(try_from = "Fields", rename_all = "snake_case")]
pub enum Reference {
    Id(Id),
    AgentId(String),
}

#[derive(Deserialize)]
struct Fields {
    #[serde(default, deserialize_with = "crate::supplied")]
    id: Option<Id>,
    #[serde(default, deserialize_with = "crate::supplied")]
    agent_id: Option<String>,
}

impl TryFrom<Fields> for Reference {
    type Error = &'static str;

    fn try_from(fields: Fields) -> std::result::Result<Self, Self::Error> {
        match (fields.id, fields.agent_id) {
            (Some(id), None) => Ok(Self::Id(id)),
            (None, Some(id)) => Ok(Self::AgentId(id)),
            _ => Err("Supply exactly one of id and agent_id."),
        }
    }
}

impl Reference {
    /// The pair is (owner circuit, target). A circuit is its own scope.
    pub(super) fn resolve(
        &self,
        project: &Project,
        kind: AgentIdKind,
        scope: Option<Id>,
    ) -> Result<(Id, Id)> {
        match self {
            Self::Id(id) => by_numeric(project, kind, scope, *id),
            Self::AgentId(text) => {
                let (owner, id) = by_agent_id(project, kind, text)?;
                if scope.is_some_and(|cid| cid != owner) {
                    return Err(Error::new(
                        "invalid_edit",
                        format!("{text} belongs to circuit {owner}."),
                    ));
                }
                Ok((owner, id))
            }
        }
    }
}

fn by_numeric(project: &Project, kind: AgentIdKind, scope: Option<Id>, id: Id) -> Result<(Id, Id)> {
    let mut owners = project
        .circuits
        .iter()
        .filter(|def| scope.is_none_or(|cid| cid == def.id))
        .filter(|def| match kind {
            AgentIdKind::Circuit => def.id == id,
            AgentIdKind::Component => def.circuit.component(id).is_some(),
            AgentIdKind::Wire => def.circuit.wires.iter().any(|w| w.id == id),
        });
    let owner = owners
        .next()
        .ok_or_else(|| Error::new("invalid_edit", format!("No such {kind:?} {id} in scope.")))?;
    if owners.next().is_some() {
        return Err(Error::new(
            "ambiguous_id",
            format!("Specify circuit for item {id}."),
        ));
    }
    Ok((owner.id, id))
}

pub(super) fn by_agent_id(project: &Project, kind: AgentIdKind, text: &str) -> Result<(Id, Id)> {
    let tag = kind
        .decode(text)
        .ok_or_else(|| Error::invalid(format!("Invalid {kind:?} agent_id: {text}.")))?;
    let mut found = None;
    let mut record = |owner, id| {
        if found.replace((owner, id)).is_some() {
            return Err(Error::new(
                "ambiguous_id",
                format!("Duplicate agent_id {text}."),
            ));
        }
        Ok(())
    };
    for def in &project.circuits {
        let circuit = &def.circuit;
        let ids = &circuit.agent_ids;
        match kind {
            AgentIdKind::Circuit => {
                if ids.circuit == Some(tag) {
                    record(def.id, def.id)?;
                }
            }
            AgentIdKind::Component => {
                for c in &circuit.components {
                    if ids.items.get(&c.id) == Some(&tag) {
                        record(def.id, c.id)?;
                    }
                }
            }
            AgentIdKind::Wire => {
                for w in &circuit.wires {
                    if ids.items.get(&w.id) == Some(&tag) {
                        record(def.id, w.id)?;
                    }
                }
            }
        }
    }
    found.ok_or_else(|| Error::new("invalid_edit", format!("Unknown agent_id {text}.")))
}

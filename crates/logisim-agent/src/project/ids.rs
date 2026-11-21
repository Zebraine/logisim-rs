//! Only agent sessions allocate persistent IDs. A preview reserves nothing in the live session.
use crate::{Error, Result};
use logisim_core::{AgentIdKind, Circuit, Id, Project};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet, btree_map::Entry};

/// Includes IDs of removed items until this session closes.
pub(super) struct Issued(HashSet<u128>);

impl Issued {
    pub(super) fn open(project: &mut Project) -> Result<Self> {
        let mut issued = Self(
            project
                .circuits
                .iter()
                .flat_map(|d| {
                    let ids = &d.circuit.agent_ids;
                    ids.circuit.into_iter().chain(ids.items.values().copied())
                })
                .collect(),
        );
        let fresh = issued.fill_missing(project)?;
        issued.commit(fresh);
        Ok(issued)
    }

    pub(super) fn fill_missing(&self, project: &mut Project) -> Result<HashSet<u128>> {
        let mut fresh = HashSet::new();
        let mut next = || loop {
            let mut bytes = [0; 16];
            getrandom::fill(&mut bytes).map_err(|e| Error::new("unavailable", e))?;
            let id = u128::from_be_bytes(bytes);
            if !self.0.contains(&id) && fresh.insert(id) {
                break Ok(id);
            }
        };
        for def in &mut project.circuits {
            let c = &mut def.circuit;
            let ids = &mut c.agent_ids;
            if ids.circuit.is_none() {
                ids.circuit = Some(next()?);
            }
            for item in c
                .components
                .iter()
                .map(|c| c.id)
                .chain(c.wires.iter().map(|w| w.id))
            {
                if let Entry::Vacant(slot) = ids.items.entry(item) {
                    slot.insert(next()?);
                }
            }
        }
        Ok(fresh)
    }

    pub(super) fn commit(&mut self, fresh: HashSet<u128>) {
        self.0.extend(fresh);
    }
}

/// Derive provisional identities from the random session nonce and the actual edit.
/// A preview consumes no randomness or session state. Committed and retired IDs
/// remain reserved even when an object is removed later in the same transaction.
pub(super) struct BatchIds<'a> {
    session: &'a str,
    issued: &'a Issued,
    pub(super) fresh: HashSet<u128>,
}

impl<'a> BatchIds<'a> {
    pub(super) fn new(session: &'a str, issued: &'a Issued) -> Self {
        Self {
            session,
            issued,
            fresh: HashSet::new(),
        }
    }

    pub(super) fn allocate(&mut self, key: impl serde::Serialize) -> u128 {
        let mut bytes =
            serde_json::to_vec(&(self.session, key)).expect("serializable identity key");
        loop {
            let hash = crate::digest::sha256(&bytes);
            let id = u128::from_str_radix(&hash[..32], 16).expect("SHA-256 hex");
            if !self.issued.0.contains(&id) && self.fresh.insert(id) {
                return id;
            }
            bytes.push(0);
        }
    }

    pub(super) fn add_item(
        &mut self,
        circuit: &mut Circuit,
        kind: AgentIdKind,
        id: Id,
        change: &mut Value,
    ) {
        change["id"] = id.into();
        let tag = circuit
            .agent_ids
            .items
            .entry(id)
            .or_insert_with(|| self.allocate(&*change));
        change["agent_id"] = kind.encode(*tag).into();
    }

    /// Give pre-existing wire identities priority during a merge, even when the
    /// core retains a new segment's numeric ID. New split segments are tagged last.
    pub(super) fn normalize_wires(
        &mut self,
        owner: Id,
        circuit: &mut Circuit,
    ) -> BTreeMap<Id, BTreeSet<Id>> {
        let mut provisional = BTreeMap::new();
        let items = &mut circuit.agent_ids.items;
        for wire in &circuit.wires {
            if let Entry::Occupied(entry) = items.entry(wire.id)
                && self.fresh.contains(entry.get())
            {
                provisional.insert(wire.id, entry.remove());
            }
        }
        let mapped = circuit.normalize_wires();
        for wire in &circuit.wires {
            circuit.agent_ids.items.entry(wire.id).or_insert_with(|| {
                provisional.get(&wire.id).copied().unwrap_or_else(|| {
                    self.allocate((
                        "wire_segment",
                        owner,
                        wire.id,
                        wire.a.x,
                        wire.a.y,
                        wire.b.x,
                        wire.b.y,
                    ))
                })
            });
        }
        mapped
    }
}

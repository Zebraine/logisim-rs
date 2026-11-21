//! Only agent sessions allocate persistent IDs. A preview reserves nothing in the live session.
use crate::{Error, Result};
use logisim_core::Project;
use std::collections::{HashSet, btree_map::Entry};

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

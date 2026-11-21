//! Assign missing tags on a portable copy. Keep repeated saves/conversions of the same
//! live item stable without dirtying the editor or changing legacy saves.
use super::*;
use std::collections::btree_map::Entry;

#[derive(Default)]
pub struct IdentityAssignments {
    assigned: BTreeMap<(Id, Option<Id>), u128>,
}
impl IdentityAssignments {
    pub fn complete(&mut self, project: &mut Project) -> Result<()> {
        let mut used: BTreeSet<_> = project
            .circuits
            .iter()
            .flat_map(|d| {
                let tags = &d.circuit.agent_ids;
                tags.circuit.into_iter().chain(tags.items.values().copied())
            })
            .collect();
        let mut live = BTreeMap::new();
        let mut next = |key| -> Result<u128> {
            if let Some(&id) = self.assigned.get(&key)
                && used.insert(id)
            {
                live.insert(key, id);
                return Ok(id);
            }
            loop {
                let mut bytes = [0; 16];
                getrandom::fill(&mut bytes)
                    .map_err(|e| Error(format!("Cannot allocate persistent identity: {e}")))?;
                let id = u128::from_be_bytes(bytes);
                if used.insert(id) {
                    live.insert(key, id);
                    return Ok(id);
                }
            }
        };
        for d in &mut project.circuits {
            let c = &mut d.circuit;
            if c.agent_ids.circuit.is_none() {
                c.agent_ids.circuit = Some(next((d.id, None))?);
            }
            for id in c
                .components
                .iter()
                .map(|c| c.id)
                .chain(c.wires.iter().map(|w| w.id))
            {
                if let Entry::Vacant(e) = c.agent_ids.items.entry(id) {
                    e.insert(next((d.id, Some(id)))?);
                }
            }
        }
        self.assigned = live;
        Ok(())
    }
}

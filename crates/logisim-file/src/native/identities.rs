//! Persistent metadata for live editor items that came from untagged legacy files.
use super::*;

#[derive(Default)]
pub struct IdentityAssignments {
    assigned: BTreeMap<(Id, Option<Id>), u128>,
}
impl IdentityAssignments {
    /// Reserve identities without modifying the document or its dirty baseline.
    pub fn reserve(&mut self, project: &Project) -> Result<()> {
        let mut used: BTreeSet<_> = project
            .circuits
            .iter()
            .flat_map(|d| {
                let tags = &d.circuit.agent_ids;
                tags.circuit.into_iter().chain(tags.items.values().copied())
            })
            .collect();
        let mut live = BTreeMap::new();
        let mut reserve = |key| -> Result<()> {
            if let Some(&id) = self.assigned.get(&key)
                && used.insert(id)
            {
                live.insert(key, id);
                return Ok(());
            }
            loop {
                let mut bytes = [0; 16];
                getrandom::fill(&mut bytes)
                    .map_err(|e| Error(format!("Cannot allocate persistent identity: {e}")))?;
                let id = u128::from_be_bytes(bytes);
                if used.insert(id) {
                    live.insert(key, id);
                    return Ok(());
                }
            }
        };
        for d in &project.circuits {
            let c = &d.circuit;
            if c.agent_ids.circuit.is_none() {
                reserve((d.id, None))?;
            }
            for id in c
                .components
                .iter()
                .map(|c| c.id)
                .chain(c.wires.iter().map(|w| w.id))
            {
                if !c.agent_ids.items.contains_key(&id) {
                    reserve((d.id, Some(id)))?;
                }
            }
        }
        self.assigned = live;
        Ok(())
    }

    pub fn complete(&mut self, project: &mut Project) -> Result<()> {
        self.reserve(project)?;
        for d in &mut project.circuits {
            for ((_, item), &tag) in self.assigned.range((d.id, None)..=(d.id, Some(Id::MAX))) {
                match item {
                    Some(id) => {
                        d.circuit.agent_ids.items.insert(*id, tag);
                    }
                    None => d.circuit.agent_ids.circuit = Some(tag),
                }
            }
        }
        Ok(())
    }
}

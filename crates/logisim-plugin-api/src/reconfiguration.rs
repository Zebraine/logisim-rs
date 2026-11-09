//! Optional state-preserving property edits, independent of the simulation ABI.
//!
//! `logisim_reconfiguration_version() -> u32` returns 1. The companion
//! `logisim_reconfiguration() -> u64` returns bounded length/offset JSON [`Policy`].
//! Missing or unknown versions preserve no edited state. Malformed recognized
//! extensions fail loading. This metadata is never saved in component snapshots.
//!
//! A listed property promises that its valid values share the same state encoding
//! and meaning. When ONLY listed properties change, the host may create a fresh VM,
//! retain explicit state and edge history, and propagate current inputs WITHOUT Reset.
//! Outputs and phase caches are invalidated. Identity, version, Tool schema and exact
//! loaded module must stay unchanged. Faulted instances never retain state this way.
use crate::{Component, MAX_MESSAGE, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const VERSION: u32 = 1;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Tool ID -> property IDs that can change together without resetting state.
    pub preserve: BTreeMap<String, Vec<String>>,
}
impl Policy {
    pub fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        if self.preserve.len() > manifest.tools.len()
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_MESSAGE
        {
            return Err("Plugin reconfiguration policy exceeds limits.".into());
        }
        for (id, properties) in &self.preserve {
            let tool = manifest
                .tools
                .iter()
                .find(|t| &t.id == id)
                .ok_or("Unknown reconfiguration tool.")?;
            if properties.is_empty()
                || properties.len() > tool.properties.len()
                || properties.iter().collect::<BTreeSet<_>>().len() != properties.len()
                || properties
                    .iter()
                    .any(|p| !tool.properties.iter().any(|s| &s.id == p))
            {
                return Err("Invalid state-preserving properties.".into());
            }
        }
        Ok(())
    }
    pub fn allows(&self, old: &Component, new: &Component) -> bool {
        old.plugin == new.plugin
            && old.version == new.version
            && old.tool == new.tool
            && old.validate().is_ok()
            && new.validate().is_ok()
            && self.preserve.get(&old.tool.id).is_some_and(|allowed| {
                old.properties.iter().all(|(key, value)| {
                    new.properties.get(key) == Some(value) || allowed.contains(key)
                })
            })
    }
}

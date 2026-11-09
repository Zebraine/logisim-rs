//! Import an independent project as a portable library snapshot, without filesystem access.
use crate::{Id, Kind, MAX_CIRCUITS, Project};
use std::collections::BTreeMap;

/// Prepared import. Hosts may validate format budgets before committing this document.
pub struct LibraryImport {
    pub project: Project,
    pub name: String,
    pub circuits: Vec<Id>,
}

impl Project {
    /// Remap definition identities; component/Pin identities are local to each circuit.
    /// Existing geometry, main selection and the source library remain untouched.
    pub fn with_library(&self, library: &Self, name: &str) -> Result<LibraryImport, String> {
        self.validate()?;
        library.validate()?;
        if self.circuits.len() + library.circuits.len() > MAX_CIRCUITS {
            return Err("The library would exceed the project circuit limit.".into());
        }
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err("Invalid library name.".into());
        }
        let mut namespace = name.to_owned();
        let mut number = 2;
        while library.circuits.iter().any(|c| {
            self.named(&format!("{namespace}::{name}", name = c.name))
                .is_some()
        }) {
            namespace = format!("{name} ({number})");
            number += 1;
        }
        let mut project = self.clone();
        for source in &library.plugin_sources {
            if !project
                .plugin_sources
                .iter()
                .any(|s| s.plugin == source.plugin && s.version == source.version)
            {
                project.plugin_sources.push(source.clone());
            }
        }
        let mut mapping = BTreeMap::new();
        let mut circuits = Vec::new();
        for source in &library.circuits {
            let id = project.add(format!("{namespace}::{}", source.name))?;
            mapping.insert(source.id, id);
            circuits.push(id);
        }
        for source in &library.circuits {
            let mut circuit = source.circuit.clone();
            for component in &mut circuit.components {
                if let Kind::Subcircuit(id) = &mut component.kind {
                    *id = mapping[id];
                }
            }
            project.circuit_mut(mapping[&source.id]).unwrap().circuit = circuit;
        }
        project.refresh_interfaces(false);
        project.validate()?;
        Ok(LibraryImport {
            project,
            name: namespace,
            circuits,
        })
    }
}

#[cfg(test)]
mod tests;

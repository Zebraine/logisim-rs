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
mod tests {
    use super::*;
    use crate::{Appearance, Component, Point};

    #[test]
    fn nested_references_and_custom_pin_bindings_survive_identity_collisions() {
        let mut source = Project::default();
        let child = source.add("child".into()).unwrap();
        let circuit = &mut source.circuit_mut(child).unwrap().circuit;
        let pin = circuit.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
        let mut appearance = Appearance {
            shapes: vec![],
            ports: BTreeMap::new(),
            anchor: Point::ZERO,
            facing: crate::Facing::East,
        };
        appearance.ports.insert(pin, Point::new(50, 50));
        circuit.appearance = Some(appearance);
        source.refresh_interfaces(false);
        let instance = source.instance(child, Point::new(200, 200)).unwrap();
        source.main_circuit_mut().circuit.add_component(instance);
        let mut destination = Project::default();
        destination.add("occupied".into()).unwrap();
        let old = destination.clone();
        let imported = destination.with_library(&source, "Memory").unwrap();
        assert_eq!(destination, old);
        assert_eq!(imported.project.main, destination.main);
        assert_eq!(imported.project.circuits[..2], destination.circuits);
        let a = imported.project.circuit(imported.circuits[0]).unwrap();
        let b = imported.project.circuit(imported.circuits[1]).unwrap();
        assert_eq!(a.circuit.components[0].kind, Kind::Subcircuit(b.id));
        assert_eq!(
            a.circuit.components[0].ports(),
            source.main_circuit().circuit.components[0].ports()
        );
        assert_eq!(b.circuit, source.circuit(child).unwrap().circuit);
        let again = imported.project.with_library(&source, "Memory").unwrap();
        assert_eq!(again.name, "Memory (2)");
        assert_ne!(again.circuits, imported.circuits);
    }

    #[test]
    fn invalid_or_oversized_libraries_leave_destination_unchanged() {
        let destination = Project::default();
        let mut source = Project::default();
        source
            .main_circuit_mut()
            .circuit
            .add_component(Component::new(Kind::Subcircuit(99), Point::ZERO));
        assert!(destination.with_library(&source, "bad").is_err());
        source = Project::default();
        for i in 1..MAX_CIRCUITS {
            source.add(format!("c{i}")).unwrap();
        }
        assert!(destination.with_library(&source, "large").is_err());
        assert!(
            destination
                .with_library(&Project::default(), &"x".repeat(1024))
                .is_err()
        );
        assert_eq!(destination, Project::default());
    }
}

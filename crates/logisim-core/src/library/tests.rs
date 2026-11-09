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

use super::*;
use crate::tests::support::Temp;
use logisim_core::{Component, Kind, Point};
use logisim_sim::ProjectSimulator;
use std::fs;

fn library() -> Project {
    let mut p = Project::default();
    let leaf = p.add("leaf".into()).unwrap();
    let at = Point::new(100, 100);
    let mut pin = Component::new(Kind::Pin, at);
    pin.bits = 8;
    pin.output = true;
    let mut source = Component::new(Kind::Constant, at);
    source.bits = 8;
    source.constant = 42;
    let circuit = &mut p.circuit_mut(leaf).unwrap().circuit;
    circuit.add_component(source);
    circuit.add_component(pin.clone());
    p.refresh_interfaces(false);
    let instance = p.instance(leaf, at).unwrap();
    p.main_circuit_mut().circuit.add_component(instance);
    p.main_circuit_mut().circuit.add_component(pin);
    p.refresh_interfaces(false);
    p
}

#[test]
fn nested_library_simulates_and_saved_project_no_longer_needs_source() {
    let temp = Temp::new();
    let path = temp.path("Memory.circ");
    let source = logisim_file::serialize(&library()).unwrap();
    fs::write(&path, &source).unwrap();
    let imported = load(&path, &Project::default()).unwrap();
    let mut project = imported.project;
    let at = Point::new(300, 200);
    let instance = project.instance(imported.circuits[0], at).unwrap();
    project.main_circuit_mut().circuit.add_component(instance);
    let mut output = Component::new(Kind::Pin, at);
    output.output = true;
    output.bits = 8;
    output.label = "result".into();
    project.main_circuit_mut().circuit.add_component(output);
    let xml = logisim_file::serialize(&project).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    fs::remove_file(path).unwrap();
    let parsed = logisim_file::parse(&xml).unwrap();
    assert!(parsed.warnings.is_empty());
    let project = parsed.document;
    let pin = project
        .main_circuit()
        .circuit
        .components
        .iter()
        .find(|c| c.label == "result")
        .unwrap()
        .id;
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, project.main);
    sim.advance(256);
    assert_eq!(
        sim.view(&[]).unwrap().port_value(pin, 0).unwrap().number(),
        Some(42)
    );
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
}

#[test]
fn omitted_components_and_nested_external_files_are_rejected_without_disk_changes() {
    let temp = Temp::new();
    let path = temp.path("unsupported.circ");
    let xml = logisim_file::serialize(&library()).unwrap();
    for bad in [
        xml.replace("name=\"Constant\"", "name=\"Unknown Component\""),
        xml.replace("desc=\"#Wiring\"", "desc=\"file#missing.circ\""),
        "not XML".into(),
    ] {
        fs::write(&path, &bad).unwrap();
        let project = Project::default();
        assert!(load(&path, &project).is_err());
        assert_eq!(project, Project::default());
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
    }
}

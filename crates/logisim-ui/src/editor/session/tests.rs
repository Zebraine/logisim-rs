use super::*;
use logisim_core::{Component, Point};

#[test]
fn checkpoint_captures_unpublished_work_and_restores_its_context() {
    let mut project = Project::default();
    project.main = project.add("selected".into()).unwrap();
    let mut session = EditSession::new(project);
    let id = session
        .circuit
        .add_component(Component::new(Kind::Text, Point::new(80, 40)));
    session.selection.insert(id);
    session.context.appearance_mode = true;
    let checkpoint = session.snapshot();
    // Taking a checkpoint must neither publish a preview nor lose that preview.
    assert!(session.project.main_circuit().circuit.components.is_empty());
    assert_eq!(checkpoint.circuit(), &session.circuit);
    assert_eq!(
        &checkpoint.project.main_circuit().circuit,
        checkpoint.circuit()
    );

    session.circuit.components[0].label = "later edit".into();
    session.synchronize();
    let discarded = session.project.add("discarded".into()).unwrap();
    session.switch_circuit(discarded);
    session.context.appearance_mode = false;
    session.restore(checkpoint.clone());
    assert_eq!(session.project, checkpoint.project);
    assert_eq!(session.circuit, *checkpoint.circuit());
    assert_eq!(session.context, checkpoint.context);
    assert_eq!(session.selection, checkpoint.selection);
    // Historical restoration cannot reuse a definition identity from the future.
    assert_ne!(
        session.project.add("replacement".into()).unwrap(),
        discarded
    );
}

#[test]
fn prepared_project_replacement_cannot_be_overwritten_by_the_old_working_copy() {
    let mut session = EditSession::default();
    session
        .circuit
        .add_component(Component::new(Kind::Text, Point::ZERO));
    session.synchronize();
    let context = session.context.clone();
    let mut prepared = session.project.clone();
    prepared.main_circuit_mut().circuit.components[0].label = "replacement".into();
    prepared.add("imported".into()).unwrap();

    session.replace_project(prepared.clone());
    session.synchronize();
    assert_eq!(session.project, prepared);
    assert_eq!(session.circuit, prepared.main_circuit().circuit);
    assert_eq!(session.context, context);
    assert_eq!(session.snapshot().project, prepared);
}

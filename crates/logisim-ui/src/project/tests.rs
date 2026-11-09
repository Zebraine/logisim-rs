use super::*;
use logisim_core::{Facing, Point};
#[test]
fn circuit_label_edits_update_instances_and_undo_without_moving_wires() {
    let mut e = Editor::default();
    e.edit_project(CircuitCommand::Add, "child".into()).unwrap();
    let child = e.session.context.active;
    e.set_tool(Tool::Place(Kind::Pin));
    e.place(Point::new(100, 100));
    e.switch_circuit(1);
    e.set_tool(Tool::Place(Kind::Subcircuit(child)));
    e.place(Point::new(200, 200));
    e.session
        .circuit
        .add_wire(Point::new(100, 200), Point::new(200, 200));
    e.sync_project();
    let original = e.session.project.main_circuit().circuit.clone();
    e.switch_circuit(child);
    let mut label = e.session.circuit.label.clone();
    label.text = "ALU".into();
    label.font.bold = true;
    label.up = Facing::North;
    e.update_circuit_label(label.clone());
    e.commit_properties();
    e.sync_project();
    let main = &e.session.project.main_circuit().circuit;
    assert_eq!(main.components[0].interface.label, label);
    assert_eq!(main.wires, original.wires);
    assert_eq!(main.components[0].ports(), original.components[0].ports());
    e.undo();
    e.sync_project();
    assert!(
        e.session.project.main_circuit().circuit.components[0]
            .interface
            .label
            .text
            .is_empty()
    );
    e.redo();
    e.sync_project();
    assert_eq!(
        e.session.project.main_circuit().circuit.components[0]
            .interface
            .label,
        label
    );
}
#[test]
fn global_history_restores_definitions_instances_and_main_selection() {
    let mut e = Editor::default();
    e.edit_project(CircuitCommand::Add, "part".into()).unwrap();
    let part = e.session.context.active;
    e.set_tool(Tool::Place(Kind::Pin));
    e.place(Point::new(100, 100));
    e.template.output = true;
    e.template.facing = Facing::West;
    e.place(Point::new(200, 100));
    e.switch_circuit(1);
    e.set_tool(Tool::Place(Kind::Subcircuit(part)));
    e.place(Point::new(200, 200));
    assert_eq!(
        e.session.circuit.components[0].ports(),
        vec![Point::new(170, 200), Point::new(200, 200)]
    );
    e.switch_circuit(part);
    e.edit_project(CircuitCommand::Rename, "renamed".into())
        .unwrap();
    assert_eq!(
        e.session.project.main_circuit().circuit.components[0]
            .interface
            .name,
        "renamed"
    );
    e.undo();
    assert_eq!(
        e.session.project.main_circuit().circuit.components[0]
            .interface
            .name,
        "part"
    );
    e.redo();
    e.edit_project(CircuitCommand::Main, String::new()).unwrap();
    assert_eq!(e.session.project.main, part);
    e.undo();
    assert_eq!(e.session.project.main, 1);
    e.undo();
    e.undo();
    assert_eq!(e.session.context.active, 1);
    assert!(e.session.circuit.components.is_empty());
    e.redo();
    assert_eq!(e.session.circuit.components.len(), 1);
}
#[test]
fn pin_edit_updates_parent_wires_and_one_undo_restores_both_circuits() {
    let mut e = Editor::default();
    e.edit_project(CircuitCommand::Add, "part".into()).unwrap();
    let part = e.session.context.active;
    e.set_tool(Tool::Place(Kind::Pin));
    e.place(Point::new(100, 100));
    e.place(Point::new(100, 200));
    e.switch_circuit(1);
    e.set_tool(Tool::Place(Kind::Subcircuit(part)));
    e.place(Point::new(200, 200));
    let before = e.snapshot();
    e.session
        .circuit
        .add_wire(Point::new(100, 200), Point::new(200, 200));
    e.record(before);
    let parent = e.session.circuit.clone();
    e.switch_circuit(part);
    let mut pin = e.session.circuit.components[0].clone();
    pin.position.y = 300;
    e.update_property(pin);
    e.commit_properties();
    assert_ne!(&parent, &e.session.project.main_circuit().circuit);
    e.undo();
    assert_eq!(&parent, &e.session.project.main_circuit().circuit);
}
#[test]
fn clipboard_cannot_create_cycles_and_instance_navigation_keeps_context() {
    let mut e = Editor::default();
    e.edit_project(CircuitCommand::Add, "part".into()).unwrap();
    let part = e.session.context.active;
    e.switch_circuit(1);
    e.set_tool(Tool::Place(Kind::Subcircuit(part)));
    e.place(Point::new(200, 200));
    let id = e.session.circuit.components[0].id;
    e.copy();
    e.enter_instance(id);
    assert_eq!(e.session.context.root, 1);
    assert_eq!(e.session.context.path, vec![id]);
    assert_eq!(e.session.context.active, part);
    assert!(!e.can_paste());
    e.paste();
    assert!(e.session.circuit.components.is_empty());
    assert!(e.project_error.is_some());
    e.leave_instance();
    assert_eq!(e.session.context.active, 1);
    assert!(e.session.context.path.is_empty());
    e.switch_circuit(part);
    assert!(
        e.edit_project(CircuitCommand::Remove, String::new())
            .is_err()
    );
    e.switch_circuit(1);
    e.session.selection.insert(id);
    e.delete();
    e.switch_circuit(part);
    e.edit_project(CircuitCommand::Remove, String::new())
        .unwrap();
    e.undo();
    assert!(e.session.project.circuit(part).is_some());
}
#[test]
fn undo_then_new_definition_cannot_retarget_a_stale_clipboard() {
    let mut e = Editor::default();
    e.edit_project(CircuitCommand::Add, "old".into()).unwrap();
    let old = e.session.context.active;
    e.switch_circuit(1);
    e.set_tool(Tool::Place(Kind::Subcircuit(old)));
    e.place(Point::ZERO);
    e.copy();
    e.undo();
    e.undo();
    assert_eq!(e.session.project, Project::default());
    e.edit_project(CircuitCommand::Add, "new".into()).unwrap();
    assert_ne!(e.session.context.active, old);
    e.switch_circuit(1);
    assert!(!e.can_paste());
}

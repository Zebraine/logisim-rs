use super::*;
#[test]
fn edits_restore_geometry_and_selection_through_undo_redo() {
    let mut e = Editor::default();
    e.place(Point::new(200, 100));
    let placed = e.session.circuit.clone();
    let id = *e.session.selection.first().unwrap();
    e.nudge(Point::new(40, 20));
    e.rotate();
    e.delete();
    assert!(e.session.circuit.components.is_empty());
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(e.session.circuit, placed);
    assert!(e.session.selection.contains(&id));
    e.redo();
    assert_eq!(
        e.session.circuit.component(id).unwrap().position,
        Point::new(240, 120)
    );
}
#[test]
fn repeated_paste_assigns_unique_ids_and_leaves_original_untouched() {
    let mut e = Editor::default();
    e.place(Point::new(100, 100));
    e.copy();
    e.paste();
    e.paste();
    assert_eq!(e.session.circuit.components.len(), 3);
    assert_eq!(e.session.circuit.ids().len(), 3);
    assert_eq!(
        e.session.circuit.components[0].position,
        Point::new(100, 100)
    );
    e.undo();
    assert_eq!(e.session.circuit.components.len(), 2);
    e.delete();
    assert!(!e.can_redo());
}
#[test]
fn property_typing_is_one_undo_transaction() {
    let mut e = Editor {
        template: Component::new(Kind::Tunnel, Point::ZERO),
        ..Editor::default()
    };
    e.place(Point::new(100, 100));
    for label in ["D", "DA", "DAT", "DATA"] {
        let mut c = e.session.circuit.components[0].clone();
        c.label = label.into();
        e.update_property(c);
    }
    e.commit_properties();
    e.undo();
    assert_eq!(e.session.circuit.components[0].label, "tunnel");
    e.redo();
    assert_eq!(e.session.circuit.components[0].label, "DATA");
}

#[test]
fn properties_reverted_before_commit_preserve_the_redo_branch() {
    let mut e = Editor::default();
    e.place(Point::new(100, 100));
    e.place(Point::new(200, 100));
    e.undo();
    assert!(e.can_redo());
    let original = e.session.circuit.components[0].clone();
    let mut changed = original.clone();
    changed.label = "temporary".into();
    e.update_property(changed);
    assert!(!e.can_redo());
    e.update_property(original);
    e.commit_properties();
    assert!(e.can_redo());
    e.redo();
    assert_eq!(e.session.circuit.components.len(), 2);
}

#[test]
fn grouped_properties_commit_before_leaving_an_instance_and_restore_the_same_context() {
    let mut project = Project::default();
    let child = project.add("child".into()).unwrap();
    let pin = project
        .circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    let instance = project.instance(child, Point::new(300, 200)).unwrap();
    let instance = project.main_circuit_mut().circuit.add_component(instance);
    let mut e = Editor::new(project);
    e.enter_instance(instance);
    e.session.context.appearance_mode = true;
    e.session.selection.insert(pin);
    let before = e.snapshot();

    let mut component = e.session.circuit.component(pin).unwrap().clone();
    component.label = "DATA".into();
    e.update_property(component);
    let mut label = e.session.circuit.label.clone();
    label.text = "CELL".into();
    e.update_circuit_label(label);
    let appearance = crate::appearance::AppearanceEditor::current(&e);
    e.update_appearance(appearance);
    e.leave_instance();
    let committed = e.session.project.clone();
    assert_eq!(e.session.context.active, committed.main);
    assert!(e.session.context.path.is_empty());
    assert_eq!(
        committed.main_circuit().circuit.components[0]
            .interface
            .label
            .text,
        "CELL"
    );
    // Simulation state is owned separately and must not be rolled back with a document.
    e.simulation.view.ticks = 41;
    e.undo();
    assert!(!e.can_undo());
    assert_eq!(e.session.project, before.project);
    assert_eq!(e.session.context, before.context);
    assert_eq!(e.session.selection, before.selection);
    assert_eq!(e.simulation.view.ticks, 41);
    e.redo();
    assert_eq!(e.session.project, committed);
    assert_eq!(e.session.context.active, child);
    assert_eq!(e.session.context.root, committed.main);
    assert_eq!(e.session.context.path, vec![instance]);
    // Navigation turns off appearance mode before it commits pending properties.
    assert!(!e.session.context.appearance_mode);
    assert_eq!(e.simulation.view.ticks, 41);
}

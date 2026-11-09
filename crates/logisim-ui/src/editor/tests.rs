use super::*;
#[test]
fn edits_restore_geometry_and_selection_through_undo_redo() {
    let mut e = Editor::default();
    e.place(Point::new(200, 100));
    let placed = e.circuit.clone();
    let id = *e.selection.first().unwrap();
    e.nudge(Point::new(40, 20));
    e.rotate();
    e.delete();
    assert!(e.circuit.components.is_empty());
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(e.circuit, placed);
    assert!(e.selection.contains(&id));
    e.redo();
    assert_eq!(
        e.circuit.component(id).unwrap().position,
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
    assert_eq!(e.circuit.components.len(), 3);
    assert_eq!(e.circuit.ids().len(), 3);
    assert_eq!(e.circuit.components[0].position, Point::new(100, 100));
    e.undo();
    assert_eq!(e.circuit.components.len(), 2);
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
        let mut c = e.circuit.components[0].clone();
        c.label = label.into();
        e.update_property(c);
    }
    e.commit_properties();
    e.undo();
    assert_eq!(e.circuit.components[0].label, "tunnel");
    e.redo();
    assert_eq!(e.circuit.components[0].label, "DATA");
}

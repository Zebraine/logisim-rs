use super::*;

fn edit(history: &mut History, session: &mut EditSession, value: usize) {
    let before = session.snapshot();
    session.circuit.label.text = value.to_string();
    session.synchronize();
    session.normalize_wires_since(&before);
    history.record(before, session);
}

#[test]
fn bounded_history_preserves_redo_for_view_changes_and_discards_it_for_new_edits() {
    let mut history = History::default();
    let mut session = EditSession::default();
    for value in 1..=105 {
        edit(&mut history, &mut session, value);
    }
    let mut count = 0;
    while let Some(snapshot) = history.undo() {
        session.restore(snapshot);
        count += 1;
    }
    assert_eq!(count, 100);
    assert_eq!(session.circuit.label.text, "5");

    let before = session.snapshot();
    session.context.appearance_mode = true;
    history.record(before, &session);
    assert!(!history.can_undo());
    assert!(history.can_redo());
    let mut count = 0;
    while let Some(snapshot) = history.redo() {
        session.restore(snapshot);
        count += 1;
    }
    assert_eq!(count, 100);
    assert_eq!(session.circuit.label.text, "105");

    session.restore(history.undo().unwrap());
    edit(&mut history, &mut session, 200);
    assert!(!history.can_redo());
    session.restore(history.undo().unwrap());
    assert_eq!(session.circuit.label.text, "104");
}

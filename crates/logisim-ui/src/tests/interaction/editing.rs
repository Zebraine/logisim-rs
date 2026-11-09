//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use egui::{Event, Key, Modifiers};
use logisim_core::{Component, Gate, Kind, Point};

use super::support::*;

#[test]
fn text_focus_keeps_delete_arrows_and_clipboard_out_of_circuit_commands() {
    let (ctx, mut app) = setup();
    app.editor
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor.set_tool(Tool::Place(Kind::Text));
    let at = app.canvas.screen(Point::new(300, 300));
    click(&ctx, &mut app, at);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    key(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("memo".into())],
        Modifiers::NONE,
    );
    assert_eq!(app.editor.circuit.components[1].label, "memo");
    key(&ctx, &mut app, Key::ArrowLeft, Modifiers::NONE);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    frame(&ctx, &mut app, vec![Event::Copy], Modifiers::COMMAND);
    assert_eq!(app.editor.circuit.components.len(), 2);
    assert_eq!(
        app.editor.circuit.components[1].position,
        Point::new(300, 300)
    );
    assert_eq!(app.editor.selection.len(), 1);
}

#[test]
fn platform_clipboard_events_copy_and_paste_the_selection() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Place(Kind::Pin));
    let at = app.canvas.screen(Point::new(100, 100));
    click(&ctx, &mut app, at);
    frame(&ctx, &mut app, vec![Event::Copy], Modifiers::COMMAND);
    frame(
        &ctx,
        &mut app,
        vec![Event::Paste(
            "logisim-rs selection (paste into the same window)".into(),
        )],
        Modifiers::COMMAND,
    );
    assert_eq!(app.editor.circuit.components.len(), 2);
    assert_eq!(
        app.editor.circuit.components[1].position,
        Point::new(110, 110)
    );
}

#[test]
fn marquee_selects_components_and_wires_and_delete_is_undoable() {
    let (ctx, mut app) = setup();
    app.editor
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor
        .circuit
        .add_component(Component::new(Kind::Gate(Gate::Not), Point::new(240, 100)));
    app.editor
        .circuit
        .add_wire(Point::new(100, 100), Point::new(180, 100));
    let before = app.editor.circuit.clone();
    let a = app.canvas.screen(Point::new(50, 50));
    let b = app.canvas.screen(Point::new(280, 140));
    drag(&ctx, &mut app, a, b);
    assert_eq!(app.editor.selection.len(), 3);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    assert!(app.editor.circuit.ids().is_empty());
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.circuit, before);
}

//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use crate::*;
use egui::{Event, Key, Modifiers};
use logisim_core::{Component, Gate, Kind, Point};
use std::collections::BTreeSet;

use super::support::*;

#[test]
fn place_connect_move_and_undo_through_canvas_input() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Place(Kind::Gate(Gate::And)));
    let point = Point::new(300, 200);
    let at = app.canvas.screen(point);
    click(&ctx, &mut app, at);
    assert_eq!(app.editor.circuit.components.len(), 1);
    let gate = app.editor.circuit.components[0].id;
    app.editor.set_tool(Tool::Wire);
    click(&ctx, &mut app, at);
    let end = app.canvas.screen(Point::new(500, 200));
    click(&ctx, &mut app, end);
    assert_eq!(app.editor.circuit.wires.len(), 1);
    app.editor.set_tool(Tool::Select);
    let body = app.canvas.screen(Point::new(270, 200));
    drag(&ctx, &mut app, body, body + vec2(80.0, 50.0));
    assert_eq!(
        app.editor.circuit.component(gate).unwrap().position,
        Point::new(380, 250)
    );
    assert!(
        app.editor
            .circuit
            .wires
            .iter()
            .any(|w| w.contains(Point::new(380, 250)))
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.circuit.component(gate).unwrap().position, point);
    assert_eq!(app.editor.circuit.wires.len(), 1);
}

#[test]
fn endpoint_drag_changes_wire_without_creating_a_component() {
    let (ctx, mut app) = setup();
    let id = app
        .editor
        .circuit
        .add_wire(Point::new(100, 100), Point::new(300, 100))
        .unwrap();
    app.editor.selection.insert(id);
    let start = app.canvas.screen(Point::new(300, 100));
    let end = app.canvas.screen(Point::new(350, 170));
    drag(&ctx, &mut app, start, end);
    assert!(app.editor.circuit.components.is_empty());
    assert!(
        app.editor
            .circuit
            .wires
            .iter()
            .any(|w| w.contains(Point::new(350, 170)))
    );
    assert!(
        app.editor
            .circuit
            .wires
            .iter()
            .all(|w| w.a.x == w.b.x || w.a.y == w.b.y)
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.circuit.wires.len(), 1);
    assert_eq!(app.editor.circuit.wires[0].b, Point::new(300, 100));
}

#[test]
fn escape_cancels_partial_wire_and_move() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wire);
    let at = app.canvas.screen(Point::new(100, 100));
    click(&ctx, &mut app, at);
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(app.editor.circuit.wires.is_empty());
    app.editor.set_tool(Tool::Place(Kind::Pin));
    click(&ctx, &mut app, at);
    let before = app.editor.circuit.clone();
    let body = at - vec2(20.0, 0.0);
    button(&ctx, &mut app, body, true);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(body + vec2(40.0, 40.0))],
        Modifiers::NONE,
    );
    assert_ne!(app.editor.circuit, before);
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    button(&ctx, &mut app, body + vec2(40.0, 40.0), false);
    assert_eq!(app.editor.circuit, before);
}

#[test]
fn drawn_extensions_merge_with_selection_undo_redo_and_cancelled_moves() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Wire);
    for (a, b) in [(100, 200), (200, 300), (150, 350)] {
        let start = app.canvas.screen(Point::new(a, 100));
        let end = app.canvas.screen(Point::new(b, 100));
        click(&ctx, &mut app, start);
        click(&ctx, &mut app, end);
        assert_eq!(app.circuit().wires.len(), 1);
        assert_eq!(
            app.editor.selection,
            BTreeSet::from([app.circuit().wires[0].id])
        );
    }
    assert_eq!(
        (app.circuit().wires[0].a, app.circuit().wires[0].b),
        (Point::new(100, 100), Point::new(350, 100))
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().wires[0].b, Point::new(300, 100));
    key(
        &ctx,
        &mut app,
        Key::Z,
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
    );
    assert_eq!(app.circuit().wires[0].b, Point::new(350, 100));
    app.editor.set_tool(Tool::Select);
    let before = app.circuit().clone();
    let at = app.canvas.screen(Point::new(200, 100));
    button(&ctx, &mut app, at, true);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(at + vec2(0.0, 80.0))],
        Modifiers::NONE,
    );
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    button(&ctx, &mut app, at, false);
    assert_eq!(app.circuit(), &before);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    assert!(app.circuit().wires.is_empty());
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &before);
}

#[test]
fn endpoint_drag_join_merges_both_wires_and_undo_restores_the_gap() {
    let (ctx, mut app) = setup();
    app.editor
        .circuit
        .add_wire(Point::new(100, 100), Point::new(200, 100));
    app.editor
        .circuit
        .add_wire(Point::new(300, 100), Point::new(400, 100));
    let at = app.canvas.screen(Point::new(150, 100));
    button_with(&ctx, &mut app, at, true, Modifiers::CTRL);
    button_with(&ctx, &mut app, at, false, Modifiers::CTRL);
    let a = app.canvas.screen(Point::new(200, 100));
    let b = app.canvas.screen(Point::new(300, 100));
    drag(&ctx, &mut app, a, b);
    assert_eq!(app.circuit().wires.len(), 1);
    assert_eq!(
        (app.circuit().wires[0].a, app.circuit().wires[0].b),
        (Point::new(100, 100), Point::new(400, 100))
    );
    assert_eq!(
        app.editor.selection,
        BTreeSet::from([app.circuit().wires[0].id])
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().wires.len(), 2);
    assert_eq!(app.circuit().wires[0].b, Point::new(200, 100));
    assert_eq!(app.circuit().wires[1].a, Point::new(300, 100));
}

#[test]
fn plain_wire_drags_branch_on_selected_and_unselected_imported_wires_at_every_zoom() {
    for zoom in [0.5, 1.0, 2.0] {
        for vertical in [false, true] {
            for selected in [false, true] {
                let (ctx, mut app) = setup();
                app.canvas.camera.zoom = zoom;
                let xy = |x, y| {
                    if vertical {
                        Point::new(y, x)
                    } else {
                        Point::new(x, y)
                    }
                };
                let original = app
                    .editor
                    .circuit
                    .add_wire(xy(100, 103), xy(300, 103))
                    .unwrap();
                app.editor.circuit.add_wire(xy(100, 207), xy(300, 207));
                if selected {
                    app.editor.selection.insert(original);
                }
                for _ in 0..2 {
                    frame(&ctx, &mut app, vec![], Modifiers::NONE);
                }
                let before = app.circuit().clone();
                let start = app.canvas.screen(xy(202, 105));
                let end = app.canvas.screen(xy(201, 205));
                drag(&ctx, &mut app, start, end);
                let c = app.circuit();
                assert_eq!(
                    c.wires.len(),
                    5,
                    "zoom={zoom}, vertical={vertical}, selected={selected}"
                );
                assert_eq!(c.junctions(), vec![xy(200, 103), xy(200, 207)]);
                assert!(c.wires.iter().any(|w| w.contains(xy(200, 150))));
                assert_eq!(c.connected_wires(original).len(), 5);
                key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
                assert_eq!(app.circuit(), &before);
                assert!(!app.editor.can_undo());
            }
        }
    }
}

#[test]
fn clicking_a_wire_starts_a_branch_with_cancel_preview_and_single_undo() {
    let (ctx, mut app) = setup();
    app.editor
        .circuit
        .add_wire(Point::new(100, 100), Point::new(300, 100));
    let before = app.circuit().clone();
    let start = app.canvas.screen(Point::new(200, 100));
    let end = app.canvas.screen(Point::new(200, 250));
    click(&ctx, &mut app, start);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(end)],
        Modifiers::NONE,
    );
    assert_eq!(app.circuit(), &before);
    assert!(!app.editor.can_undo());
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    click(&ctx, &mut app, end);
    assert_eq!(app.circuit(), &before);
    click(&ctx, &mut app, start);
    click(&ctx, &mut app, end);
    assert_eq!(app.circuit().wires.len(), 3);
    assert_eq!(app.circuit().junctions(), vec![Point::new(200, 100)]);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &before);
    assert!(!app.editor.can_undo());
}

#[test]
fn ctrl_wire_drags_move_selection_and_cancel_or_undo_without_leaving_branches() {
    for selected in [false, true] {
        let (ctx, mut app) = setup();
        let id = app
            .editor
            .circuit
            .add_wire(Point::new(100, 100), Point::new(300, 100))
            .unwrap();
        let pin = app
            .editor
            .circuit
            .add_component(Component::new(Kind::Pin, Point::new(450, 100)));
        if selected {
            app.editor.selection.extend([id, pin]);
        }
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
        }
        let before = app.circuit().clone();
        let start = app.canvas.screen(Point::new(200, 100));
        let end = app.canvas.screen(Point::new(200, 200));
        for cancel in [true, false] {
            button_with(&ctx, &mut app, start, true, Modifiers::CTRL);
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(end)],
                Modifiers::CTRL,
            );
            assert_ne!(app.circuit(), &before);
            if cancel {
                key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
            }
            button_with(&ctx, &mut app, end, false, Modifiers::CTRL);
            if cancel {
                assert_eq!(app.circuit(), &before);
                assert!(!app.editor.can_undo());
            } else {
                assert_eq!(app.circuit().wires.len(), 1);
                assert_eq!(
                    (app.circuit().wires[0].a, app.circuit().wires[0].b),
                    (Point::new(100, 200), Point::new(300, 200))
                );
                assert_eq!(
                    app.circuit().component(pin).unwrap().position,
                    Point::new(450, if selected { 200 } else { 100 })
                );
                key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
                assert_eq!(app.circuit(), &before);
            }
        }
    }
}

//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use crate::*;
use egui::{Key, Modifiers};
use logisim_core::{Kind, Point};
use std::collections::BTreeSet;

use super::support::*;

#[test]
fn wiring_tools_place_duplicate_and_undo_and_fixed_extender_rejects_rotation() {
    let (ctx, mut app) = setup();
    for (i, kind) in [
        Kind::Constant,
        Kind::Power,
        Kind::Ground,
        Kind::Probe,
        Kind::Splitter,
        Kind::BitExtender,
    ]
    .into_iter()
    .enumerate()
    {
        app.editor.set_tool(Tool::Place(kind));
        if kind == Kind::BitExtender {
            key(&ctx, &mut app, Key::R, Modifiers::NONE);
            assert_eq!(app.editor.template.facing, logisim_core::Facing::East);
        }
        let at = app.canvas.screen(Point::new(140 + i as i32 * 110, 200));
        click(&ctx, &mut app, at);
    }
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_eq!(
        app.editor.circuit.components.last().unwrap().facing,
        logisim_core::Facing::East
    );
    key(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::D, Modifiers::COMMAND);
    assert_eq!(app.editor.circuit.components.len(), 12);
    assert_eq!(app.editor.circuit.ids().len(), 12);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.circuit.components.len(), 6);
}

#[test]
fn plexers_place_connect_rotate_and_undo_using_canvas_events() {
    let (ctx, mut app) = setup();
    for (i, kind) in logisim_core::Plexer::ALL.into_iter().enumerate() {
        app.editor.set_tool(Tool::Place(Kind::Plexer(kind)));
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
        }
        let at = app.canvas.screen(Point::new(
            200 + (i % 3) as i32 * 180,
            200 + (i / 3) as i32 * 200,
        ));
        click(&ctx, &mut app, at);
        assert_eq!(
            app.circuit().components.last().unwrap().kind,
            Kind::Plexer(kind)
        );
    }
    let c = app.circuit().components[0].clone();
    let out = c.ports()[0];
    app.editor.set_tool(Tool::Wire);
    let start = app.canvas.screen(out);
    click(&ctx, &mut app, start);
    let end = app.canvas.screen(out + Point::new(50, 0));
    click(&ctx, &mut app, end);
    app.editor.set_tool(Tool::Select);
    app.editor.selection = BTreeSet::from([c.id]);
    let before = app.circuit().clone();
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_eq!(
        app.circuit().component(c.id).unwrap().facing,
        logisim_core::Facing::South
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &before);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert!(app.circuit().wires.is_empty());
}

#[test]
fn arithmetic_canvas_places_wires_and_keeps_fixed_orientation() {
    let (ctx, mut app) = setup();
    for (i, kind) in logisim_core::Arithmetic::ALL.into_iter().enumerate() {
        app.editor.set_tool(Tool::Place(Kind::Arithmetic(kind)));
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
        }
        let at = app.canvas.screen(Point::new(
            180 + (i % 4) as i32 * 100,
            160 + (i / 4) as i32 * 100,
        ));
        click(&ctx, &mut app, at);
        let c = app.circuit().components.last().unwrap();
        assert_eq!(c.kind, Kind::Arithmetic(kind));
        assert_eq!(c.bits, 8);
        let bounds = crate::drawing::bounds(c);
        assert!(bounds.contains(crate::drawing::pos(c.position + Point::new(-20, 0))));
        for port in c.ports() {
            assert!(bounds.contains(crate::drawing::pos(port)));
        }
    }
    let c = app.circuit().components[0].clone();
    app.editor.set_tool(Tool::Wire);
    let start = app.canvas.screen(c.ports()[4]);
    click(&ctx, &mut app, start);
    let end = app.canvas.screen(c.ports()[4] + Point::new(0, 60));
    click(&ctx, &mut app, end);
    app.editor.set_tool(Tool::Select);
    app.editor.selection = BTreeSet::from([c.id]);
    let before = app.circuit().clone();
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_eq!(app.circuit(), &before);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert!(app.circuit().wires.is_empty());
}

#[test]
fn bus_components_place_rotate_connect_move_and_undo_through_canvas() {
    let (ctx, mut app) = setup();
    for (i, kind) in [
        Kind::ControlledBuffer,
        Kind::ControlledInverter,
        Kind::PullResistor,
    ]
    .into_iter()
    .enumerate()
    {
        app.editor.set_tool(Tool::Place(kind));
        let at = app.canvas.screen(Point::new(200 + i as i32 * 180, 200));
        click(&ctx, &mut app, at);
    }
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert_eq!(app.circuit().components.len(), 3);
    let pull = app.circuit().components[2].id;
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_eq!(
        app.circuit().component(pull).unwrap().facing,
        logisim_core::Facing::West
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(
        app.circuit().component(pull).unwrap().facing,
        logisim_core::Facing::South
    );
    app.editor.set_tool(Tool::Wire);
    let a = app.canvas.screen(Point::new(200, 200));
    let b = app.canvas.screen(Point::new(560, 200));
    click(&ctx, &mut app, a);
    click(&ctx, &mut app, b);
    assert!(!app.circuit().wires.is_empty());
    app.editor.set_tool(Tool::Select);
    let body = app.canvas.screen(Point::new(560, 175));
    drag(&ctx, &mut app, body, body + vec2(0.0, 80.0));
    assert_eq!(
        app.circuit().component(pull).unwrap().position,
        Point::new(560, 280)
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(
        app.circuit().component(pull).unwrap().position,
        Point::new(560, 200)
    );
    key(&ctx, &mut app, Key::D, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 4);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 3);
}

#[test]
fn transistors_place_wire_move_rotate_duplicate_and_undo_through_canvas() {
    let (ctx, mut app) = setup();
    for (i, kind) in [Kind::Transistor, Kind::TransmissionGate]
        .into_iter()
        .enumerate()
    {
        app.editor.set_tool(Tool::Place(kind));
        let at = app.canvas.screen(Point::new(200 + i as i32 * 220, 200));
        click(&ctx, &mut app, at);
    }
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert_eq!(app.circuit().components.len(), 2);
    let gate = app.circuit().components[1].id;
    app.editor.set_tool(Tool::Wire);
    let a = app.canvas.screen(Point::new(200, 200));
    let b = app.canvas.screen(Point::new(380, 200));
    click(&ctx, &mut app, a);
    click(&ctx, &mut app, b);
    assert!(!app.circuit().wires.is_empty());
    app.editor.set_tool(Tool::Select);
    let body = app.canvas.screen(Point::new(400, 200));
    click(&ctx, &mut app, body);
    let wires = app.circuit().wires.clone();
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_ne!(
        app.circuit().component(gate).unwrap().facing,
        logisim_core::Facing::East
    );
    assert_eq!(app.circuit().wires, wires);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(
        app.circuit().component(gate).unwrap().facing,
        logisim_core::Facing::East
    );
    drag(&ctx, &mut app, body, body + vec2(0.0, 80.0));
    assert_eq!(
        app.circuit().component(gate).unwrap().position,
        Point::new(420, 280)
    );
    assert!(
        app.circuit()
            .wires
            .iter()
            .any(|w| w.a == Point::new(380, 280) || w.b == Point::new(380, 280))
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(
        app.circuit().component(gate).unwrap().position,
        Point::new(420, 200)
    );
    key(&ctx, &mut app, Key::D, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 3);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 2);
}

#[test]
fn parity_place_wire_rotate_move_duplicate_and_undo_through_canvas() {
    use logisim_core::{Facing, Parity};
    let (ctx, mut app) = setup();
    for (i, parity) in Parity::ALL.into_iter().enumerate() {
        app.editor.set_tool(Tool::Place(Kind::Parity(parity)));
        let at = app.canvas.screen(Point::new(200 + i as i32 * 220, 200));
        click(&ctx, &mut app, at);
    }
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert_eq!(app.circuit().components.len(), 2);
    let gate = app.circuit().components[1].id;
    app.editor.set_tool(Tool::Wire);
    let a = app.canvas.screen(Point::new(200, 200));
    let b = app.canvas.screen(Point::new(390, 190));
    click(&ctx, &mut app, a);
    click(&ctx, &mut app, b);
    assert!(!app.circuit().wires.is_empty());
    app.editor.set_tool(Tool::Select);
    let body = app.canvas.screen(Point::new(405, 200));
    click(&ctx, &mut app, body);
    let wires = app.circuit().wires.clone();
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_ne!(app.circuit().component(gate).unwrap().facing, Facing::East);
    assert_eq!(app.circuit().wires, wires);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().component(gate).unwrap().facing, Facing::East);
    drag(&ctx, &mut app, body, body + vec2(0.0, 80.0));
    assert_eq!(
        app.circuit().component(gate).unwrap().position,
        Point::new(420, 280)
    );
    assert!(
        app.circuit()
            .wires
            .iter()
            .any(|w| w.a == Point::new(390, 270) || w.b == Point::new(390, 270))
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::D, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 3);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 2);
}

#[test]
fn ttl_canvas_placement_wiring_rotation_move_duplicate_and_undo() {
    use logisim_core::{Facing, Ttl};
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Place(Kind::Ttl(Ttl::Ic7400)));
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let at = app.canvas.screen(Point::new(200, 200));
    click(&ctx, &mut app, at);
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    let id = app.circuit().components[0].id;
    app.editor.set_tool(Tool::Wire);
    let a = app.canvas.screen(Point::new(210, 230));
    let b = app.canvas.screen(Point::new(210, 330));
    click(&ctx, &mut app, a);
    click(&ctx, &mut app, b);
    assert_eq!(app.circuit().wires.len(), 1);
    app.editor.set_tool(Tool::Select);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let body = app.canvas.screen(Point::new(270, 200));
    click(&ctx, &mut app, body);
    assert!(app.editor.selection.contains(&id));
    let before = app.circuit().clone();
    key(&ctx, &mut app, Key::R, Modifiers::NONE);
    assert_ne!(app.circuit().component(id).unwrap().facing, Facing::East);
    assert_eq!(app.circuit().wires, before.wires);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &before);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let body = app.canvas.screen(Point::new(270, 200));
    let target = app.canvas.screen(Point::new(350, 200));
    drag(&ctx, &mut app, body, target);
    assert_eq!(
        app.circuit().component(id).unwrap().position,
        Point::new(280, 200)
    );
    assert!(
        app.circuit()
            .wires
            .iter()
            .any(|w| w.a == Point::new(290, 230) || w.b == Point::new(290, 230))
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &before);
    key(&ctx, &mut app, Key::D, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 2);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &before);
}

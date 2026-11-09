//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use crate::*;
use egui::{Event, Key, Modifiers};
use logisim_core::{Component, Kind, Point};

use super::support::*;

#[test]
fn poke_and_simulation_shortcuts_emit_commands_without_editing_the_circuit() {
    let (ctx, mut app) = setup();
    let id = app
        .editor
        .session
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(200, 100)));
    app.editor.simulation.view.inputs.insert(
        id,
        InputValue {
            bits: 1,
            value: 0,
            ..Default::default()
        },
    );
    let before = app.editor.session.circuit.clone();
    let body = app.canvas.screen(Point::new(180, 100));
    click(&ctx, &mut app, body);
    key(&ctx, &mut app, Key::P, Modifiers::NONE);
    assert_eq!(app.editor.tool, Tool::Poke);
    click(&ctx, &mut app, body);
    key(&ctx, &mut app, Key::Num1, Modifiers::NONE);
    key(&ctx, &mut app, Key::E, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::I, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::R, Modifiers::COMMAND);
    assert_eq!(
        app.take_simulation_actions(),
        vec![
            SimulationAction::ToggleInput { id, bit: 0 },
            SimulationAction::SetInput { id, value: 1 },
            SimulationAction::ToggleEnabled,
            SimulationAction::Step,
            SimulationAction::Reset,
        ]
    );
    assert_eq!(app.editor.session.circuit, before);
    assert!(!app.editor.can_undo());
    drag(&ctx, &mut app, body, body + vec2(70.0, 30.0));
    assert_eq!(app.editor.session.circuit, before);
    app.editor.simulation.view.enabled = true;
    key(&ctx, &mut app, Key::I, Modifiers::COMMAND);
    assert!(app.take_simulation_actions().is_empty());
}

#[test]
fn simulation_shortcuts_are_suppressed_while_editing_a_label() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Place(Kind::Text));
    let at = app.canvas.screen(Point::new(200, 100));
    click(&ctx, &mut app, at);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.editor.simulation.view.has_clocks = true;
    for k in [Key::E, Key::I, Key::R, Key::T, Key::K] {
        key(&ctx, &mut app, k, Modifiers::COMMAND);
    }
    key(&ctx, &mut app, Key::P, Modifiers::NONE);
    key(&ctx, &mut app, Key::Num1, Modifiers::NONE);
    assert!(app.take_simulation_actions().is_empty());
    assert_eq!(app.editor.tool, Tool::Select);
}

#[test]
fn multibit_poke_focuses_hex_input_and_enter_applies_without_global_commands() {
    let (ctx, mut app) = setup();
    let mut pin = Component::new(Kind::Pin, Point::new(200, 100));
    pin.bits = 4;
    let id = app.editor.session.circuit.add_component(pin);
    app.editor.simulation.view.inputs.insert(
        id,
        InputValue {
            bits: 4,
            value: 0,
            ..Default::default()
        },
    );
    app.editor.set_tool(Tool::Poke);
    let body = app.canvas.screen(Point::new(180, 100));
    click(&ctx, &mut app, body);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    key(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("A".into())],
        Modifiers::NONE,
    );
    for k in [Key::E, Key::I, Key::R] {
        key(&ctx, &mut app, k, Modifiers::COMMAND);
    }
    assert!(app.take_simulation_actions().is_empty());
    key(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert_eq!(
        app.take_simulation_actions(),
        vec![SimulationAction::SetInput { id, value: 10 }]
    );
    assert!(!app.editor.can_undo());
}

#[test]
fn sequential_placement_poke_and_tick_shortcuts_preserve_edit_history() {
    let (ctx, mut app) = setup();
    let mut ids = vec![];
    for (kind, point) in [
        (Kind::Clock, Point::new(200, 100)),
        (
            Kind::FlipFlop(logisim_core::FlipFlop::D),
            Point::new(400, 200),
        ),
        (Kind::Register, Point::new(600, 200)),
    ] {
        app.editor.set_tool(Tool::Place(kind));
        // Allow the attribute panel to settle before converting world coordinates to pixels.
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
        }
        let at = app.canvas.screen(point);
        click(&ctx, &mut app, at);
        ids.push(app.editor.session.circuit.components.last().unwrap().id);
    }
    let before = app.editor.session.circuit.clone();
    app.editor.simulation.view.has_clocks = true;
    app.editor.simulation.view.clocks.insert(ids[0]);
    key(&ctx, &mut app, Key::P, Modifiers::NONE);
    for point in [
        Point::new(190, 100),
        Point::new(370, 220),
        Point::new(585, 200),
    ] {
        let at = app.canvas.screen(point);
        click(&ctx, &mut app, at);
    }
    key(&ctx, &mut app, Key::T, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::K, Modifiers::COMMAND);
    assert_eq!(
        app.take_simulation_actions(),
        vec![
            SimulationAction::PokeClock { id: ids[0] },
            SimulationAction::ToggleStored { id: ids[1] },
            SimulationAction::Tick,
            SimulationAction::ToggleTicks,
        ]
    );
    assert_eq!(app.editor.session.circuit, before);
    app.editor.simulation.view.pending = 1;
    key(&ctx, &mut app, Key::T, Modifiers::COMMAND);
    assert!(app.take_simulation_actions().is_empty());
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.session.circuit.components.len(), 2);
    assert!(app.editor.session.circuit.component(ids[2]).is_none());
}

#[test]
fn latch_canvas_placement_fixed_orientation_poke_and_undo_use_real_input() {
    let (ctx, mut app) = setup();
    let mut ids = vec![];
    for (kind, point) in [
        (logisim_core::Latch::D, Point::new(300, 200)),
        (logisim_core::Latch::Sr, Point::new(500, 200)),
    ] {
        app.editor.set_tool(Tool::Place(Kind::Latch(kind)));
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
        }
        key(&ctx, &mut app, Key::R, Modifiers::NONE);
        assert_eq!(app.editor.template.facing, logisim_core::Facing::East);
        let at = app.canvas.screen(point);
        click(&ctx, &mut app, at);
        ids.push(app.circuit().components.last().unwrap().id);
    }
    let before = app.circuit().clone();
    key(&ctx, &mut app, Key::P, Modifiers::NONE);
    for point in [Point::new(270, 220), Point::new(470, 220)] {
        let at = app.canvas.screen(point);
        click(&ctx, &mut app, at);
    }
    assert_eq!(
        app.take_simulation_actions(),
        vec![
            SimulationAction::ToggleStored { id: ids[0] },
            SimulationAction::ToggleStored { id: ids[1] }
        ]
    );
    assert_eq!(app.circuit(), &before);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.circuit().components.len(), 1);
    assert!(app.circuit().component(ids[1]).is_none());
}

//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use super::*;
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use logisim_core::{Component, Point};

fn frame(ctx: &Context, app: &mut Workspace, mut events: Vec<Event>, modifiers: Modifiers) {
    events.insert(0, Event::ModifiersChanged(modifiers));
    let time = ctx.input(|i| i.time) + 0.2;
    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 820.0))),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
}
fn setup() -> (Context, Workspace) {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
    }
    (ctx, app)
}
fn button(ctx: &Context, app: &mut Workspace, pos: Pos2, pressed: bool) {
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ],
        Modifiers::NONE,
    );
}
fn click(ctx: &Context, app: &mut Workspace, pos: Pos2) {
    button(ctx, app, pos, true);
    button(ctx, app, pos, false);
}
fn key(ctx: &Context, app: &mut Workspace, key: Key, modifiers: Modifiers) {
    frame(
        ctx,
        app,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        modifiers,
    );
    frame(
        ctx,
        app,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }],
        Modifiers::NONE,
    );
}
fn drag(ctx: &Context, app: &mut Workspace, a: Pos2, b: Pos2) {
    button(ctx, app, a, true);
    frame(ctx, app, vec![Event::PointerMoved(b)], Modifiers::NONE);
    button(ctx, app, b, false);
}
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

#[test]
fn poke_and_simulation_shortcuts_emit_commands_without_editing_the_circuit() {
    let (ctx, mut app) = setup();
    let id = app
        .editor
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(200, 100)));
    app.editor
        .simulation
        .view
        .inputs
        .insert(id, InputValue { bits: 1, value: 0 });
    let before = app.editor.circuit.clone();
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
    assert_eq!(app.editor.circuit, before);
    assert!(!app.editor.can_undo());
    drag(&ctx, &mut app, body, body + vec2(70.0, 30.0));
    assert_eq!(app.editor.circuit, before);
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
    for k in [Key::E, Key::I, Key::R] {
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
    let id = app.editor.circuit.add_component(pin);
    app.editor
        .simulation
        .view
        .inputs
        .insert(id, InputValue { bits: 4, value: 0 });
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

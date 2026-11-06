//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use super::*;
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use logisim_core::{Component, Point};
use std::collections::BTreeSet;

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
    button_with(ctx, app, pos, pressed, Modifiers::NONE);
}
fn button_with(ctx: &Context, app: &mut Workspace, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers,
            },
        ],
        modifiers,
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
fn appearance_draw_cancel_undo_redo_and_mode_shortcut_use_real_canvas_input() {
    use crate::appearance::Tool as ArtTool;
    let (ctx, mut app) = setup();
    key(
        &ctx,
        &mut app,
        Key::E,
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
    );
    assert!(app.editor.appearance_mode);
    assert!(app.take_simulation_actions().is_empty());
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.tool = ArtTool::Rect;
    let start = app.canvas.screen(Point::new(150, 150));
    let end = app.canvas.screen(Point::new(220, 190));
    drag(&ctx, &mut app, start, end);
    let a = app.circuit().appearance.as_ref().unwrap().clone();
    assert_eq!(a.shapes.len(), 3);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert!(app.circuit().appearance.is_none());
    key(
        &ctx,
        &mut app,
        Key::Z,
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
    );
    assert_eq!(app.circuit().appearance.as_ref(), Some(&a));
    button(&ctx, &mut app, start, true);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(end)],
        Modifiers::NONE,
    );
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    button(&ctx, &mut app, end, false);
    assert_eq!(app.circuit().appearance.as_ref(), Some(&a));
    app.appearance.tool = ArtTool::Rect;
    button(&ctx, &mut app, start, true);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(end)],
        Modifiers::NONE,
    );
    key(&ctx, &mut app, Key::Space, Modifiers::NONE);
    button(&ctx, &mut app, end, false);
    assert_eq!(app.circuit().appearance.as_ref(), Some(&a));
}
#[test]
fn appearance_port_overlapping_anchor_can_move_independently_and_undo() {
    use crate::appearance::{AppearanceEditor, Target};
    let (ctx, mut app) = setup();
    let pin = app
        .editor
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor.sync_project();
    app.editor.appearance_mode = true;
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let before = AppearanceEditor::current(&app.editor);
    let anchor = before.anchor;
    assert_eq!(before.ports[&pin], anchor);
    app.appearance.selected.insert(Target::Port(pin));
    let start = app.canvas.screen(anchor);
    let end = app.canvas.screen(anchor + Point::new(-20, 30));
    drag(&ctx, &mut app, start, end);
    let after = app.circuit().appearance.as_ref().unwrap();
    assert_eq!(after.anchor, anchor);
    assert_eq!(after.ports[&pin], anchor + Point::new(-20, 30));
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    assert!(
        app.circuit()
            .appearance
            .as_ref()
            .unwrap()
            .ports
            .contains_key(&pin)
    );
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert!(app.circuit().appearance.is_none());
}
#[test]
fn appearance_clipboard_events_do_not_duplicate_paste_and_keep_ports_protected() {
    use crate::appearance::{Command, Target};
    let (ctx, mut app) = setup();
    app.editor.appearance_mode = true;
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.generate_package(&mut app.editor);
    app.appearance.command(&mut app.editor, Command::All);
    let count = app.circuit().appearance.as_ref().unwrap().shapes.len();
    app.appearance.command(&mut app.editor, Command::Copy);
    let at = app.canvas.screen(Point::new(250, 200));
    click(&ctx, &mut app, at);
    frame(
        &ctx,
        &mut app,
        vec![
            Event::Paste("logisim-rs appearance".into()),
            Event::Key {
                key: Key::V,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
        ],
        Modifiers::COMMAND,
    );
    assert_eq!(
        app.circuit().appearance.as_ref().unwrap().shapes.len(),
        count * 2
    );
    assert!(
        app.appearance
            .selected
            .iter()
            .all(|t| matches!(t, Target::Shape(_)))
    );
    app.appearance.command(&mut app.editor, Command::All);
    app.appearance.command(&mut app.editor, Command::Delete);
    assert!(app.circuit().appearance.as_ref().unwrap().shapes.is_empty());
    assert!(app.appearance.selected.contains(&Target::Anchor));
    app.editor.undo();
    assert_eq!(
        app.circuit().appearance.as_ref().unwrap().shapes.len(),
        count * 2
    );
}
#[test]
fn appearance_text_focus_keeps_edit_keys_out_of_artwork_commands() {
    use crate::appearance::Tool as ArtTool;
    let (ctx, mut app) = setup();
    app.editor.appearance_mode = true;
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.tool = ArtTool::Text;
    let at = app.canvas.screen(Point::new(200, 80));
    click(&ctx, &mut app, at);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    assert!(ctx.egui_wants_keyboard_input());
    let count = app.circuit().appearance.as_ref().unwrap().shapes.len();
    key(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("Name".into())],
        Modifiers::NONE,
    );
    assert_eq!(
        app.circuit().appearance.as_ref().unwrap().shapes.len(),
        count
    );
    assert!(
        app.circuit()
            .appearance
            .as_ref()
            .unwrap()
            .shapes
            .iter()
            .any(|s| matches!(&s.geometry,logisim_core::Geometry::Text {text,..} if text=="Name"))
    );
}

#[test]
fn generated_package_fits_the_viewport_and_revert_is_undoable() {
    let (ctx, mut app) = setup();
    for y in [100, 200] {
        app.editor
            .circuit
            .add_component(Component::new(Kind::Pin, Point::new(100, y)));
    }
    app.editor.sync_project();
    app.editor.appearance_mode = true;
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.generate_package(&mut app.editor);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let saved = app.circuit().appearance.as_ref().unwrap().clone();
    let (a, b) = saved.bounds();
    assert!(app.canvas.rect.contains(app.canvas.screen(a)));
    assert!(app.canvas.rect.contains(app.canvas.screen(b)));
    app.appearance.revert(&mut app.editor);
    assert!(app.circuit().appearance.is_none());
    app.editor.undo();
    assert_eq!(app.circuit().appearance.as_ref(), Some(&saved));
}

#[test]
fn appearance_curve_polygon_handles_and_layer_order_use_canvas_events() {
    use crate::appearance::{Command, Target, Tool as ArtTool};
    use logisim_core::Geometry;
    let (ctx, mut app) = setup();
    app.editor.appearance_mode = true;
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.tool = ArtTool::Curve;
    let a = app.canvas.screen(Point::new(160, 140));
    let b = app.canvas.screen(Point::new(220, 140));
    drag(&ctx, &mut app, a, b);
    assert!(app.circuit().appearance.is_none());
    let control = app.canvas.screen(Point::new(190, 190));
    click(&ctx, &mut app, control);
    let curve = app
        .circuit()
        .appearance
        .as_ref()
        .unwrap()
        .shapes
        .last()
        .unwrap()
        .id;
    app.appearance.tool = ArtTool::Select;
    let moved = app.canvas.screen(Point::new(190, 170));
    drag(&ctx, &mut app, control, moved);
    assert!(
        matches!(&app.circuit().appearance.as_ref().unwrap().shapes.last().unwrap().geometry,Geometry::Curve{control,..} if *control==Point::new(190,170))
    );
    app.appearance.tool = ArtTool::Polygon;
    for p in [
        Point::new(250, 140),
        Point::new(290, 140),
        Point::new(270, 180),
    ] {
        let at = app.canvas.screen(p);
        click(&ctx, &mut app, at);
    }
    key(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    let polygon = app
        .circuit()
        .appearance
        .as_ref()
        .unwrap()
        .shapes
        .last()
        .unwrap()
        .id;
    app.appearance.vertex = Some((polygon, 0));
    app.appearance.command(&mut app.editor, Command::AddVertex);
    assert!(
        matches!(&app.circuit().appearance.as_ref().unwrap().shapes.last().unwrap().geometry,Geometry::Poly{points,..} if points.len()==4)
    );
    app.appearance.selected = std::collections::BTreeSet::from([Target::Shape(curve)]);
    app.appearance.command(&mut app.editor, Command::Front);
    assert_eq!(
        app.circuit()
            .appearance
            .as_ref()
            .unwrap()
            .shapes
            .last()
            .unwrap()
            .id,
        curve
    );
    app.editor.undo();
    assert_eq!(
        app.circuit()
            .appearance
            .as_ref()
            .unwrap()
            .shapes
            .last()
            .unwrap()
            .id,
        polygon
    );
}

#[test]
fn appearance_grid_coordinate_typing_waits_for_enter_before_snapping() {
    use crate::appearance::Target;
    let (ctx, mut app) = setup();
    let pin = app
        .editor
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor.sync_project();
    app.editor.appearance_mode = true;
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.selected.insert(Target::Port(pin));
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    // Locate the rendered coordinate label; the project tree changes the inspector height.
    let output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 820.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    let field = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text.starts_with("X ") && t.pos.x < 300.0 => {
                Some(t.pos + vec2(5.0, 5.0))
            }
            _ => None,
        })
        .expect("coordinate field is rendered");
    click(&ctx, &mut app, field);
    assert!(ctx.egui_wants_keyboard_input());
    key(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    for ch in ["1", "2", "0"] {
        frame(
            &ctx,
            &mut app,
            vec![Event::Text(ch.into())],
            Modifiers::NONE,
        );
        assert!(app.circuit().appearance.is_none());
    }
    key(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert_eq!(
        app.circuit().appearance.as_ref().unwrap().ports[&pin].x,
        120
    );
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
        ids.push(app.editor.circuit.components.last().unwrap().id);
    }
    let before = app.editor.circuit.clone();
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
    assert_eq!(app.editor.circuit, before);
    app.editor.simulation.view.pending = 1;
    key(&ctx, &mut app, Key::T, Modifiers::COMMAND);
    assert!(app.take_simulation_actions().is_empty());
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.circuit.components.len(), 2);
    assert!(app.editor.circuit.component(ids[2]).is_none());
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

#[test]
fn plexers_place_connect_rotate_and_undo_using_canvas_events() {
    let (ctx, mut app) = setup();
    for (i, kind) in logisim_core::Plexer::ALL.into_iter().enumerate() {
        app.editor.set_tool(Tool::Place(Kind::Plexer(kind)));
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
        }
        let at = app.canvas.screen(Point::new(200 + i as i32 * 130, 200));
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

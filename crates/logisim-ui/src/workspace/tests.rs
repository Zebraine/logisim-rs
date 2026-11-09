use crate::*;
use egui::{
    Context, Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput,
    Rect, TouchPhase, Vec2, pos2,
};
use logisim_core::Kind;

fn frame(ctx: &Context, app: &mut Workspace, size: Vec2, events: Vec<Event>) {
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    // These tests exercise layout and input without a renderer.
    output.textures_delta.clear();
}

fn wheel() -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn canvas_keeps_the_right_hand_layout_at_both_window_sizes() {
    for size in [vec2(1200.0, 820.0), vec2(720.0, 480.0)] {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        frame(&ctx, &mut app, size, vec![]);
        let canvas = app.canvas.rect;
        assert!(canvas.left() >= 230.0 && canvas.left() < size.x * 0.5);
        let toolbar_bottom = if size.x < 1000.0 {
            95.0..115.0 // A second toolbar row keeps runtime telemetry visible.
        } else {
            65.0..90.0
        };
        assert!(
            toolbar_bottom.contains(&canvas.top()),
            "{size:?}: {canvas:?}"
        );
        assert_eq!(canvas.right(), size.x);
        assert_eq!(
            canvas.bottom(),
            size.y,
            "No full-width bottom bar may shrink the canvas"
        );
        assert!(canvas.width() > 450.0);
    }
}

#[test]
fn wheel_zooms_only_canvas_and_keeps_the_pointer_anchor() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200.0, 820.0);
    let pointer = pos2(620.0, 310.0);
    frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
    frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
    let world = app.canvas.camera.world_at(pointer - app.canvas.rect.min);
    frame(&ctx, &mut app, size, vec![wheel()]);
    assert!((app.canvas.camera.zoom - 1.1).abs() < 0.001);
    assert!((app.canvas.camera.world_at(pointer - app.canvas.rect.min) - world).length() < 0.001);
    let zoom = app.canvas.camera.zoom;
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(pos2(80.0, 160.0)), wheel()],
    );
    assert_eq!(
        app.canvas.camera.zoom, zoom,
        "Scrolling the tree must not zoom the canvas"
    );
}

#[test]
fn reset_shortcut_works_after_the_canvas_takes_focus() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200.0, 820.0);
    let pos = pos2(620.0, 310.0);
    frame(&ctx, &mut app, size, vec![Event::PointerMoved(pos)]);
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    app.canvas.camera.pan(vec2(120.0, -70.0));
    app.canvas.set_zoom(2.0);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Key {
            key: egui::Key::Num0,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
    );
    assert_eq!(app.canvas.camera.zoom, 1.0);
    assert_eq!(app.canvas.camera.offset, canvas::Camera::default().offset);
}

#[test]
fn middle_button_drag_moves_the_viewport() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200.0, 820.0);
    let start = pos2(620.0, 310.0);
    frame(&ctx, &mut app, size, vec![Event::PointerMoved(start)]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Middle,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    let before = app.canvas.camera.offset;
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(start + vec2(80.0, 45.0))],
    );
    assert!((app.canvas.camera.offset - before - vec2(80.0, 45.0)).length() < 0.001);
}
#[test]
fn file_shortcuts_emit_one_command_and_modal_disabling_blocks_them() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200.0, 820.0);
    for (key, modifiers, action) in [
        (egui::Key::O, Modifiers::COMMAND, FileAction::Open),
        (egui::Key::S, Modifiers::COMMAND, FileAction::Save),
        (
            egui::Key::S,
            Modifiers::COMMAND.plus(Modifiers::SHIFT),
            FileAction::SaveAs,
        ),
        (egui::Key::N, Modifiers::COMMAND, FileAction::New),
        (egui::Key::Q, Modifiers::COMMAND, FileAction::Close),
    ] {
        let event = Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(&ctx, &mut app, size, vec![event.clone()]);
        assert_eq!(app.take_file_actions(), vec![action]);
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events: vec![event],
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(false, |ui| app.show(ui));
            },
        );
        output.textures_delta.clear();
        assert!(app.take_file_actions().is_empty());
    }
}
#[test]
fn loading_resets_history_selection_and_fits_negative_coordinates() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    app.editor.place(logisim_core::Point::new(100, 100));
    assert!(app.editor.can_undo());
    let mut circuit = logisim_core::Circuit::default();
    circuit.add_component(logisim_core::Component::new(
        Kind::Text,
        logisim_core::Point::new(-1000, -800),
    ));
    app.replace_circuit(circuit.clone());
    frame(&ctx, &mut app, vec2(1200.0, 820.0), vec![]);
    assert!(!app.editor.can_undo());
    assert!(app.editor.session.selection.is_empty());
    assert_eq!(app.circuit(), &circuit);
    assert!(
        app.canvas
            .rect
            .contains(app.canvas.screen(circuit.components[0].position))
    );
}
#[test]
fn add_circuit_shortcut_takes_precedence_over_new_file_and_modal_blocks_other_commands() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200.0, 820.0);
    let key = |key, modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::N, Modifiers::COMMAND.plus(Modifiers::SHIFT))],
    );
    assert!(app.name_dialog.is_some());
    assert!(app.take_file_actions().is_empty());
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::S, Modifiers::COMMAND)],
    );
    assert!(app.take_file_actions().is_empty());
    frame(&ctx, &mut app, size, vec![Event::Text("Pass".into())]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Enter, Modifiers::NONE)],
    );
    assert!(app.name_dialog.is_none());
    assert_eq!(app.project().circuits.len(), 2);
    assert_eq!(app.circuit_name(), "Pass");
    app.editor.undo();
    assert_eq!(app.project().circuits.len(), 1);
    app.editor.redo();
    assert_eq!(app.circuit_name(), "Pass");
}

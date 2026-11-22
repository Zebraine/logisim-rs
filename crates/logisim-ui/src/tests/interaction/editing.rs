//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use egui::{Event, Key, Modifiers};
use logisim_core::{Component, Gate, Kind, Point};

use super::support::*;

#[test]
fn tool_commands_use_current_bindings_and_canvas_scope() {
    use crate::shortcuts::Command::*;
    let tools = [
        (ToolSelect, Tool::Select, Key::V, Key::F5),
        (ToolPoke, Tool::Poke, Key::P, Key::F6),
        (ToolWire, Tool::Wire, Key::W, Key::F7),
        (ToolText, Tool::Place(Kind::Text), Key::T, Key::F8),
        (ToolTunnel, Tool::Place(Kind::Tunnel), Key::U, Key::F9),
    ];
    for custom in [false, true] {
        let (ctx, mut app) = setup();
        let mut settings = app.preferences();
        if custom {
            for (command, _, _, key) in tools {
                settings
                    .shortcuts
                    .bind(command, key.name().parse().unwrap())
                    .unwrap();
            }
            settings
                .shortcuts
                .bind(ToolRotate, "f10".parse().unwrap())
                .unwrap();
        }
        app.apply_settings(settings);
        let project = app.project().clone();
        app.simulation_mut().steps = 17;
        let camera = app.canvas.camera;
        let hover = |app: &mut crate::Workspace, pos| {
            frame(&ctx, app, vec![Event::PointerMoved(pos)], Modifiers::NONE)
        };
        let canvas = app.canvas.rect.center();
        let sidebar = egui::pos2(15., 100.);
        hover(&mut app, sidebar);
        let poke = if custom { Key::F6 } else { Key::P };
        key(&ctx, &mut app, poke, Modifiers::NONE);
        assert_eq!(app.editor.tool, Tool::Select);
        hover(&mut app, canvas);
        let rotate = if custom { Key::F10 } else { Key::R };
        assert!(!app.editor.can_rotate());
        key(&ctx, &mut app, rotate, Modifiers::NONE);
        assert!(!app.editor.can_undo());
        for (_, tool, default_key, new_key) in tools {
            key(
                &ctx,
                &mut app,
                if custom { new_key } else { default_key },
                Modifiers::NONE,
            );
            assert_eq!(app.editor.tool, tool);
        }
        let facing = app.editor.template.facing;
        key(&ctx, &mut app, rotate, Modifiers::NONE);
        assert_eq!(app.editor.template.facing, facing.next());
        if custom {
            for old_key in [Key::R, Key::V, Key::W, Key::P, Key::T, Key::U] {
                key(&ctx, &mut app, old_key, Modifiers::NONE);
                assert_eq!(app.editor.tool, Tool::Place(Kind::Tunnel));
                assert_eq!(app.editor.template.facing, facing.next());
            }
        }
        app.editor.set_tool(Tool::Select);
        click(&ctx, &mut app, canvas);
        hover(&mut app, sidebar);
        key(&ctx, &mut app, poke, Modifiers::NONE);
        assert_eq!(app.editor.tool, Tool::Poke);
        hover(&mut app, canvas);
        app.editor.session.context.appearance_mode = true;
        key(
            &ctx,
            &mut app,
            if custom { Key::F7 } else { Key::W },
            Modifiers::NONE,
        );
        assert_eq!(app.editor.tool, Tool::Poke);
        app.editor.session.context.appearance_mode = false;
        app.settings_open = true;
        key(
            &ctx,
            &mut app,
            if custom { Key::F7 } else { Key::W },
            Modifiers::NONE,
        );
        assert_eq!(app.editor.tool, Tool::Poke);
        assert_eq!(app.project(), &project);
        assert!(!app.editor.can_undo());
        assert!(app.editor.session.selection.is_empty());
        assert_eq!(app.canvas.camera.zoom, camera.zoom);
        assert_eq!(app.canvas.camera.offset, camera.offset);
        assert_eq!(app.simulation_mut().steps, 17);
        assert!(app.take_simulation_actions().is_empty());
    }
}

#[test]
fn text_focus_keeps_delete_arrows_and_clipboard_out_of_circuit_commands() {
    let (ctx, mut app) = setup();
    app.editor
        .session
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
    assert_eq!(app.editor.session.circuit.components[1].label, "memo");
    app.preferences
        .shortcuts
        .bind(crate::shortcuts::Command::ToolWire, "f7".parse().unwrap())
        .unwrap();
    key(&ctx, &mut app, Key::F7, Modifiers::NONE);
    assert_eq!(app.editor.tool, Tool::Select);
    key(&ctx, &mut app, Key::ArrowLeft, Modifiers::NONE);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    frame(&ctx, &mut app, vec![Event::Copy], Modifiers::COMMAND);
    assert_eq!(app.editor.session.circuit.components.len(), 2);
    assert_eq!(
        app.editor.session.circuit.components[1].position,
        Point::new(300, 300)
    );
    assert_eq!(app.editor.session.selection.len(), 1);
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
    assert_eq!(app.editor.session.circuit.components.len(), 2);
    assert_eq!(
        app.editor.session.circuit.components[1].position,
        Point::new(110, 110)
    );
}

#[test]
fn marquee_selects_components_and_wires_and_delete_is_undoable() {
    let (ctx, mut app) = setup();
    app.editor
        .session
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor
        .session
        .circuit
        .add_component(Component::new(Kind::Gate(Gate::Not), Point::new(240, 100)));
    app.editor
        .session
        .circuit
        .add_wire(Point::new(100, 100), Point::new(180, 100));
    let before = app.editor.session.circuit.clone();
    let a = app.canvas.screen(Point::new(50, 50));
    let b = app.canvas.screen(Point::new(280, 140));
    drag(&ctx, &mut app, a, b);
    assert_eq!(app.editor.session.selection.len(), 3);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    assert!(app.editor.session.circuit.ids().is_empty());
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(app.editor.session.circuit, before);
}

#[test]
fn cancelled_marquee_restores_the_selection_before_the_drag() {
    for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
        let (ctx, mut app) = setup();
        let a = Point::new(80, 80);
        let b = Point::new(280, 180);
        let first = app
            .editor
            .session
            .circuit
            .add_component(Component::new(Kind::Pin, a));
        let second = app
            .editor
            .session
            .circuit
            .add_component(Component::new(Kind::Pin, b));
        app.editor.session.selection.insert(first);
        let start = app.canvas.screen(b + Point::new(-75, -60));
        let end = app.canvas.screen(b + Point::new(60, 50));
        button_with(&ctx, &mut app, start, true, modifiers);
        frame(&ctx, &mut app, vec![Event::PointerMoved(end)], modifiers);
        assert!(app.editor.session.selection.contains(&second));
        key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
        button(&ctx, &mut app, end, false);
        assert_eq!(app.editor.session.selection, [first].into());
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn context_click_targets_the_object_or_preserves_its_selected_group() {
    let (ctx, mut app) = setup();
    let a = Point::new(80, 80);
    let b = Point::new(280, 180);
    let first = app
        .editor
        .session
        .circuit
        .add_component(Component::new(Kind::Pin, a));
    let second = app
        .editor
        .session
        .circuit
        .add_component(Component::new(Kind::Pin, b));
    let right_click = |app: &mut crate::Workspace, pos| {
        for pressed in [true, false] {
            frame(
                &ctx,
                app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
                Modifiers::NONE,
            );
        }
        egui::Popup::close_all(&ctx);
        frame(&ctx, app, vec![], Modifiers::NONE);
    };
    let at = app.canvas.screen(b);
    app.editor.session.selection.insert(first);
    right_click(&mut app, at);
    assert_eq!(app.editor.session.selection, [second].into());
    app.editor.session.selection.insert(first);
    right_click(&mut app, at);
    assert_eq!(app.editor.session.selection, [first, second].into());
    right_click(&mut app, at + egui::vec2(90., 90.));
    assert!(app.editor.session.selection.is_empty() && !app.editor.can_undo());
}

#[test]
fn escape_closes_the_canvas_context_menu_before_editing_shortcuts_resume() {
    let (ctx, mut app) = setup();
    let p = app.canvas.rect.center();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
    }
    assert!(egui::Popup::is_any_open(&ctx));
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(!egui::Popup::is_any_open(&ctx));
    key(&ctx, &mut app, Key::S, Modifiers::COMMAND);
    assert_eq!(app.take_file_actions(), [crate::FileAction::Save]);
}

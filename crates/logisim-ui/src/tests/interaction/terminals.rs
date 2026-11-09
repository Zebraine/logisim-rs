//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use crate::*;
use egui::{Event, Key, Modifiers};
use logisim_core::{Component, Kind, Point};

use super::support::*;

#[test]
fn keyboard_poke_routes_text_and_controls_without_editor_shortcuts_or_history() {
    let (ctx, mut app) = setup();
    let k = app
        .editor
        .session
        .circuit
        .add_component(Component::new(Kind::Keyboard, Point::new(200, 200)));
    app.editor.set_tool(Tool::Poke);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let pos = app.canvas.screen(Point::new(250, 200));
    click(&ctx, &mut app, pos);
    assert_eq!(app.editor.simulation.keyboard_focus, Some(k));
    let before = app.editor.session.circuit.clone();
    let camera = app.canvas.camera.offset;
    for (key, text) in [
        (Key::W, "w"),
        (Key::R, "r"),
        (Key::V, "v"),
        (Key::Num1, "1"),
        (Key::Space, " "),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![
                Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
                Event::Text(text.into()),
            ],
            Modifiers::NONE,
        );
        frame(
            &ctx,
            &mut app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
        );
    }
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("é中😀".into()), Event::Paste("AB".into())],
        Modifiers::NONE,
    );
    key(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    key(&ctx, &mut app, Key::Backspace, Modifiers::NONE);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    let expected = [119, 114, 118, 49, 32, 0xe9, 0x4e2d, 0xf600, 65, 66, 10, 8]
        .map(|value| SimulationAction::TypeCharacter { id: k, value });
    assert_eq!(app.take_simulation_actions(), expected);
    assert_eq!(app.editor.session.circuit, before);
    assert_eq!(app.canvas.camera.offset, camera);
    assert_eq!(app.editor.tool, Tool::Poke);
    key(&ctx, &mut app, Key::R, Modifiers::COMMAND);
    assert_eq!(app.take_simulation_actions(), vec![SimulationAction::Reset]);
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert_eq!(app.editor.simulation.keyboard_focus, None);
    key(&ctx, &mut app, Key::W, Modifiers::NONE);
    assert_eq!(app.editor.tool, Tool::Wire);
}

#[test]
fn keyboard_focus_does_not_survive_modal_window_tool_or_context_changes() {
    for case in 0..6 {
        let (ctx, mut app) = setup();
        let k = app
            .editor
            .session
            .circuit
            .add_component(Component::new(Kind::Keyboard, Point::new(200, 200)));
        app.editor.set_tool(Tool::Poke);
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
        let pos = app.canvas.screen(Point::new(250, 200));
        click(&ctx, &mut app, pos);
        assert_eq!(app.editor.simulation.keyboard_focus, Some(k));
        match case {
            0 => frame(
                &ctx,
                &mut app,
                vec![Event::WindowFocused(false)],
                Modifiers::NONE,
            ),
            1 => {
                app.settings_open = true;
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            2 => {
                app.editor.set_tool(Tool::Select);
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            3 => key(&ctx, &mut app, Key::Tab, Modifiers::NONE),
            4 => {
                app.editor.delete();
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            _ => {
                let empty = app.canvas.screen(Point::new(600, 400));
                click(&ctx, &mut app, empty);
            }
        }
        assert_eq!(app.editor.simulation.keyboard_focus, None, "{case}");
        frame(
            &ctx,
            &mut app,
            vec![Event::Text("should not enqueue".into())],
            Modifiers::NONE,
        );
        assert!(app.take_simulation_actions().is_empty());
    }
}

#[test]
fn pla_modal_blocks_canvas_and_file_shortcuts_and_escape_cancels() {
    let (ctx, mut app) = setup();
    app.editor.set_tool(Tool::Place(Kind::PlaRom));
    app.editor.place(Point::new(300, 200));
    app.editor.set_tool(Tool::Select);
    let id = app.circuit().components[0].id;
    app.editor.session.selection.insert(id);
    let original = app.circuit().clone();
    app.editor.pla_target = Some((false, id));
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    key(&ctx, &mut app, Key::Delete, Modifiers::NONE);
    key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::S, Modifiers::COMMAND);
    key(&ctx, &mut app, Key::O, Modifiers::COMMAND);
    assert_eq!(app.circuit(), &original);
    assert!(app.take_file_actions().is_empty());
    assert!(app.editor.pla_target.is_some());
    key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(app.editor.pla_target.is_none());
    assert_eq!(app.circuit(), &original);
}

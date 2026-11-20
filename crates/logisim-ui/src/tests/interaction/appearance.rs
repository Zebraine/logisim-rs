//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::*;
use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect};
use logisim_core::{Component, Kind, Point};

use super::support::*;

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
    assert!(app.editor.session.context.appearance_mode);
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
fn appearance_layer_shortcuts_restore_the_old_chords_and_follow_rebinding() {
    use crate::appearance::{Target, Tool as ArtTool};
    use crate::shortcuts::Command::AppearanceFront;
    let (ctx, mut app) = setup();
    let primary_shift = Modifiers::COMMAND.plus(Modifiers::SHIFT);
    key(&ctx, &mut app, Key::E, primary_shift);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    app.appearance.tool = ArtTool::Rect;
    let start = app.canvas.screen(Point::new(150, 150));
    let end = app.canvas.screen(Point::new(220, 190));
    drag(&ctx, &mut app, start, end);
    let initial = app.circuit().appearance.clone().unwrap();
    assert_eq!(initial.shapes.len(), 3);
    app.appearance.tool = ArtTool::Select;
    app.appearance.selected = [Target::Shape(initial.shapes[0].id)].into();
    let mut raised = initial.clone();
    raised.shapes.rotate_left(1);
    for (keycode, modifiers, expected) in [
        (Key::ArrowUp, Modifiers::SHIFT, &initial),
        (Key::ArrowUp, primary_shift, &raised),
        (Key::ArrowDown, Modifiers::SHIFT, &raised),
        (Key::ArrowDown, primary_shift, &initial),
    ] {
        key(&ctx, &mut app, keycode, modifiers);
        assert_eq!(
            app.circuit().appearance.as_ref(),
            Some(expected),
            "{keycode:?} {modifiers:?}"
        );
    }
    let mut prefs = app.preferences();
    prefs
        .shortcuts
        .bind(AppearanceFront, "f5".parse().unwrap())
        .unwrap();
    app.apply_settings(prefs);
    key(&ctx, &mut app, Key::ArrowUp, primary_shift);
    assert_eq!(app.circuit().appearance.as_ref(), Some(&initial));
    key(&ctx, &mut app, Key::F5, Modifiers::NONE);
    assert_eq!(app.circuit().appearance.as_ref(), Some(&raised));

    key(&ctx, &mut app, Key::E, primary_shift);
    assert!(!app.editor.session.context.appearance_mode);
    key(&ctx, &mut app, Key::ArrowDown, primary_shift);
    assert_eq!(app.circuit().appearance.as_ref(), Some(&raised));
}

#[test]
fn appearance_port_overlapping_anchor_can_move_independently_and_undo() {
    use crate::appearance::{AppearanceEditor, Target};
    let (ctx, mut app) = setup();
    let pin = app
        .editor
        .session
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor.sync_project();
    app.editor.session.context.appearance_mode = true;
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
    app.editor.session.context.appearance_mode = true;
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
fn native_clipboard_publishes_appearance_shapes_once_and_preserves_the_circuit_clipboard() {
    use crate::appearance::AppearanceEditor;
    use crate::editor::CLIPBOARD_MARKER;
    let (ctx, mut app) = setup();
    app.editor.set_tool(crate::editor::Tool::Place(Kind::Pin));
    app.editor.place(Point::new(100, 100));
    app.editor.copy();
    let size = egui::vec2(1200., 820.);
    let send = |app: &mut Workspace, events| {
        let mut output = ctx.run_ui(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        output
            .platform_output
            .commands
            .into_iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(s) => Some(s),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        send(&mut app, vec![Event::PointerMoved(egui::pos2(700., 400.))]),
        [CLIPBOARD_MARKER]
    );
    app.editor.session.context.appearance_mode = true;
    send(&mut app, vec![]);
    key(&ctx, &mut app, Key::A, Modifiers::COMMAND);
    let initial = AppearanceEditor::current(&app.editor).shapes;
    let press = |key| Event::Key {
        key,
        physical_key: None,
        modifiers: Modifiers::COMMAND,
        pressed: true,
        repeat: false,
    };
    let copied = send(&mut app, vec![press(Key::C), Event::Copy]);
    assert_eq!(copied, ["logisim-rs appearance (paste in the same window)"]);
    assert_eq!(app.appearance.clipboard, initial);
    assert_eq!(send(&mut app, vec![Event::Cut, press(Key::X)]), copied);
    assert_eq!(app.appearance.clipboard, initial);
    assert!(AppearanceEditor::current(&app.editor).shapes.is_empty());
    send(
        &mut app,
        vec![Event::Paste(copied[0].clone()), press(Key::V)],
    );
    assert_eq!(
        AppearanceEditor::current(&app.editor).shapes.len(),
        initial.len()
    );
    app.editor.session.context.appearance_mode = false;
    send(&mut app, vec![Event::Paste(copied[0].clone())]);
    assert_eq!(app.circuit().components.len(), 1);
    send(
        &mut app,
        vec![Event::Paste(CLIPBOARD_MARKER.into()), press(Key::V)],
    );
    assert_eq!(app.circuit().components.len(), 2);
}

#[test]
fn appearance_text_focus_keeps_edit_keys_out_of_artwork_commands() {
    use crate::appearance::Tool as ArtTool;
    let (ctx, mut app) = setup();
    app.editor.session.context.appearance_mode = true;
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
            .session
            .circuit
            .add_component(Component::new(Kind::Pin, Point::new(100, y)));
    }
    app.editor.sync_project();
    app.editor.session.context.appearance_mode = true;
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
    app.editor.session.context.appearance_mode = true;
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
        .session
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    app.editor.sync_project();
    app.editor.session.context.appearance_mode = true;
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

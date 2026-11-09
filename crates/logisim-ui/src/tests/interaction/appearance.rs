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

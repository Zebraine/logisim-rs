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
fn canvas_keeps_the_right_hand_layout_when_resizing_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        for density in [Density::Comfortable, Density::Compact] {
            app.apply_settings(Settings {
                density,
                ..app.preferences()
            });
            let m = density.metrics();
            for size in [vec2(1440., 900.), vec2(1200., 820.), vec2(720., 480.)] {
                for _ in 0..3 {
                    frame(&ctx, &mut app, size, vec![]);
                }
                let canvas = app.canvas.rect;
                assert_eq!(canvas.left(), m.sidebar.default.min(size.x - 452.));
                assert!(canvas.width() > 450.);
                let toolbar = if size.x == 720. {
                    m.toolbar_wrapped
                } else {
                    m.toolbar_wide
                };
                assert_eq!(canvas.top(), m.menu + toolbar, "{density:?} {size:?}");
                assert_eq!(canvas.right_bottom(), size.to_pos2());
            }
        }
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
        app.about_open = true;
        frame(&ctx, &mut app, size, vec![event.clone()]);
        assert!(app.about_open && app.take_file_actions().is_empty());
        app.about_open = false;
        app.project_options_open = true;
        frame(&ctx, &mut app, size, vec![event.clone()]);
        assert!(app.project_options_open && app.take_file_actions().is_empty());
        app.project_options_open = false;
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

#[test]
fn initial_zoom_follows_instance_navigation_in_both_directions() {
    use logisim_core::{Point, Project};
    let mut project = Project::default();
    let main = project.main;
    let child = project.add("child".into()).unwrap();
    for (id, width) in [(main, 1600), (child, 2600)] {
        project
            .circuit_mut(id)
            .unwrap()
            .circuit
            .add_wire(Point::ZERO, Point::new(width, 0))
            .unwrap();
    }
    let instance = project.instance(child, Point::new(210, 140)).unwrap();
    let instance = project.main_circuit_mut().circuit.add_component(instance);
    let size = vec2(1200., 820.);
    for default_zoom in [DefaultZoom::Actual, DefaultZoom::Fit] {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.apply_settings(Settings {
            default_zoom,
            ..Settings::default()
        });
        app.replace_project(project.clone());
        let check_zoom = |app: &Workspace, width: f32| {
            let expected = match default_zoom {
                DefaultZoom::Actual => 1.0,
                DefaultZoom::Fit => (app.canvas.rect.width() - 80.) / width,
            };
            assert!((app.canvas.camera.zoom - expected).abs() < 0.001);
        };
        frame(&ctx, &mut app, size, vec![]);
        check_zoom(&app, 1600.);
        app.canvas.set_zoom(2.0);
        app.editor.open_instance = Some(instance);
        frame(&ctx, &mut app, size, vec![]);
        frame(&ctx, &mut app, size, vec![]);
        assert_eq!(app.simulation_path(), &[instance]);
        check_zoom(&app, 2600.);
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::Key {
                key: egui::Key::ArrowLeft,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::ALT,
            }],
        );
        frame(&ctx, &mut app, size, vec![]);
        assert!(app.simulation_path().is_empty());
        assert_eq!(app.active_circuit(), main);
        check_zoom(&app, 1600.);
    }
}

#[test]
fn restored_sidebar_projects_onto_narrow_canvas_without_losing_preferred_width() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let layout = WorkspaceLayout {
        sidebar_width: 374.,
        split: 0.63,
        tab: LayoutTab::Components,
    };
    app.set_layout(layout);
    for width in [1200., 720., 1200.] {
        for _ in 0..3 {
            frame(&ctx, &mut app, vec2(width, 820.), vec![]);
        }
        let canvas = app.canvas.rect;
        assert!(canvas.width() > 450.);
        assert_eq!(canvas.left(), layout.sidebar_width.min(width - 452.));
        assert_eq!(app.layout(), layout);
    }
    let camera = app.canvas.camera;
    app.set_layout(WorkspaceLayout::default());
    frame(&ctx, &mut app, vec2(1200., 820.), vec![]);
    assert_eq!(app.canvas.rect.left(), 320.);
    assert_eq!(app.canvas.camera.zoom, camera.zoom);
    assert_eq!(app.canvas.camera.offset, camera.offset);
}

#[test]
fn inverted_wheel_keeps_anchor_limits_and_pinch_direction() {
    let size = vec2(1200., 820.);
    let pointer = pos2(650., 355.);
    for inverted in [false, true] {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.apply_settings(Settings {
            invert_wheel_zoom: inverted,
            ..app.preferences()
        });
        for _ in 0..2 {
            frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
        }
        let anchor = pointer - app.canvas.rect.min;
        let world = app.canvas.camera.world_at(anchor);
        frame(&ctx, &mut app, size, vec![wheel()]);
        let expected = if inverted { 1. / 1.1 } else { 1.1 };
        assert!((app.canvas.camera.zoom - expected).abs() < 0.0001);
        frame(&ctx, &mut app, size, vec![Event::Zoom(1.5)]);
        assert!((app.canvas.camera.zoom - expected * 1.5).abs() < 0.0001);
        for _ in 0..6 {
            frame(&ctx, &mut app, size, vec![wheel(); 8]);
        }
        assert_eq!(
            app.canvas.camera.zoom,
            if inverted {
                canvas::MIN_ZOOM
            } else {
                canvas::MAX_ZOOM
            }
        );
        assert!((app.canvas.camera.world_at(anchor) - world).length() < 0.002);
    }
}

#[test]
fn chrome_switches_preserve_document_selection_history_camera_and_simulation() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let mut circuit = logisim_core::Circuit::default();
    circuit.add_wire(
        logisim_core::Point::new(40, 40),
        logisim_core::Point::new(140, 40),
    );
    let wire = circuit.wires[0].id;
    app.replace_circuit(circuit);
    let size = vec2(1200., 820.);
    frame(&ctx, &mut app, size, vec![]);
    app.editor.session.selection.insert(wire);
    app.canvas.camera.zoom = 1.25;
    app.canvas.camera.offset = vec2(73., -61.);
    app.simulation_mut().ticks = 41;
    app.simulation_mut().enabled = false;
    let document = app.project().clone();
    for density in [Density::Comfortable, Density::Compact] {
        for accent in [Accent::Indigo, Accent::Blue, Accent::Teal] {
            app.apply_settings(Settings {
                density,
                accent,
                ..app.preferences()
            });
            frame(&ctx, &mut app, size, vec![]);
            assert_eq!(app.project(), &document);
            assert!(!app.document.dirty && !app.editor.can_undo());
            assert_eq!(
                app.editor
                    .session
                    .selection
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                [wire]
            );
            assert_eq!(app.canvas.camera.zoom, 1.25);
            assert_eq!(app.canvas.camera.offset, vec2(73., -61.));
            assert_eq!(app.simulation_mut().ticks, 41);
            assert!(!app.simulation_mut().enabled);
            assert!(app.take_simulation_actions().is_empty());
        }
    }
}

#[test]
fn selection_and_primary_button_paint_with_each_accent() {
    use egui::Shape;
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let mut circuit = logisim_core::Circuit::default();
    circuit.add_wire(
        logisim_core::Point::new(50, 50),
        logisim_core::Point::new(200, 50),
    );
    let wire = circuit.wires[0].id;
    app.replace_circuit(circuit);
    app.editor.session.selection.insert(wire);
    let size = vec2(1200., 820.);
    for accent in [Accent::Indigo, Accent::Blue, Accent::Teal] {
        app.apply_settings(Settings {
            accent,
            ..app.preferences()
        });
        frame(&ctx, &mut app, size, vec![]);
        let output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        let c = accent.colors();
        let shapes = || output.shapes.iter().map(|s| &s.shape);
        assert!(shapes().any(|s| matches!(s,Shape::Rect(r) if r.fill==c.base && r.rect.width()>=59. && r.rect.top()<80.)), "primary button");
        assert!(shapes().any(|s| matches!(s,Shape::Rect(r) if r.fill==c.selected && r.rect.width()>80. && r.rect.left()<320.)), "selected catalogue tab");
        assert!(shapes().any(|s| matches!(s,Shape::Rect(r) if r.rect.size()==crate::drawing::thumbnails::SIZE && r.stroke.color==c.base)), "thumbnail");
        assert!(shapes().any(|s| matches!(s,Shape::LineSegment{stroke,..} if stroke.color==c.base.gamma_multiply(0.25))), "selected wire halo");
        assert_eq!(shapes().filter(|s|matches!(s,Shape::Rect(r) if r.fill==c.base && r.rect.size()==Vec2::splat(6.))).count(),2);
    }
}

#[test]
fn remapped_commands_follow_scope_and_dispatch_their_own_actions() {
    use crate::shortcuts::Command;
    use logisim_core::{Point, Project};
    let ctx = Context::default();
    configure(&ctx);
    let mut w = Workspace::default();
    let mut settings = w.preferences();
    for (i, c) in Command::ALL
        .into_iter()
        .filter(|c| c.scope() != crate::shortcuts::Scope::Appearance)
        .enumerate()
    {
        settings
            .shortcuts
            .bind(c, format!("f{}", i + 1).parse().unwrap())
            .unwrap();
    }
    w.apply_settings(settings);
    let size = vec2(1200., 820.);
    let press = |w: &mut Workspace, c: Command| {
        let key = egui::Key::from_name(&settings.shortcuts.get(c).label(false)).unwrap();
        frame(
            &ctx,
            w,
            size,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    };
    frame(
        &ctx,
        &mut w,
        size,
        vec![Event::PointerMoved(pos2(15., 100.))],
    );
    for (c, action) in [
        (Command::New, FileAction::New),
        (Command::Open, FileAction::Open),
        (Command::Save, FileAction::Save),
        (Command::SaveAs, FileAction::SaveAs),
        (Command::Exit, FileAction::Close),
    ] {
        press(&mut w, c);
        assert_eq!(w.take_file_actions(), [action]);
    }
    press(&mut w, Command::Reset);
    assert!(w.take_simulation_actions().is_empty());
    press(&mut w, Command::AddCircuit);
    assert!(w.name_dialog.take().is_some());
    for _ in 0..2 {
        frame(&ctx, &mut w, size, vec![]);
    }
    press(&mut w, Command::ToggleAppearance);
    assert!(w.editor.session.context.appearance_mode);
    press(&mut w, Command::ToggleAppearance);
    let mut project = Project::default();
    let child = project.add("child".into()).unwrap();
    let instance = project.instance(child, Point::ZERO).unwrap();
    let id = project.main_circuit_mut().circuit.add_component(instance);
    w.replace_project(project);
    w.editor.enter_instance(id);
    press(&mut w, Command::ReturnParent);
    assert!(w.editor.session.context.path.is_empty());
    frame(
        &ctx,
        &mut w,
        size,
        vec![Event::PointerMoved(pos2(710., 400.))],
    );
    w.editor.place(Point::new(200, 200));
    let count = w.circuit().components.len();
    press(&mut w, Command::Undo);
    assert_eq!(w.circuit().components.len(), count - 1);
    press(&mut w, Command::Redo);
    assert_eq!(w.circuit().components.len(), count);
    press(&mut w, Command::Copy);
    assert!(w.editor.can_paste());
    press(&mut w, Command::Cut);
    assert_eq!(w.circuit().components.len(), count - 1);
    press(&mut w, Command::Paste);
    assert_eq!(w.circuit().components.len(), count);
    press(&mut w, Command::Duplicate);
    assert_eq!(w.circuit().components.len(), count + 1);
    press(&mut w, Command::SelectAll);
    assert_eq!(w.editor.session.selection.len(), count + 1);
    w.simulation_mut().has_clocks = true;
    w.simulation_mut().enabled = false;
    for (c, action) in [
        (Command::Toggle, SimulationAction::ToggleEnabled),
        (Command::Step, SimulationAction::Step),
        (Command::Reset, SimulationAction::Reset),
        (Command::Tick, SimulationAction::Tick),
        (Command::AutomaticTicks, SimulationAction::ToggleTicks),
    ] {
        press(&mut w, c);
        assert_eq!(w.take_simulation_actions(), [action]);
    }
    let tick = Event::Key {
        key: egui::Key::from_name(&settings.shortcuts.get(Command::Tick).label(false)).unwrap(),
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    frame(&ctx, &mut w, size, vec![tick.clone(), tick]);
    assert_eq!(
        w.take_simulation_actions(),
        [SimulationAction::Tick, SimulationAction::Tick]
    );
    w.simulation_mut().limited = true;
    w.simulation_mut().has_clocks = false;
    for c in [Command::Step, Command::Tick, Command::AutomaticTicks] {
        press(&mut w, c);
    }
    assert!(w.take_simulation_actions().is_empty());
    w.settings_open = true;
    press(&mut w, Command::Save);
    press(&mut w, Command::Reset);
    assert!(w.take_file_actions().is_empty() && w.take_simulation_actions().is_empty());
}

#[test]
fn text_focus_popup_and_ime_block_custom_workspace_commands() {
    use crate::shortcuts::Command;
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    app.preferences
        .shortcuts
        .bind(Command::Save, "f5".parse().unwrap())
        .unwrap();
    app.preferences
        .shortcuts
        .bind(Command::Reset, "f8".parse().unwrap())
        .unwrap();
    let size = vec2(1200., 820.);
    let key = |key, modifiers| Event::Key {
        key,
        modifiers,
        physical_key: None,
        pressed: true,
        repeat: false,
    };
    let attempt = || {
        vec![
            key(egui::Key::F5, Modifiers::NONE),
            key(egui::Key::F8, Modifiers::NONE),
        ]
    };
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(pos2(900., 600.))],
    );
    let mut ime = attempt();
    ime.push(Event::Ime(egui::ImeEvent::Preedit {
        text: "拼".into(),
        active_range_chars: None,
    }));
    frame(&ctx, &mut app, size, ime);
    assert!(app.take_file_actions().is_empty() && app.take_simulation_actions().is_empty());
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::F, Modifiers::COMMAND)],
    );
    frame(&ctx, &mut app, size, attempt());
    assert!(ctx.text_edit_focused());
    assert!(app.take_file_actions().is_empty() && app.take_simulation_actions().is_empty());
    crate::tests::support::widgets::click(&ctx, &mut app, "File");
    let menu = crate::tests::support::widgets::frame(&ctx, &mut app, vec![]);
    assert!(crate::tests::support::widgets::has(&menu, "Save · F5"));
    frame(&ctx, &mut app, size, attempt());
    assert!(app.take_file_actions().is_empty() && app.take_simulation_actions().is_empty());
}

#[test]
fn entering_appearance_mode_blocks_a_later_tool_key_in_the_same_frame() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200., 820.);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(pos2(800., 400.))],
    );
    let key = |key, modifiers| Event::Key {
        key,
        modifiers,
        pressed: true,
        repeat: false,
        physical_key: None,
    };
    frame(
        &ctx,
        &mut app,
        size,
        vec![
            key(egui::Key::E, Modifiers::COMMAND.plus(Modifiers::SHIFT)),
            key(egui::Key::W, Modifiers::NONE),
        ],
    );
    assert!(app.editor.session.context.appearance_mode);
    assert_eq!(app.editor.tool, crate::editor::Tool::Select);
}

#[test]
fn appearance_bindings_leave_text_ime_and_modals_and_terminal_input_alone() {
    use crate::shortcuts::Command::AppearanceSelectAll;
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    app.editor.session.context.appearance_mode = true;
    app.preferences
        .shortcuts
        .bind(AppearanceSelectAll, "f6".parse().unwrap())
        .unwrap();
    let size = vec2(1200., 820.);
    let key = |key, modifiers| Event::Key {
        key,
        modifiers,
        physical_key: None,
        pressed: true,
        repeat: false,
    };
    let attempt = || key(egui::Key::F6, Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(pos2(650., 420.))],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![
            attempt(),
            Event::Ime(egui::ImeEvent::Preedit {
                text: "字".into(),
                active_range_chars: None,
            }),
        ],
    );
    assert!(app.appearance.selected.is_empty());
    app.editor.simulation.keyboard_focus = Some(1);
    frame(&ctx, &mut app, size, vec![attempt()]);
    assert!(app.appearance.selected.is_empty());
    app.editor.simulation.release_keyboard();
    app.settings_open = true;
    frame(&ctx, &mut app, size, vec![attempt()]);
    assert!(app.appearance.selected.is_empty());
    app.settings_open = false;
    frame(&ctx, &mut app, size, vec![]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::F, Modifiers::COMMAND)],
    );
    frame(&ctx, &mut app, size, vec![attempt()]);
    assert!(ctx.text_edit_focused());
    assert!(app.appearance.selected.is_empty());
}

#[test]
fn appearance_commands_follow_shape_selection_clipboard_history_and_layer_order() {
    use crate::{
        appearance::{AppearanceEditor, Target},
        shortcuts::Command::*,
    };
    use egui::Key::*;
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let size = vec2(1200., 820.);
    let mut settings = app.preferences();
    for (i, command) in [
        AppearanceUndo,
        AppearanceRedo,
        AppearanceCut,
        AppearanceCopy,
        AppearancePaste,
        AppearanceDuplicate,
        AppearanceSelectAll,
        AppearanceRaise,
        AppearanceLower,
        AppearanceFront,
        AppearanceBack,
    ]
    .into_iter()
    .enumerate()
    {
        settings
            .shortcuts
            .bind(command, format!("f{}", i + 1).parse().unwrap())
            .unwrap();
    }
    let press = |app: &mut Workspace, key, modifiers| {
        frame(
            &ctx,
            app,
            size,
            vec![Event::Key {
                key,
                modifiers,
                pressed: true,
                repeat: false,
                physical_key: None,
            }],
        )
    };
    app.editor.session.context.appearance_mode = true;
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(pos2(800., 400.))],
    );
    app.appearance.selected.insert(Target::Anchor);
    let project = app.project().clone();
    let camera = app.canvas.camera;
    app.simulation_mut().steps = 9;
    app.apply_settings(settings);
    assert_eq!(app.project(), &project);
    assert_eq!(app.appearance.selected, [Target::Anchor].into());
    assert_eq!(app.canvas.camera.offset, camera.offset);
    assert_eq!(app.canvas.camera.zoom, camera.zoom);
    assert_eq!(app.simulation_mut().steps, 9);
    assert!(app.take_simulation_actions().is_empty());
    let shape_actions = [
        AppearanceCut,
        AppearanceCopy,
        AppearanceDuplicate,
        AppearanceRaise,
        AppearanceLower,
        AppearanceFront,
        AppearanceBack,
    ];
    assert!(shape_actions.into_iter().all(|c| !app.command_enabled(c)));
    assert!(
        [AppearanceUndo, AppearanceRedo, AppearancePaste]
            .into_iter()
            .all(|c| !app.command_enabled(c))
    );
    assert!(app.command_enabled(AppearanceSelectAll));
    let initial = AppearanceEditor::current(&app.editor).shapes;
    let n = initial.len();
    let first = initial[0].id;
    press(&mut app, F7, Modifiers::NONE);
    assert!(shape_actions.into_iter().all(|c| app.command_enabled(c)));
    press(&mut app, F4, Modifiers::NONE);
    assert_eq!(app.appearance.clipboard, initial);
    assert!(app.command_enabled(AppearancePaste));
    let count = |app: &Workspace| AppearanceEditor::current(&app.editor).shapes.len();
    for (key, expected) in [(F6, 2 * n), (F5, 3 * n), (F3, 2 * n), (F1, 3 * n)] {
        press(&mut app, key, Modifiers::NONE);
        assert_eq!(count(&app), expected);
    }
    assert!(app.appearance.selected.is_empty());
    assert!(app.command_enabled(AppearanceRedo));
    press(&mut app, Y, Modifiers::COMMAND);
    assert_eq!(count(&app), 3 * n);
    press(&mut app, F2, Modifiers::NONE);
    assert_eq!(count(&app), 2 * n);
    app.appearance.selected.insert(Target::Shape(first));
    for (key, index) in [(F8, 1), (F9, 0), (F10, 2 * n - 1), (F11, 0)] {
        press(&mut app, key, Modifiers::NONE);
        assert_eq!(
            AppearanceEditor::current(&app.editor).shapes[index].id,
            first
        );
    }
    let before = app.project().clone();
    for (key, mods) in [(ArrowUp, Modifiers::SHIFT), (ArrowDown, Modifiers::COMMAND)] {
        press(&mut app, key, mods);
    }
    assert_eq!(app.project(), &before);
}

use super::*;
#[test]
fn edits_restore_geometry_and_selection_through_undo_redo() {
    let mut e = Editor::default();
    e.place(Point::new(200, 100));
    let placed = e.session.circuit.clone();
    let id = *e.session.selection.first().unwrap();
    e.nudge(Point::new(40, 20));
    e.rotate();
    e.delete();
    assert!(e.session.circuit.components.is_empty());
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(e.session.circuit, placed);
    assert!(e.session.selection.contains(&id));
    e.redo();
    assert_eq!(
        e.session.circuit.component(id).unwrap().position,
        Point::new(240, 120)
    );
}
#[test]
fn repeated_paste_assigns_unique_ids_and_leaves_original_untouched() {
    let mut e = Editor::default();
    e.place(Point::new(100, 100));
    e.copy();
    e.paste();
    e.paste();
    assert_eq!(e.session.circuit.components.len(), 3);
    assert_eq!(e.session.circuit.ids().len(), 3);
    assert_eq!(
        e.session.circuit.components[0].position,
        Point::new(100, 100)
    );
    e.undo();
    assert_eq!(e.session.circuit.components.len(), 2);
    e.delete();
    assert!(!e.can_redo());
}
#[test]
fn property_typing_is_one_undo_transaction() {
    let mut e = Editor {
        template: Component::new(Kind::Tunnel, Point::ZERO),
        ..Editor::default()
    };
    e.place(Point::new(100, 100));
    for label in ["D", "DA", "DAT", "DATA"] {
        let mut c = e.session.circuit.components[0].clone();
        c.label = label.into();
        e.update_property(c);
    }
    e.commit_properties();
    e.undo();
    assert_eq!(e.session.circuit.components[0].label, "tunnel");
    e.redo();
    assert_eq!(e.session.circuit.components[0].label, "DATA");
}

#[test]
fn properties_reverted_before_commit_preserve_the_redo_branch() {
    let mut e = Editor::default();
    e.place(Point::new(100, 100));
    e.place(Point::new(200, 100));
    e.undo();
    assert!(e.can_redo());
    let original = e.session.circuit.components[0].clone();
    let mut changed = original.clone();
    changed.label = "temporary".into();
    e.update_property(changed);
    assert!(!e.can_redo());
    e.update_property(original);
    e.commit_properties();
    assert!(e.can_redo());
    e.redo();
    assert_eq!(e.session.circuit.components.len(), 2);
}

#[test]
fn grouped_properties_commit_before_leaving_an_instance_and_restore_the_same_context() {
    let mut project = Project::default();
    let child = project.add("child".into()).unwrap();
    let pin = project
        .circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    let instance = project.instance(child, Point::new(300, 200)).unwrap();
    let instance = project.main_circuit_mut().circuit.add_component(instance);
    let mut e = Editor::new(project, crate::Settings::default());
    e.enter_instance(instance);
    e.session.context.appearance_mode = true;
    e.session.selection.insert(pin);
    let before = e.snapshot();

    let mut component = e.session.circuit.component(pin).unwrap().clone();
    component.label = "DATA".into();
    e.update_property(component);
    let mut label = e.session.circuit.label.clone();
    label.text = "CELL".into();
    e.update_circuit_label(label);
    let appearance = crate::appearance::AppearanceEditor::current(&e);
    e.update_appearance(appearance);
    e.leave_instance();
    let committed = e.session.project.clone();
    assert_eq!(e.session.context.active, committed.main);
    assert!(e.session.context.path.is_empty());
    assert_eq!(
        committed.main_circuit().circuit.components[0]
            .interface
            .label
            .text,
        "CELL"
    );
    // Simulation state is owned separately and must not be rolled back with a document.
    e.simulation.view.ticks = 41;
    e.undo();
    assert!(!e.can_undo());
    assert_eq!(e.session.project, before.project);
    assert_eq!(e.session.context, before.context);
    assert_eq!(e.session.selection, before.selection);
    assert_eq!(e.simulation.view.ticks, 41);
    e.redo();
    assert_eq!(e.session.project, committed);
    assert_eq!(e.session.context.active, child);
    assert_eq!(e.session.context.root, committed.main);
    assert_eq!(e.session.context.path, vec![instance]);
    // Navigation turns off appearance mode before it commits pending properties.
    assert!(!e.session.context.appearance_mode);
    assert_eq!(e.simulation.view.ticks, 41);
}

#[test]
fn display_switches_change_wire_paint_only() {
    use crate::{Settings, Workspace};
    use egui::{Context, RawInput, Shape, vec2};
    let mut project = Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    let join = Point::new(140, 40);
    let end = Point::new(140, 150);
    let selected = circuit.add_wire(Point::new(40, 40), join).unwrap();
    circuit.add_wire(join, end);
    circuit.add_wire(join, Point::new(240, 40));
    let connected = circuit.connected_wires(selected);
    let dangling = circuit.dangling_ends();
    let ctx = Context::default();
    let mut app = Workspace::default();
    app.replace_project(project.clone());
    app.editor.session.selection.insert(selected);
    app.simulation_mut().ticks = 91;
    for (highlight_net, show_connection_markers) in
        [(true, true), (false, true), (true, false), (false, false)]
    {
        app.apply_settings(Settings {
            highlight_net,
            show_connection_markers,
            ..app.preferences()
        });
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200., 820.))),
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        let branch = [app.canvas.screen(join), app.canvas.screen(end)];
        let trunk = [
            app.canvas.screen(Point::new(40, 40)),
            app.canvas.screen(join),
        ];
        let widths = |segment| {
            output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    Shape::LineSegment { points, stroke } if *points == segment => {
                        Some(stroke.width)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(widths(trunk), [7., 1.8]);
        assert_eq!(
            widths(branch),
            if highlight_net {
                vec![5., 1.8]
            } else {
                vec![1.8]
            }
        );
        for center in branch {
            assert_eq!(
                output.shapes.iter().any(
                    |s| matches!(&s.shape, Shape::Circle(c) if c.center == center && c.radius == 3.)
                ),
                show_connection_markers
            );
        }
        assert_eq!(app.editor.hit(vec2(90., 40.), 2.), Some(selected));
        assert_eq!(app.circuit().connected_wires(selected), connected);
        assert_eq!(app.circuit().dangling_ends(), dangling);
        assert_eq!(app.editor.session.selection, BTreeSet::from([selected]));
        assert_eq!(app.project(), &project);
        assert_eq!(app.simulation_mut().ticks, 91);
        assert!(!app.editor.can_undo());
        assert!(app.take_simulation_actions().is_empty());
    }
}

#[test]
fn placement_defaults_apply_only_when_rebuilding_a_pin_or_probe_template() {
    use crate::{Settings, Workspace};
    use logisim_core::Radix;
    let mut app = Workspace::default();
    app.editor.set_tool(Tool::Place(Kind::Pin));
    app.editor.template.bits = 3;
    app.editor.template.label = "manual".into();
    let manual = app.editor.template.clone();
    app.editor.place(Point::new(20, 20));
    let old_id = *app.editor.session.selection.first().unwrap();
    let original = app.project().clone();
    app.editor.copy();
    app.editor.set_tool(Tool::Place(Kind::Pin));
    app.apply_settings(Settings {
        default_pin_bits: 8,
        default_probe_radix: Radix::Hex,
        ..app.preferences()
    });
    assert_eq!(app.editor.template, manual);
    assert_eq!(app.project(), &original);
    app.editor.set_tool(Tool::Place(Kind::Probe));
    assert_eq!(app.editor.template.radix, Radix::Hex);
    app.editor.template.radix = Radix::Octal;
    app.apply_settings(Settings {
        default_probe_radix: Radix::Unsigned,
        ..app.preferences()
    });
    app.editor.set_tool(Tool::Place(Kind::Probe));
    assert_eq!(app.editor.template.radix, Radix::Octal);
    app.editor.place(Point::new(80, 80));
    let probe = *app.editor.session.selection.first().unwrap();
    app.editor.set_tool(Tool::Place(Kind::Pin));
    let mut expected = Component::new(Kind::Pin, Point::ZERO);
    expected.bits = 8;
    assert_eq!(app.editor.template, expected);
    app.editor.place(Point::new(100, 100));
    assert_eq!(app.circuit().components.last().unwrap().bits, 8);
    app.editor.paste();
    assert_eq!(app.circuit().components.last().unwrap().bits, 3);
    assert_eq!(app.circuit().component(old_id).unwrap().bits, 3);
    assert_eq!(app.circuit().component(probe).unwrap().radix, Radix::Octal);
    app.replace_project(original);
    app.editor.set_tool(Tool::Place(Kind::Probe));
    assert_eq!(app.editor.template.radix, Radix::Unsigned);
    assert!(!app.editor.can_undo());
}

#[test]
fn wire_badges_leave_inspection_hit_selection_and_simulation_available() {
    use crate::{Settings, SignalPalette, SignalTone, SignalView, Workspace};
    use egui::{Context, Event, RawInput, Shape, vec2};
    let ctx = Context::default();
    ctx.global_style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let mut project = Project::default();
    let id = project
        .main_circuit_mut()
        .circuit
        .add_wire(Point::new(50, 80), Point::new(350, 80))
        .unwrap();
    let mut app = Workspace::default();
    app.replace_project(project.clone());
    app.editor.session.selection.insert(id);
    app.simulation_mut().wires.insert(
        id,
        SignalView {
            text: "A5".into(),
            binary: "10100101".into(),
            tone: SignalTone::Bus,
        }
        .into(),
    );
    app.simulation_mut().ticks = 19;
    let mut now = 0.;
    let mut frame = |app: &mut Workspace, events| {
        now += 0.2;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200., 820.))),
                time: Some(now),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        output.shapes
    };
    for (palette, visible) in [
        (SignalPalette::Default, true),
        (SignalPalette::HighContrast, false),
        (SignalPalette::ColorblindSafe, true),
    ] {
        app.apply_settings(Settings {
            signal_palette: palette,
            show_wire_values: visible,
            ..app.preferences()
        });
        frame(&mut app, vec![Event::PointerGone]);
        let painted = frame(&mut app, vec![]);
        let text =
            |s: &Shape, expected: &str| matches!(s,Shape::Text(t) if t.galley.job.text==expected);
        assert_eq!(painted.iter().any(|s| text(&s.shape, "A5")), visible);
        assert!(painted.iter().any(|s| text(&s.shape, "Signal: A5")));
        let segment = [
            app.canvas.screen(Point::new(50, 80)),
            app.canvas.screen(Point::new(350, 80)),
        ];
        assert!(painted.iter().any(|s| matches!(&s.shape,Shape::LineSegment{points,stroke} if *points==segment && stroke.width==3. && stroke.color==palette.color(SignalTone::Bus))));
        assert_eq!(app.editor.hit(vec2(100., 80.), 2.), Some(id));
        let pointer = app.canvas.screen(Point::new(100, 80));
        frame(&mut app, vec![Event::PointerMoved(pointer)]);
        for _ in 0..3 {
            frame(&mut app, vec![]);
        }
        let hover = frame(&mut app, vec![]);
        assert!(
            hover.iter().any(|s| text(&s.shape, "8 bit")),
            "bus hover remains available"
        );
        assert_eq!(app.editor.session.selection, BTreeSet::from([id]));
        assert_eq!(app.project(), &project);
        assert!(!app.document.dirty && !app.editor.can_undo());
        assert_eq!(app.simulation_mut().ticks, 19);
        assert!(app.take_simulation_actions().is_empty());
    }
}

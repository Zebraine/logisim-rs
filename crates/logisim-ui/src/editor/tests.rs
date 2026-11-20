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
    frame(&mut app, vec![]);
    let camera = (app.canvas.camera.offset, app.canvas.camera.zoom);
    for (mode, palette, visible) in [
        (crate::ThemeMode::Light, SignalPalette::Default, true),
        (crate::ThemeMode::Dark, SignalPalette::HighContrast, false),
        (crate::ThemeMode::Dark, SignalPalette::ColorblindSafe, true),
    ] {
        app.apply_settings(Settings {
            theme: mode,
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
        assert!(painted.iter().any(|s| matches!(&s.shape,Shape::LineSegment{points,stroke} if *points==segment && stroke.width==3. && stroke.color==palette.color(mode, SignalTone::Bus))));
        if visible && mode == crate::ThemeMode::Dark {
            let p = crate::theme::palette(&ctx);
            let badge = painted
                .iter()
                .find_map(|s| match &s.shape {
                    Shape::Text(t) if t.galley.job.text == "A5" => Some(t),
                    _ => None,
                })
                .unwrap();
            assert_eq!(badge.galley.job.sections[0].format.color, p.ink);
            assert!(painted.iter().any(|s| matches!(&s.shape, Shape::Rect(r) if r.rect.contains(badge.pos) && r.fill == p.surface && r.stroke.color == p.border_strong)));
        }
        assert_eq!((app.canvas.camera.offset, app.canvas.camera.zoom), camera);
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

#[test]
fn programmatic_pin_width_is_bounded_when_placed() {
    for (input, expected) in [(0, 1), (255, 32)] {
        let mut app = crate::Workspace::default();
        app.apply_settings(crate::Settings {
            default_pin_bits: input,
            ..app.preferences()
        });
        app.editor.set_tool(Tool::Place(Kind::Pin));
        app.editor.place(Point::new(20, 20));
        assert_eq!(app.circuit().components.last().unwrap().bits, expected);
        assert_eq!(app.preferences().default_pin_bits, expected);
    }
}

#[test]
fn fresh_placements_apply_label_size_and_respect_each_gate_family_limit() {
    use logisim_core::Parity;
    let mut project = Project::default();
    let child = project.add("child".into()).unwrap();
    for (font, inputs) in [(None, 2), (Some(24), 4), (Some(32), 32)] {
        let mut e = Editor::new(
            project.clone(),
            crate::Settings {
                default_font_size: font,
                default_gate_inputs: inputs,
                ..Default::default()
            },
        );
        let kinds = Gate::ALL
            .map(Kind::Gate)
            .into_iter()
            .chain(Parity::ALL.map(Kind::Parity))
            .chain([
                Kind::Joystick,
                Kind::Text,
                Kind::PlaRom,
                Kind::Pin,
                Kind::Subcircuit(child),
            ]);
        for (i, kind) in kinds.enumerate() {
            e.set_tool(Tool::Place(kind));
            e.place(Point::new(50 + i as i32 * 100, 50));
            let c = e.session.circuit.components.last().unwrap();
            let expected = match kind {
                Kind::Joystick => 12,
                Kind::PlaRom => font.unwrap_or(12),
                _ => font.unwrap_or(14),
            };
            assert_eq!(c.font.size, expected, "{kind:?}");
            match kind {
                Kind::Gate(g) if g.unary() => {
                    assert_eq!(
                        c.gate_options.inputs,
                        Component::new(kind, Point::ZERO).gate_options.inputs
                    );
                    assert_eq!(c.input_count(), 1);
                }
                Kind::Gate(_) => assert_eq!(c.gate_options.inputs, inputs.min(Gate::MAX_INPUTS)),
                Kind::Parity(_) => assert_eq!(c.parity.inputs, inputs),
                _ => {}
            }
        }
    }
}

#[test]
fn changing_placement_defaults_preserves_manual_templates_clipboard_and_workspace() {
    let mut app = crate::Workspace::default();
    let initial = crate::Settings {
        remember_last_properties: true,
        ..app.preferences()
    };
    app.apply_settings(initial);
    let and = Tool::Place(Kind::Gate(Gate::And));
    app.editor.set_tool(and);
    app.editor.template.font.family = "Custom label face".into();
    app.editor.template.font.size = 19;
    app.editor.template.gate_options.inputs = 6;
    let manual = app.editor.template.clone();
    app.editor.place(Point::new(100, 100));
    app.editor.copy();
    let original = app.circuit().clone();
    let selection = app.editor.session.selection.clone();
    let camera = app.canvas.camera;
    app.editor.simulation.view.steps = 42;
    app.apply_settings(crate::Settings {
        default_font_size: Some(24),
        default_font_family: crate::FontPreset::Monospace,
        default_font_bold: true,
        default_font_italic: true,
        default_gate_inputs: 4,
        ..initial
    });
    assert_eq!(app.editor.template, manual);
    assert_eq!(app.circuit(), &original);
    assert_eq!(app.editor.session.selection, selection);
    assert_eq!(
        (app.canvas.camera.zoom, app.canvas.camera.offset),
        (camera.zoom, camera.offset)
    );
    assert_eq!(app.editor.simulation.view.steps, 42);
    assert!(app.take_simulation_actions().is_empty());
    app.editor.set_tool(Tool::Place(Kind::Gate(Gate::Or)));
    assert_eq!(
        (
            app.editor.template.font.size,
            app.editor.template.gate_options.inputs
        ),
        (24, 4)
    );
    app.apply_settings(initial);
    app.editor.set_tool(and);
    assert_eq!(app.editor.template, manual);
    app.apply_settings(crate::Settings::default());
    assert_eq!(app.editor.template, manual);
    app.editor.paste();
    let pasted = app.circuit().components.last().unwrap();
    assert_eq!(pasted.font, manual.font);
    assert_eq!(pasted.gate_options.inputs, 6);
    app.editor.undo();
    assert_eq!(app.circuit(), &original);
    app.editor.undo();
    assert!(app.circuit().components.is_empty() && !app.editor.can_undo());
}

#[test]
fn new_gate_parity_and_tunnel_templates_use_current_defaults_only_on_creation() {
    use crate::{Settings, Workspace};
    use logisim_core::{GateSize, Parity};
    let mut app = Workspace::default();
    app.apply_settings(Settings {
        default_gate_size: GateSize::Wide,
        default_tunnel_width: 8,
        ..app.preferences()
    });
    // AND also happens to be the editor's initial, unused fallback.
    app.editor.set_tool(Tool::Place(Kind::Gate(Gate::And)));
    assert_eq!(app.editor.template.gate_options.size, GateSize::Wide);
    for parity in Parity::ALL {
        app.editor.set_tool(Tool::Place(Kind::Parity(parity)));
        assert_eq!(app.editor.template.parity.size, GateSize::Wide);
        assert_eq!(app.editor.template.gate_width(), 70);
    }
    app.editor.set_tool(Tool::Place(Kind::Tunnel));
    assert_eq!(app.editor.template.bits, 8);
    app.editor.template.label = "local".into();
    app.editor.place(Point::new(60, 60));
    app.editor.copy();
    let original = app.project().clone();
    let selection = app.editor.session.selection.clone();
    let camera = app.canvas.camera;
    app.editor.simulation.view.steps = 37;
    for width in [0, 32, 255] {
        app.apply_settings(Settings {
            default_gate_size: GateSize::Medium,
            default_tunnel_width: width,
            ..app.preferences()
        });
        assert_eq!(app.preferences().default_tunnel_width, width.clamp(1, 32));
        assert_eq!(app.editor.template.bits, 8);
        assert_eq!(app.project(), &original);
        assert_eq!(app.editor.session.selection, selection);
        assert_eq!(app.canvas.camera.offset, camera.offset);
        assert_eq!(app.canvas.camera.zoom, camera.zoom);
        assert_eq!(app.editor.simulation.view.steps, 37);
        assert!(app.take_simulation_actions().is_empty());
    }
    app.editor.set_tool(Tool::Place(Kind::Gate(Gate::And)));
    assert_eq!(app.editor.template.gate_options.size, GateSize::Medium);
    app.editor.set_tool(Tool::Place(Kind::Tunnel));
    assert_eq!(app.editor.template.bits, 32);
    assert_eq!(app.editor.template.label, "tunnel");
    app.editor.paste();
    assert_eq!(app.circuit().components.last().unwrap().bits, 8);
    assert_eq!(app.circuit().components.last().unwrap().label, "local");
}

#[test]
fn remembered_templates_follow_builtin_kinds_and_survive_document_changes_only_in_session() {
    use crate::{Settings, Workspace};
    use logisim_core::{GateSize, Parity};
    let mut app = Workspace::default();
    let settings = Settings {
        remember_last_properties: true,
        default_gate_size: GateSize::Medium,
        ..app.preferences()
    };
    app.apply_settings(settings);
    assert!(!app.document.dirty && !app.editor.can_undo());
    let pin = Tool::Place(Kind::Pin);
    let parity = Tool::Place(Kind::Parity(Parity::Odd));
    let a = Tool::Place(Kind::Gate(Gate::And));
    let b = Tool::Place(Kind::Gate(Gate::Or));
    app.editor.set_tool(pin);
    app.editor.template.bits = 12;
    app.editor.template.label = "address".into();
    let manual = app.editor.template.clone();
    app.editor.place(Point::new(50, 50));
    app.editor.set_tool(a);
    app.editor.template.gate_options.size = GateSize::Wide;
    app.editor.set_tool(b);
    assert_eq!(app.editor.template.gate_options.size, GateSize::Medium);
    app.editor.set_tool(parity);
    app.editor.template.parity.size = GateSize::Narrow;
    app.editor.set_tool(a);
    assert_eq!(app.editor.template.gate_options.size, GateSize::Wide);
    app.editor.set_tool(pin);
    assert_eq!(app.editor.template, manual);
    let stored = app.project().clone();
    app.replace_project(Project::default());
    for tool in [parity, pin] {
        app.editor.set_tool(tool);
        if tool == pin {
            assert_eq!(app.editor.template, manual);
        } else {
            assert_eq!(app.editor.template.parity.size, GateSize::Narrow);
        }
    }
    let before = app.editor.template.clone();
    app.apply_settings(Settings {
        remember_last_properties: false,
        ..settings
    });
    assert_eq!(app.editor.template, before);
    app.editor.set_tool(pin);
    assert_eq!(app.editor.template, before);
    app.editor.set_tool(a);
    assert_eq!(app.editor.template.gate_options.size, GateSize::Medium);
    app.editor.set_tool(pin);
    assert_eq!(app.editor.template.bits, 1);
    app.apply_settings(settings);
    app.editor.set_tool(a);
    assert_eq!(app.editor.template.gate_options.size, GateSize::Medium);
    assert_eq!(stored.main_circuit().circuit.components[0].bits, 12);
    assert!(!app.editor.can_undo());
    let mut restarted = Editor::new(stored, settings);
    restarted.set_tool(pin);
    assert_eq!(restarted.template.bits, 1);
}

#[test]
fn project_and_plugin_templates_stay_out_of_builtin_memory() {
    let mut project = Project::default();
    let child = project.add("child".into()).unwrap();
    let mut e = Editor::new(
        project,
        crate::Settings {
            remember_last_properties: true,
            ..Default::default()
        },
    );
    for kind in [Kind::Plugin, Kind::Subcircuit(child)] {
        e.set_tool(Tool::Place(kind));
        e.template.label = "transient".into();
        e.set_tool(Tool::Place(Kind::Pin));
        assert!(
            !e.remembered_templates
                .as_ref()
                .unwrap()
                .iter()
                .any(|c| c.kind == kind)
        );
        e.set_tool(Tool::Place(kind));
        assert_ne!(e.template.label, "transient");
        if let Kind::Subcircuit(id) = kind {
            assert_eq!(
                e.template,
                e.session.project.instance(id, Point::ZERO).unwrap()
            );
        }
    }
}

#[test]
fn a_manually_wide_not_survives_defaults_tool_switches_and_paste() {
    let settings = crate::Settings {
        remember_last_properties: true,
        ..Default::default()
    };
    let mut app = crate::Workspace::default();
    app.apply_settings(settings);
    let not = Tool::Place(Kind::Gate(Gate::Not));
    app.editor.set_tool(not);
    app.editor.template.gate_options.size = GateSize::Wide;
    app.editor.place(Point::new(100, 100));
    app.editor.copy();
    app.apply_settings(crate::Settings {
        default_gate_size: GateSize::Medium,
        ..settings
    });
    assert_eq!(app.editor.template.gate_options.size, GateSize::Wide);
    app.editor.set_tool(Tool::Place(Kind::Gate(Gate::Buffer)));
    app.editor.set_tool(not);
    assert_eq!(app.editor.template.gate_options.size, GateSize::Wide);
    app.editor.paste();
    assert_eq!(app.circuit().components.len(), 2);
    assert!(
        app.circuit()
            .components
            .iter()
            .all(|c| c.kind == Kind::Gate(Gate::Not) && c.gate_options.size == GateSize::Wide)
    );
}

#[test]
fn document_replacement_uses_incoming_template_memory_setting() {
    use crate::Settings;
    for old_enabled in [false, true] {
        for new_enabled in [false, true] {
            let settings = Settings {
                remember_last_properties: old_enabled,
                ..Default::default()
            };
            let incoming = Settings {
                remember_last_properties: new_enabled,
                ..settings
            };
            let mut editor = Editor::new(Project::default(), settings);
            let mut project = Project::default();
            let id = project
                .main_circuit_mut()
                .circuit
                .add_component(Component::new(Kind::Pin, Point::new(100, 80)));
            editor.replace_document(project.clone(), incoming);
            assert_eq!(
                editor.remembered_templates.as_deref(),
                new_enabled.then_some([].as_slice()),
                "{old_enabled} -> {new_enabled}"
            );
            assert_eq!(editor.session.project, project);
            assert!(editor.session.selection.is_empty() && !editor.can_undo());
            assert_eq!(editor.tool, Tool::Select);
            assert!(editor.simulation.actions.is_empty());
            editor.set_tool(Tool::Place(Kind::Pin));
            editor.template.bits = 9;
            editor.set_tool(Tool::Place(Kind::Tunnel));
            editor.set_tool(Tool::Place(Kind::Pin));
            assert_eq!(editor.template.bits, if new_enabled { 9 } else { 1 });
            assert_eq!(editor.session.circuit.component(id).unwrap().bits, 1);

            let mut editor = Editor::new(Project::default(), settings);
            editor.set_tool(Tool::Place(Kind::Pin));
            editor.template.bits = 6;
            editor.set_tool(Tool::Place(Kind::Tunnel));
            editor.template.label = "current".into();
            editor.replace_document(project.clone(), incoming);
            editor.set_tool(Tool::Place(Kind::Tunnel));
            assert_eq!(
                editor.template.label,
                if new_enabled { "current" } else { "tunnel" }
            );
            editor.set_tool(Tool::Place(Kind::Pin));
            assert_eq!(
                editor.template.bits,
                if old_enabled && new_enabled { 6 } else { 1 }
            );
            assert_eq!(editor.session.project, project);
        }
    }
}

#[test]
fn supported_size_defaults_are_used_for_each_new_gate_template() {
    use logisim_core::{
        GateSize::{Medium, Narrow, Wide},
        Parity,
    };
    for (default, not_size) in [(Narrow, Narrow), (Medium, Narrow), (Wide, Wide)] {
        let mut editor = Editor::new(
            Project::default(),
            crate::Settings {
                default_gate_size: default,
                ..Default::default()
            },
        );
        for gate in Gate::ALL {
            editor.set_tool(Tool::Place(Kind::Gate(gate)));
            let expected = match gate {
                Gate::Buffer => Narrow,
                Gate::Not => not_size,
                _ => default,
            };
            assert_eq!(
                editor.template.gate_options.size, expected,
                "{gate:?} / {default:?}"
            );
        }
        for parity in Parity::ALL {
            editor.set_tool(Tool::Place(Kind::Parity(parity)));
            assert_eq!(editor.template.parity.size, default);
        }
    }
}

use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};
use logisim_core::{Component, Kind, Point};

fn frame(ctx: &Context, app: &mut Workspace, events: Vec<Event>) -> Vec<(String, Rect)> {
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 820.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    fn collect(shape: Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            Shape::Text(t) => out.push((
                t.galley.job.text.clone(),
                Rect::from_min_size(t.pos, t.galley.size()),
            )),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, out);
                }
            }
            _ => {}
        }
    }
    output.textures_delta.clear();
    let mut out = vec![];
    for shape in output.shapes {
        collect(shape.shape, &mut out);
    }
    out
}
fn click(ctx: &Context, app: &mut Workspace, text: &str) {
    let mut labels = frame(ctx, app, vec![]);
    for _ in 0..2 {
        labels = frame(ctx, app, vec![]);
    }
    let pos = labels
        .iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("Missing {text}: {labels:?}"))
        .1
        .center();
    for pressed in [true, false] {
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
        );
    }
}
fn has(labels: &[(String, Rect)], expected: &str) -> bool {
    labels.iter().any(|(text, _)| text == expected)
}

#[test]
fn generic_plugin_palette_properties_actions_and_strings_in_both_languages() {
    use logisim_core::plugin::api;
    let tool = api::Tool {
        id: "first".into(),
        name: "Plugin tool A".into(),
        bounds: [[-20, -20], [20, 20]],
        ports: vec![api::Port {
            id: "out".into(),
            label: "Output".into(),
            at: [20, 0],
            width: 4,
            output: true,
        }],
        graphics: vec![api::Shape::Text {
            at: [-20, -20],
            text: "Pin".into(),
            size: 10,
            color: [0, 0, 0, 255],
        }],
        properties: vec![api::Property {
            id: "mode".into(),
            label: "Address".into(),
            default: "first".into(),
            kind: api::PropertyKind::Choice {
                values: vec!["first".into(), "second".into()],
            },
        }],
        actions: vec![api::Action {
            id: "go".into(),
            label: "User action".into(),
            argument: None,
        }],
    };
    let manifest = api::Manifest {
        abi: 1,
        id: "test.graphics".into(),
        version: "1".into(),
        name: "User library".into(),
        tools: vec![
            tool.clone(),
            api::Tool {
                id: "second".into(),
                name: "Plugin tool B".into(),
                ..tool
            },
        ],
    };
    manifest.validate().unwrap();
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        app.set_plugin_catalogue(vec![manifest.clone()]);
        click(&ctx, &mut app, "Plugin tool A");
        assert_eq!(
            app.editor.template.plugin.as_ref().unwrap().tool.id,
            "first"
        );
        click(&ctx, &mut app, "Plugin tool B");
        assert_eq!(
            app.editor.template.plugin.as_ref().unwrap().tool.id,
            "second"
        );
        let pos = app.canvas.screen(Point::new(200, 200));
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        app.editor.set_tool(Tool::Select);
        let c = app.editor.circuit.components.last().unwrap().clone();
        assert_eq!(c.plugin.as_ref().unwrap().tool.id, "second");
        assert_eq!(c.ports(), vec![c.position + Point::new(20, 0)]);
        app.editor.selection = [c.id].into();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, "Address")); // Plugin-owned text is never translated.
        assert!(has(&labels, "Pin"));
        click(&ctx, &mut app, "first");
        click(&ctx, &mut app, "second");
        app.finish_edit();
        assert_eq!(
            app.editor
                .circuit
                .component(c.id)
                .unwrap()
                .plugin
                .as_ref()
                .unwrap()
                .properties["mode"],
            "second"
        );
        app.editor.undo();
        assert_eq!(
            app.editor
                .circuit
                .component(c.id)
                .unwrap()
                .plugin
                .as_ref()
                .unwrap()
                .properties["mode"],
            "first"
        );
        app.editor.selection = [c.id].into();
        click(&ctx, &mut app, "User action");
        assert!(app.editor.simulation.actions.iter().any(|a| matches!(a, SimulationAction::PluginAction { id, action, argument } if *id==c.id && action=="go" && argument.is_empty())));
        app.editor.simulation.view.plugin_error_count = 1;
        app.editor
            .simulation
            .view
            .plugin_errors
            .insert(c.id, "Missing test plugin".into());
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, "Missing test plugin"));
        assert!(
            i18n::with_language(language, || app.editor.simulation.status()).contains(
                if language == Language::English {
                    "plugin error"
                } else {
                    "插件错误"
                }
            )
        );
        let before = app.project().clone();
        let sources = vec![logisim_core::plugin::PluginSource {
            plugin: manifest.id.clone(),
            version: manifest.version.clone(),
            path: "/plugins/repaired.wasm".into(),
            fallback: String::new(),
        }];
        app.set_plugin_sources(sources.clone());
        assert_eq!(app.project().plugin_sources, sources);
        assert_eq!(app.project().circuits, before.circuits);
        app.editor.undo();
        assert_eq!(app.project(), &before);
        app.editor.redo();
        assert_eq!(app.project().plugin_sources, sources);
    }
}

#[test]
fn joystick_library_properties_and_rotation_undo_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Input/Output"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(150.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -200.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                    source: egui::MouseWheelSource::Unknown,
                },
            ],
        );
        click(&ctx, &mut app, &t("Joystick"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Joystick));
        let labels = frame(&ctx, &mut app, vec![]);
        for s in ["Data bits", "Facing", "Stick color", "Label color"] {
            assert!(has(&labels, &t(s)), "{s}");
        }
        for s in ["Label", "Label location", "Font size", "Off color"] {
            assert!(!has(&labels, &t(s)), "{s}");
        }
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        app.editor
            .circuit
            .add_wire(Point::new(300, 210), Point::new(400, 210));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("East"));
        click(&ctx, &mut app, &t("North"));
        assert_eq!(
            app.circuit().component(id).unwrap().facing,
            logisim_core::Facing::North
        );
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(id).unwrap().facing,
            logisim_core::Facing::North
        );
    }
}

#[test]
fn latch_library_modes_layout_controls_and_undo_work_in_both_languages() {
    use logisim_core::{ControlPosition, Latch, LatchEnable};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Memory"));
        click(&ctx, &mut app, &t("D Latch"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Latch(Latch::D)));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("Facing")));
        assert!(!has(&labels, &t("Data bits")));
        click(&ctx, &mut app, &t("High level"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("Without enable")));
        assert!(!has(&labels, &t("Rising edge")));
        click(&ctx, &mut app, &t("Low level"));
        assert_eq!(app.editor.template.latch_enable, LatchEnable::Low);
        click(&ctx, &mut app, &t("S-R Latch"));
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        app.editor
            .circuit
            .add_wire(Point::new(100, 200), Point::new(240, 200));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("High level"));
        click(&ctx, &mut app, &t("Without enable"));
        assert!(!app.circuit().component(id).unwrap().latch_has_enable());
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert!(!app.circuit().component(id).unwrap().latch_has_enable());
        for (label, expected) in [
            ("New layout", false),
            ("New layout", true),
            ("Active-low preset / clear", true),
        ] {
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![]);
            }
            let labels = frame(&ctx, &mut app, vec![]);
            let row = labels.iter().find(|(s, _)| s == &t(label)).unwrap().1;
            let checkbox = labels
                .iter()
                .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
                .unwrap()
                .1;
            let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            let c = app.circuit().component(id).unwrap();
            if label == "New layout" {
                assert_eq!(c.sequential.new_layout, expected);
                assert_eq!(
                    c.sequential.controls,
                    if expected {
                        ControlPosition::BelowAbove
                    } else {
                        ControlPosition::Legacy
                    }
                );
                if !expected {
                    // The old layout still allows all three control positions.
                    click(&ctx, &mut app, &t("Legacy"));
                    click(&ctx, &mut app, &t("Below / above"));
                    assert_eq!(
                        app.circuit().component(id).unwrap().sequential.controls,
                        ControlPosition::BelowAbove
                    );
                }
            } else {
                assert_eq!(c.sequential.negate_controls, expected);
            }
            assert_eq!(app.circuit().wires, before.wires);
        }
    }
}
#[test]
fn settings_click_switches_languages_without_touching_document_history_or_simulation() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    app.editor.set_tool(Tool::Place(Kind::Pin));
    app.editor.template.label = "Text".into();
    app.editor.place(Point::new(100, 100));
    app.editor.set_tool(Tool::Select);
    app.editor.simulation.view.enabled = false;
    app.editor.simulation.view.pending = 2;
    app.editor.simulation.view.steps = 7;
    let document = app.project().clone();
    let selection = app.editor.selection.clone();
    assert!(app.editor.can_undo());
    click(&ctx, &mut app, "File");
    click(&ctx, &mut app, "Settings");
    assert!(app.settings_open);
    click(&ctx, &mut app, "English");
    click(&ctx, &mut app, "简体中文");
    assert_eq!(app.language(), Language::SimplifiedChinese);
    let labels = frame(&ctx, &mut app, vec![]);
    assert!(has(&labels, "设置"));
    assert!(has(&labels, "文件"));
    assert!(has(&labels, "已暂停 · 2 项待传播"));
    assert_eq!(app.project(), &document);
    assert_eq!(app.editor.selection, selection);
    assert_eq!(app.editor.simulation.view.steps, 7);
    assert!(app.editor.can_undo());
    assert!(!app.editor.can_redo());
    assert!(app.take_file_actions().is_empty());
    assert!(app.take_simulation_actions().is_empty());
    click(&ctx, &mut app, "简体中文");
    click(&ctx, &mut app, "English");
    assert_eq!(app.language(), Language::English);
    assert_eq!(app.project(), &document);
    app.settings_open = false;
    // The original edit is still the next undo operation.
    app.editor.undo();
    assert!(app.circuit().components.is_empty());
}
#[test]
fn translated_properties_and_appearance_preserve_user_names_and_shortcuts() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let mut circuit = logisim_core::Circuit::default();
        let mut text = Component::new(Kind::Text, Point::new(100, 100));
        text.label = "Save · 中文标签 {}".into();
        let id = circuit.add_component(text);
        app.replace_project(logisim_core::Project::single("File", circuit));
        app.set_language(language);
        app.editor.selection.insert(id);
        let project = app.project().clone();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, "Save · 中文标签 {}"));
        let expected = i18n::with_language(language, || i18n::tr("Font size").to_owned());
        assert!(has(&labels, &expected));
        frame(
            &ctx,
            &mut app,
            vec![Event::Key {
                key: Key::E,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT),
            }],
        );
        assert!(app.editor.appearance_mode);
        let labels = frame(&ctx, &mut app, vec![]);
        let expected = i18n::with_language(language, || tf!("Appearance: {}", "File"));
        assert!(has(&labels, &expected));
        assert_eq!(app.project(), &project);
        let document = DocumentView {
            title: "Save".into(),
            untitled: false,
            ..Default::default()
        };
        assert_eq!(
            i18n::with_language(language, || document.display_title().to_owned()),
            "Save"
        );
    }
}
#[test]
fn settings_retry_is_explicit_and_blocks_editor_shortcuts() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace {
        settings_open: true,
        ..Default::default()
    };
    app.set_language(Language::SimplifiedChinese);
    app.set_settings_error(Some("User configuration directory is unavailable.".into()));
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let labels = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: Key::N,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
    );
    assert!(app.take_file_actions().is_empty());
    assert!(has(&labels, "无法确定用户配置目录。"), "{labels:?}");
    assert!(!app.take_settings_retry());
    click(&ctx, &mut app, "重试");
    assert!(app.take_settings_retry());
    assert!(!app.take_settings_retry());
}

#[test]
fn sequential_library_properties_and_controls_work_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        click(&ctx, &mut app, &translated("Memory"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -150.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("J-K Flip-Flop"));
        assert_eq!(
            app.editor.tool,
            Tool::Place(Kind::FlipFlop(logisim_core::FlipFlop::Jk))
        );
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Trigger")));
        click(&ctx, &mut app, &translated("Rising edge"));
        click(&ctx, &mut app, &translated("Falling edge"));
        assert_eq!(
            app.editor.template.sequential.trigger,
            logisim_core::Trigger::Falling
        );
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -200.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("Register"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Chip select active on")));
        app.editor.simulation.view.has_clocks = true;
        click(&ctx, &mut app, &translated("Tick"));
        click(&ctx, &mut app, &translated("Auto ticks"));
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::Tick, SimulationAction::ToggleTicks]
        );
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn plexers_library_and_output_properties_work_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        click(&ctx, &mut app, &translated("Plexers"));
        for kind in logisim_core::Plexer::ALL {
            if kind == logisim_core::Plexer::DisplayDecoder {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(130.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -100.0),
                            modifiers: Modifiers::NONE,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                        },
                    ],
                );
                for _ in 0..20 {
                    frame(&ctx, &mut app, vec![]);
                }
            }
            click(&ctx, &mut app, &translated(kind.name()));
            assert_eq!(app.editor.tool, Tool::Place(Kind::Plexer(kind)));
            let labels = frame(&ctx, &mut app, vec![]);
            assert_eq!(
                has(&labels, &translated("Select bits")),
                kind != logisim_core::Plexer::DisplayDecoder
            );
            assert_eq!(
                has(&labels, &translated("Bus input")),
                kind == logisim_core::Plexer::DisplayDecoder
            );
            assert_eq!(has(&labels, &translated("Data bits")), kind.has_data_bits());
            assert_eq!(
                has(&labels, &translated("No input state")),
                kind == logisim_core::Plexer::PriorityEncoder
            );
        }
        click(&ctx, &mut app, &translated("Multiplexer"));
        click(&ctx, &mut app, &translated("Floating"));
        click(&ctx, &mut app, &translated("Zero output"));
        assert!(app.editor.template.plexing.disabled_zero);
        click(&ctx, &mut app, &translated("Bottom / left"));
        click(&ctx, &mut app, &translated("Top / right"));
        assert!(app.editor.template.plexing.top_right);
        app.editor.place(Point::new(300, 300));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        click(&ctx, &mut app, &translated("Zero output"));
        click(&ctx, &mut app, &translated("Floating"));
        assert!(!app.circuit().component(id).unwrap().plexing.disabled_zero);
        app.editor.undo();
        assert!(app.circuit().component(id).unwrap().plexing.disabled_zero);
        app.editor.redo();
        assert!(!app.circuit().component(id).unwrap().plexing.disabled_zero);
    }
}

#[test]
fn arithmetic_library_and_modes_edit_in_both_languages_with_undo() {
    use logisim_core::{Arithmetic, Shift};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        click(&ctx, &mut app, &translated("Arithmetic"));
        for kind in Arithmetic::ALL {
            if kind == Arithmetic::Negator {
                // The expanded library exceeds the project pane: use its real scroll path.
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(130.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -170.0),
                            modifiers: Modifiers::NONE,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                        },
                    ],
                );
                for _ in 0..20 {
                    frame(&ctx, &mut app, vec![]);
                }
            }
            click(&ctx, &mut app, &translated(kind.name()));
            assert_eq!(app.editor.tool, Tool::Place(Kind::Arithmetic(kind)));
            let labels = frame(&ctx, &mut app, vec![]);
            assert!(has(&labels, &translated("Data bits")));
            assert!(!has(&labels, &translated("Facing")));
            assert!(!has(&labels, &translated("Label")));
            assert_eq!(
                has(&labels, &translated("Numeric type")),
                kind == Arithmetic::Comparator
            );
            assert_eq!(
                has(&labels, &translated("Shift type")),
                kind == Arithmetic::Shifter
            );
            assert_eq!(
                has(&labels, &translated("Search type")),
                kind == Arithmetic::BitFinder
            );
            assert_eq!(
                has(&labels, &translated("Inputs")),
                kind == Arithmetic::BitAdder
            );
        }
        click(&ctx, &mut app, &translated("Shifter"));
        for shift in Shift::ALL.into_iter().skip(1) {
            let previous = app.editor.template.arithmetic.shift;
            click(&ctx, &mut app, &translated(previous.name()));
            click(&ctx, &mut app, &translated(shift.name()));
            assert_eq!(app.editor.template.arithmetic.shift, shift);
        }
        click(&ctx, &mut app, &translated("Comparator"));
        click(&ctx, &mut app, &translated("Two's complement"));
        click(&ctx, &mut app, &translated("Unsigned"));
        assert!(!app.editor.template.arithmetic.signed);
        app.editor.place(Point::new(300, 300));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        click(&ctx, &mut app, &translated("Unsigned"));
        click(&ctx, &mut app, &translated("Two's complement"));
        assert!(app.circuit().component(id).unwrap().arithmetic.signed);
        app.editor.undo();
        assert!(!app.circuit().component(id).unwrap().arithmetic.signed);
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().arithmetic.signed);
    }
}

#[test]
fn bit_search_and_input_count_edits_use_translated_widgets_and_undo() {
    use logisim_core::{Arithmetic, BitSearch};
    fn number(ctx: &Context, app: &mut Workspace, old: &str, new: &str) {
        click(ctx, app, old);
        frame(
            ctx,
            app,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text(new.into()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        app.editor
            .set_tool(Tool::Place(Kind::Arithmetic(Arithmetic::BitFinder)));
        for mode in BitSearch::ALL.into_iter().skip(1) {
            let old = app.editor.template.arithmetic.search;
            click(&ctx, &mut app, &translated(old.name()));
            click(&ctx, &mut app, &translated(mode.name()));
            assert_eq!(app.editor.template.arithmetic.search, mode);
        }
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        click(&ctx, &mut app, &translated(BitSearch::HighestZero.name()));
        click(&ctx, &mut app, &translated(BitSearch::LowestOne.name()));
        assert_eq!(
            app.circuit().component(id).unwrap().arithmetic.search,
            BitSearch::LowestOne
        );
        app.editor.undo();
        assert_eq!(
            app.circuit().component(id).unwrap().arithmetic.search,
            BitSearch::HighestZero
        );
        app.editor.redo();
        assert_eq!(
            app.circuit().component(id).unwrap().arithmetic.search,
            BitSearch::LowestOne
        );

        app.editor
            .set_tool(Tool::Place(Kind::Arithmetic(Arithmetic::BitAdder)));
        number(&ctx, &mut app, "1", "4");
        assert_eq!(app.editor.template.arithmetic.bit_inputs, 4);
        assert_eq!(app.editor.template.arithmetic_output_bits(), 6);
        app.editor.place(Point::new(300, 400));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components.last().unwrap().clone();
        app.editor.selection.insert(c.id);
        for port in [c.ports()[1], c.ports()[4]] {
            app.editor.circuit.add_wire(port - Point::new(80, 0), port);
        }
        let before = app.circuit().clone();
        number(&ctx, &mut app, "4", "2");
        assert_eq!(
            app.circuit().component(c.id).unwrap().arithmetic.bit_inputs,
            2
        );
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(c.id).unwrap().arithmetic.bit_inputs,
            2
        );
    }
}

#[test]
fn extra_plexer_property_widgets_edit_groups_and_formats_with_undo_in_both_languages() {
    use logisim_core::Plexer;
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor
            .set_tool(Tool::Place(Kind::Plexer(Plexer::BitSelector)));
        click(&ctx, &mut app, "1");
        frame(
            &ctx,
            &mut app,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text("3".into()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            &ctx,
            &mut app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.editor.template.plexing.group_bits, 3);
        assert_eq!(app.editor.template.bit_selector_select_bits(), 2);
        click(&ctx, &mut app, &translated("Bottom / left"));
        click(&ctx, &mut app, &translated("Top / right"));
        app.editor.place(Point::new(300, 200));
        assert!(app.circuit().components[0].plexing.top_right);

        app.editor
            .set_tool(Tool::Place(Kind::Plexer(Plexer::DisplayDecoder)));
        app.editor.place(Point::new(300, 400));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components.last().unwrap().clone();
        app.editor.selection.insert(c.id);
        app.editor
            .circuit
            .add_wire(Point::new(160, 400), Point::new(260, 400));
        let before = app.circuit().clone();
        let mut labels = vec![];
        for _ in 0..3 {
            labels = frame(&ctx, &mut app, vec![]);
        }
        assert!(!has(&labels, &translated("Data bits")));
        let x = labels
            .iter()
            .find(|(t, _)| *t == translated("East"))
            .unwrap()
            .1
            .min
            .x
            + 2.0;
        let y = labels
            .iter()
            .find(|(t, _)| *t == translated("Bus input"))
            .unwrap()
            .1
            .center()
            .y;
        for pressed in [true, false] {
            let pos = Pos2::new(x, y);
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(!app.circuit().component(c.id).unwrap().plexing.multibit);
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert!(!app.circuit().component(c.id).unwrap().plexing.multibit);
    }
}

#[test]
fn counter_shift_widgets_edit_modes_width_and_undo_in_both_languages() {
    fn number(ctx: &Context, app: &mut Workspace, old: &str, new: &str) {
        click(ctx, app, old);
        frame(
            ctx,
            app,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text(new.into()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor.set_tool(Tool::Place(Kind::Counter));
        app.editor.template.sequential.counter_max = 5;
        number(&ctx, &mut app, "8", "4");
        assert_eq!(app.editor.template.bits, 4);
        assert_eq!(app.editor.template.sequential.counter_max, 15);
        for action in logisim_core::CounterOverflow::ALL.into_iter().skip(1) {
            let previous = t(app.editor.template.sequential.counter_overflow.name());
            click(&ctx, &mut app, &previous);
            click(&ctx, &mut app, &t(action.name()));
            assert_eq!(app.editor.template.sequential.counter_overflow, action);
        }
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        app.editor
            .circuit
            .add_wire(Point::new(100, 210), Point::new(270, 210));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("New behavior"));
        click(&ctx, &mut app, &t("Old behavior"));
        assert!(app.circuit().component(id).unwrap().sequential.counter_old);
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().sequential.counter_old);
        click(&ctx, &mut app, &t("Rising edge"));
        click(&ctx, &mut app, &t("Falling edge"));
        assert_eq!(
            app.circuit().component(id).unwrap().sequential.trigger,
            logisim_core::Trigger::Falling
        );
        app.editor.set_tool(Tool::Place(Kind::ShiftRegister));
        number(&ctx, &mut app, "8", "32");
        assert_eq!(app.editor.template.bits, 32);
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &t("Number of stages")));
        assert!(!has(&labels, &t("Data bits")));
        assert!(!has(&labels, &t("Facing")));
        app.editor.place(Point::new(500, 200));
        assert_eq!(app.circuit().components.last().unwrap().bits, 32);
    }
}

#[test]
fn memory_editor_real_input_keeps_rom_history_and_ram_instance_commands_separate() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = logisim_core::Circuit::default();
        let rom = circuit.add_component(Component::new(Kind::Rom, Point::new(200, 100)));
        let ram = circuit.add_component(Component::new(Kind::Ram, Point::new(400, 100)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(rom);
        click(&ctx, &mut app, &translated("Edit contents…"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("ROM contents")));
        let rect = labels
            .iter()
            .find(|(s, _)| s == &translated("Value (hex)"))
            .unwrap()
            .1;
        let pos = Pos2::new(rect.right() + 50.0, rect.center().y);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        frame(
            &ctx,
            &mut app,
            vec![
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Text("A5".into()),
            ],
        );
        click(&ctx, &mut app, &translated("Write word"));
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0xa5
        );
        assert!(app.take_simulation_actions().is_empty());
        click(&ctx, &mut app, &translated("Close"));
        app.editor.undo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0
        );
        app.editor.redo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0xa5
        );
        let before = app.project().clone();
        app.editor.selection.clear();
        app.editor.selection.insert(ram);
        let mut contents = logisim_core::MemoryContents::default();
        contents.set(0, 0x5a);
        app.editor.simulation.view.memories.insert(ram, contents);
        click(&ctx, &mut app, &translated("Edit contents…"));
        assert!(has(
            &frame(&ctx, &mut app, vec![]),
            &translated("RAM contents")
        ));
        click(&ctx, &mut app, &translated("Clear contents"));
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::FillMemory {
                id: ram,
                start: 0,
                end: 256,
                value: 0
            }]
        );
        assert_eq!(app.project(), &before);
        click(&ctx, &mut app, &translated("Close"));
        app.editor.undo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0
        );
    }
}

#[test]
fn io_library_properties_and_momentary_inspector_work_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &translated("Input/Output"));
        for kind in [
            Kind::Button,
            Kind::Switch,
            Kind::DipSwitch,
            Kind::Slider,
            Kind::Led,
            Kind::RgbLed,
            Kind::SevenSegment,
            Kind::HexDisplay,
            Kind::DotMatrix,
        ] {
            if matches!(kind, Kind::DipSwitch | Kind::RgbLed | Kind::DotMatrix) {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(130.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -170.0),
                            modifiers: Modifiers::NONE,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                        },
                    ],
                );
                for _ in 0..20 {
                    frame(&ctx, &mut app, vec![]);
                }
            }
            click(&ctx, &mut app, &translated(kind.name()));
            assert_eq!(app.editor.tool, Tool::Place(kind));
            let labels = frame(&ctx, &mut app, vec![]);
            assert!(has(
                &labels,
                &translated(if kind == Kind::Button {
                    "Button color"
                } else if matches!(kind, Kind::Switch | Kind::DipSwitch | Kind::Slider) {
                    "Body color"
                } else if kind == Kind::RgbLed {
                    "8-bit color channels"
                } else {
                    "On color"
                })
            ));
            assert_eq!(
                has(&labels, &translated("Data bits")),
                matches!(kind, Kind::Switch | Kind::Slider)
            );
            if kind == Kind::DipSwitch {
                assert!(has(&labels, &translated("Number of switches")));
            }
            if kind == Kind::Slider {
                assert!(has(&labels, &translated("Value (hex)")));
                click(&ctx, &mut app, &translated("Left to right"));
                click(&ctx, &mut app, &translated("Right to left"));
                assert!(app.editor.template.io.slider_reverse);
                click(&ctx, &mut app, &translated("Binary"));
                click(&ctx, &mut app, &translated("Hexadecimal"));
                assert_eq!(app.editor.template.radix, logisim_core::Radix::Hex);
            }
            if matches!(kind, Kind::Led | Kind::SevenSegment) {
                let rect = labels
                    .iter()
                    .find(|(s, _)| s == &translated("Active on high"))
                    .unwrap()
                    .1;
                let checkbox = labels
                    .iter()
                    .find(|(s, r)| s.is_empty() && (r.center().y - rect.center().y).abs() < 1.0)
                    .unwrap()
                    .1;
                let pos = Pos2::new(checkbox.left() - 10.0, rect.center().y);
                for pressed in [true, false] {
                    frame(
                        &ctx,
                        &mut app,
                        vec![
                            Event::PointerMoved(pos),
                            Event::PointerButton {
                                pos,
                                button: PointerButton::Primary,
                                pressed,
                                modifiers: Modifiers::NONE,
                            },
                        ],
                    );
                }
                assert!(!app.editor.template.io.active_high);
            }
        }
        let mut circuit = logisim_core::Circuit::default();
        let id = circuit.add_component(Component::new(Kind::Button, Point::new(200, 200)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(id);
        let before = app.project().clone();
        let mut labels = frame(&ctx, &mut app, vec![]);
        for _ in 0..2 {
            labels = frame(&ctx, &mut app, vec![]);
        }
        let pos = labels
            .iter()
            .find(|(s, _)| s == &translated("Hold to press"))
            .unwrap()
            .1
            .center();
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::PressButton { id }]
        );
        let pos = egui::pos2(1100.0, 750.0);
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::ReleaseButtons]
        );
        assert_eq!(app.project(), &before);
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn display_properties_edit_real_widgets_with_localization_undo_and_stable_ids() {
    use logisim_core::MatrixInput;
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = logisim_core::Circuit::default();
        let matrix = circuit.add_component(Component::new(Kind::DotMatrix, Point::new(300, 300)));
        let rgb = circuit.add_component(Component::new(Kind::RgbLed, Point::new(100, 200)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(matrix);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        for text in [
            "Matrix columns",
            "Matrix rows",
            "Light persistence (ticks)",
            "Dot shape",
        ] {
            assert!(has(&labels, &translated(text)), "{text}");
        }
        assert!(!has(&labels, &translated("Data bits")));
        click(&ctx, &mut app, &translated("Columns"));
        click(&ctx, &mut app, &translated("Select rows/columns"));
        assert_eq!(
            app.editor
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_input,
            MatrixInput::Select
        );
        click(&ctx, &mut app, &translated("Square"));
        click(&ctx, &mut app, &translated("Circular"));
        assert!(
            app.editor
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_circular
        );
        app.editor.undo();
        assert!(
            !app.editor
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_circular
        );
        app.editor.redo();
        assert!(
            app.editor
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_circular
        );
        app.editor.selection.clear();
        app.editor.selection.insert(rgb);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Label location")));
        assert!(!has(&labels, &translated("On color")));
        let rect = labels
            .iter()
            .find(|(s, _)| s == &translated("8-bit color channels"))
            .unwrap()
            .1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - rect.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, rect.center().y);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.editor.circuit.component(rgb).unwrap().io.rgb_multibit);
        assert_eq!(
            app.editor.circuit.component(rgb).unwrap().port_width(0),
            Some(8)
        );
        app.editor.undo();
        assert_eq!(
            app.editor.circuit.component(rgb).unwrap().port_width(0),
            Some(1)
        );
    }
}

#[test]
fn bus_control_properties_use_translated_widgets_and_keep_wires_with_undo() {
    use logisim_core::{Circuit, GateSize, Pull};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = Circuit::default();
        let gate = circuit.add_component(Component::new(
            Kind::ControlledInverter,
            Point::new(300, 300),
        ));
        let resistor =
            circuit.add_component(Component::new(Kind::PullResistor, Point::new(500, 300)));
        let p = circuit.component(gate).unwrap().ports()[2];
        circuit.add_wire(p, p + Point::new(0, 80));
        app.replace_circuit(circuit);
        app.editor.selection.insert(gate);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let wires = app.circuit().wires.clone();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Control location")));
        assert!(has(&labels, &translated("Negate control input")));
        click(&ctx, &mut app, &translated("Right-handed"));
        click(&ctx, &mut app, &translated("Left-handed"));
        assert!(app.circuit().component(gate).unwrap().bus.control_left);
        assert_eq!(app.circuit().wires, wires);
        click(&ctx, &mut app, &translated("Narrow"));
        click(&ctx, &mut app, &translated("Wide"));
        assert_eq!(
            app.circuit().component(gate).unwrap().gate_size,
            GateSize::Wide
        );
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert_eq!(
            app.circuit().component(gate).unwrap().gate_size,
            GateSize::Narrow
        );
        app.editor.redo();
        let labels = frame(&ctx, &mut app, vec![]);
        let row = labels
            .iter()
            .find(|(s, _)| s == &translated("Negate control input"))
            .unwrap()
            .1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.circuit().component(gate).unwrap().bus.negate_control);
        app.editor.undo();
        assert!(!app.circuit().component(gate).unwrap().bus.negate_control);
        app.editor.selection.clear();
        app.editor.selection.insert(resistor);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Pull direction")));
        assert!(!has(&labels, &translated("Data bits")));
        assert!(!has(&labels, &translated("Label")));
        click(&ctx, &mut app, &translated("Zero"));
        click(&ctx, &mut app, &translated("One"));
        assert_eq!(
            app.circuit().component(resistor).unwrap().bus.pull,
            Pull::One
        );
        click(&ctx, &mut app, &translated("One"));
        click(&ctx, &mut app, &translated("Error"));
        assert_eq!(
            app.circuit().component(resistor).unwrap().bus.pull,
            Pull::Error
        );
        app.editor.undo();
        assert_eq!(
            app.circuit().component(resistor).unwrap().bus.pull,
            Pull::One
        );
        // Library entries are real placement tools in either language.
        click(&ctx, &mut app, &translated("Gates"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -200.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("Controlled Buffer"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::ControlledBuffer));
    }
}

#[test]
fn transistor_properties_and_library_tools_work_in_both_languages() {
    use logisim_core::{Circuit, TransistorType};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = Circuit::default();
        let transistor =
            circuit.add_component(Component::new(Kind::Transistor, Point::new(300, 300)));
        let transmission =
            circuit.add_component(Component::new(Kind::TransmissionGate, Point::new(500, 300)));
        let port = circuit.component(transistor).unwrap().ports()[2];
        circuit.add_wire(port, port + Point::new(0, -80));
        circuit.normalize_wires();
        app.replace_circuit(circuit);
        app.editor.selection.insert(transistor);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let wires = app.circuit().wires.clone();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Transistor type")));
        assert!(has(&labels, &translated("Gate location")));
        assert!(has(&labels, &translated("Data bits")));
        assert!(!has(&labels, &translated("Label")));
        click(&ctx, &mut app, &translated("P-Type"));
        click(&ctx, &mut app, &translated("N-Type"));
        assert_eq!(
            app.circuit()
                .component(transistor)
                .unwrap()
                .transistor
                .transistor_type,
            TransistorType::N
        );
        click(&ctx, &mut app, &translated("Top/Left"));
        click(&ctx, &mut app, &translated("Bottom/Right"));
        assert!(
            app.circuit()
                .component(transistor)
                .unwrap()
                .transistor
                .gate_bottom_right
        );
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert!(
            !app.circuit()
                .component(transistor)
                .unwrap()
                .transistor
                .gate_bottom_right
        );
        app.editor.redo();
        app.editor.selection.clear();
        app.editor.selection.insert(transmission);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &translated("Transistor type")));
        assert!(has(&labels, &translated("Gate location")));
        click(&ctx, &mut app, &translated("Top/Left"));
        click(&ctx, &mut app, &translated("Bottom/Right"));
        assert!(
            app.circuit()
                .component(transmission)
                .unwrap()
                .transistor
                .gate_bottom_right
        );
        assert_eq!(app.circuit().wires, wires);
        click(&ctx, &mut app, &translated("Wiring"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -180.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("Transistor"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Transistor));
        click(&ctx, &mut app, &translated("Transmission Gate"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::TransmissionGate));
    }
}

#[test]
fn parity_properties_and_library_tools_work_in_both_languages() {
    use logisim_core::{Circuit, GateSize, Parity, ParityOutput};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = Circuit::default();
        let gate = circuit.add_component(Component::new(
            Kind::Parity(Parity::Odd),
            Point::new(300, 300),
        ));
        let port = circuit.component(gate).unwrap().ports()[1];
        circuit.add_wire(port, port + Point::new(-100, 0));
        circuit.normalize_wires();
        app.replace_circuit(circuit);
        app.editor.selection.insert(gate);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let wires = app.circuit().wires.clone();
        let labels = frame(&ctx, &mut app, vec![]);
        for name in [
            "Inputs",
            "Gate size",
            "Output value",
            "Invert input 1",
            "Invert input 2",
        ] {
            let expected = if let Some(n) = name.strip_prefix("Invert input ") {
                i18n::with_language(language, || tf!("Invert input {}", n))
            } else {
                translated(name)
            };
            assert!(has(&labels, &expected), "{language:?} {name}");
        }
        click(&ctx, &mut app, "0/1");
        click(&ctx, &mut app, &translated("0/floating"));
        assert_eq!(
            app.circuit().component(gate).unwrap().parity.output,
            ParityOutput::ZeroFloating
        );
        click(&ctx, &mut app, &translated("Narrow"));
        click(&ctx, &mut app, &translated("Wide"));
        assert_eq!(
            app.circuit().component(gate).unwrap().gate_size,
            GateSize::Wide
        );
        assert_eq!(app.circuit().wires, wires);
        let labels = frame(&ctx, &mut app, vec![]);
        let row = labels
            .iter()
            .find(|(s, _)| s == &i18n::with_language(language, || tf!("Invert input {}", 1)))
            .unwrap()
            .1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.circuit().component(gate).unwrap().parity_negated(0));
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert!(!app.circuit().component(gate).unwrap().parity_negated(0));
        app.editor.redo();
        assert!(app.circuit().component(gate).unwrap().parity_negated(0));
        click(&ctx, &mut app, &translated("Gates"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -160.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        for parity in Parity::ALL {
            click(&ctx, &mut app, &translated(parity.name()));
            assert_eq!(app.editor.tool, Tool::Place(Kind::Parity(parity)));
        }
    }
}

#[test]
fn character_terminal_properties_translate_and_edit_without_moving_wires() {
    for language in Language::ALL {
        for kind in [Kind::Keyboard, Kind::Tty] {
            let ctx = Context::default();
            configure(&ctx);
            let mut app = Workspace::default();
            app.set_language(language);
            let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
            app.editor.set_tool(Tool::Place(kind));
            app.editor.place(Point::new(300, 300));
            let id = app.circuit().components[0].id;
            app.editor
                .circuit
                .add_wire(Point::new(200, 300), Point::new(300, 300));
            let before = app.circuit().clone();
            let labels = frame(&ctx, &mut app, vec![]);
            for s in [
                "Character encoding",
                "Trigger",
                if kind == Kind::Keyboard {
                    "Buffer length"
                } else {
                    "Display rows"
                },
            ] {
                assert!(has(&labels, &t(s)), "{s}");
            }
            for s in ["Facing", "Label", "Data bits"] {
                assert!(!has(&labels, &t(s)), "{s}");
            }
            click(&ctx, &mut app, &t("ASCII (7-bit)"));
            click(&ctx, &mut app, &t("UTF-16 (16-bit)"));
            assert_eq!(app.circuit().component(id).unwrap().bits, 16);
            assert_eq!(app.circuit().wires, before.wires);
            app.editor.undo();
            assert_eq!(app.circuit(), &before);
            app.editor.redo();
            click(&ctx, &mut app, &t("Rising edge"));
            click(&ctx, &mut app, &t("Falling edge"));
            assert_eq!(
                app.circuit().component(id).unwrap().terminal.trigger,
                logisim_core::Trigger::Falling
            );
            assert_eq!(app.circuit().wires, before.wires);
        }
    }
}

#[test]
fn pla_table_apply_cancel_clear_history_and_document_replacement_in_both_languages() {
    use logisim_core::PlaInput;
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Memory"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(150.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -370.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                    source: egui::MouseWheelSource::Unknown,
                },
            ],
        );
        click(&ctx, &mut app, &t("PLA ROM"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::PlaRom));
        app.editor.template.pla.resize(1, 1, 1);
        let labels = frame(&ctx, &mut app, vec![]);
        for label in [
            "Input bits",
            "AND terms",
            "Output bits",
            "Chip select active on",
        ] {
            assert!(has(&labels, &t(label)), "{label}");
        }
        assert!(!has(&labels, &t("Facing")));
        assert!(!has(&labels, &t("Data bits")));
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, "—");
        click(&ctx, &mut app, "·");
        assert!(app.circuit().components.is_empty());
        assert_eq!(app.editor.template.pla.input(0, 0), PlaInput::Disconnected);
        click(&ctx, &mut app, &t("Apply"));
        assert_eq!(app.editor.template.pla.input(0, 0), PlaInput::Direct);
        assert!(app.editor.template.pla.output(0, 0));
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        app.editor
            .circuit
            .add_wire(Point::new(100, 200), Point::new(300, 200));
        let original = app.circuit().clone();
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, &t("Clear contents"));
        click(&ctx, &mut app, &t("Cancel"));
        assert_eq!(app.circuit(), &original);
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, &t("Clear contents"));
        click(&ctx, &mut app, &t("Apply"));
        assert_eq!(
            app.circuit().component(id).unwrap().pla.input(0, 0),
            PlaInput::Disconnected
        );
        assert!(!app.circuit().component(id).unwrap().pla.output(0, 0));
        assert_eq!(app.circuit().wires, original.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &original);
        app.editor.redo();
        assert!(!app.circuit().component(id).unwrap().pla.output(0, 0));
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, "—");
        app.replace_circuit(original.clone());
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("PLA programming table")));
        assert_eq!(app.circuit(), &original);
    }
}

#[test]
fn random_library_seed_trigger_and_undo_are_available_in_both_languages() {
    fn number(ctx: &Context, app: &mut Workspace, old: &str, new: &str) {
        click(ctx, app, old);
        frame(
            ctx,
            app,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text(new.into()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }

    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Memory"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(150.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -300.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                    source: egui::MouseWheelSource::Unknown,
                },
            ],
        );
        click(&ctx, &mut app, &t("Random Generator"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Random));
        let labels = frame(&ctx, &mut app, vec![]);
        for key in ["Data bits", "Trigger", "Seed (0 = automatic)", "Label"] {
            assert!(has(&labels, &t(key)), "{key}");
        }
        assert!(!has(&labels, &t("Facing")));
        number(&ctx, &mut app, "0", "-2147483648");
        assert_eq!(app.editor.template.sequential.random_seed, i32::MIN);
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components[0].clone();
        app.editor.selection.insert(c.id);
        app.editor
            .circuit
            .add_wire(c.ports()[0], c.ports()[0] + Point::new(100, 0));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("Rising edge"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("High level")));
        click(&ctx, &mut app, &t("Falling edge"));
        assert_eq!(
            app.circuit().component(c.id).unwrap().sequential.trigger,
            logisim_core::Trigger::Falling
        );
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(c.id).unwrap().sequential.trigger,
            logisim_core::Trigger::Falling
        );
    }
}

#[test]
fn ttl_library_properties_power_internal_view_and_undo_in_both_languages() {
    fn toggle(ctx: &Context, app: &mut Workspace, text: &str) {
        frame(ctx, app, vec![]);
        let labels = frame(ctx, app, vec![]);
        let row = labels.iter().find(|(s, _)| s == text).unwrap().1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
        for pressed in [true, false] {
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
            );
        }
    }
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, "TTL");
        for chip in logisim_core::Ttl::ALL {
            if chip == logisim_core::Ttl::Ic747266 {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(150.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -120.0),
                            phase: egui::TouchPhase::Move,
                            modifiers: Modifiers::NONE,
                            source: egui::MouseWheelSource::Unknown,
                        },
                    ],
                );
            }
            click(&ctx, &mut app, chip.name());
            assert_eq!(app.editor.tool, Tool::Place(Kind::Ttl(chip)));
            let labels = frame(&ctx, &mut app, vec![]);
            for key in [
                "Facing",
                "Include Vcc/Gnd pins",
                "Show internal structure",
                chip.description(),
            ] {
                assert!(has(&labels, &t(key)), "{key}");
            }
            for key in ["Data bits", "Inputs", "Gate size"] {
                assert!(!has(&labels, &t(key)));
            }
        }
        toggle(&ctx, &mut app, &t("Include Vcc/Gnd pins"));
        toggle(&ctx, &mut app, &t("Show internal structure"));
        assert!(app.editor.template.ttl.power_pins && app.editor.template.ttl.internal);
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components[0].clone();
        app.editor.selection.insert(c.id);
        let p = c.ports()[0];
        app.editor.circuit.add_wire(p, p + Point::new(0, 100));
        let before = app.circuit().clone();
        toggle(&ctx, &mut app, &t("Include Vcc/Gnd pins"));
        assert_eq!(app.circuit().component(c.id).unwrap().ports().len(), 12);
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        click(&ctx, &mut app, &t("East"));
        click(&ctx, &mut app, &t("North"));
        assert_eq!(
            app.circuit().component(c.id).unwrap().facing,
            logisim_core::Facing::North
        );
        assert_eq!(app.circuit().wires, before.wires);
        let edited = app.circuit().clone();
        app.set_language(if language == Language::English {
            Language::SimplifiedChinese
        } else {
            Language::English
        });
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.circuit(), &edited);
        let pin = i18n::with_language(language, || {
            crate::ttl::port_label(&c, c.ttl_index(3).unwrap())
        });
        assert!(pin.contains('3'));
    }
}

#[test]
fn memory_import_button_history_template_and_stale_target_are_safe() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = logisim_core::Circuit::default();
        let mut component = Component::new(Kind::Rom, Point::new(200, 100));
        component.memory.contents.fill(0, 256, 0xaa);
        let rom = circuit.add_component(component);
        let ram = circuit.add_component(Component::new(Kind::Ram, Point::new(400, 100)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(rom);
        click(&ctx, &mut app, &t("Edit contents…"));
        let before = app.project().clone();
        click(&ctx, &mut app, &t("Import .hex…"));
        assert_eq!(app.take_file_actions(), [crate::FileAction::ImportMemory]);
        let target = app.memory_import_target().unwrap();
        assert_eq!(
            app.project(),
            &before,
            "Opening the picker must not edit memory"
        );
        let mut contents = logisim_core::MemoryContents::default();
        contents.set(0, 0x1234);
        contents.set(1, 0xff);
        assert!(app.complete_memory_import(&target, contents.clone()));
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0x34
        );
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(2),
            0
        );
        assert!(
            !app.complete_memory_import(&target, Default::default()),
            "Old requests cannot overwrite later edits"
        );
        click(&ctx, &mut app, &t("Close"));
        app.editor.undo();
        assert_eq!(app.project(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(1),
            0xff
        );

        app.editor.selection.clear();
        app.editor.selection.insert(ram);
        click(&ctx, &mut app, &t("Edit contents…"));
        click(&ctx, &mut app, &t("Import .hex…"));
        assert_eq!(app.take_file_actions(), [crate::FileAction::ImportMemory]);
        let target = app.memory_import_target().unwrap();
        let before = app.project().clone();
        assert!(app.complete_memory_import(&target, contents.clone()));
        assert_eq!(app.project(), &before, "RAM image is runtime data");
        app.editor.path.push(999);
        assert!(!app.memory_import_is_current(&target));
        app.editor.path.clear();
        click(&ctx, &mut app, &t("Close"));
        assert!(!app.complete_memory_import(&target, contents.clone()));

        app.editor.set_tool(crate::editor::Tool::Place(Kind::Rom));
        app.editor.memory_target = Some((true, app.editor.template.id));
        let target = app.memory_import_target().unwrap();
        assert!(app.complete_memory_import(&target, contents));
        assert_eq!(app.project(), &before);
        assert_eq!(app.editor.template.memory.contents.get(0), 0x34);
        app.replace_project(before);
        assert!(!app.memory_import_is_current(&target));
    }
}

#[test]
fn pin_pull_label_and_tristate_properties_edit_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor.set_tool(Tool::Place(Kind::Pin));
        app.editor.place(Point::new(300, 300));
        let id = app.circuit().components[0].id;
        app.editor
            .circuit
            .add_wire(Point::new(300, 300), Point::new(400, 300));
        let wires = app.circuit().wires.clone();
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        for name in ["Label location", "Three-state input", "Pull behavior"] {
            assert!(has(&labels, &t(name)));
        }
        let row = labels
            .iter()
            .find(|(s, _)| *s == t("Three-state input"))
            .unwrap()
            .1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.circuit().component(id).unwrap().pin.three_state);
        click(&ctx, &mut app, &t("Unchanged"));
        click(&ctx, &mut app, &t("Pull down"));
        assert_eq!(
            app.circuit().component(id).unwrap().pin.pull,
            logisim_core::PinPull::Down
        );
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert_eq!(
            app.circuit().component(id).unwrap().pin.pull,
            logisim_core::PinPull::None
        );
        app.editor.redo();
        assert_eq!(
            app.circuit().component(id).unwrap().pin.pull,
            logisim_core::PinPull::Down
        );
    }
}

#[test]
fn ordinary_gate_xor_mode_and_input_inversion_edit_with_undo_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor
            .set_tool(Tool::Place(Kind::Gate(logisim_core::Gate::Xor)));
        app.editor.place(Point::new(300, 300));
        let c = &app.circuit().components[0];
        let id = c.id;
        let port = c.ports()[1];
        app.editor.circuit.add_wire(port, port + Point::new(-40, 0));
        let geometry = |c: &logisim_core::Circuit| {
            c.wires
                .iter()
                .flat_map(|w| (w.a.x.min(w.b.x)..=w.a.x.max(w.b.x)).map(move |x| (x, w.a.y)))
                .collect::<std::collections::BTreeSet<_>>()
        };
        let wires = geometry(app.circuit());
        click(&ctx, &mut app, &t("Exactly one"));
        click(&ctx, &mut app, &t("Odd parity"));
        assert!(app.circuit().component(id).unwrap().gate_options.xor_odd);
        let labels = frame(&ctx, &mut app, vec![]);
        let row_label = i18n::with_language(language, || tf!("Invert input {}", 1));
        let row = labels.iter().find(|(s, _)| *s == row_label).unwrap().1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.circuit().component(id).unwrap().gate_negated(0));
        assert_eq!(geometry(app.circuit()), wires);
        app.editor.undo();
        assert!(!app.circuit().component(id).unwrap().gate_negated(0));
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().gate_negated(0));
    }
}

#[test]
fn centered_button_label_is_editable_and_drawn_inside_the_button_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor.set_tool(Tool::Place(Kind::Button));
        app.editor.place(Point::new(300, 300));
        app.editor.circuit.components[0].label = "C".into();
        let id = app.editor.circuit.components[0].id;
        click(&ctx, &mut app, &t("North"));
        click(&ctx, &mut app, &t("Center"));
        assert!(app.circuit().component(id).unwrap().label_center);
        let labels = frame(&ctx, &mut app, vec![]);
        let label = labels
            .iter()
            .find(|(s, r)| s == "C" && r.center().x > 300.0)
            .unwrap()
            .1;
        let center = app.canvas.screen_vec(
            crate::drawing::bounds(app.circuit().component(id).unwrap())
                .center()
                .to_vec2(),
        );
        assert!(label.center().distance(center) < 1.0);
        app.editor.undo();
        assert!(!app.circuit().component(id).unwrap().label_center);
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().label_center);
    }
}

#[test]
fn tick_frequency_presets_custom_validation_cancel_and_modal_shortcuts_in_both_languages() {
    fn replace_input(ctx: &Context, app: &mut Workspace, value: &str) {
        // The modal's input initially owns focus; Ctrl+A must not select the circuit.
        frame(
            ctx,
            app,
            vec![Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        frame(ctx, app, vec![Event::Text(value.into())]);
        assert_eq!(
            app.editor.simulation.tick_rate_draft.as_deref(),
            Some(value)
        );
    }
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        app.editor.simulation.view.tick_hz = DEFAULT_TICK_HZ;
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        let before = app.project().clone();
        click(&ctx, &mut app, &t("Simulate"));
        click(&ctx, &mut app, &t("Tick frequency"));
        click(&ctx, &mut app, "4096 Hz");
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::SetTickRate(4096)]
        );
        app.editor.simulation.view.tick_hz = 4096;
        click(&ctx, &mut app, &t("Simulate"));
        click(&ctx, &mut app, &t("Tick frequency"));
        click(&ctx, &mut app, &t("Custom…"));
        frame(&ctx, &mut app, vec![]);
        for bad in ["0", "4097", "abc", "1.5"] {
            replace_input(&ctx, &mut app, bad);
            let labels = frame(&ctx, &mut app, vec![]);
            assert!(has(
                &labels,
                &i18n::with_language(language, || tf!(
                    "Enter a whole number from 1 to {}.",
                    MAX_TICK_HZ
                ))
            ));
            click(&ctx, &mut app, &t("Apply"));
            assert!(app.take_simulation_actions().is_empty());
            assert!(app.editor.simulation.tick_rate_draft.is_some());
            click(&ctx, &mut app, bad); // restore input focus for the next edit
        }
        replace_input(&ctx, &mut app, "250");
        frame(
            &ctx,
            &mut app,
            vec![Event::Key {
                key: Key::N,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        assert!(app.take_file_actions().is_empty());
        click(&ctx, &mut app, &t("Apply"));
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::SetTickRate(250)]
        );
        assert!(app.editor.simulation.tick_rate_draft.is_none());
        app.editor.simulation.view.tick_hz = 250;
        click(&ctx, &mut app, &t("Simulate"));
        click(&ctx, &mut app, &t("Tick frequency"));
        click(&ctx, &mut app, &t("Custom…"));
        frame(&ctx, &mut app, vec![]);
        replace_input(&ctx, &mut app, "37");
        click(&ctx, &mut app, &t("Cancel"));
        assert!(app.take_simulation_actions().is_empty());
        assert!(app.editor.simulation.tick_rate_draft.is_none());
        assert_eq!(app.project(), &before);
        assert!(!app.editor.can_undo());
    }
}

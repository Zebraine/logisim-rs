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
            click(&ctx, &mut app, &translated(kind.name()));
            assert_eq!(app.editor.tool, Tool::Place(Kind::Plexer(kind)));
            let labels = frame(&ctx, &mut app, vec![]);
            assert!(has(&labels, &translated("Select bits")));
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

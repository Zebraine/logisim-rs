use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers};
use logisim_core::{Kind, Point};

#[test]
fn runtime_rates_are_visible_beside_controls_in_both_languages_and_narrow_windows() {
    for language in Language::ALL {
        for size in [egui::vec2(1200.0, 820.0), egui::vec2(720.0, 480.0)] {
            let ctx = Context::default();
            configure(&ctx);
            let mut app = Workspace::default();
            app.set_language(language);
            app.set_simulation(SimulationView {
                enabled: true,
                has_clocks: true,
                tick_hz: 4096,
                actual_tick_hz: 1766.5,
                frames_per_second: Some(59.9),
                ..Default::default()
            });
            let before = app.project().clone();
            let mut labels = vec![];
            for _ in 0..3 {
                labels = frame_at_size(&ctx, &mut app, vec![], size);
            }
            let expected = i18n::with_language(language, || {
                [
                    tf!("FPS: {}", "59.9"),
                    tf!("Target: {} Hz", 4096),
                    tf!("Actual: {} Hz", "1766.5"),
                ]
            });
            let auto = i18n::with_language(language, || i18n::tr("Auto ticks"));
            let auto = labels.iter().find(|(text, _)| text == auto).unwrap().1;
            for expected in expected {
                let rect = labels
                    .iter()
                    .find(|(text, _)| text == &expected)
                    .unwrap_or_else(|| panic!("Missing {expected}: {labels:?}"))
                    .1;
                assert!(
                    rect.left() >= 0.0 && rect.right() <= size.x,
                    "{expected}: {rect:?}"
                );
                assert!(
                    rect.bottom() <= app.canvas.rect.top(),
                    "{expected} is outside the toolbar"
                );
                assert!(rect.left() >= auto.right() || rect.top() >= auto.bottom());
            }
            assert_eq!(app.project(), &before);
            assert!(!app.editor.can_undo());
            assert!(app.take_simulation_actions().is_empty());
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
    let selection = app.editor.session.selection.clone();
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
    assert_eq!(app.editor.session.selection, selection);
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

#[test]
fn settings_are_localized_and_separate_from_document_and_runtime_state() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(language);
        app.simulation_mut().ticks = 17;
        app.simulation_mut().pending = 2;
        let document = app.project().clone();
        let t = |key| i18n::with_language(language, || i18n::tr(key).to_owned());
        assert!(!app.truth_table_generation());
        for mode in [
            SimulationMode::Event,
            SimulationMode::Stable,
            SimulationMode::Event,
        ] {
            let old = i18n::with_language(language, || app.simulation_mode().name().to_owned());
            let new = i18n::with_language(language, || mode.name().to_owned());
            click(&ctx, &mut app, &old);
            click(&ctx, &mut app, &new);
            let was_enabled = app.truth_table_generation();
            click(&ctx, &mut app, &t("Enable truth table generation"));
            assert_eq!(app.truth_table_generation(), !was_enabled);
            assert_eq!(app.simulation_mode(), mode);
            assert_eq!(app.project(), &document);
            assert!(!app.editor.can_undo());
            assert_eq!(app.simulation_mut().ticks, 17);
            assert_eq!(app.simulation_mut().pending, 2);
            assert!(app.take_file_actions().is_empty());
            assert!(app.take_simulation_actions().is_empty());
        }
        let small = Context::default();
        configure(&small);
        let size = egui::vec2(720.0, 480.0);
        let mut labels = vec![];
        for _ in 0..3 {
            labels = frame_at_size(&small, &mut app, vec![], size);
        }
        for text in [
            t("Simulation mode"),
            t("Event propagation"),
            t("Enable truth table generation"),
        ] {
            let rect = labels.iter().find(|(label, _)| label == &text).unwrap().1;
            assert!(rect.min.x >= 0.0 && rect.max.x <= size.x && rect.max.y <= size.y);
        }
        app.replace_project(document);
        assert_eq!(app.simulation_mode(), SimulationMode::Event);
        assert!(app.truth_table_generation());
    }
}

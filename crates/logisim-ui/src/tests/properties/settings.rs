use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers};
use logisim_core::{Kind, Point};

#[test]
fn runtime_rates_are_visible_beside_controls_in_both_languages_and_narrow_windows() {
    for language in Language::ALL {
        for (size, density, scale) in [
            (egui::vec2(1200., 820.), Density::Comfortable, UiScale::P100),
            (egui::vec2(720., 480.), Density::Comfortable, UiScale::P100),
            (egui::vec2(720., 480.), Density::Compact, UiScale::P100),
            (egui::vec2(480., 320.), Density::Comfortable, UiScale::P150),
            (egui::vec2(480., 320.), Density::Compact, UiScale::P150),
        ] {
            for (target_hz, actual_hz, target, actual) in [
                (512, 512.3, "512", "512.3"),
                (1024, 1024.0, "1k", "1.0k"),
                (1536, 1766.5, "1.5k", "1.7k"),
                (32768, 32768.0, "32k", "32.0k"),
            ] {
                let ctx = Context::default();
                configure(&ctx);
                let mut app = Workspace::default();
                app.apply_settings(Settings {
                    language,
                    density,
                    ui_scale: scale,
                    ..app.preferences()
                });
                app.set_simulation(SimulationView {
                    enabled: true,
                    has_clocks: true,
                    tick_hz: target_hz,
                    actual_tick_hz: actual_hz,
                    frames_per_second: Some(59.9),
                    ..Default::default()
                });
                let before = app.project().clone();
                let mut labels = vec![];
                for _ in 0..3 {
                    labels = frame_at_size(&ctx, &mut app, vec![], size);
                }
                assert!(app.canvas.rect.width() * (f32::from(scale.percent()) / 100.) > 450.);
                let grid = i18n::with_language(language, || i18n::tr("Grid"));
                let grid_rect = labels.iter().find(|(text, _)| text == grid).unwrap().1;
                assert!(
                    grid_rect.right() <= app.canvas.rect.left() && grid_rect.bottom() <= size.y,
                    "{density:?} {scale:?} grid {grid_rect:?} beside {:?}",
                    app.canvas.rect
                );
                let expected = i18n::with_language(language, || {
                    [
                        tf!("FPS: {}", "59.9"),
                        tf!("Target: {} Hz", target),
                        tf!("Actual: {} Hz", actual),
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
        for (label, hz) in [
            ("512 Hz", 512),
            ("1k Hz", 1024),
            ("2k Hz", 2048),
            ("4k Hz", 4096),
            ("8k Hz", 8192),
            ("16k Hz", 16384),
            ("32k Hz", 32768),
        ] {
            click(&ctx, &mut app, &t("Simulate"));
            click(&ctx, &mut app, &t("Tick frequency"));
            click(&ctx, &mut app, label);
            assert_eq!(
                app.take_simulation_actions(),
                vec![SimulationAction::SetTickRate(hz)]
            );
            app.editor.simulation.view.tick_hz = hz;
        }
        click(&ctx, &mut app, &t("Simulate"));
        click(&ctx, &mut app, &t("Tick frequency"));
        click(&ctx, &mut app, &t("Custom…"));
        frame(&ctx, &mut app, vec![]);
        for bad in ["0", "32769", "abc", "1.5"] {
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
        replace_input(&ctx, &mut app, "32768");
        click(&ctx, &mut app, &t("Apply"));
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::SetTickRate(32768)]
        );
        app.editor.simulation.view.tick_hz = 32768;
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
            click(&ctx, &mut app, &t("Simulation"));
            let old = i18n::with_language(language, || app.simulation_mode().name().to_owned());
            let new = i18n::with_language(language, || mode.name().to_owned());
            click(&ctx, &mut app, &old);
            click(&ctx, &mut app, &new);
            let was_enabled = app.truth_table_generation();
            click(&ctx, &mut app, &t("Circuit generation"));
            click(&ctx, &mut app, &t(if was_enabled { "On" } else { "Off" }));
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
            t("Show truth tables in circuit generation."),
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

#[test]
fn search_matches_localized_descriptions_and_reset_requires_confirmation() {
    let key = |key, modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(language);
        app.apply_settings(Settings {
            density: Density::Compact,
            theme: ThemeMode::Dark,
            accent: Accent::Blue,
            ..app.preferences()
        });
        app.simulation_mut().steps = 23;
        let document = app.project().clone();
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(Key::F, Modifiers::COMMAND)]);
        frame(
            &ctx,
            &mut app,
            vec![Event::Text(
                if language == Language::English {
                    "animations"
                } else {
                    "动画"
                }
                .into(),
            )],
        );
        assert!(has(&frame(&ctx, &mut app, vec![]), &t("Reduce motion")));
        click(&ctx, &mut app, &t("Off"));
        assert!(app.preferences().reduce_motion);
        frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(app.settings_open);
        click(&ctx, &mut app, &t("Advanced"));
        click(&ctx, &mut app, &t("Reset all"));
        click(&ctx, &mut app, &t("Cancel"));
        assert!(app.preferences().reduce_motion);
        click(&ctx, &mut app, &t("Reset all"));
        click(&ctx, &mut app, &t("Confirm reset"));
        assert_eq!(app.preferences(), Settings::default());
        assert_eq!(app.project(), &document);
        assert_eq!(app.simulation_mut().steps, 23);
        assert!(!app.editor.can_undo());
        frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(!app.settings_open);
    }
}

#[test]
fn grid_zoom_frequency_motion_and_telemetry_share_the_workbench_controls() {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    let mut project = logisim_core::Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    circuit.add_component(logisim_core::Component::new(
        Kind::Pin,
        Point::new(9000, 3000),
    ));
    circuit.add_component(logisim_core::Component::new(Kind::Pin, Point::ZERO));
    app.replace_project(project.clone());
    frame(&ctx, &mut app, vec![]);
    let fit = app.canvas.camera.zoom;
    assert!(fit < 1.);
    let offset = app.canvas.camera.offset;
    app.simulation_mut().ticks = 19;
    click(&ctx, &mut app, "Grid");
    assert!(!app.preferences().show_grid);
    app.settings_open = true;
    click(&ctx, &mut app, "Editor");
    let key = |key, modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    frame(&ctx, &mut app, vec![key(Key::F, Modifiers::COMMAND)]);
    frame(&ctx, &mut app, vec![Event::Text("editor.show_grid".into())]);
    click(&ctx, &mut app, "Off");
    frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.preferences().show_grid);
    click(&ctx, &mut app, "Fit to canvas");
    click(&ctx, &mut app, "100%");
    assert_eq!(app.canvas.camera.zoom, fit);
    assert_eq!(app.canvas.camera.offset, offset);
    click(&ctx, &mut app, "Appearance");
    click(&ctx, &mut app, "On"); // telemetry
    click(&ctx, &mut app, "Off"); // reverse paint order selects telemetry again
    assert!(app.preferences().show_telemetry);
    let settings = Settings {
        show_telemetry: false,
        reduce_motion: true,
        ui_scale: UiScale::P125,
        ..app.preferences()
    };
    app.apply_settings(settings);
    app.settings_open = false;
    let mut labels = vec![];
    for _ in 0..3 {
        labels = frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(ctx.zoom_factor(), 1.25);
    assert_eq!(ctx.global_style().animation_time, 0.);
    assert!(!labels.iter().any(|(s, _)| s.starts_with("FPS:")));
    assert!(has(&labels, "Paused"));
    assert_eq!(app.project(), &project);
    assert_eq!(app.simulation_mut().ticks, 19);
    assert!(!app.editor.can_undo());
    app.replace_project(project);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.canvas.camera.zoom, 1.);
    app.apply_settings(Settings {
        tick_hz: TickHz::new(512).unwrap(),
        ..Settings::default()
    });
    assert_eq!(
        app.take_simulation_actions(),
        vec![SimulationAction::SetTickRate(512)]
    );
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(
        ctx.global_style().animation_time,
        crate::theme::ANIMATION_SECONDS
    );
    click(&ctx, &mut app, "Simulate");
    click(&ctx, &mut app, "Tick frequency");
    click(&ctx, &mut app, "8 Hz");
    assert_eq!(app.preferences().tick_hz.get(), 8);
    assert_eq!(
        app.take_simulation_actions(),
        vec![SimulationAction::SetTickRate(8)]
    );
}

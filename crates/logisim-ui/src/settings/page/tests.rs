use super::*;
use crate::tests::support::widgets::frame_at_size;

#[test]
fn registry_restores_each_field_without_changing_other_preferences() {
    let defaults = Settings::default();
    let alternative = Settings {
        shortcuts: Default::default(),
        files: FileSettings {
            autosave: false,
            autosave_minutes: 30,
        },
        workspace: WorkspaceSettings {
            recent_limit: 3,
            reopen_last: true,
            restore_window: false,
            restore_panels: false,
        },
        language: Language::SimplifiedChinese,
        density: crate::Density::Compact,
        accent: crate::Accent::Teal,
        ui_scale: UiScale::P150,
        reduce_motion: true,
        show_telemetry: false,
        signal_palette: SignalPalette::ColorblindSafe,
        show_wire_values: false,
        show_origin_marker: false,
        show_grid: false,
        default_zoom: DefaultZoom::Actual,
        highlight_net: false,
        show_connection_markers: false,
        invert_wheel_zoom: true,
        default_probe_radix: logisim_core::Radix::Hex,
        default_pin_bits: 8,
        default_gate_size: logisim_core::GateSize::Medium,
        default_tunnel_width: 13,
        remember_last_properties: true,
        simulation_mode: SimulationMode::Event,
        tick_hz: TickHz::new(1).unwrap(),
        truth_table: true,
    };
    let mut keys = std::collections::BTreeSet::new();
    let mut restored = alternative;
    for e in REGISTRY {
        assert!(keys.insert(e.key));
        assert!(e.changed(alternative, defaults), "{}", e.key);
        let mut changed = defaults;
        e.copy(&mut changed, alternative);
        assert_eq!(
            REGISTRY
                .iter()
                .filter(|f| f.changed(changed, defaults))
                .count(),
            1
        );
        e.copy(&mut changed, defaults);
        assert_eq!(changed, defaults);
        e.copy(&mut restored, defaults);
    }
    assert_eq!(restored, defaults);
}

#[test]
fn dialog_stays_centered_and_keeps_its_controls_visible_on_resize() {
    for lang in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(lang);
        app.set_config_path(Some(format!(
            "/home/{}/preferences.json",
            "long-directory/".repeat(20)
        )));
        for density in crate::Density::ALL {
            for (width, height, scale) in [
                (1200., 820., UiScale::P100),
                (1440., 900., UiScale::P100),
                (720., 480., UiScale::P100),
                (720., 480., UiScale::P80),
                (720., 480., UiScale::P90),
                (720., 480., UiScale::P110),
                (720., 480., UiScale::P125),
                (720., 480., UiScale::P150),
                (1200., 820., UiScale::P100),
            ] {
                app.apply_settings(Settings {
                    density,
                    ui_scale: scale,
                    ..app.preferences()
                });
                let screen = egui::vec2(width, height) / (f32::from(scale.percent()) / 100.);
                for cat in Category::ALL {
                    app.settings_page.select(cat);
                    let mut labels = vec![];
                    for _ in 0..4 {
                        labels = frame_at_size(&ctx, &mut app, vec![], screen);
                    }
                    let bounds = ctx
                        .memory(|m| m.area_rect(egui::Id::unique("settings-window")))
                        .unwrap();
                    assert!(
                        bounds.width() <= screen.x * 0.9 + 1.
                            && bounds.height() <= screen.y * 0.9 + 1.,
                        "{cat:?} {screen} {bounds:?}"
                    );
                    assert!(
                        (bounds.center().to_vec2() - screen * 0.5).length() <= 1.,
                        "off-center {cat:?} {screen} {bounds:?}"
                    );
                    let close = crate::i18n::with_language(lang, || tr("Close"));
                    let close_rect = labels.iter().rev().find(|(s, _)| s == close).unwrap().1;
                    assert!(bounds.contains_rect(close_rect));
                    assert!(
                        egui::Rect::from_min_size(egui::Pos2::ZERO, screen).contains_rect(bounds)
                    );
                    if cat == Category::Appearance {
                        let scale_rect = labels
                            .iter()
                            .rev()
                            .find(|(s, _)| s == &format!("{}%", scale.percent()))
                            .unwrap()
                            .1;
                        assert!(bounds.contains_rect(scale_rect));
                        assert!(scale_rect.bottom() < close_rect.top());
                    }
                }
            }
        }
    }
}

#[test]
fn dragging_frequency_commits_only_on_release() {
    use crate::tests::support::widgets::frame;
    use egui::{Event, PointerButton};
    let ctx = Context::default();
    crate::configure(&ctx);
    let mut app = Workspace {
        settings_open: true,
        ..Default::default()
    };
    app.settings_page.select(Category::Simulation);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let labels = frame(&ctx, &mut app, vec![]);
    let start = labels
        .iter()
        .rev()
        .find(|(s, _)| s == "4")
        .unwrap()
        .1
        .center();
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(start), press(start, true)],
    );
    for dx in [12., 24., 48.] {
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(start + vec2(dx, 0.))],
        );
        assert_eq!(app.preferences().tick_hz, TickHz::default());
        assert!(app.take_simulation_actions().is_empty());
    }
    frame(&ctx, &mut app, vec![press(start + vec2(48., 0.), false)]);
    let rate = app.preferences().tick_hz;
    assert!(rate.get() > TickHz::default().get());
    assert_eq!(
        app.take_simulation_actions(),
        [crate::SimulationAction::SetTickRate(rate.get())]
    );
}

#[test]
fn workspace_history_actions_and_resets_leave_document_and_runtime_intact() {
    use crate::tests::support::widgets::{click, frame, has};
    let ctx = Context::default();
    crate::configure(&ctx);
    let mut app = Workspace {
        settings_open: true,
        ..Default::default()
    };
    app.settings_page.select(Category::Workspace);
    app.set_recent(vec!["/work/first.circ".into()]);
    let layout = crate::WorkspaceLayout {
        tab: crate::LayoutTab::Components,
        sidebar_width: 370.,
        split: 0.62,
    };
    app.set_layout(layout);
    app.editor.simulation.view.steps = 19;
    let project = app.project().clone();
    click(&ctx, &mut app, "Off");
    assert!(app.preferences().workspace.reopen_last);
    click(&ctx, &mut app, "first.circ");
    assert!(!app.settings_open);
    assert_eq!(app.take_file_actions(), [crate::FileAction::OpenRecent(0)]);
    app.settings_open = true;
    click(&ctx, &mut app, "Clear recent");
    assert_eq!(app.take_file_actions(), [crate::FileAction::ClearRecent]);
    app.settings_page.select(Category::Advanced);
    app.settings_page.frequency_draft = Some(36);
    click(&ctx, &mut app, "Reset all");
    click(&ctx, &mut app, "Confirm reset");
    assert_eq!(app.settings_page.frequency_draft, None);
    assert_eq!(app.preferences(), Settings::default());
    assert_eq!(app.recent(), ["/work/first.circ"]);
    assert_eq!(app.layout(), layout);
    app.settings_page.select(Category::Workspace);
    click(&ctx, &mut app, "Reset workspace layout");
    assert_eq!(app.layout(), crate::WorkspaceLayout::default());
    assert!(app.take_window_reset());
    assert!(!app.take_window_reset());
    assert_eq!(&project, app.project());
    assert!(!app.editor.can_undo());
    assert_eq!(app.editor.simulation.view.steps, 19);
    assert!(app.take_simulation_actions().is_empty());
    for language in Language::ALL {
        app.set_language(language);
        app.settings_page.query = "workspace.restore_panels".into();
        let labels = frame(&ctx, &mut app, vec![]);
        let label = crate::i18n::with_language(language, || tr("Restore inspector layout"));
        assert!(has(&labels, label));
    }
}

#[test]
fn appearance_and_editor_settings_are_searchable_by_key_and_localized_label() {
    use crate::tests::support::widgets::{frame, has};
    for language in Language::ALL {
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(language);
        for entry in REGISTRY
            .iter()
            .filter(|s| matches!(s.category, Category::Editor | Category::Appearance))
        {
            let label = crate::i18n::with_language(language, || tr(entry.label));
            for query in [entry.key, label] {
                app.settings_page.query = query.into();
                for _ in 0..2 {
                    frame(&ctx, &mut app, vec![]);
                }
                let labels = frame(&ctx, &mut app, vec![]);
                assert!(has(&labels, label), "{query}");
            }
        }
    }
}

#[test]
fn compact_appearance_and_editor_scroll_exposes_every_setting_in_both_languages() {
    use egui::{Event, MouseWheelSource, MouseWheelUnit, TouchPhase};
    for (lang, category) in Language::ALL
        .into_iter()
        .flat_map(|lang| [Category::Editor, Category::Appearance].map(|category| (lang, category)))
    {
        for (density, scale) in [
            (crate::Density::Comfortable, UiScale::P100),
            (crate::Density::Compact, UiScale::P100),
            (crate::Density::Compact, UiScale::P150),
        ] {
            let ctx = Context::default();
            crate::configure(&ctx);
            let mut app = Workspace {
                settings_open: true,
                ..Default::default()
            };
            app.apply_settings(Settings {
                language: lang,
                density,
                ui_scale: scale,
                ..app.preferences()
            });
            app.settings_page.select(category);
            let size = vec2(720., 480.) / (f32::from(scale.percent()) / 100.);
            let entries: Vec<_> = REGISTRY.iter().filter(|e| e.category == category).collect();
            let mut visible = std::collections::BTreeSet::new();
            for step in 0..36 {
                let mut events = vec![Event::PointerMoved(egui::pos2(size.x * 0.6, size.y * 0.65))];
                if step >= 3 {
                    events.push(Event::MouseWheel {
                        unit: MouseWheelUnit::Line,
                        delta: vec2(0., -1.),
                        phase: TouchPhase::Move,
                        source: MouseWheelSource::Unknown,
                        modifiers: Modifiers::NONE,
                    });
                }
                let labels = frame_at_size(&ctx, &mut app, events, size);
                if step < 2 {
                    continue;
                }
                // Use the fixed category picker and footer as the scrollable pane's bounds.
                let close = crate::i18n::with_language(lang, || tr("Close"));
                let close_rect = labels.iter().rfind(|(s, _)| s == close).unwrap().1;
                let picker = crate::i18n::with_language(lang, || category.label());
                let bounds = ctx
                    .memory(|m| m.area_rect(egui::Id::unique("settings-window")))
                    .unwrap();
                let picker_rect = labels
                    .iter()
                    .find(|(s, r)| {
                        s == picker && bounds.contains_rect(*r) && r.top() > bounds.top() + 40.
                    })
                    .unwrap()
                    .1;
                for e in &entries {
                    let label = crate::i18n::with_language(lang, || tr(e.label));
                    if labels.iter().any(|(s, r)| {
                        s == label
                            && r.top() > picker_rect.bottom()
                            && r.bottom() < close_rect.top()
                            && r.left() >= bounds.left()
                            && r.right() <= bounds.right()
                    }) {
                        visible.insert(e.key);
                    }
                }
            }
            assert_eq!(
                visible.len(),
                entries.len(),
                "{lang:?} {density:?} {scale:?}: {visible:?}"
            );
        }
    }
}

#[test]
fn recovery_status_and_explicit_clear_leave_document_and_simulation_intact() {
    use crate::tests::support::widgets::{click, frame, has};
    for language in Language::ALL {
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(language);
        app.settings_page.select(Category::Files);
        let label = |key: &'static str| crate::i18n::with_language(language, || tr(key));
        let project = app.project().clone();
        app.simulation_mut().ticks = 31;
        for state in [
            SnapshotState::Empty,
            SnapshotState::Unavailable {
                snapshot_present: false,
            },
            SnapshotState::Damaged,
            SnapshotState::Available {
                source: None,
                saved_at: 0,
            },
        ] {
            app.set_recovery_view(RecoveryView {
                owned: true,
                directory: Some("/config/recovery".into()),
                error: matches!(
                    state,
                    SnapshotState::Damaged | SnapshotState::Unavailable { .. }
                )
                .then(|| "Recovery storage is busy.".into()),
                state: state.clone(),
            });
            let mut labels = Vec::new();
            for _ in 0..3 {
                labels = frame_at_size(&ctx, &mut app, vec![], vec2(720., 480.));
            }
            let message = match state {
                SnapshotState::Empty => "No recovery snapshot",
                SnapshotState::Unavailable { .. } => "Recovery storage unavailable",
                SnapshotState::Damaged => "Damaged recovery snapshot",
                SnapshotState::Available { .. } => "Untitled",
            };
            assert!(has(&labels, label(message)));
            if matches!(
                state,
                SnapshotState::Available { .. } | SnapshotState::Damaged
            ) {
                click(&ctx, &mut app, label("Clear recovery"));
                assert!(app.take_file_actions().is_empty());
                click(&ctx, &mut app, label("Cancel"));
                assert!(app.take_file_actions().is_empty());
                click(&ctx, &mut app, label("Clear recovery"));
                click(&ctx, &mut app, label("Confirm clear"));
                assert_eq!(
                    app.take_file_actions(),
                    [crate::FileAction::ClearRecovery {
                        disabled_autosave: false
                    }]
                );
            }
            frame(&ctx, &mut app, vec![]);
            assert_eq!(app.project(), &project);
            assert!(!app.editor.can_undo());
            assert_eq!(app.simulation_mut().ticks, 31);
            assert!(app.take_simulation_actions().is_empty());
        }
    }
}

#[test]
fn clearing_discloses_an_autosave_change_only_when_required() {
    use crate::tests::support::widgets::{click, frame, has};
    for lang in Language::ALL {
        let label = |key: &'static str| crate::i18n::with_language(lang, || tr(key));
        for (state, owned, dirty, autosave, clearable) in [
            (
                SnapshotState::Available {
                    source: None,
                    saved_at: 0,
                },
                false,
                true,
                true,
                false,
            ),
            (
                SnapshotState::Available {
                    source: None,
                    saved_at: 0,
                },
                true,
                true,
                true,
                true,
            ),
            (
                SnapshotState::Available {
                    source: None,
                    saved_at: 0,
                },
                true,
                false,
                true,
                true,
            ),
            (
                SnapshotState::Available {
                    source: None,
                    saved_at: 0,
                },
                true,
                true,
                false,
                true,
            ),
            (SnapshotState::Damaged, false, true, true, true),
            (
                SnapshotState::Unavailable {
                    snapshot_present: true,
                },
                false,
                true,
                true,
                true,
            ),
            (
                SnapshotState::Unavailable {
                    snapshot_present: false,
                },
                false,
                true,
                true,
                false,
            ),
        ] {
            let ctx = Context::default();
            crate::configure(&ctx);
            let mut app = Workspace {
                settings_open: true,
                ..Default::default()
            };
            app.set_language(lang);
            app.settings_page.select(Category::Files);
            let before_edit = app.project().clone();
            app.editor
                .set_tool(crate::editor::Tool::Place(logisim_core::Kind::Pin));
            app.editor.place(logisim_core::Point::new(20, 20));
            let selection = app.editor.session.selection.clone();
            app.document.dirty = dirty;
            app.preferences.files.autosave = autosave;
            app.set_recovery_view(RecoveryView {
                state: state.clone(),
                owned,
                ..Default::default()
            });
            let project = app.project().clone();
            app.simulation_mut().enabled = true;
            app.simulation_mut().ticks = 29;
            for size in [vec2(720., 480.), vec2(1200., 820.)] {
                let mut labels = vec![];
                for _ in 0..3 {
                    labels = frame_at_size(&ctx, &mut app, vec![], size);
                }
                assert_eq!(has(&labels, label("Clear recovery")), clearable);
                assert_eq!(
                    has(&labels, label("Recovery copy is owned by another window.")),
                    matches!(state, SnapshotState::Available { .. }) && !owned
                );
            }
            if clearable {
                let disable = dirty && autosave;
                click(&ctx, &mut app, label("Clear recovery"));
                let labels = frame(&ctx, &mut app, vec![]);
                assert_eq!(
                    has(
                        &labels,
                        label(
                            "Autosave is on. Clearing will also turn Autosave off so this copy stays removed."
                        )
                    ),
                    disable
                );
                click(&ctx, &mut app, label("Cancel"));
                assert_eq!(app.preferences().files.autosave, autosave);
                assert!(app.take_file_actions().is_empty());
                click(&ctx, &mut app, label("Clear recovery"));
                click(&ctx, &mut app, label("Confirm clear"));
                assert_eq!(app.preferences().files.autosave, autosave && !disable);
                assert_eq!(
                    app.take_file_actions(),
                    [crate::FileAction::ClearRecovery {
                        disabled_autosave: disable
                    }]
                );
            }
            assert_eq!(app.project(), &project);
            assert_eq!(app.document.dirty, dirty);
            assert_eq!(app.editor.session.selection, selection);
            assert!(app.editor.can_undo() && !app.editor.can_redo());
            assert!(app.simulation_mut().enabled);
            assert_eq!(app.simulation_mut().ticks, 29);
            assert!(app.take_simulation_actions().is_empty());
            app.editor.undo();
            assert_eq!(app.project(), &before_edit);
            assert!(!app.editor.can_undo());
            app.editor.redo();
            assert_eq!(app.project(), &project);
        }
    }
}

#[test]
fn ownership_error_and_retry_fit_the_recovery_card_in_both_languages() {
    use crate::tests::support::widgets::{click, has};
    for language in Language::ALL {
        let label = |key: &'static str| crate::i18n::with_language(language, || tr(key));
        for size in [vec2(720., 480.), vec2(1200., 820.)] {
            let ctx = Context::default();
            crate::configure(&ctx);
            let mut app = Workspace {
                settings_open: true,
                ..Default::default()
            };
            app.set_language(language);
            app.settings_page.select(Category::Files);
            for state in [
                SnapshotState::Available {
                    source: None,
                    saved_at: 0,
                },
                SnapshotState::Damaged,
                SnapshotState::Unavailable {
                    snapshot_present: true,
                },
            ] {
                app.set_recovery_view(RecoveryView {
                    state: state.clone(),
                    owned: false,
                    error: Some(RecoveryView::FOREIGN_OWNER.into()),
                    directory: Some("/test/recovery".into()),
                });
                let mut labels = vec![];
                for _ in 0..3 {
                    labels = frame_at_size(&ctx, &mut app, vec![], size);
                }
                let foreign = label(RecoveryView::FOREIGN_OWNER);
                assert_eq!(labels.iter().filter(|(t, _)| t == foreign).count(), 1);
                assert!(has(&labels, "/test/recovery"));
                assert!(!has(&labels, label("Clear recovery")));
                let retry = !matches!(state, SnapshotState::Available { .. });
                assert_eq!(has(&labels, label("Retry")), retry);
                let bottom = labels
                    .iter()
                    .find(|(t, _)| t == label("Close"))
                    .unwrap()
                    .1
                    .top();
                for (_, r) in labels
                    .iter()
                    .filter(|(t, _)| t == foreign || t == "/test/recovery" || t == label("Retry"))
                {
                    assert!(
                        r.left() >= 0.
                            && r.right() <= size.x
                            && r.top() > 0.
                            && r.bottom() < bottom,
                        "{language:?} {size:?} {r:?}"
                    );
                }
                if retry {
                    click(&ctx, &mut app, label("Retry"));
                    assert!(app.take_file_actions().is_empty());
                    click(&ctx, &mut app, label("Confirm clear"));
                    assert_eq!(
                        app.take_file_actions(),
                        [crate::FileAction::ClearRecovery {
                            disabled_autosave: false
                        }]
                    );
                }
            }
        }
    }
}

#[test]
fn shortcut_capture_conflicts_cancel_and_resets_keep_workspace_state() {
    use crate::{
        shortcuts::{Command, Shortcuts},
        tests::support::widgets::{click, frame, has},
    };
    use egui::{Event, PointerButton};
    let ctx = Context::default();
    crate::configure(&ctx);
    let mut app = Workspace {
        settings_open: true,
        ..Default::default()
    };
    app.settings_page.select(Category::Shortcuts);
    app.settings_page.query = "file.save".to_owned();
    let defaults = Shortcuts::default();
    let save = |app: &Workspace| app.preferences().shortcuts.get(Command::Save);
    let key = |k, mods, repeat| Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: mods,
    };
    let send = |app: &mut Workspace, k, mods| {
        let mut up = key(k, mods, false);
        if let Event::Key { pressed, .. } = &mut up {
            *pressed = false;
        }
        frame(&ctx, app, vec![key(k, mods, false), up])
    };
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    app.editor.select_all();
    app.editor.simulation.view.steps = 42;
    app.canvas.camera.pan(vec2(77., 23.));
    let selection = app.editor.session.selection.clone();
    let camera = app.canvas.camera;
    let ctrl_s = defaults.get(Command::Save).label(ctx.os().is_mac());
    frame(&ctx, &mut app, vec![key(Key::F11, Modifiers::NONE, false)]);
    click(&ctx, &mut app, &ctrl_s);
    for k in [
        Key::ControlLeft,
        Key::ShiftRight,
        Key::AltLeft,
        Key::SuperRight,
    ] {
        let labels = send(&mut app, k, Modifiers::CTRL);
        assert!(has(&labels, "Press a shortcut… Esc to cancel"));
    }
    let labels = frame(&ctx, &mut app, vec![key(Key::F11, Modifiers::NONE, true)]);
    assert!(has(&labels, "Press a shortcut… Esc to cancel"));
    assert!(has(
        &send(&mut app, Key::O, Modifiers::COMMAND),
        "Already assigned to “Open…”."
    ));
    assert!(has(
        &send(&mut app, Key::F, Modifiers::COMMAND),
        "Reserved by a fixed shortcut."
    ));
    assert_eq!(save(&app), Command::Save.default_chord());
    send(&mut app, Key::Escape, Modifiers::NONE);
    assert!(app.settings_open);
    assert!(!has(
        &frame(&ctx, &mut app, vec![]),
        "Press a shortcut… Esc to cancel"
    ));
    click(&ctx, &mut app, &ctrl_s);
    let bound = send(&mut app, Key::F5, Modifiers::NONE);
    assert!(has(&bound, "F5"));
    assert_eq!(save(&app).to_string(), "f5");
    let right = bound.iter().find(|(s, _)| s == "F5").unwrap().1;
    let pos = right.right_center() + vec2(25., 0.);
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
    assert_eq!(save(&app), Command::Save.default_chord());
    app.settings_page.select(Category::Shortcuts);
    click(&ctx, &mut app, &ctrl_s);
    send(&mut app, Key::F6, Modifiers::NONE);
    click(&ctx, &mut app, "Reset all shortcuts");
    assert_eq!(save(&app).to_string(), "f6");
    click(&ctx, &mut app, "Cancel");
    click(&ctx, &mut app, "Reset all shortcuts");
    click(&ctx, &mut app, "Confirm reset");
    assert_eq!(app.preferences().shortcuts, defaults);
    click(&ctx, &mut app, &ctrl_s);
    send(&mut app, Key::F7, Modifiers::NONE);
    assert!(!app.document.dirty && !app.editor.can_undo());
    assert_eq!(app.editor.session.selection, selection);
    assert_eq!(
        (app.canvas.camera.zoom, app.canvas.camera.offset),
        (camera.zoom, camera.offset)
    );
    assert_eq!(app.editor.simulation.view.steps, 42);
    assert!(app.take_file_actions().is_empty() && app.take_simulation_actions().is_empty());
    app.settings_page.select(Category::Advanced);
    click(&ctx, &mut app, "Reset all");
    click(&ctx, &mut app, "Confirm reset");
    assert_eq!(app.preferences().shortcuts, defaults);
}

#[test]
fn menu_and_toolbar_follow_the_effective_binding() {
    use crate::{
        shortcuts::Command,
        tests::support::widgets::{click, frame, has},
    };
    let ctx = Context::default();
    crate::configure(&ctx);
    let mut app = Workspace::default();
    app.editor.simulation.view.has_clocks = true;
    click(&ctx, &mut app, "Simulate");
    let default_tick = format!(
        "Tick once · {}",
        Command::Tick.default_chord().label(ctx.os().is_mac())
    );
    assert!(has(&frame(&ctx, &mut app, vec![]), &default_tick));
    app.preferences
        .shortcuts
        .bind(Command::Tick, "f8".parse().unwrap())
        .unwrap();
    assert!(has(&frame(&ctx, &mut app, vec![]), "Tick once · F8"));
    click(&ctx, &mut app, "Simulate");
    let buttons = frame(&ctx, &mut app, vec![]);
    let tick = buttons
        .iter()
        .find(|(s, _)| s == "Tick")
        .unwrap()
        .1
        .center();
    let mut labels = frame(&ctx, &mut app, vec![egui::Event::PointerMoved(tick)]);
    for _ in 0..75 {
        labels = frame(&ctx, &mut app, vec![]);
    }
    assert!(has(
        &labels,
        "Queue one clock tick; paused simulation uses Step or Run · F8"
    ));
}

#[test]
fn tool_search_matches_both_languages_and_restores_an_individual_binding() {
    use crate::shortcuts::Command;
    use crate::tests::support::widgets::has;
    for language in Language::ALL {
        for size in [vec2(720., 480.), vec2(1200., 820.)] {
            let ctx = Context::default();
            crate::configure(&ctx);
            let mut app = Workspace {
                settings_open: true,
                ..Default::default()
            };
            app.set_language(language);
            app.preferences
                .shortcuts
                .bind(Command::ToolPoke, "f6".parse().unwrap())
                .unwrap();
            app.preferences
                .shortcuts
                .bind(Command::ToolRotate, "f10".parse().unwrap())
                .unwrap();
            for query in ["tools", "工具", "tool.", "rotate", "旋转", "tool.rotate"] {
                app.settings_page.query = query.into();
                let mut labels = vec![];
                for _ in 0..4 {
                    labels = frame_at_size(&ctx, &mut app, vec![], size);
                }
                let group = crate::i18n::with_language(language, || tr("Tools").to_uppercase());
                assert!(has(&labels, &group), "{language:?} {size:?} {query}");
                let chord = if ["tools", "工具", "tool."].contains(&query) {
                    "F6"
                } else {
                    "F10"
                };
                let bounds = ctx
                    .memory(|m| m.area_rect(Id::unique("settings-window")))
                    .unwrap();
                let rect = labels
                    .iter()
                    .find(|(s, r)| s == chord && bounds.contains_rect(*r))
                    .unwrap()
                    .1;
                if query == "tool.rotate" {
                    let pos = egui::pos2(bounds.right() - 36., rect.center().y);
                    for pressed in [true, false] {
                        frame_at_size(
                            &ctx,
                            &mut app,
                            vec![
                                egui::Event::PointerMoved(pos),
                                egui::Event::PointerButton {
                                    pos,
                                    button: egui::PointerButton::Primary,
                                    pressed,
                                    modifiers: Modifiers::NONE,
                                },
                            ],
                            size,
                        );
                    }
                }
            }
            assert_eq!(
                app.preferences.shortcuts.get(Command::ToolRotate),
                Command::ToolRotate.default_chord()
            );
            assert_eq!(
                app.preferences.shortcuts.get(Command::ToolPoke).to_string(),
                "f6"
            );
        }
    }
}

#[test]
fn all_tool_hints_and_the_canvas_rotate_menu_show_the_remapped_chord() {
    use crate::{
        editor::Tool,
        shortcuts::Command::*,
        tests::support::widgets::{frame_at_size, has},
    };
    let ctx = Context::default();
    crate::configure(&ctx);
    ctx.global_style_mut(|style| {
        style.interaction.tooltip_delay = 0.;
        style.interaction.show_tooltips_only_when_still = false;
    });
    let size = vec2(1200., 820.);
    let mut app = Workspace::default();
    app.editor.set_tool(Tool::Place(logisim_core::Kind::Tunnel));
    for (command, chord, label, x) in [
        (ToolSelect, "f5", "Select · F5", 53.),
        (
            ToolPoke,
            "f6",
            "Poke · toggle inputs / inspect signals · F6",
            22.,
        ),
        (ToolWire, "f7", "Wire · drag or click two points · F7", 86.),
        (ToolText, "f8", "Text · F8", 116.),
        (ToolTunnel, "f9", "Tunnel · F9", 192.),
        (ToolRotate, "f10", "Rotate clockwise · F10", 351.),
    ] {
        app.preferences
            .shortcuts
            .bind(command, chord.parse().unwrap())
            .unwrap();
        frame_at_size(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(egui::pos2(x, 55.))],
            size,
        );
        let mut labels = vec![];
        for _ in 0..4 {
            labels = frame_at_size(&ctx, &mut app, vec![], size);
        }
        assert!(has(&labels, label), "{label}");
    }
    let pos = app.canvas.rect.center();
    for pressed in [true, false] {
        frame_at_size(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
    assert!(has(
        &frame_at_size(&ctx, &mut app, vec![], size),
        "Rotate · F10"
    ));
    // Give the Inspector enough vertical space to display its rotate action.
    let large = vec2(1200., 1600.);
    frame_at_size(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(egui::pos2(650., 350.))],
        large,
    );
    assert!(has(
        &frame_at_size(&ctx, &mut app, vec![], large),
        "Rotate clockwise · F10"
    ));
}

#[test]
fn shortcut_search_and_conflict_fit_the_viewport_in_both_languages() {
    use crate::{shortcuts::Command, tests::support::widgets::click};
    for language in Language::ALL {
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(language);
        for size in [vec2(720., 480.), vec2(1200., 820.)] {
            app.settings_page.select(Category::General);
            // Search by stable ID from a different category.
            app.settings_page.query = "project.toggle_appearance".into();
            let current = Command::ToggleAppearance
                .default_chord()
                .label(ctx.os().is_mac());
            click(&ctx, &mut app, &current);
            let key = |pressed| egui::Event::Key {
                key: Key::Y,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            };
            frame_at_size(&ctx, &mut app, vec![key(true), key(false)], size);
            let labels = frame_at_size(&ctx, &mut app, vec![], size);
            crate::i18n::with_language(language, || {
                let locate = |label: &str| {
                    labels
                        .iter()
                        .rev()
                        .find(|(s, _)| s == label)
                        .unwrap_or_else(|| panic!("Missing {label} at {size:?} in {language:?}"))
                        .1
                };
                let error = locate(tr("Reserved by a fixed shortcut."));
                let bounds = ctx
                    .memory(|m| m.area_rect(Id::unique("settings-window")))
                    .unwrap();
                assert!(bounds.contains_rect(error) && error.bottom() < locate(tr("Close")).top());
                assert!(bounds.contains_rect(locate(&current)));
                assert!(
                    locate(tr(Command::ToggleAppearance.label())).right() < locate(&current).left()
                );
            });
        }
    }
}

#[test]
fn editor_default_rows_reset_at_both_window_sizes_without_editing_the_circuit() {
    use crate::tests::support::widgets::click;
    use egui::{Event, PointerButton};
    use logisim_core::{Component, GateSize, Kind, Point, Project};
    for language in Language::ALL {
        for size in [vec2(720., 480.), vec2(1200., 820.)] {
            let ctx = Context::default();
            crate::configure(&ctx);
            let mut app = Workspace {
                settings_open: true,
                ..Default::default()
            };
            let mut project = Project::default();
            let id = project
                .main_circuit_mut()
                .circuit
                .add_component(Component::new(Kind::Tunnel, Point::ZERO));
            app.replace_project(project.clone());
            app.editor.session.selection.insert(id);
            app.apply_settings(Settings {
                language,
                default_gate_size: GateSize::Wide,
                default_tunnel_width: 17,
                remember_last_properties: true,
                ..app.preferences()
            });
            app.simulation_mut().steps = 39;
            let t = |label| crate::i18n::with_language(language, || tr(label));
            for (key, value) in [
                ("editor.default_gate_size", t("Wide")),
                ("editor.default_tunnel_width", "17"),
                ("editor.remember_last_properties", t("On")),
            ] {
                app.settings_page.query = key.into();
                let mut labels = vec![];
                for _ in 0..4 {
                    labels = frame_at_size(&ctx, &mut app, vec![], size);
                }
                let bounds = ctx
                    .memory(|m| m.area_rect(Id::unique("settings-window")))
                    .unwrap();
                let value_rect = labels
                    .iter()
                    .rev()
                    .find(|(s, r)| s == value && bounds.contains_rect(*r))
                    .unwrap()
                    .1;
                let pos = egui::pos2(bounds.right() - 36., value_rect.center().y);
                let help = t(REGISTRY.iter().find(|entry| entry.key == key).unwrap().help);
                let help_rect = labels.iter().find(|(text, _)| text == help).unwrap().1;
                assert!(bounds.contains_rect(help_rect));
                assert!(
                    help_rect.right() < value_rect.left(),
                    "{language:?} {size:?} {key}"
                );
                for pressed in [true, false] {
                    frame_at_size(
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
                        size,
                    );
                }
                let values = app.preferences();
                match key {
                    "editor.default_gate_size" => assert_eq!(
                        (
                            values.default_gate_size,
                            values.default_tunnel_width,
                            values.remember_last_properties
                        ),
                        (GateSize::Narrow, 17, true)
                    ),
                    "editor.default_tunnel_width" => assert_eq!(
                        (values.default_tunnel_width, values.remember_last_properties),
                        (1, true)
                    ),
                    _ => assert!(!values.remember_last_properties),
                }
            }
            app.apply_settings(Settings {
                default_gate_size: GateSize::Medium,
                default_tunnel_width: 12,
                remember_last_properties: true,
                ..app.preferences()
            });
            app.settings_page.select(Category::Advanced);
            click(&ctx, &mut app, t("Reset all"));
            click(&ctx, &mut app, t("Confirm reset"));
            assert_eq!(app.preferences(), Settings::default());
            assert_eq!(app.project(), &project);
            assert_eq!(
                app.editor.session.selection,
                std::collections::BTreeSet::from([id])
            );
            assert!(!app.document.dirty && !app.editor.can_undo());
            assert_eq!(app.simulation_mut().steps, 39);
            assert!(app.take_simulation_actions().is_empty());
        }
    }
}

#[test]
fn leaving_a_shortcut_button_returns_input_to_search_and_other_settings() {
    use crate::{
        shortcuts::{Command, Shortcuts},
        tests::support::widgets::{click, frame, has},
    };
    use egui::Event;
    let ctx = Context::default();
    crate::configure(&ctx);
    let mut app = Workspace {
        settings_open: true,
        ..Default::default()
    };
    app.settings_page.select(Category::Shortcuts);
    let save = Command::Save.default_chord().label(ctx.os().is_mac());
    let tick = Command::Tick.default_chord().label(ctx.os().is_mac());
    let key = |key, modifiers| Event::Key {
        key,
        modifiers,
        pressed: true,
        repeat: false,
        physical_key: None,
    };
    click(&ctx, &mut app, &save);
    click(&ctx, &mut app, "Search settings · Ctrl+F");
    frame(
        &ctx,
        &mut app,
        vec![key(Key::F, Modifiers::SHIFT), Event::Text("F".into())],
    );
    frame(
        &ctx,
        &mut app,
        vec![key(Key::Num5, Modifiers::NONE), Event::Text("5".into())],
    );
    assert_eq!(app.settings_page.query, "F5");
    assert_eq!(app.preferences().shortcuts, Shortcuts::default());

    app.settings_page.query = "simulation.tick".into();
    click(&ctx, &mut app, &tick);
    click(&ctx, &mut app, "Tick frequency");
    let labels = frame(&ctx, &mut app, vec![key(Key::F8, Modifiers::NONE)]);
    assert!(!has(&labels, "Press a shortcut… Esc to cancel"));
    assert_eq!(app.preferences().shortcuts, Shortcuts::default());

    app.settings_page.select(Category::Shortcuts);
    click(&ctx, &mut app, &save);
    let open = Command::Open.default_chord().label(ctx.os().is_mac());
    click(&ctx, &mut app, &open);
    frame(&ctx, &mut app, vec![key(Key::F7, Modifiers::NONE)]);
    assert_eq!(
        app.preferences().shortcuts.get(Command::Save),
        Command::Save.default_chord()
    );
    assert_eq!(
        app.preferences().shortcuts.get(Command::Open).to_string(),
        "f7"
    );
    click(&ctx, &mut app, &save);
    click(&ctx, &mut app, "Reset all shortcuts");
    frame(&ctx, &mut app, vec![key(Key::F6, Modifiers::NONE)]);
    assert!(app.settings_page.bindings.confirm_reset);
    click(&ctx, &mut app, "Confirm reset");
    assert_eq!(app.preferences().shortcuts, Shortcuts::default());
}

#[test]
fn shortcut_capture_ends_on_window_or_keyboard_focus_loss() {
    use crate::{
        shortcuts::{Command, Shortcuts},
        tests::support::widgets::{click, frame, has},
    };
    let ctx = Context::default();
    crate::configure(&ctx);
    let mut app = Workspace {
        settings_open: true,
        ..Default::default()
    };
    app.settings_page.select(Category::Shortcuts);
    let save = Command::Save.default_chord().label(ctx.os().is_mac());
    for window_loss in [false, true] {
        click(&ctx, &mut app, &save);
        assert!(has(
            &frame(&ctx, &mut app, vec![]),
            "Press a shortcut… Esc to cancel"
        ));
        if window_loss {
            let _ = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
        } else {
            ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
        }
        let labels = frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: Key::F9,
                modifiers: Modifiers::NONE,
                physical_key: None,
                pressed: true,
                repeat: false,
            }],
        );
        assert!(!has(&labels, "Press a shortcut… Esc to cancel"));
        assert_eq!(app.preferences().shortcuts, Shortcuts::default());
        assert!(app.settings_open);
    }
}

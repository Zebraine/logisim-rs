use super::*;
use crate::tests::support::widgets::frame_at_size;

#[test]
fn registry_restores_each_field_without_changing_other_preferences() {
    let defaults = Settings::default();
    let alternative = Settings {
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
                    bounds.width() <= screen.x * 0.9 + 1. && bounds.height() <= screen.y * 0.9 + 1.,
                    "{cat:?} {screen} {bounds:?}"
                );
                assert!(
                    (bounds.center().to_vec2() - screen * 0.5).length() <= 1.,
                    "off-center {cat:?} {screen} {bounds:?}"
                );
                let close = crate::i18n::with_language(lang, || tr("Close"));
                let close_rect = labels.iter().rev().find(|(s, _)| s == close).unwrap().1;
                assert!(bounds.contains_rect(close_rect));
                assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, screen).contains_rect(bounds));
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
fn editor_keys_find_the_same_control_in_both_languages() {
    use crate::tests::support::widgets::{frame, has};
    for language in Language::ALL {
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(language);
        for entry in REGISTRY.iter().filter(|s| s.category == Category::Editor) {
            app.settings_page.query = entry.key.into();
            for _ in 0..2 {
                frame(&ctx, &mut app, vec![]);
            }
            let labels = frame(&ctx, &mut app, vec![]);
            let label = crate::i18n::with_language(language, || tr(entry.label));
            assert!(has(&labels, label), "{}", entry.key);
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
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            settings_open: true,
            ..Default::default()
        };
        app.set_language(lang);
        app.settings_page.select(category);
        let size = vec2(720., 480.);
        let entries: Vec<_> = REGISTRY.iter().filter(|e| e.category == category).collect();
        let mut visible = std::collections::BTreeSet::new();
        for step in 0..18 {
            let mut events = vec![Event::PointerMoved(egui::pos2(420., 330.))];
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
            let picker_rect = labels
                .iter()
                .find(|(s, r)| s == picker && r.center().y > 100.)
                .unwrap()
                .1;
            for e in &entries {
                let label = crate::i18n::with_language(lang, || tr(e.label));
                if labels.iter().any(|(s, r)| {
                    s == label
                        && r.top() > picker_rect.bottom()
                        && r.bottom() < close_rect.top()
                        && r.left() >= 0.
                        && r.right() <= size.x
                }) {
                    visible.insert(e.key);
                }
            }
        }
        assert_eq!(visible.len(), entries.len(), "{lang:?}: {visible:?}");
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
                assert_eq!(app.take_file_actions(), [crate::FileAction::ClearRecovery]);
            }
            frame(&ctx, &mut app, vec![]);
            assert_eq!(app.project(), &project);
            assert!(!app.editor.can_undo());
            assert_eq!(app.simulation_mut().ticks, 31);
            assert!(app.take_simulation_actions().is_empty());
        }
    }
}

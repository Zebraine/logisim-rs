use super::*;
use crate::tests::support::widgets::frame_at_size;

#[test]
fn registry_restores_each_field_without_changing_other_preferences() {
    let defaults = Settings::default();
    let alternative = Settings {
        language: Language::SimplifiedChinese,
        ui_scale: UiScale::P150,
        reduce_motion: true,
        show_telemetry: false,
        show_grid: false,
        default_zoom: DefaultZoom::Actual,
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

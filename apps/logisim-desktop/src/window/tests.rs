use super::*;
use eframe::egui::Rect;

#[test]
fn bounds_center_missing_or_unreachable_positions_and_keep_valid_positions() {
    let monitor = vec2(1024., 768.);
    let centered = WindowState {
        width: 20.,
        height: -2.,
        x: Some(10000.),
        y: Some(-100.),
        ..Default::default()
    }
    .bounded(Some(monitor));
    assert_eq!(centered.size(), MIN);
    assert_eq!(centered.position(), Some(pos2(152., 144.)));
    assert_eq!(
        WindowState::default().bounded(Some(monitor)).size(),
        monitor
    );
    let visible = WindowState {
        x: Some(56.),
        y: Some(28.),
        ..centered
    };
    assert_eq!(visible.bounded(Some(monitor)), visible);
    assert!(WindowState::default().bounded(None).position().is_none());
}

#[test]
fn maximized_keeps_normal_rectangle_and_positionless_backend_captures_size() {
    let mut normal = WindowState::default();
    let mut info = ViewportInfo {
        outer_rect: Some(Rect::from_min_size(pos2(32., 56.), MIN)),
        ..Default::default()
    };
    normal.observe(&info, vec2(840., 520.), 1.25);
    let original = normal;
    assert_eq!(normal.size(), vec2(1050., 650.));
    info.maximized = Some(true);
    normal.observe(&info, vec2(1600., 900.), 1.);
    assert_eq!(
        normal,
        WindowState {
            maximized: true,
            ..original
        }
    );
    normal.observe(
        &ViewportInfo {
            maximized: Some(false),
            ..Default::default()
        },
        MIN,
        1.,
    );
    assert_eq!(normal.size(), MIN);
    assert_eq!(normal.position(), None);
}

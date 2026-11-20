use super::*;

fn assert_near(a: Vec2, b: Vec2) {
    assert!((a - b).length() < 0.001, "{a:?} != {b:?}");
}

#[test]
fn zoom_keeps_world_point_under_pointer_even_at_limits() {
    let mut camera = Camera::default();
    camera.pan(vec2(-320.0, 85.0));
    let pointer = vec2(381.0, 212.0);
    let world = camera.world_at(pointer);
    for target in [1.1, 2.0, 100.0, 0.001, 1.0] {
        camera.zoom_at(target, pointer);
        assert_near(camera.world_at(pointer), world);
        assert!((MIN_ZOOM..=MAX_ZOOM).contains(&camera.zoom));
    }
}

#[test]
fn drag_distance_is_in_screen_points_at_every_zoom() {
    for zoom in [MIN_ZOOM, 1.0, MAX_ZOOM] {
        let mut camera = Camera {
            zoom,
            ..Camera::default()
        };
        let before = camera.offset;
        camera.pan(vec2(100.0, -50.0));
        assert_near(camera.offset - before, vec2(100.0, -50.0));
    }
}

#[test]
fn invalid_input_does_not_poison_camera() {
    let mut camera = Camera::default();
    camera.zoom_at(f32::NAN, Vec2::ZERO);
    camera.pan(vec2(f32::INFINITY, 0.0));
    assert_eq!(camera.zoom, 1.0);
    assert!(camera.offset.is_finite());
}

#[test]
fn toggling_origin_changes_only_the_two_cross_strokes() {
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    canvas.camera.zoom_at(1.75, vec2(0., 0.));
    canvas.camera.pan(vec2(11., 19.));
    let camera = canvas.camera;
    for (theme, show_grid) in [crate::ThemeMode::Light, crate::ThemeMode::Dark]
        .into_iter()
        .flat_map(|mode| [false, true].map(|grid| (mode, grid)))
    {
        crate::Settings {
            theme,
            ..Default::default()
        }
        .apply_appearance(&ctx);
        let p = crate::theme::palette(&ctx);
        let mut render = |show_origin_marker| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(240., 200.))),
                    ..Default::default()
                },
                |ui| {
                    canvas.show(
                        ui,
                        &crate::Settings {
                            show_grid,
                            show_origin_marker,
                            ..Default::default()
                        },
                    );
                },
            );
            out.textures_delta.clear();
            out.shapes.into_iter().map(|s| s.shape).collect::<Vec<_>>()
        };
        let off = render(false);
        assert!(
            off.iter()
                .any(|s| matches!(s, egui::Shape::Rect(r) if r.fill == p.canvas))
        );
        assert_eq!(off.iter().any(|s| matches!(s, egui::Shape::Mesh(m) if !m.vertices.is_empty() && m.vertices.iter().all(|v| v.color == p.grid()))),show_grid);
        let mut on = render(true);
        let origin = canvas.screen(logisim_core::Point::ZERO);
        for delta in [vec2(0., 3.), vec2(3., 0.)] {
            assert_eq!(
                on.pop(),
                Some(egui::Shape::line_segment(
                    [origin - delta, origin + delta],
                    egui::Stroke::new(1., p.faint)
                ))
            );
        }
        assert_eq!(on, off);
        assert_eq!(canvas.camera.zoom, camera.zoom);
        assert_eq!(canvas.camera.offset, camera.offset);
        assert_near(canvas.world(origin), Vec2::ZERO);
    }
}

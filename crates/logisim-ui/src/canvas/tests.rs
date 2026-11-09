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

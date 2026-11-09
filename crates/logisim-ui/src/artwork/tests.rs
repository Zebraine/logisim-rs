use super::*;
#[test]
fn concave_fill_and_hit_testing_preserve_the_empty_notch_in_both_windings() {
    let points = vec![
        Point::new(0, 0),
        Point::new(10, 0),
        Point::new(10, 4),
        Point::new(4, 4),
        Point::new(4, 10),
        Point::new(0, 10),
    ];
    for reverse in [false, true] {
        let mut points = points.clone();
        if reverse {
            points.reverse();
        }
        let shape = Artwork {
            id: 1,
            geometry: Geometry::Poly {
                points,
                closed: true,
            },
            paint: logisim_core::Paint {
                stroke: None,
                fill: Some([10, 20, 30]),
                width: 1,
            },
        };
        let (outline, _) = outline(&shape.geometry);
        let screen: Vec<_> = outline.into_iter().map(Vec2::to_pos2).collect();
        let area: f32 = triangles(&screen)
            .into_iter()
            .map(|[a, b, c]| cross(screen[b] - screen[a], screen[c] - screen[a]).abs() / 2.0)
            .sum();
        assert!((area - 64.0).abs() < 0.001);
        assert!(hit(&shape, vec2(2.0, 8.0), 0.0));
        assert!(!hit(&shape, vec2(8.0, 8.0), 0.0));
    }
}

use super::*;
use crate::simulation::{SignalTone, SignalView};

fn white_fill(c: &Component, zoom: f32, ghost: bool) -> (Canvas, Vec<[Pos2; 3]>) {
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    canvas.camera.zoom = zoom;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        canvas.rect = ui.max_rect();
        component(ui.painter(), &canvas, c, false, ghost, None);
    });
    output.textures_delta.clear();
    let mut triangles = vec![];
    for primitive in ctx.tessellate(output.shapes, output.pixels_per_point) {
        if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
            for &[a, b, c] in mesh.indices.as_chunks::<3>().0 {
                let vertices = [a, b, c].map(|i| mesh.vertices[i as usize]);
                if vertices.iter().all(|v| v.color == Color32::WHITE) {
                    triangles.push(vertices.map(|v| v.pos));
                }
            }
        }
    }
    (canvas, triangles)
}

fn covered(triangles: &[[Pos2; 3]], point: Pos2) -> bool {
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    triangles.iter().any(|&[a, b, c]| {
        if cross(b - a, c - a).abs() < 0.0001 {
            return false;
        }
        let sides = [
            cross(b - a, point - a),
            cross(c - b, point - b),
            cross(a - c, point - c),
        ];
        sides.iter().all(|s| *s >= 0.0) || sides.iter().all(|s| *s <= 0.0)
    })
}

#[test]
fn open_symbols_do_not_erase_nearby_labels_with_a_background() {
    for kind in [
        Kind::Splitter,
        Kind::Power,
        Kind::Ground,
        Kind::PullResistor,
        Kind::Transistor,
    ] {
        for facing in Facing::ALL {
            let mut c = Component::new(kind, Point::new(200, 200));
            c.facing = facing;
            c.transistor.transistor_type = logisim_core::TransistorType::N;
            let (_, triangles) = white_fill(&c, 1.0, false);
            assert!(
                triangles.is_empty(),
                "{kind:?} {facing:?} has an opaque background"
            );
        }
    }
}

#[test]
fn gate_fill_preserves_empty_corners_and_concave_notches_when_rotated() {
    for gate in [Gate::Not, Gate::And, Gate::Or, Gate::Xor] {
        for facing in Facing::ALL {
            for zoom in [0.5, 1.0, 2.0] {
                let mut c = Component::new(Kind::Gate(gate), Point::new(200, 200));
                c.facing = facing;
                let scale = c.gate_depth() as f32 / 60.0;
                let (canvas, triangles) = white_fill(&c, zoom, false);
                let at = |x, y| canvas.screen_vec(local(&c, vec2(x * scale, y)));
                assert!(
                    covered(&triangles, at(-35.0, 0.0)),
                    "missing body fill: {gate:?}"
                );
                assert!(
                    !covered(&triangles, at(-15.0, half_height(&c) - 2.0)),
                    "filled empty corner: {gate:?}"
                );
                if matches!(gate, Gate::Or | Gate::Xor) {
                    assert!(
                        !covered(&triangles, at(-48.0, 0.0)),
                        "filled concave notch: {gate:?}"
                    );
                }
                let (_, ghost_fill) = white_fill(&c, zoom, true);
                assert!(
                    ghost_fill.is_empty(),
                    "placement preview should stay transparent"
                );
            }
        }
    }
}

#[test]
fn unselected_open_ends_have_signal_colored_dots_without_marking_elbows() {
    let mut c = Circuit::default();
    c.add_wire(Point::new(50, 50), Point::new(150, 50));
    c.add_wire(Point::new(150, 50), Point::new(150, 100));
    let mut view = SimulationView::default();
    for w in &c.wires {
        view.wires.insert(
            w.id,
            SignalView {
                text: "1".into(),
                binary: "1".into(),
                tone: SignalTone::High,
            }
            .into(),
        );
    }
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    let mut output = ctx.run_ui(Default::default(), |ui| {
        canvas.rect = ui.max_rect();
        circuit(
            ui.painter(),
            &canvas,
            &c,
            &BTreeSet::new(),
            &view,
            &mut CircuitCache::default(),
        );
    });
    output.textures_delta.clear();
    let dots: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Circle(circle)
                if circle.radius == 3.0 && circle.fill == SignalTone::High.color() =>
            {
                Some(circle.center)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        dots,
        vec![
            canvas.screen(Point::new(50, 50)),
            canvas.screen(Point::new(150, 100))
        ]
    );
}

use super::*;
use crate::{
    editor::Editor,
    i18n::{Language, with_language},
};

fn markers(c: &Circuit) -> Vec<(Point, Option<Id>)> {
    c.junctions()
        .into_iter()
        .chain(c.dangling_ends())
        .map(|p| (p, c.wires.iter().find(|w| w.contains(p)).map(|w| w.id)))
        .collect()
}

#[test]
fn geometry_tracks_gestures_history_replacement_and_tunnel_renaming() {
    let mut editor = Editor::default();
    let a = editor
        .session
        .circuit
        .add_wire(Point::ZERO, Point::new(100, 0))
        .unwrap();
    let b = editor
        .session
        .circuit
        .add_wire(Point::new(300, 0), Point::new(400, 0))
        .unwrap();
    for x in [0, 300] {
        let mut tunnel = Component::new(Kind::Tunnel, Point::new(x, 0));
        tunnel.label = "bus".into();
        editor.session.circuit.add_component(tunnel);
    }
    let mut cache = CircuitCache::default();
    let selected = BTreeSet::from([a]);
    cache.synchronize(&editor.session.circuit, &selected);
    assert_eq!(cache.markers, markers(&editor.session.circuit));
    assert!(cache.selected_net.contains(&b));
    let marker_allocation = cache.markers.as_ptr();
    editor.session.circuit.components[1].label = "separate".into();
    cache.synchronize(&editor.session.circuit, &selected);
    assert_eq!(
        cache.markers.as_ptr(),
        marker_allocation,
        "labels do not rebuild connection geometry"
    );
    assert!(!cache.selected_net.contains(&b));
    let before = editor.snapshot();
    editor.session.circuit.components[0].position = Point::new(50, 0);
    cache.synchronize(&editor.session.circuit, &selected); // before gesture commit
    assert_eq!(cache.markers, markers(&editor.session.circuit));
    assert!(cache.markers.iter().any(|(p, _)| *p == Point::new(50, 0)));
    editor.record(before);
    editor.undo();
    cache.synchronize(&editor.session.circuit, &selected);
    assert_eq!(cache.markers, markers(&editor.session.circuit));
    assert!(!cache.markers.iter().any(|(p, _)| *p == Point::new(50, 0)));
    editor.redo();
    cache.synchronize(&editor.session.circuit, &selected);
    assert_eq!(cache.markers, markers(&editor.session.circuit));
    editor.session.circuit.remove(&BTreeSet::from([a]));
    cache.synchronize(&editor.session.circuit, &selected);
    assert_eq!(cache.markers, markers(&editor.session.circuit));
    let mut replacement = Circuit::default();
    replacement.add_wire(Point::new(50, 300), Point::new(100, 300));
    cache.synchronize(&replacement, &selected); // reuses ID, different circuit
    assert_eq!(cache.markers, markers(&replacement));
    cache.synchronize(&Circuit::default(), &BTreeSet::new());
    assert!(cache.markers.is_empty());
    assert!(cache.visual_bounds.is_empty());
    assert!(cache.selected_net.is_empty());
}

fn frame(
    ctx: &egui::Context,
    cache: &mut CircuitCache,
    c: &Circuit,
    canvas: &mut Canvas,
    selected: &BTreeSet<Id>,
    view: &SimulationView,
) -> Vec<Shape> {
    let mut result = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
            ..Default::default()
        },
        |ui| {
            canvas.rect = ui.max_rect();
            super::super::circuit(ui.painter(), canvas, c, selected, view, cache);
        },
    );
    result.textures_delta.clear();
    result
        .shapes
        .into_iter()
        .filter(|s| s.shape.visual_bounding_rect().intersects(s.clip_rect))
        .map(|s| s.shape)
        .collect()
}

#[test]
fn viewport_cache_preserves_visible_shapes_when_panning_zooming_and_editing() {
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    let mut circuit = Circuit::default();
    for (i, facing) in Facing::ALL.into_iter().enumerate() {
        let mut tunnel = Component::new(Kind::Tunnel, Point::new(550, i as i32 * 110));
        tunnel.facing = facing;
        tunnel.label = "WWWW wide label\n第二行 ABC".into();
        tunnel.font.size = 35;
        tunnel.font.bold = true;
        tunnel.font.italic = true;
        circuit.add_component(tunnel);
    }
    let mut text = Component::new(Kind::Text, Point::new(-100, 100));
    text.label = "Long label whose anchor is outside the viewport".into();
    text.font.size = 24;
    circuit.add_component(text);
    let mut gate = Component::new(Kind::Gate(Gate::And), Point::new(300, 500));
    gate.label = "far away label\nwith multiple lines\nreaching the viewport".into();
    gate.font.size = 70;
    circuit.add_component(gate);
    circuit.add_component(Component::new(Kind::Tunnel, Point::new(400, 100)));
    circuit.add_component(Component::new(Kind::Tunnel, Point::new(5000, 5000)));
    circuit.add_wire(Point::new(-1000, 20), Point::new(2000, 20));
    circuit.add_wire(Point::new(2000, 20), Point::new(2000, 300));
    let mut cache = CircuitCache::default();
    let mut selected = BTreeSet::new();
    let mut view = SimulationView::default();
    for language in Language::ALL {
        with_language(language, || {
            for zoom in [0.2, 1.0, 2.0, 4.0] {
                canvas.camera.zoom = zoom;
                for offset in [vec2(0.0, 0.0), vec2(-450.0, -80.0), vec2(150.0, 30.0)] {
                    canvas.camera.offset = offset;
                    frame(&ctx, &mut cache, &circuit, &mut canvas, &selected, &view);
                    let cached = frame(&ctx, &mut cache, &circuit, &mut canvas, &selected, &view);
                    let fresh = frame(
                        &ctx,
                        &mut CircuitCache::default(),
                        &circuit,
                        &mut canvas,
                        &selected,
                        &view,
                    );
                    assert_same(&cached, &fresh);
                }
                selected = circuit.components.iter().map(|c| c.id).collect();
                let signal = std::sync::Arc::new(crate::SignalView {
                    text: "1".into(),
                    binary: "1".into(),
                    tone: crate::SignalTone::High,
                });
                view.ports = circuit
                    .components
                    .iter()
                    .map(|c| ((c.id, 0), signal.clone()))
                    .collect();
                // Selection and signal color must update even when geometry is reused.
                let cached = frame(&ctx, &mut cache, &circuit, &mut canvas, &selected, &view);
                let fresh = frame(
                    &ctx,
                    &mut CircuitCache::default(),
                    &circuit,
                    &mut canvas,
                    &selected,
                    &view,
                );
                assert_same(&cached, &fresh);
            }
        });
    }
    circuit.components[7].position = Point::new(300, 100);
    circuit.components[7].label = "Moved into view".into();
    circuit.components[7].font.size = 45;
    canvas.camera.offset = vec2(0.0, 0.0);
    canvas.camera.zoom = 1.0;
    let cached = frame(&ctx, &mut cache, &circuit, &mut canvas, &selected, &view);
    let fresh = frame(
        &ctx,
        &mut CircuitCache::default(),
        &circuit,
        &mut canvas,
        &selected,
        &view,
    );
    assert_same(&cached, &fresh);
    ctx.set_pixels_per_point(2.0);
    ctx.set_fonts(egui::FontDefinitions::empty());
    // egui applies a pixel-scale change to the viewport on its first pass.
    frame(&ctx, &mut cache, &circuit, &mut canvas, &selected, &view);
    let cached = frame(&ctx, &mut cache, &circuit, &mut canvas, &selected, &view);
    let fresh = frame(
        &ctx,
        &mut CircuitCache::default(),
        &circuit,
        &mut canvas,
        &selected,
        &view,
    );
    assert_same(&cached, &fresh);
}

#[test]
fn custom_artwork_and_changing_plugin_graphics_survive_cached_frames() {
    use logisim_core::{Appearance, ArtworkFont, Geometry, Paint, TextAnchor, plugin::api};
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    let mut c = Circuit::default();
    let mut subcircuit = Component::new(Kind::Subcircuit(10), Point::new(450, 200));
    let mut appearance = Appearance {
        shapes: vec![],
        ports: Default::default(),
        anchor: Point::ZERO,
        facing: Facing::East,
    };
    appearance.add(
        Geometry::Text {
            at: Point::ZERO,
            text: "Wide custom artwork".into(),
            font: ArtworkFont {
                size: 55,
                bold: true,
                ..Default::default()
            },
            anchor: TextAnchor::End,
        },
        Paint::default(),
    );
    let (min, max) = appearance.bounds();
    subcircuit.interface.min = min;
    subcircuit.interface.max = max;
    subcircuit.interface.appearance = Some(appearance);
    c.add_component(subcircuit);
    let manifest = api::Manifest {
        abi: 1,
        id: "test.render".into(),
        version: "1".into(),
        name: "Render".into(),
        tools: vec![api::Tool {
            id: "display".into(),
            name: "Display".into(),
            bounds: [[0, 0], [300, 100]],
            ports: vec![],
            graphics: vec![],
            properties: vec![],
            actions: vec![],
        }],
    };
    manifest.validate().unwrap();
    let plugin = c.add_component(Component::from_plugin(
        manifest.components().remove(0),
        Point::new(100, 100),
    ));
    let mut view = SimulationView::default();
    let mut cache = CircuitCache::default();
    let selected = BTreeSet::new();
    frame(&ctx, &mut cache, &c, &mut canvas, &selected, &view);
    for text in ["short", "a growing dynamic display"] {
        view.plugin_graphics.insert(
            plugin,
            vec![api::Shape::Text {
                at: [0, 0],
                text: text.into(),
                size: 16,
                color: [0, 0, 0, 255],
            }],
        );
        let cached = frame(&ctx, &mut cache, &c, &mut canvas, &selected, &view);
        let fresh = frame(
            &ctx,
            &mut CircuitCache::default(),
            &c,
            &mut canvas,
            &selected,
            &view,
        );
        assert_same(&cached, &fresh);
        assert!(
            cached
                .iter()
                .any(|s| matches!(s, Shape::Text(t) if t.galley.job.text == text))
        );
    }
}

fn assert_same(cached: &[Shape], fresh: &[Shape]) {
    assert_eq!(cached.len(), fresh.len(), "visible shape count");
    for (i, (a, b)) in cached.iter().zip(fresh).enumerate() {
        if let (Shape::Text(a), Shape::Text(b)) = (a, b) {
            assert_eq!(a.pos, b.pos, "text {i} origin");
            assert_eq!(a.angle, b.angle, "text {i} angle");
            assert_eq!(a.galley.job, b.galley.job, "text {i} content");
            assert_eq!(a.galley.rect, b.galley.rect, "text {i} layout");
            assert_eq!(
                a.galley.mesh_bounds, b.galley.mesh_bounds,
                "text {i} visual extent"
            );
        } else {
            assert_eq!(a, b, "shape {i}");
        }
    }
}

#[test]
fn an_invisible_pass_does_not_hide_labels_when_the_canvas_reappears() {
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    let mut c = Circuit::default();
    let mut gate = Component::new(Kind::Gate(Gate::And), Point::new(600, 100));
    gate.label = "A visible label from a body outside the viewport".into();
    gate.font.size = 60;
    c.add_component(gate);
    let mut cache = CircuitCache::default();
    let selected = BTreeSet::new();
    let view = SimulationView::default();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
            ..Default::default()
        },
        |ui| {
            canvas.rect = ui.max_rect();
            let mut painter = ui.painter().clone();
            painter.set_opacity(0.0);
            super::super::circuit(&painter, &canvas, &c, &selected, &view, &mut cache);
        },
    );
    output.textures_delta.clear();
    let cached = frame(&ctx, &mut cache, &c, &mut canvas, &selected, &view);
    let fresh = frame(
        &ctx,
        &mut CircuitCache::default(),
        &c,
        &mut canvas,
        &selected,
        &view,
    );
    assert_same(&cached, &fresh);
    assert!(cached.iter().any(|s| matches!(s, Shape::Text(_))));
}

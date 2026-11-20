use super::*;
use crate::simulation::{SignalTone, SignalView};

fn white_fill(c: &Component, zoom: f32, ghost: bool) -> (Canvas, Vec<[Pos2; 3]>) {
    let ctx = egui::Context::default();
    let mut canvas = Canvas::default();
    canvas.camera.zoom = zoom;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        canvas.rect = ui.max_rect();
        component(ui.painter(), &canvas, c, false, ghost, None, None);
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
            crate::Settings::default().editor_display(),
        );
    });
    output.textures_delta.clear();
    let dots: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Circle(circle)
                if circle.radius == 3.0
                    && circle.fill == crate::theme::signal(&ctx, SignalTone::High) =>
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

#[test]
fn signal_patterns_distinguish_faults_even_without_color() {
    use SignalTone::*;
    for tone in [Low, High, Bus, Unknown] {
        assert!(dash_pattern(tone).is_none());
    }
    let (error, _) = dash_pattern(Error).unwrap();
    let (mismatch, _) = dash_pattern(WidthMismatch).unwrap();
    assert_eq!(error.len(), 1);
    assert!(mismatch.windows(2).any(|pair| pair[0] != pair[1]));
}

#[test]
fn live_component_ink_tracks_palette_and_preserves_document_artwork() {
    use crate::SignalPalette;
    let ctx = egui::Context::default();
    let canvas = Canvas {
        rect: Rect::from_min_size(Pos2::ZERO, vec2(500., 400.)),
        ..Default::default()
    };
    let custom_lamp = Color32::from_rgb(27, 165, 220);
    let label_color = Color32::from_rgb(139, 10, 147);
    for kind in [
        Kind::Led,
        Kind::SevenSegment,
        Kind::HexDisplay,
        Kind::Probe,
        Kind::Pin,
        Kind::Register,
        Kind::Latch(logisim_core::Latch::D),
        Kind::Ram,
    ] {
        let mut c = Component::new(kind, Point::new(200, 160));
        c.label = "document label".into();
        c.color = [139, 10, 147];
        c.bits = 1;
        c.io.on_color = custom_lamp.to_array();
        let mut view = SimulationView::default();
        let high = SignalView {
            text: "1".into(),
            binary: "1".into(),
            tone: SignalTone::High,
        };
        for (index, _) in c.ports().iter().enumerate() {
            let mut signal = high.clone();
            if kind == Kind::HexDisplay && index == 0 {
                signal.tone = SignalTone::Bus;
            }
            view.ports.insert((c.id, index), signal.into());
        }
        view.indicators.insert(c.id, [true; 8]);
        view.storage.insert(c.id, high);
        view.memories.entry(c.id).or_default().fill(0, 1, 1);
        for mode in [crate::ThemeMode::Light, crate::ThemeMode::Dark] {
            for (palette, accent) in SignalPalette::ALL.into_iter().zip([
                crate::Accent::Teal,
                crate::Accent::Indigo,
                crate::Accent::Blue,
            ]) {
                crate::theme::apply_chrome(&ctx, mode, crate::Density::Compact, accent);
                palette.apply(&ctx);
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    component(ui.painter(), &canvas, &c, false, false, Some(&view), None)
                });
                out.textures_delta.clear();
                let shapes = || out.shapes.iter().map(|s| &s.shape);
                let text_has = |label: &str, color| {
                    shapes().any(|s| matches!(s, Shape::Text(t)
                        if t.galley.job.text == label
                            && t.galley.job.sections.iter().any(|section| section.format.color == color)))
                };
                assert!(
                    text_has("document label", label_color),
                    "{kind:?} document label"
                );
                let high = palette.color(mode, SignalTone::High);
                match kind {
                    Kind::Led => assert!(shapes().any(|s| matches!(s, Shape::Circle(c)
                        if c.fill == custom_lamp && c.stroke.color == high))),
                    Kind::SevenSegment | Kind::HexDisplay => {
                        for ink in [high, custom_lamp] {
                            let segment = shapes().any(|s| matches!(s, Shape::LineSegment { stroke, .. } if stroke.color == ink));
                            assert!(segment, "{kind:?} segment");
                        }
                    }
                    _ => assert!(
                        text_has("1", palette.text_color(mode, SignalTone::High)),
                        "{kind:?} stored or port value"
                    ),
                }
            }
        }
    }
}

#[test]
fn dark_default_bodies_selection_and_ghost_preserve_stored_colors() {
    let ctx = egui::Context::default();
    crate::Settings {
        theme: crate::ThemeMode::Dark,
        ..Default::default()
    }
    .apply_appearance(&ctx);
    let p = theme::palette(&ctx);
    let accent = theme::colors(&ctx).base;
    let canvas = Canvas {
        rect: Rect::from_min_size(Pos2::ZERO, vec2(500., 400.)),
        ..Default::default()
    };
    for (rgb, expected) in [
        ([51, 65, 85], p.ink),
        ([0, 0, 0], p.ink),
        ([58, 122, 171], Color32::from_rgb(58, 122, 171)),
    ] {
        let mut c = Component::new(Kind::Gate(Gate::And), Point::new(200, 160));
        c.color = rgb;
        c.color_alpha = 187;
        let original = c.clone();
        for ghost in [false, true] {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                component(ui.painter(), &canvas, &c, !ghost, ghost, None, None)
            });
            output.textures_delta.clear();
            let shapes = || output.shapes.iter().map(|s| &s.shape);
            let ink = if ghost {
                accent.gamma_multiply(0.45)
            } else {
                Color32::from_rgba_unmultiplied(expected.r(), expected.g(), expected.b(), 187)
            };
            assert!(
                shapes().any(|s| matches!(s,Shape::LineSegment{stroke,..} if stroke.color == ink))
            );
            assert_eq!(shapes().any(|s| matches!(s,Shape::Mesh(m) if m.vertices.iter().any(|v| v.color == p.surface))),!ghost);
            assert_eq!(shapes().filter(|s| matches!(s,Shape::Rect(r) if r.fill==accent && r.rect.size()==Vec2::splat(5.))).count(),if ghost {0} else {4});
            assert_eq!(c, original);
        }
    }
}

fn render_artwork(
    c: &Component,
    settings: crate::Settings,
    zoom: f32,
    selected: bool,
    view: &SimulationView,
) -> Vec<Shape> {
    let ctx = egui::Context::default();
    settings.apply_appearance(&ctx);
    let mut doc = Circuit::default();
    let id = doc.add_component(c.clone());
    let mut selection = BTreeSet::new();
    if selected {
        selection.insert(id);
    }
    let mut canvas = Canvas::default();
    canvas.camera.zoom = zoom;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        canvas.rect = ui.max_rect();
        circuit(
            ui.painter(),
            &canvas,
            &doc,
            &selection,
            view,
            &mut CircuitCache::default(),
            settings.editor_display(),
        );
    });
    out.textures_delta.clear();
    out.shapes.into_iter().map(|s| s.shape).collect()
}

#[test]
fn only_low_contrast_authored_edges_gain_one_screen_pixel() {
    use logisim_core::{Appearance, Geometry, Paint};
    let mut c = Component::new(Kind::Subcircuit(2), Point::new(60, 60));
    let mut art = Appearance {
        anchor: Point::ZERO,
        facing: Facing::East,
        ports: Default::default(),
        shapes: vec![],
    };
    for (stroke, fill) in [
        (Some([0; 3]), None),
        (None, Some([13, 15, 20])),
        (Some([240; 3]), None),
        (Some([0; 3]), Some([240; 3])),
        (Some([240; 3]), Some([0; 3])),
        (None, None),
    ] {
        let x = 20 * art.shapes.len() as i32;
        art.add(
            Geometry::Rect {
                a: Point::new(x, 0),
                b: Point::new(x + 10, 10),
                radius: 0,
            },
            Paint {
                stroke,
                fill,
                width: 2,
            },
        )
        .unwrap();
    }
    c.interface.appearance = Some(art);
    let original = c.clone();
    for mode in [crate::ThemeMode::Light, crate::ThemeMode::Dark] {
        for zoom in [0.5, 1., 2.] {
            for selected in [false, true] {
                let render = |enabled| {
                    render_artwork(
                        &c,
                        crate::Settings {
                            theme: mode,
                            dark_artwork_contrast: enabled,
                            ..Default::default()
                        },
                        zoom,
                        selected,
                        &SimulationView::default(),
                    )
                };
                let baseline = render(false);
                let mut shapes = render(true);
                if mode == crate::ThemeMode::Dark && !selected {
                    let edge = theme::ArtworkContrast::new(mode.palette(), 1., true)
                        .unwrap()
                        .edge;
                    let mut widths = vec![];
                    shapes.retain(|s| {
                        if let Shape::Path(path) = s
                            && path.stroke.color == egui::epaint::ColorMode::Solid(edge.color)
                        {
                            assert!(path.closed && path.fill == Color32::TRANSPARENT);
                            widths.push(path.stroke.width);
                            false
                        } else {
                            true
                        }
                    });
                    assert_eq!(widths, [2. * zoom + 2., 2.]); // stroke and fill-only
                }
                assert_eq!(shapes, baseline);
            }
        }
    }
    assert_eq!(c, original);
}

#[test]
fn plugin_boundary_is_outside_and_preserves_authored_rgba_and_runtime_overlays() {
    use logisim_core::plugin::api;
    let tint = [13, 22, 30, 93];
    let c = Component::from_plugin(
        api::Component {
            plugin: "test.outline".into(),
            version: "1".into(),
            properties: Default::default(),
            tool: api::Tool {
                id: "display".into(),
                name: "Display".into(),
                bounds: [[-30, -10], [30, 10]],
                ports: vec![],
                properties: vec![],
                actions: vec![],
                graphics: vec![api::Shape::Rect {
                    min: [-30, -10],
                    max: [30, 10],
                    fill: tint,
                    stroke: [0, 0, 0, 255],
                }],
            },
        },
        Point::new(100, 100),
    );
    let original = c.clone();
    let id = Circuit::default().add_component(c.clone());
    let mut view = SimulationView::default();
    view.plugin_graphics.insert(
        id,
        vec![api::Shape::Line {
            a: [-20, 0],
            b: [20, 0],
            color: [0, 0, 0, 175],
        }],
    );
    for state in ["idle", "selected", "focused", "failed"] {
        view.keyboard_focus = (state == "focused").then_some(id);
        if state == "failed" {
            view.plugin_errors.insert(id, "test".into());
        }
        for mode in [crate::ThemeMode::Light, crate::ThemeMode::Dark] {
            let render = |enabled| {
                render_artwork(
                    &c,
                    crate::Settings {
                        theme: mode,
                        dark_artwork_contrast: enabled,
                        ..Default::default()
                    },
                    1.5,
                    state == "selected",
                    &view,
                )
            };
            let baseline = render(false);
            let mut shapes = render(true);
            let body = baseline
                .iter()
                .find_map(|s| match s {
                    Shape::Rect(r)
                        if r.fill
                            == Color32::from_rgba_unmultiplied(
                                tint[0], tint[1], tint[2], tint[3],
                            ) =>
                    {
                        Some(r.rect)
                    }
                    _ => None,
                })
                .unwrap();
            if mode == crate::ThemeMode::Dark && state == "idle" {
                let index = shapes.iter().position(|s| matches!(s,Shape::Rect(r) if r.rect == body && r.stroke_kind == StrokeKind::Outside && r.fill == Color32::TRANSPARENT && r.stroke.width == 1.)).unwrap();
                let _ = shapes.remove(index);
            }
            assert_eq!(shapes, baseline);
        }
    }
    assert_eq!(c, original);
}

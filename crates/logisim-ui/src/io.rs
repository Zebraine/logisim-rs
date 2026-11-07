use crate::{canvas::Canvas, i18n::tr, simulation::SimulationView};
use egui::{Color32, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Kind, Point};
pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match c.kind {
        Kind::Button => "Output",
        Kind::Led => "Input",
        Kind::HexDisplay => {
            if index == 0 {
                "Hexadecimal input"
            } else {
                "Decimal point"
            }
        }
        Kind::SevenSegment => [
            "Middle segment (g)",
            "Upper-left segment (f)",
            "Top segment (a)",
            "Upper-right segment (b)",
            "Lower-left segment (e)",
            "Bottom segment (d)",
            "Lower-right segment (c)",
            "Decimal point",
        ][index],
        _ => "",
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    if !c.kind.is_io() {
        return;
    }
    ui.label(tr(if c.kind == Kind::Button {
        "Button color"
    } else {
        "On color"
    }));
    ui.color_edit_button_srgba_unmultiplied(&mut c.io.on_color);
    ui.end_row();
    if c.kind != Kind::Button {
        ui.label(tr("Off color"));
        ui.color_edit_button_srgba_unmultiplied(&mut c.io.off_color);
        ui.end_row();
    }
    if matches!(c.kind, Kind::SevenSegment | Kind::HexDisplay) {
        ui.label(tr("Background"));
        ui.color_edit_button_srgba_unmultiplied(&mut c.io.background);
        ui.end_row();
    }
    if matches!(c.kind, Kind::Led | Kind::SevenSegment) {
        ui.label(tr("Active on high"));
        ui.checkbox(&mut c.io.active_high, "");
        ui.end_row();
    }
}
fn color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    ink: Color32,
    simulation: Option<&SimulationView>,
) {
    let at = |point: Point| canvas.screen(c.position + c.facing.transform(point));
    let [min, max] = c.io_body();
    let body = Rect::from_two_pos(at(min), at(max));
    let z = canvas.camera.zoom;
    let stroke = Stroke::new(1.5, ink);
    let lights = simulation
        .and_then(|s| s.indicators.get(&c.id))
        .copied()
        .unwrap_or_default();
    match c.kind {
        Kind::Button => {
            let pressed = simulation
                .and_then(|s| s.buttons.get(&c.id))
                .copied()
                .unwrap_or(false);
            let fill = color(c.io.on_color);
            if !pressed {
                p.rect_filled(
                    body.translate(egui::vec2(2.0, 2.0) * z),
                    2.0 * z,
                    Color32::from_gray(140),
                );
            }
            p.rect(body, 2.0 * z, fill, stroke, StrokeKind::Inside);
            if pressed {
                p.rect_stroke(
                    body.shrink(2.0 * z),
                    1.0 * z,
                    Stroke::new(1.0, ink),
                    StrokeKind::Inside,
                );
            }
        }
        Kind::Led => {
            p.circle(
                body.center(),
                10.0 * z,
                color(if lights[0] {
                    c.io.on_color
                } else {
                    c.io.off_color
                }),
                stroke,
            );
        }
        Kind::SevenSegment | Kind::HexDisplay => {
            p.rect(
                body,
                3.0 * z,
                color(c.io.background),
                stroke,
                StrokeKind::Inside,
            );
            // The seven visual segments share the measured standalone display geometry.
            let segments = [
                ((10, 30), (28, 30)),
                ((6, 10), (6, 27)),
                ((10, 8), (28, 8)),
                ((31, 10), (31, 27)),
                ((6, 33), (6, 50)),
                ((10, 52), (28, 52)),
                ((31, 33), (31, 50)),
            ];
            for (i, ((x1, y1), (x2, y2))) in segments.into_iter().enumerate() {
                p.line_segment(
                    [at(min + Point::new(x1, y1)), at(min + Point::new(x2, y2))],
                    Stroke::new(
                        3.3 * z,
                        color(if lights[i] {
                            c.io.on_color
                        } else {
                            c.io.off_color
                        }),
                    ),
                );
            }
            p.circle_filled(
                at(min + Point::new(35, 53)),
                2.1 * z,
                color(if lights[7] {
                    c.io.on_color
                } else {
                    c.io.off_color
                }),
            );
        }
        _ => {}
    }
}

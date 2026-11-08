use crate::{canvas::Canvas, i18n::tr};
use egui::{Color32, Painter, Shape, Stroke, Ui};
use logisim_core::{Component, Kind, Point, TransistorType};

pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match index {
        0 => "Output",
        1 => "Data input",
        3 => "N control (active high)",
        _ if c.kind == Kind::TransmissionGate
            || c.transistor.transistor_type == TransistorType::P =>
        {
            "P control (active low)"
        }
        _ => "N control (active high)",
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    if !c.kind.is_transistor() {
        return;
    }
    if c.kind == Kind::Transistor {
        ui.label(tr("Transistor type"));
        egui::ComboBox::from_id_salt("transistor_type")
            .selected_text(tr(c.transistor.transistor_type.name()))
            .show_ui(ui, |ui| {
                for t in TransistorType::ALL {
                    ui.selectable_value(&mut c.transistor.transistor_type, t, tr(t.name()));
                }
            });
        ui.end_row();
    }
    ui.label(tr("Gate location"));
    egui::ComboBox::from_id_salt("transistor_gate_location")
        .selected_text(tr(if c.transistor.gate_bottom_right {
            "Bottom/Right"
        } else {
            "Top/Left"
        }))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut c.transistor.gate_bottom_right, false, tr("Top/Left"));
            ui.selectable_value(
                &mut c.transistor.gate_bottom_right,
                true,
                tr("Bottom/Right"),
            );
        })
        .response
        .on_hover_text(tr(
            "Changing gate location or facing keeps wire positions; reconnect moved ports.",
        ));
    ui.end_row();
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32, fill: Color32) {
    let at = |x, y| canvas.screen(c.position + c.facing.transform(Point::new(x, y)));
    let stroke = Stroke::new(1.7, color);
    let line = |a: (i32, i32), b: (i32, i32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    let sign = c.transistor_gate_y() / 20;
    if c.kind == Kind::TransmissionGate {
        line((-40, 0), (-30, 0));
        line((-10, 0), (0, 0));
        p.add(Shape::convex_polygon(
            vec![at(-30, -8), at(-10, -8), at(-10, 8), at(-30, 8)],
            fill,
            stroke,
        ));
        line((-31, sign * 10), (-9, sign * 10));
        line((-31, -sign * 10), (-9, -sign * 10));
        line((-20, -sign * 10), (-20, -sign * 20));
    } else {
        p.add(Shape::line(
            vec![
                at(-40, 0),
                at(-30, 0),
                at(-30, sign * 7),
                at(-10, sign * 7),
                at(-10, 0),
                at(0, 0),
            ],
            stroke,
        ));
        line((-30, sign * 10), (-10, sign * 10));
    }
    if c.kind == Kind::TransmissionGate || c.transistor.transistor_type == TransistorType::P {
        p.circle(
            at(-20, sign * 13),
            3.0 * canvas.camera.zoom,
            Color32::WHITE,
            stroke,
        );
        line((-20, sign * 16), (-20, sign * 20));
    } else {
        line((-20, sign * 10), (-20, sign * 20));
    }
    line((-22, -3), (-18, 0));
    line((-18, 0), (-22, 3));
}

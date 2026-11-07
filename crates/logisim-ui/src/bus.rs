use crate::{canvas::Canvas, i18n::tr};
use egui::{Align2, Color32, FontId, Painter, Shape, Stroke, Ui};
use logisim_core::{Component, GateSize, Kind, Point, Pull};

pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    if c.kind == Kind::PullResistor {
        "Pull"
    } else {
        match index {
            0 => "Output",
            1 => "Data input",
            _ if c.bus.negate_control => "Enable (active low)",
            _ => "Enable (active high)",
        }
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    if c.kind == Kind::PullResistor {
        ui.label(tr("Pull direction"));
        egui::ComboBox::from_id_salt("bus_pull")
            .selected_text(tr(c.bus.pull.name()))
            .show_ui(ui, |ui| {
                for pull in Pull::ALL {
                    ui.selectable_value(&mut c.bus.pull, pull, tr(pull.name()));
                }
            });
        ui.end_row();
    }
    if !c.kind.is_controlled() {
        return;
    }
    if c.kind == Kind::ControlledInverter {
        ui.label(tr("Gate size"));
        egui::ComboBox::from_id_salt("bus_gate_size")
            .selected_text(tr(c.gate_size.name()))
            .show_ui(ui, |ui| {
                for size in [GateSize::Narrow, GateSize::Wide] {
                    ui.selectable_value(&mut c.gate_size, size, tr(size.name()));
                }
            });
        ui.end_row();
    }
    ui.label(tr("Control location"));
    egui::ComboBox::from_id_salt("bus_control_side")
        .selected_text(tr(if c.bus.control_left {
            "Left-handed"
        } else {
            "Right-handed"
        }))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut c.bus.control_left, false, tr("Right-handed"));
            ui.selectable_value(&mut c.bus.control_left, true, tr("Left-handed"));
        })
        .response
        .on_hover_text(tr(
            "Changing control location or gate size keeps wire positions; reconnect moved ports.",
        ));
    ui.end_row();
    ui.label(tr("Negate control input"));
    ui.checkbox(&mut c.bus.negate_control, "");
    ui.end_row();
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32) {
    let at = |x, y| canvas.screen(c.position + c.facing.transform(Point::new(x, y)));
    let stroke = Stroke::new(1.7, color);
    let line = |a: (i32, i32), b: (i32, i32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    if c.kind == Kind::PullResistor {
        let mut points = vec![at(0, 0), at(-5, 0)];
        for i in 0..6 {
            points.push(at(-7 - i * 4, if i % 2 == 0 { -4 } else { 4 }));
        }
        points.extend([at(-30, 0), at(-32, 0)]);
        p.add(Shape::line(points, stroke));
        line((-32, -6), (-32, 6));
        p.text(
            at(-40, 0),
            Align2::CENTER_CENTER,
            match c.bus.pull {
                Pull::Zero => "0",
                Pull::One => "1",
                Pull::Error => "E",
            },
            FontId::monospace(11.0 * canvas.camera.zoom),
            color,
        );
        return;
    }
    let depth = c.gate_width();
    let tip = if c.kind == Kind::ControlledInverter {
        -7
    } else {
        -2
    };
    p.add(Shape::closed_line(
        vec![at(-depth, -8), at(tip, 0), at(-depth, 8)],
        stroke,
    ));
    if c.kind == Kind::ControlledInverter {
        p.circle(at(-4, 0), 3.0 * canvas.camera.zoom, Color32::WHITE, stroke);
    }
    line((-1, 0), (0, 0));
    let sign = if c.bus.control_left { -1 } else { 1 };
    let x = 10 - depth;
    if c.bus.negate_control {
        p.circle(
            at(x, sign * 6),
            3.0 * canvas.camera.zoom,
            Color32::WHITE,
            stroke,
        );
        line((x, sign * 9), (x, sign * 10));
    } else {
        line((x, sign * 4), (x, sign * 10));
    }
}

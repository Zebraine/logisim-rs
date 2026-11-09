use crate::{canvas::Canvas, i18n::tr, tf};
use egui::{Align2, Color32, FontId, Painter, Shape, Stroke, Ui};
use logisim_core::{Component, GateSize, Kind, Parity, ParityOutput, Point};

pub(crate) fn port_label(c: &Component, index: usize) -> String {
    if index == 0 {
        tr("Output").into()
    } else if c.parity_negated((index - 1) as u8) {
        tf!("Input {} (inverted)", index)
    } else {
        tf!("Input {}", index)
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    if !matches!(c.kind, Kind::Parity(_)) {
        return false;
    }
    ui.label(tr("Inputs"));
    let response = ui.add(egui::DragValue::new(&mut c.parity.inputs).range(2..=32));
    let active = response.has_focus() || response.dragged();
    ui.end_row();
    ui.label(tr("Gate size"));
    egui::ComboBox::from_id_salt("parity_gate_size")
        .selected_text(tr(c.parity.size.name()))
        .show_ui(ui, |ui| {
            for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                ui.selectable_value(&mut c.parity.size, size, tr(size.name()));
            }
        });
    ui.end_row();
    ui.label(tr("Output value"));
    egui::ComboBox::from_id_salt("parity_output")
        .selected_text(tr(c.parity.output.name()))
        .show_ui(ui, |ui| {
            for mode in ParityOutput::ALL {
                ui.selectable_value(&mut c.parity.output, mode, tr(mode.name()));
            }
        });
    ui.end_row();
    for i in 0..c.input_count() {
        ui.label(tf!("Invert input {}", i + 1));
        let mut inverted = c.parity_negated(i);
        if ui
            .push_id(("parity_inversion", i), |ui| ui.checkbox(&mut inverted, ""))
            .inner
            .changed()
        {
            c.parity.negated ^= 1 << i;
        }
        ui.end_row();
    }
    active
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32, fill: Color32) {
    let at = |x, y| canvas.screen(c.position + c.facing.transform(Point::new(x, y)));
    let stroke = Stroke::new(1.7, color);
    let width = c.gate_width();
    let height = c.parity_half_height();
    p.add(Shape::convex_polygon(
        vec![
            at(-width, -height),
            at(0, -height),
            at(0, height),
            at(-width, height),
        ],
        fill,
        stroke,
    ));
    for (i, port) in c.parity_ports().into_iter().skip(1).enumerate() {
        if c.parity_negated(i as u8) {
            p.circle(
                at(-width - 5, port.y),
                4.0 * canvas.camera.zoom,
                Color32::WHITE,
                stroke,
            );
            p.line_segment([at(port.x, port.y), at(-width - 9, port.y)], stroke);
        }
    }
    p.text(
        at(-width / 2, 0),
        Align2::CENTER_CENTER,
        if c.kind == Kind::Parity(Parity::Odd) {
            "2k+1"
        } else {
            "2k"
        },
        FontId::proportional(10.0 * canvas.camera.zoom),
        color,
    );
}

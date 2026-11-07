use crate::{canvas::Canvas, i18n::tr, simulation::SimulationView};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, ControlPosition, Kind, Latch, LatchEnable, LatchRole, Point};

pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match c.latch_terminals()[index].0 {
        LatchRole::Q => "Q",
        LatchRole::NotQ => "/Q",
        LatchRole::Enable => "Enable",
        LatchRole::D => "D",
        LatchRole::S => "S",
        LatchRole::R => "R",
        LatchRole::Preset => "Preset: set all bits",
        LatchRole::Clear => "Clear: reset to zero",
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    if !matches!(c.kind, Kind::Latch(_)) {
        return;
    }
    ui.label(tr("Trigger"));
    egui::ComboBox::from_id_salt("latch_enable")
        .selected_text(tr(c.latch_enable.name()))
        .show_ui(ui, |ui| {
            for mode in LatchEnable::ALL {
                if mode != LatchEnable::None || c.kind == Kind::Latch(Latch::Sr) {
                    ui.selectable_value(&mut c.latch_enable, mode, tr(mode.name()));
                }
            }
        });
    ui.end_row();
    ui.label(tr("New layout"));
    if ui.checkbox(&mut c.sequential.new_layout, "").changed() && !c.sequential.new_layout {
        c.sequential.controls = ControlPosition::Legacy;
    }
    ui.end_row();
    ui.label(tr("Preset / clear positions"));
    egui::ComboBox::from_id_salt("latch_controls")
        .selected_text(tr(c.sequential.controls.name()))
        .show_ui(ui, |ui| {
            for position in ControlPosition::ALL {
                ui.selectable_value(&mut c.sequential.controls, position, tr(position.name()));
            }
        });
    ui.end_row();
    ui.label(tr("Active-low preset / clear"));
    ui.checkbox(&mut c.sequential.negate_controls, "");
    ui.end_row();
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    color: Color32,
    simulation: Option<&SimulationView>,
) {
    let at = |point: Point| canvas.screen(c.position + c.facing.transform(point));
    let (min, max) = crate::sequential::body(c);
    let z = canvas.camera.zoom;
    let stroke = Stroke::new(1.6, color);
    let fill = Color32::from_rgb(244, 248, 255);
    p.rect(
        Rect::from_two_pos(at(min), at(max)),
        3.0 * z,
        fill,
        stroke,
        StrokeKind::Inside,
    );
    for (role, port) in c.latch_terminals() {
        let edge = Point::new(port.x.clamp(min.x, max.x), port.y.clamp(min.y, max.y));
        p.line_segment([at(port), at(edge)], stroke);
        let (label, inside, inverted) = match role {
            LatchRole::Q => ("Q", Point::new(max.x - 6, port.y), false),
            LatchRole::NotQ => ("Q", Point::new(max.x - 6, port.y), true),
            LatchRole::Enable => (
                "E",
                Point::new(min.x + 6, port.y),
                c.latch_enable == LatchEnable::Low,
            ),
            LatchRole::D => ("D", Point::new(min.x + 6, port.y), false),
            LatchRole::S => ("S", Point::new(min.x + 6, port.y), false),
            LatchRole::R => ("R", Point::new(min.x + 6, port.y), false),
            LatchRole::Preset | LatchRole::Clear => (
                if role == LatchRole::Preset { "S" } else { "R" },
                Point::new(
                    port.x,
                    if port.y <= min.y {
                        min.y + 5
                    } else {
                        max.y - 5
                    },
                ),
                c.sequential.negate_controls,
            ),
        };
        p.text(
            at(inside),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(8.0 * z),
            color,
        );
        if inverted {
            p.circle(at(edge), 2.5 * z, fill, stroke);
        }
    }
    let value = simulation.and_then(|s| s.storage.get(&c.id));
    p.text(
        at(Point::new((min.x + max.x) / 2, (min.y + max.y) / 2)),
        Align2::CENTER_CENTER,
        value.map_or("0", |s| s.text.as_str()),
        FontId::monospace(10.0 * z),
        value.map_or(color, |s| s.tone.color()),
    );
}

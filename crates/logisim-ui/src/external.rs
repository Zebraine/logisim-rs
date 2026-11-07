use crate::{
    SimulationAction,
    canvas::Canvas,
    i18n::tr,
    simulation::{SimulationUi, SimulationView},
};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Id, Kind, Point, Trigger};

pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    if c.kind == Kind::RegisterFile {
        ui.label(tr("Trigger"));
        egui::ComboBox::from_id_salt("register-file-trigger")
            .selected_text(tr(c.sequential.trigger.name()))
            .show_ui(ui, |ui| {
                for trigger in Trigger::ALL {
                    ui.selectable_value(&mut c.sequential.trigger, trigger, tr(trigger.name()));
                }
            });
        ui.end_row();
    }
}
fn instruction(view: Option<&SimulationView>, id: Id) -> Option<u32> {
    let signal = view?.ports.get(&(id, 0))?;
    u32::from_str_radix(&signal.binary, 2).ok()
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    color: Color32,
    view: Option<&SimulationView>,
) {
    let at = |x, y| canvas.screen(c.position + Point::new(x, y));
    let z = canvas.camera.zoom;
    let [min, max] = c.external_body();
    let body = Rect::from_min_max(at(min.x, min.y), at(max.x, max.y));
    p.rect(
        body,
        3.0 * z,
        Color32::from_rgb(237, 242, 249),
        Stroke::new(1.3, color),
        StrokeKind::Inside,
    );
    if c.kind == Kind::RiscVProbe {
        if let Some(word) = instruction(view, c.id) {
            let label = crate::riscv::decode(word);
            let size = (120.0 / (label.chars().count().max(1) as f32 * 0.6)).min(10.0);
            p.text(
                body.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(size * z),
                if word == 0 {
                    Color32::from_rgb(200, 45, 55)
                } else {
                    crate::ACCENT
                },
            );
        }
        return;
    }
    let words = view.and_then(|s| s.register_files.get(&c.id));
    for i in 0..32 {
        let x = -140 + 76 * (i / 16) as i32;
        let y = -80 + 10 * (i % 16) as i32;
        p.text(
            at(x, y),
            Align2::LEFT_CENTER,
            format!("{i:02}"),
            FontId::monospace(6.0 * z),
            color,
        );
        p.text(
            at(x + 14, y),
            Align2::LEFT_CENTER,
            words.and_then(|w| w.get(i)).map_or("00000000", |v| &v.text),
            FontId::monospace(7.5 * z),
            color,
        );
    }
    for (x, y, name) in [
        (-153, -10, "W"),
        (-153, 80, "CLK"),
        (-7, -50, "A"),
        (-7, 50, "B"),
        (-120, 82, "WE"),
        (-90, 82, "rW"),
        (-50, 82, "rA"),
        (-30, 82, "rB"),
    ] {
        p.text(
            at(x, y),
            Align2::CENTER_CENTER,
            name,
            FontId::monospace(6.5 * z),
            color,
        );
    }
}

#[derive(Default)]
pub(crate) struct RegisterEditor {
    target: Option<(Id, u8)>,
    address: u8,
    text: String,
}
pub(crate) fn runtime(ui: &mut Ui, sim: &mut SimulationUi, c: &Component) {
    if c.kind == Kind::RiscVProbe {
        if let Some(word) = instruction(Some(&sim.view), c.id) {
            ui.monospace(crate::riscv::decode(word));
        }
        return;
    }
    for index in 0..2 {
        if let Some(signal) = sim.view.ports.get(&(c.id, index)) {
            ui.label(format!(
                "{}: {}",
                tr(c.external_port_label(index)),
                signal.text
            ))
            .on_hover_text(&signal.binary);
        }
    }
    let Some(words) = sim.view.register_files.get(&c.id) else {
        return;
    };
    let edit = &mut sim.register_editor;
    egui::CollapsingHeader::new(tr("Register contents"))
        .id_salt(("register-contents", c.id))
        .show(ui, |ui| {
            ui.weak(tr(
                "Runtime values only; reset clears registers. Register 0 is read-only.",
            ));
            ui.horizontal(|ui| {
                ui.label(tr("Address"));
                ui.add(egui::DragValue::new(&mut edit.address).range(0..=31));
            });
            let Some(word) = words.get(usize::from(edit.address)) else {
                return;
            };
            if edit.target != Some((c.id, edit.address)) {
                edit.target = Some((c.id, edit.address));
                edit.text = word.text.clone();
            }
            ui.monospace(&word.text).on_hover_text(&word.binary);
            ui.label(tr("Input (hex)"));
            let response = ui.add(
                egui::TextEdit::singleline(&mut edit.text)
                    .id_salt(("register-word", c.id, edit.address))
                    .char_limit(8)
                    .desired_width(130.0),
            );
            let value = u32::from_str_radix(edit.text.trim(), 16).ok();
            let apply = ui
                .add_enabled(
                    edit.address != 0 && value.is_some(),
                    egui::Button::new(tr("Apply")),
                )
                .clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if apply
                && edit.address != 0
                && let Some(value) = value
            {
                sim.actions.push(SimulationAction::SetRegisterFileWord {
                    id: c.id,
                    address: edit.address,
                    value,
                });
            }
        });
}

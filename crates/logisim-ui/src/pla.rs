use crate::{canvas::Canvas, editor::Editor, i18n::tr, tf};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Id, Kind, Pla, PlaInput, Point};

pub(crate) fn port_label(index: usize) -> &'static str {
    match index {
        0 => "Output",
        1 => "Input",
        _ => "Chip select",
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    if c.kind != Kind::PlaRom {
        return false;
    }
    let before = c.pla.dimensions();
    let (mut inputs, mut terms, mut outputs) = before;
    let mut active = false;
    for (label, value) in [
        ("Input bits", &mut inputs),
        ("AND terms", &mut terms),
        ("Output bits", &mut outputs),
    ] {
        ui.label(tr(label));
        let response = ui.add(egui::DragValue::new(value).range(1..=32));
        active |= response.has_focus() || response.dragged();
        ui.end_row();
    }
    if before != (inputs, terms, outputs) {
        c.pla.resize(inputs, terms, outputs);
    }
    ui.label(tr("Chip select active on"));
    egui::ComboBox::from_id_salt("pla_select")
        .selected_text(tr(if c.pla.select_high {
            "High level"
        } else {
            "Low level"
        }))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut c.pla.select_high, false, tr("Low level"));
            ui.selectable_value(&mut c.pla.select_high, true, tr("High level"));
        });
    ui.end_row();
    active
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32) {
    let at = |x, y| canvas.screen(c.position + Point::new(x, y));
    let z = canvas.camera.zoom;
    p.rect(
        Rect::from_min_max(at(0, -30), at(60, 30)),
        4.0 * z,
        Color32::from_rgb(244, 248, 255),
        Stroke::new(1.6, color),
        StrokeKind::Inside,
    );
    let label = if c.label.is_empty() {
        "PLA ROM"
    } else {
        &c.label
    };
    let size = if c.label.is_empty() {
        10.0
    } else {
        f32::from(c.font_size)
    };
    p.text(
        at(30, -9),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(size * z),
        color,
    );
    let (inputs, terms, outputs) = c.pla.dimensions();
    p.text(
        at(30, 9),
        Align2::CENTER_CENTER,
        format!("{inputs} / {terms} / {outputs}"),
        FontId::monospace(7.0 * z),
        color,
    );
    p.text(
        at(30, 24),
        Align2::CENTER_CENTER,
        if c.pla.select_high { "CS" } else { "/CS" },
        FontId::monospace(6.0 * z),
        color,
    );
}

#[derive(Default)]
pub(crate) struct PlaEditor {
    context: Option<(Id, Vec<Id>, bool, Id)>,
    original: Pla,
    draft: Pla,
}
impl PlaEditor {
    pub fn show(&mut self, ctx: &egui::Context, editor: &mut Editor) {
        let Some((placing, id)) = editor.pla_target else {
            self.context = None;
            return;
        };
        let target = if placing {
            Some(&editor.template)
        } else {
            editor.circuit.component(id)
        };
        let Some(c) = target
            .filter(|c| c.id == id && c.kind == Kind::PlaRom)
            .cloned()
        else {
            editor.pla_target = None;
            self.context = None;
            return;
        };
        let context = (editor.active, editor.path.clone(), placing, id);
        if let Some(previous) = &self.context {
            if *previous != context || self.original != c.pla {
                editor.pla_target = None;
                self.context = None;
                return;
            }
        } else {
            editor.commit_properties();
            self.original = c.pla.clone();
            self.draft = c.pla.clone();
            self.context = Some(context);
        }
        let (inputs, terms, outputs) = self.draft.dimensions();
        let mut apply = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::unique("pla_program_editor")).show(ctx, |ui| {
            let width = (100.0 + f32::from(inputs + outputs) * 44.0)
                .clamp(640.0, 1100.0)
                .min((ctx.content_rect().width() - 80.0).max(260.0));
            ui.set_width(width);
            ui.heading(tr("PLA programming table"));
            if !c.label.is_empty() {
                ui.label(&c.label);
            }
            ui.label(tf!(
                "{} inputs · {} AND terms · {} outputs",
                inputs,
                terms,
                outputs
            ));
            ui.weak(tr(
                "Input: — disconnected, 1 direct, 0 inverted. Click to cycle. Output: ● connected.",
            ));
            ui.weak(tr(
                "Columns run from high bit to low bit. Empty terms or outputs evaluate to E.",
            ));
            ui.separator();
            egui::ScrollArea::both()
                .id_salt("pla_planes_scroll")
                .max_height((ctx.content_rect().height() - 280.0).clamp(100.0, 600.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("pla_planes")
                        .spacing(egui::vec2(4.0, 4.0))
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong(tr("Term"));
                            for col in 0..inputs {
                                ui.monospace(format!("I{}", inputs - 1 - col));
                            }
                            ui.label("→");
                            for col in 0..outputs {
                                ui.monospace(format!("Q{}", outputs - 1 - col));
                            }
                            ui.end_row();
                            for row in 0..terms {
                                ui.monospace(format!("{}", row + 1));
                                for col in 0..inputs {
                                    ui.push_id(("input", row, col), |ui| {
                                        let value = self.draft.input(row, col);
                                        let label = match value {
                                            PlaInput::Disconnected => "—",
                                            PlaInput::Direct => "1",
                                            PlaInput::Inverted => "0",
                                        };
                                        if ui
                                            .add_sized(
                                                [30.0, 25.0],
                                                egui::Button::new(label)
                                                    .selected(value != PlaInput::Disconnected),
                                            )
                                            .on_hover_text(tf!(
                                                "Term {} · input bit {}",
                                                row + 1,
                                                inputs - 1 - col
                                            ))
                                            .clicked()
                                        {
                                            self.draft.set_input(row, col, value.next());
                                        }
                                    });
                                }
                                ui.label("→");
                                for col in 0..outputs {
                                    ui.push_id(("output", row, col), |ui| {
                                        let value = self.draft.output(row, col);
                                        if ui
                                            .add_sized(
                                                [30.0, 25.0],
                                                egui::Button::new(if value { "●" } else { "·" })
                                                    .selected(value),
                                            )
                                            .on_hover_text(tf!(
                                                "Term {} · output bit {}",
                                                row + 1,
                                                outputs - 1 - col
                                            ))
                                            .clicked()
                                        {
                                            self.draft.set_output(row, col, !value);
                                        }
                                    });
                                }
                                ui.end_row();
                            }
                        });
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr("Clear contents")).clicked() {
                    self.draft.clear();
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
                if ui.button(tr("Apply")).on_hover_text("Ctrl+Enter").clicked() {
                    apply = true;
                }
            });
            apply |= ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
        });
        if apply {
            let mut changed = c;
            changed.pla = self.draft.clone();
            if placing {
                editor.template = changed;
            } else {
                editor.update_property(changed);
                editor.commit_properties();
            }
        }
        if apply || cancel || modal.should_close() {
            editor.pla_target = None;
            self.context = None;
        }
    }
}

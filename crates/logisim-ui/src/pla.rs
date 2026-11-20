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
        crate::theme::palette(p.ctx()).chrome,
        Stroke::new(1.6, color),
        StrokeKind::Inside,
    );
    let label = if c.label.is_empty() {
        "PLA ROM"
    } else {
        &c.label
    };
    let font = if c.label.is_empty() {
        logisim_core::ArtworkFont {
            size: 10,
            ..Default::default()
        }
    } else {
        c.font.clone()
    };
    crate::fonts::Text {
        text: label,
        font: &font,
        color,
        zoom: z,
        angle: 0.0,
    }
    .paint(p, at(30, -9), Align2::CENTER_CENTER);
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
const COLUMN: f32 = 30.;
const GAP: f32 = 4.;

impl PlaEditor {
    fn planes(&mut self, ui: &mut Ui, inputs: u8, terms: u8, outputs: u8) {
        let columns = inputs + outputs;
        // Leave room for the term number, the plane separator and the vertical scrollbar.
        let capacity =
            ((ui.available_width() - 72.) / (COLUMN + GAP)).clamp(1., f32::from(columns)) as u8;
        let planes = if columns <= capacity {
            [0..columns, columns..columns]
        } else {
            [0..inputs, inputs..columns]
        };
        for plane in planes {
            let end = plane.end;
            for start in plane.step_by(usize::from(capacity)) {
                let band = start..end.min(start + capacity);
                if start > 0 {
                    ui.separator();
                }
                egui::Grid::new(("pla-planes", start))
                    .min_col_width(0.)
                    .spacing(egui::vec2(GAP, GAP))
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong(tr("Term"));
                        for col in band.clone() {
                            if col == inputs && start < inputs {
                                ui.label("→");
                            }
                            ui.monospace(if col < inputs {
                                format!("I{}", inputs - 1 - col)
                            } else {
                                format!("Q{}", columns - 1 - col)
                            });
                        }
                        ui.end_row();
                        for row in 0..terms {
                            ui.monospace((row + 1).to_string());
                            for col in band.clone() {
                                if col == inputs && start < inputs {
                                    ui.label("→");
                                }
                                ui.push_id((row, col), |ui| {
                                    if col < inputs {
                                        let value = self.draft.input(row, col);
                                        let label = match value {
                                            PlaInput::Disconnected => "—",
                                            PlaInput::Direct => "1",
                                            PlaInput::Inverted => "0",
                                        };
                                        if ui
                                            .add_sized(
                                                [COLUMN, 25.],
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
                                    } else {
                                        let output = col - inputs;
                                        let value = self.draft.output(row, output);
                                        if ui
                                            .add_sized(
                                                [COLUMN, 25.],
                                                egui::Button::new(if value { "●" } else { "·" })
                                                    .selected(value),
                                            )
                                            .on_hover_text(tf!(
                                                "Term {} · output bit {}",
                                                row + 1,
                                                columns - 1 - col
                                            ))
                                            .clicked()
                                        {
                                            self.draft.set_output(row, output, !value);
                                        }
                                    }
                                });
                            }
                            ui.end_row();
                        }
                    });
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, editor: &mut Editor) {
        let Some((placing, id)) = editor.pla_target else {
            self.context = None;
            return;
        };
        let target = if placing {
            Some(&editor.template)
        } else {
            editor.session.circuit.component(id)
        };
        let Some(c) = target
            .filter(|c| c.id == id && c.kind == Kind::PlaRom)
            .cloned()
        else {
            editor.pla_target = None;
            self.context = None;
            return;
        };
        let context = (
            editor.session.context.active,
            editor.session.context.path.clone(),
            placing,
            id,
        );
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
        let m = crate::theme::metrics(ctx);
        let frame_width = 2. * (f32::from(m.dialog_margin) + 1.);
        let width = (frame_width + 72. + f32::from(inputs + outputs) * (COLUMN + GAP))
            .clamp(m.dialog.x, m.large_dialog.x);
        let dismiss = crate::dialog::Dialog::editor("pla_program_editor", width, m.dialog.y).show(
            ctx,
            tr("PLA programming table"),
            |dialog| {
                dialog.body(|ui| {
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
                    self.planes(ui, inputs, terms, outputs);
                    apply = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
                });
                dialog.footer("Cancel", |ui| {
                    apply |= ui.button(tr("Apply")).on_hover_text("Ctrl+Enter").clicked();
                    if ui.button(tr("Clear contents")).clicked() {
                        self.draft.clear();
                    }
                });
            },
        );
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
        if apply || dismiss != crate::dialog::Dismiss::Open {
            editor.pla_target = None;
            self.context = None;
        }
    }
}

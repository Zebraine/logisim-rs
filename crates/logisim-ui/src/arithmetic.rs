use crate::{canvas::Canvas, i18n::tr};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Arithmetic, Component, Kind, Point, Shift};

pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match (c.kind, index) {
        (Kind::Arithmetic(Arithmetic::Comparator), 0) => "Equal",
        (Kind::Arithmetic(Arithmetic::Comparator), 3) => "Greater than",
        (Kind::Arithmetic(Arithmetic::Comparator), 4) => "Less than",
        (Kind::Arithmetic(Arithmetic::Adder), 3) => "Carry in",
        (Kind::Arithmetic(Arithmetic::Adder), 4) => "Carry out",
        (Kind::Arithmetic(Arithmetic::Subtractor), 3) => "Borrow in",
        (Kind::Arithmetic(Arithmetic::Subtractor), 4) => "Borrow out",
        (Kind::Arithmetic(Arithmetic::Shifter), 1) => "Data input",
        (Kind::Arithmetic(Arithmetic::Shifter), 2) => "Shift amount",
        (_, 0) => "Result",
        (_, 1) => "Operand A",
        (_, 2) => "Operand B",
        _ => "Port",
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    match c.kind {
        Kind::Arithmetic(Arithmetic::Comparator) => {
            ui.label(tr("Numeric type"));
            let name = |signed| {
                tr(if signed {
                    "Two's complement"
                } else {
                    "Unsigned"
                })
            };
            egui::ComboBox::from_id_salt("arithmetic_numeric_type")
                .selected_text(name(c.arithmetic.signed))
                .show_ui(ui, |ui| {
                    for signed in [true, false] {
                        ui.selectable_value(&mut c.arithmetic.signed, signed, name(signed));
                    }
                });
            ui.end_row();
        }
        Kind::Arithmetic(Arithmetic::Shifter) => {
            ui.label(tr("Shift type"));
            egui::ComboBox::from_id_salt("arithmetic_shift_type")
                .selected_text(tr(c.arithmetic.shift.name()))
                .show_ui(ui, |ui| {
                    for shift in Shift::ALL {
                        ui.selectable_value(&mut c.arithmetic.shift, shift, tr(shift.name()));
                    }
                });
            ui.end_row();
            ui.label(tr("Shift amount bits"));
            ui.label(c.shift_amount_bits().to_string());
            ui.end_row();
        }
        _ => {}
    }
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32) {
    let Kind::Arithmetic(kind) = c.kind else {
        return;
    };
    let at = |x, y| canvas.screen(c.position + Point::new(x, y));
    p.rect(
        Rect::from_min_max(at(-40, -20), at(0, 20)),
        3.0,
        Color32::from_rgb(244, 248, 255),
        Stroke::new(1.6, color),
        StrokeKind::Inside,
    );
    let text = |x, y, label, size| {
        p.text(
            at(x, y),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(size * canvas.camera.zoom),
            color,
        );
    };
    text(
        -33,
        -10,
        if kind == Arithmetic::Shifter {
            "D"
        } else {
            "A"
        },
        7.0,
    );
    text(
        -33,
        10,
        if kind == Arithmetic::Shifter {
            "S"
        } else {
            "B"
        },
        7.0,
    );
    match kind {
        Arithmetic::Adder | Arithmetic::Subtractor => {
            text(
                -12,
                0,
                if kind == Arithmetic::Adder {
                    "+"
                } else {
                    "−"
                },
                18.0,
            );
            let (input, output) = if kind == Arithmetic::Adder {
                ("Ci", "Co")
            } else {
                ("Bi", "Bo")
            };
            text(-20, -13, input, 7.0);
            text(-20, 13, output, 7.0);
        }
        Arithmetic::Comparator => {
            for (y, label) in [(-10, ">"), (0, "="), (10, "<")] {
                text(-8, y, label, 10.0);
            }
        }
        Arithmetic::Shifter => {
            let label = match c.arithmetic.shift {
                Shift::LogicalLeft => "<<",
                Shift::LogicalRight => ">>",
                Shift::ArithmeticRight => ">>s",
                Shift::RotateLeft => "rol",
                Shift::RotateRight => "ror",
            };
            text(-15, 0, label, 10.0);
        }
    }
}

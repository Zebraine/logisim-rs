use crate::{canvas::Canvas, i18n::tr, tf};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Arithmetic, BitSearch, Component, Kind, Point, Shift};

pub(crate) fn port_label(c: &Component, index: usize) -> String {
    if c.kind == Kind::Arithmetic(Arithmetic::BitAdder) && index > 0 {
        return tf!("Input {}", index - 1);
    }
    tr(match (c.kind, index) {
        (Kind::Arithmetic(Arithmetic::BitAdder), 0) => "Bit count",
        (Kind::Arithmetic(Arithmetic::BitFinder), 0) => "Bit index",
        (Kind::Arithmetic(Arithmetic::BitFinder), 1) => "Data input",
        (Kind::Arithmetic(Arithmetic::BitFinder), 2) => "Found",
        (Kind::Arithmetic(Arithmetic::Comparator), 0) => "Equal",
        (Kind::Arithmetic(Arithmetic::Comparator), 3) => "Greater than",
        (Kind::Arithmetic(Arithmetic::Comparator), 4) => "Less than",
        (Kind::Arithmetic(Arithmetic::Adder), 3) => "Carry in",
        (Kind::Arithmetic(Arithmetic::Adder), 4) => "Carry out",
        (Kind::Arithmetic(Arithmetic::Subtractor), 3) => "Borrow in",
        (Kind::Arithmetic(Arithmetic::Subtractor), 4) => "Borrow out",
        (Kind::Arithmetic(Arithmetic::Multiplier), 0) => "Product low",
        (Kind::Arithmetic(Arithmetic::Multiplier), 3) => "Addend",
        (Kind::Arithmetic(Arithmetic::Multiplier), 4) => "Product high",
        (Kind::Arithmetic(Arithmetic::Divider), 0) => "Quotient",
        (Kind::Arithmetic(Arithmetic::Divider), 1) => "Dividend low",
        (Kind::Arithmetic(Arithmetic::Divider), 2) => "Divisor",
        (Kind::Arithmetic(Arithmetic::Divider), 3) => "Dividend high",
        (Kind::Arithmetic(Arithmetic::Divider), 4) => "Remainder",
        (Kind::Arithmetic(Arithmetic::Negator), 1) => "Data input",
        (Kind::Arithmetic(Arithmetic::Shifter), 1) => "Data input",
        (Kind::Arithmetic(Arithmetic::Shifter), 2) => "Shift amount",
        (_, 0) => "Result",
        (_, 1) => "Operand A",
        (_, 2) => "Operand B",
        _ => "Port",
    })
    .to_owned()
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    let mut active = false;
    match c.kind {
        Kind::Arithmetic(Arithmetic::BitAdder) => {
            ui.label(tr("Inputs"));
            let r = ui
                .add(egui::DragValue::new(&mut c.arithmetic.bit_inputs).range(1..=32))
                .on_hover_text(tr(
                    "Changing the input count keeps wire positions; reconnect moved inputs.",
                ));
            active = r.has_focus() || r.dragged();
            ui.end_row();
            ui.label(tr("Output bits"));
            ui.label(c.arithmetic_output_bits().to_string());
            ui.end_row();
        }
        Kind::Arithmetic(Arithmetic::BitFinder) => {
            ui.label(tr("Search type"));
            egui::ComboBox::from_id_salt("arithmetic_bit_search")
                .selected_text(tr(c.arithmetic.search.name()))
                .show_ui(ui, |ui| {
                    for mode in BitSearch::ALL {
                        ui.selectable_value(&mut c.arithmetic.search, mode, tr(mode.name()));
                    }
                });
            ui.end_row();
            ui.label(tr("Index bits"));
            ui.label(c.arithmetic_output_bits().to_string());
            ui.end_row();
        }
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
    active
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32) {
    let Kind::Arithmetic(kind) = c.kind else {
        return;
    };
    let at = |x, y| canvas.screen(c.position + Point::new(x, y));
    let [min, max] = c.arithmetic_body();
    p.rect(
        Rect::from_min_max(at(min.x, min.y), at(max.x, max.y)),
        3.0,
        Color32::from_rgb(244, 248, 255),
        Stroke::new(1.6, color),
        StrokeKind::Inside,
    );
    let text = |x, y, label: &str, size| {
        p.text(
            at(x, y),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(size * canvas.camera.zoom),
            color,
        );
    };
    if kind == Arithmetic::BitAdder {
        text(-12, 0, "#", 18.0);
        for (i, port) in c.local_ports().into_iter().skip(1).enumerate() {
            text(-33, port.y, &i.to_string(), 6.0);
        }
        return;
    }
    if kind == Arithmetic::BitFinder {
        text(
            -20,
            -6,
            if c.arithmetic.search.from_high() {
                "MSB"
            } else {
                "LSB"
            },
            10.0,
        );
        text(
            -20,
            6,
            if c.arithmetic.search.find_one() {
                "1"
            } else {
                "0"
            },
            10.0,
        );
        return;
    }
    if kind == Arithmetic::Negator {
        text(-33, 0, "D", 7.0);
        text(-15, 0, "−x", 14.0);
        return;
    }
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
        Arithmetic::Multiplier | Arithmetic::Divider => {
            let (symbol, input, output) = if kind == Arithmetic::Multiplier {
                ("×", "C", "H")
            } else {
                ("÷", "U", "R")
            };
            text(-12, 0, symbol, 18.0);
            text(-20, -13, input, 7.0);
            text(-20, 13, output, 7.0);
        }
        Arithmetic::Negator | Arithmetic::BitAdder | Arithmetic::BitFinder => {}
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

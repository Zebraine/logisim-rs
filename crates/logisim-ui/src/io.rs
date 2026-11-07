use crate::{canvas::Canvas, i18n::tr, simulation::SimulationView};
use egui::{Color32, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Facing, Kind, MatrixInput, Point, Radix};
pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match c.kind {
        Kind::Joystick => {
            if index == 0 {
                "X"
            } else {
                "Y"
            }
        }
        Kind::Button | Kind::DipSwitch | Kind::Slider => "Output",
        Kind::Switch => {
            if index == 0 {
                "Output"
            } else {
                "Input"
            }
        }
        Kind::Led => "Input",
        Kind::RgbLed => ["Red", "Green", "Blue"][index],
        Kind::DotMatrix => match c.io.matrix_input {
            MatrixInput::Columns => "Column data",
            MatrixInput::Rows => "Row data",
            MatrixInput::Select => {
                if index == 0 {
                    "Column data"
                } else {
                    "Row selection"
                }
            }
        },
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
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    let mut active = false;
    if !c.kind.is_io() {
        return false;
    }
    if c.kind == Kind::RgbLed {
        ui.label(tr("8-bit color channels"));
        ui.checkbox(&mut c.io.rgb_multibit, "");
        ui.end_row();
        return active;
    }
    if c.kind == Kind::DotMatrix {
        ui.label(tr("Input format"));
        egui::ComboBox::from_id_salt("matrix_input")
            .selected_text(tr(c.io.matrix_input.name()))
            .show_ui(ui, |ui| {
                for mode in MatrixInput::ALL {
                    ui.selectable_value(&mut c.io.matrix_input, mode, tr(mode.name()));
                }
            });
        ui.end_row();
        for (label, value) in [
            ("Matrix columns", &mut c.io.matrix_columns),
            ("Matrix rows", &mut c.io.matrix_rows),
        ] {
            ui.label(tr(label));
            let r = ui.add(egui::DragValue::new(value).range(1..=32));
            active |= r.has_focus() || r.dragged();
            ui.end_row();
        }
        ui.label(tr("Light persistence (ticks)"));
        let r =
            ui.add(egui::DragValue::new(&mut c.io.matrix_persistence).range(0..=i32::MAX as u32));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
        ui.label(tr("Dot shape"));
        egui::ComboBox::from_id_salt("matrix_shape")
            .selected_text(tr(if c.io.matrix_circular {
                "Circular"
            } else {
                "Square"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.io.matrix_circular, false, tr("Square"));
                ui.selectable_value(&mut c.io.matrix_circular, true, tr("Circular"));
            });
        ui.end_row();
    }
    if c.kind == Kind::DipSwitch {
        ui.label(tr("Number of switches"));
        let r = ui.add(egui::DragValue::new(&mut c.io.switch_count).range(1..=32));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    if c.kind == Kind::Slider {
        ui.label(tr("Value (hex)"));
        let max = c.slider_max();
        let r = ui.add(
            egui::DragValue::new(&mut c.io.slider_value)
                .hexadecimal(usize::from(c.bits.div_ceil(4)), false, true)
                .range(0..=max),
        );
        active |= r.has_focus() || r.dragged();
        ui.end_row();
        ui.label(tr("Direction"));
        egui::ComboBox::from_id_salt("slider_direction")
            .selected_text(tr(if c.io.slider_reverse {
                "Right to left"
            } else {
                "Left to right"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.io.slider_reverse, false, tr("Left to right"));
                ui.selectable_value(&mut c.io.slider_reverse, true, tr("Right to left"));
            });
        ui.end_row();
        ui.label(tr("Radix"));
        egui::ComboBox::from_id_salt("slider_radix")
            .selected_text(tr(c.radix.name()))
            .show_ui(ui, |ui| {
                for r in Radix::ALL {
                    ui.selectable_value(&mut c.radix, r, tr(r.name()));
                }
            });
        ui.end_row();
    }
    if c.kind == Kind::Joystick {
        ui.label(tr("Label color"));
        ui.color_edit_button_srgb(&mut c.color);
        ui.end_row();
    }
    ui.label(tr(if c.kind == Kind::Joystick {
        "Stick color"
    } else if c.kind == Kind::Button {
        "Button color"
    } else if matches!(c.kind, Kind::Switch | Kind::DipSwitch | Kind::Slider) {
        "Body color"
    } else {
        "On color"
    }));
    ui.color_edit_button_srgba_unmultiplied(&mut c.io.on_color);
    ui.end_row();
    if matches!(
        c.kind,
        Kind::Led | Kind::SevenSegment | Kind::HexDisplay | Kind::DotMatrix
    ) {
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
    active
}
/// World-space controls retain the observed ordering across all four facings.
pub(crate) fn dip_bit(c: &Component, world: egui::Vec2) -> Option<u8> {
    let bounds = crate::drawing::bounds(c);
    if !bounds.contains(world.to_pos2()) {
        return None;
    }
    let offset = if matches!(c.facing, Facing::East | Facing::West) {
        world.y - bounds.top()
    } else {
        world.x - bounds.left()
    };
    Some(((offset / 20.0) as u8).min(c.io.switch_count.clamp(1, 32) - 1))
}
pub(crate) fn slider_knob(c: &Component) -> egui::Vec2 {
    let min = crate::drawing::bounds(c).min;
    let t = c.io.slider_value.min(c.slider_max()) as f64 / f64::from(c.slider_max());
    let t = if c.io.slider_reverse { 1.0 - t } else { t };
    min.to_vec2() + egui::vec2(10.0 + (100.0 * t) as f32, 20.0)
}
pub(crate) fn move_slider(c: &mut Component, world: egui::Vec2) {
    let left = crate::drawing::bounds(c).left() + 10.0;
    let percent = (world.x - left).round().clamp(0.0, 100.0) as u8;
    c.io.slider_value = c.slider_value_at(percent);
}
fn slider_text(c: &Component) -> String {
    let v = c.io.slider_value.min(c.slider_max());
    match c.radix {
        Radix::Binary => format!("{v:0width$b}", width = usize::from(c.bits)),
        Radix::Octal => format!("{v:o}"),
        Radix::Hex => format!("{v:0width$x}", width = usize::from(c.bits.div_ceil(4))),
        Radix::Unsigned => v.to_string(),
        Radix::Signed => {
            (((v << (32 - c.bits.clamp(1, 32))) as i32) >> (32 - c.bits.clamp(1, 32))).to_string()
        }
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
    let state = simulation
        .and_then(|s| s.inputs.get(&c.id))
        .map_or(0, |v| v.value);
    match c.kind {
        Kind::Joystick => {
            let center = canvas.screen(c.joystick_center());
            let offset = simulation
                .and_then(|s| s.joysticks.get(&c.id))
                .copied()
                .unwrap_or_default();
            let knob = canvas.screen(c.joystick_center() + offset);
            p.rect(body, 4.0 * z, Color32::WHITE, stroke, StrokeKind::Inside);
            p.rect_stroke(
                body.shrink(3.0 * z),
                2.0 * z,
                Stroke::new(z, ink.gamma_multiply(0.4)),
                StrokeKind::Inside,
            );
            p.circle_filled(center, 2.0 * z, ink);
            p.line_segment([center, knob], Stroke::new(2.0 * z, ink));
            p.circle(knob, 4.0 * z, color(c.io.on_color), stroke);
        }
        Kind::RgbLed => {
            for (i, point) in c.io_ports().into_iter().enumerate() {
                p.line_segment(
                    [at(point), at(Point::new(-15, point.y))],
                    Stroke::new(2.0 * z, [Color32::RED, Color32::GREEN, Color32::BLUE][i]),
                );
            }
            let rgb = simulation
                .and_then(|s| s.rgb.get(&c.id))
                .copied()
                .unwrap_or([0; 3]);
            p.circle(
                at(Point::new(-20, 0)),
                10.0 * z,
                Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                stroke,
            );
        }
        Kind::DotMatrix => {
            p.rect_filled(body, 0.0, Color32::WHITE);
            let cols = c.io.matrix_columns.clamp(1, 32);
            let rows = c.io.matrix_rows.clamp(1, 32);
            let cells = simulation.and_then(|s| s.matrices.get(&c.id));
            for row in 0..rows {
                for col in 0..cols {
                    let index = usize::from(row) * usize::from(cols) + usize::from(col);
                    let state = cells
                        .and_then(|cells| cells.get(index))
                        .copied()
                        .unwrap_or(Some(false));
                    let fill = match state {
                        Some(true) => color(c.io.on_color),
                        Some(false) => color(c.io.off_color),
                        None => Color32::from_rgb(192, 0, 0),
                    };
                    let center =
                        at(min + Point::new(i32::from(col) * 10 + 5, i32::from(row) * 10 + 5));
                    if c.io.matrix_circular {
                        p.circle_filled(center, 4.0 * z, fill);
                    } else {
                        p.rect_filled(
                            Rect::from_center_size(center, egui::vec2(10.0, 10.0) * z),
                            0.0,
                            fill,
                        );
                    }
                }
            }
            p.rect_stroke(body, 0.0, stroke, StrokeKind::Inside);
        }
        Kind::Switch => {
            p.rect(
                body,
                2.0 * z,
                color(c.io.on_color),
                stroke,
                StrokeKind::Inside,
            );
            let closed = state != 0;
            p.line_segment(
                [
                    at(Point::new(-17, 0)),
                    at(Point::new(-3, if closed { 0 } else { -9 })),
                ],
                Stroke::new(2.5, ink),
            );
            for x in [-17, -3] {
                p.circle_filled(at(Point::new(x, 0)), 2.0 * z, ink);
            }
            p.circle_filled(
                at(Point::new(-10, 9)),
                2.0 * z,
                if closed {
                    crate::ACCENT
                } else {
                    Color32::from_gray(150)
                },
            );
        }
        Kind::DipSwitch => {
            p.rect(
                body,
                3.0 * z,
                color(c.io.on_color),
                stroke,
                StrokeKind::Inside,
            );
            let vertical = matches!(c.facing, Facing::East | Facing::West);
            for i in 0..c.io.switch_count.clamp(1, 32) {
                let on = state & (1 << i) != 0;
                let center = body.min
                    + if vertical {
                        egui::vec2(15.0, 10.0 + 20.0 * f32::from(i)) * z
                    } else {
                        egui::vec2(10.0 + 20.0 * f32::from(i), 15.0) * z
                    };
                let size = if vertical {
                    egui::vec2(22.0, 12.0)
                } else {
                    egui::vec2(12.0, 22.0)
                } * z;
                p.rect_filled(
                    Rect::from_center_size(center, size),
                    2.0 * z,
                    if on {
                        crate::ACCENT
                    } else {
                        Color32::from_gray(100)
                    },
                );
                let offset = if on { 4.0 } else { -4.0 };
                let offset = if vertical {
                    egui::vec2(offset, 0.0)
                } else {
                    egui::vec2(0.0, offset)
                } * z;
                p.rect_filled(
                    Rect::from_center_size(center + offset, egui::vec2(8.0, 8.0) * z),
                    1.0 * z,
                    Color32::WHITE,
                );
            }
        }
        Kind::Slider => {
            p.rect(
                body,
                4.0 * z,
                color(c.io.on_color),
                stroke,
                StrokeKind::Inside,
            );
            p.line_segment(
                [
                    body.min + egui::vec2(10.0, 20.0) * z,
                    body.min + egui::vec2(110.0, 20.0) * z,
                ],
                Stroke::new(2.0, ink),
            );
            p.circle(
                canvas.screen_vec(slider_knob(c)),
                5.0 * z,
                crate::ACCENT,
                Stroke::new(1.0, ink),
            );
            let text = slider_text(c);
            let size = (108.0 / (text.len().max(1) as f32 * 0.6)).min(11.0) * z;
            p.text(
                body.min + egui::vec2(60.0, 7.5) * z,
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::monospace(size),
                ink,
            );
        }
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

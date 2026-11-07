use crate::{canvas::Canvas, i18n::tr, simulation::SimulationView};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, ControlPosition, CounterOverflow, FlipFlop, Kind, Point, Trigger};

pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match c.kind {
        Kind::Clock => "Clock",
        Kind::Random => [
            "Q",
            "Enable: 0 holds data",
            "Clock",
            "Clear: restart from seed",
        ][index],
        Kind::Counter => match index {
            0 => "Q",
            1 => "D",
            2 => "Clock",
            3 if c.sequential.counter_old => "Load: 1 loads when Count=0, otherwise decrements",
            3 => "Load: 1 loads data",
            4 if c.sequential.counter_old => "Count: 0 holds or loads; otherwise counts",
            4 => "Reverse: 1 counts down",
            5 => "Carry: count is at the directional limit",
            6 => "Preset: set all bits",
            7 => "Clear: reset to zero",
            _ => "Enable: 0 stops counting",
        },
        Kind::ShiftRegister => [
            "Q",
            "D",
            "Clock",
            "Mode: 0 holds; 1 shifts toward bit 0; 2 shifts toward the high bit; 3 loads data",
            "Clear: reset to zero",
        ][index],
        Kind::Register => [
            "Q",
            "D",
            "Clock",
            "Enable: 0 holds data",
            "Chip select",
            "Preset: set all bits",
            "Clear: reset to zero",
        ][index],
        Kind::FlipFlop(ff) => {
            let count = if ff.paired() { 7 } else { 6 };
            if index == count - 1 {
                "Clear: reset to zero"
            } else if index == count - 2 {
                "Preset: set all bits"
            } else {
                match index {
                    0 => "Q",
                    1 => "/Q",
                    2 => "Clock",
                    3 => match ff {
                        FlipFlop::D => "D",
                        FlipFlop::T => "T",
                        FlipFlop::Jk => "J",
                        FlipFlop::Sr => "S",
                    },
                    _ => {
                        if ff == FlipFlop::Jk {
                            "K"
                        } else {
                            "R"
                        }
                    }
                }
            }
        }
        _ => "",
    }
}

pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    let mut active = false;
    let s = &mut c.sequential;
    if c.kind == Kind::Clock {
        for (label, value) in [
            ("High duration (ticks)", &mut s.high_ticks),
            ("Low duration (ticks)", &mut s.low_ticks),
        ] {
            ui.label(tr(label));
            let r = ui.add(egui::DragValue::new(value).range(1..=65_535));
            active |= r.has_focus() || r.dragged();
            ui.end_row();
        }
    }
    if c.kind.is_storage() && !matches!(c.kind, Kind::Latch(_)) {
        ui.label(tr("Trigger"));
        egui::ComboBox::from_id_salt("storage_trigger")
            .selected_text(tr(s.trigger.name()))
            .show_ui(ui, |ui| {
                for trigger in Trigger::ALL {
                    if c.kind == Kind::Register
                        || matches!(trigger, Trigger::Rising | Trigger::Falling)
                    {
                        ui.selectable_value(&mut s.trigger, trigger, tr(trigger.name()));
                    }
                }
            });
        ui.end_row();
    }
    if c.kind == Kind::Random {
        ui.label(tr("Seed (0 = automatic)"));
        let r = ui.add(
            egui::DragValue::new(&mut s.random_seed)
                .range(i32::MIN..=i32::MAX)
                .speed(1),
        );
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    if c.kind == Kind::Counter {
        ui.label(tr("Maximum value (hex)"));
        let mask = u32::MAX >> (32 - c.bits.clamp(1, 32));
        let r = ui.add(
            egui::DragValue::new(&mut s.counter_max)
                .range(0..=mask)
                .hexadecimal(1, false, true),
        );
        active |= r.has_focus() || r.dragged();
        ui.end_row();
        ui.label(tr("Action on overflow"));
        egui::ComboBox::from_id_salt("counter_overflow")
            .selected_text(tr(s.counter_overflow.name()))
            .show_ui(ui, |ui| {
                for v in CounterOverflow::ALL {
                    ui.selectable_value(&mut s.counter_overflow, v, tr(v.name()));
                }
            });
        ui.end_row();
        ui.label(tr("Behavior"));
        egui::ComboBox::from_id_salt("counter_behavior")
            .selected_text(tr(if s.counter_old {
                "Old behavior"
            } else {
                "New behavior"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut s.counter_old, false, tr("New behavior"));
                ui.selectable_value(&mut s.counter_old, true, tr("Old behavior"));
            });
        ui.end_row();
    }
    if c.kind == Kind::Register {
        ui.label(tr("Chip select active on"));
        egui::ComboBox::from_id_salt("register_select")
            .selected_text(tr(if s.select_high {
                "High level"
            } else {
                "Low level"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut s.select_high, false, tr("Low level"));
                ui.selectable_value(&mut s.select_high, true, tr("High level"));
            });
        ui.end_row();
    }
    if matches!(c.kind, Kind::FlipFlop(_)) {
        ui.label(tr("New layout"));
        if ui.checkbox(&mut s.new_layout, "").changed() {
            s.controls = if s.new_layout {
                ControlPosition::AboveBelow
            } else {
                ControlPosition::Legacy
            };
        }
        ui.end_row();
        ui.label(tr("Preset / clear positions"));
        ui.add_enabled_ui(s.new_layout, |ui| {
            egui::ComboBox::from_id_salt("storage_controls")
                .selected_text(tr(s.controls.name()))
                .show_ui(ui, |ui| {
                    for value in ControlPosition::ALL {
                        ui.selectable_value(&mut s.controls, value, tr(value.name()));
                    }
                });
        });
        ui.end_row();
        ui.label(tr("Active-low preset / clear"));
        ui.checkbox(&mut s.negate_controls, "");
        ui.end_row();
    }
    active
}
pub(crate) fn body(c: &Component) -> (Point, Point) {
    match c.kind {
        Kind::Clock => (Point::new(-20, -10), Point::new(0, 10)),
        Kind::Register | Kind::Counter | Kind::Random => (Point::new(-30, -20), Point::new(0, 20)),
        Kind::ShiftRegister => (Point::new(0, -20), Point::new(30, 20)),
        _ if c.sequential.new_layout => (Point::new(-50, -10), Point::new(-10, 50)),
        _ => (Point::new(-40, -10), Point::new(0, 30)),
    }
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    color: Color32,
    simulation: Option<&SimulationView>,
) {
    let at = |point: Point| canvas.screen(c.position + c.facing.transform(point));
    let xy = |x, y| at(Point::new(x, y));
    let (min, max) = body(c);
    let screen = Rect::from_two_pos(at(min), at(max));
    let z = canvas.camera.zoom;
    let stroke = Stroke::new(1.6, color);
    p.rect(
        screen,
        3.0 * z,
        Color32::from_rgb(244, 248, 255),
        stroke,
        StrokeKind::Inside,
    );
    if c.kind == Kind::Clock {
        let high = simulation
            .and_then(|s| s.ports.get(&(c.id, 0)))
            .is_some_and(|s| s.binary == "1");
        let signal_color = simulation
            .and_then(|s| s.ports.get(&(c.id, 0)))
            .map_or(color, |s| s.tone.color());
        let sign = if high { -1 } else { 1 };
        let points = [
            (-17, 4 * sign),
            (-11, 4 * sign),
            (-11, -4 * sign),
            (-5, -4 * sign),
            (-5, 4 * sign),
        ];
        p.add(egui::Shape::line(
            points.into_iter().map(|(x, y)| xy(x, y)).collect(),
            Stroke::new(2.0, signal_color),
        ));
        return;
    }
    if matches!(c.kind, Kind::Counter | Kind::ShiftRegister | Kind::Random) {
        let labels: &[(i32, i32, &str)] = if c.kind == Kind::Counter {
            &[
                (-26, 0, "D"),
                (-4, 0, "Q"),
                (-26, -10, "LD"),
                (-26, 10, if c.sequential.counter_old { "CT" } else { "DN" }),
                (-4, 10, "CO"),
                (-10, -16, "S"),
                (-10, 16, "R"),
            ]
        } else if c.kind == Kind::Random {
            &[(-4, 0, "Q"), (-26, 10, "EN"), (-20, 16, "R")]
        } else {
            &[(4, 0, "D"), (26, 0, "Q"), (10, -16, "M"), (10, 16, "R")]
        };
        for &(x, y, label) in labels {
            p.text(
                xy(x, y),
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(5.0 * z),
                color,
            );
        }
        if c.kind == Kind::Counter && !c.sequential.counter_old {
            p.text(
                xy(-20, -16),
                Align2::CENTER_CENTER,
                "EN",
                FontId::monospace(5.0 * z),
                color,
            );
        }
        let clock = c.local_ports()[2];
        let points = if c.kind == Kind::Counter {
            [
                Point::new(clock.x - 4, 20),
                Point::new(clock.x, 15),
                Point::new(clock.x + 4, 20),
            ]
        } else if c.kind == Kind::Random {
            [
                Point::new(-30, -14),
                Point::new(-25, -10),
                Point::new(-30, -6),
            ]
        } else {
            [Point::new(0, 6), Point::new(5, 10), Point::new(0, 14)]
        };
        p.add(egui::Shape::line(
            points.into_iter().map(at).collect(),
            stroke,
        ));
        if c.sequential.trigger == Trigger::Falling {
            p.circle_stroke(at(clock), 2.5 * z, stroke);
        }
        let value = simulation.and_then(|s| s.storage.get(&c.id));
        let label = value.map_or("0", |s| s.text.as_str());
        let tone = value.map_or(color, |s| s.tone.color());
        let x = if c.kind == Kind::ShiftRegister {
            15
        } else {
            -15
        };
        if label.len() > 4 && label.is_ascii() {
            let mid = label.len().div_ceil(2);
            for (y, line) in [(-5, &label[..mid]), (5, &label[mid..])] {
                p.text(
                    xy(x, y),
                    Align2::CENTER_CENTER,
                    line,
                    FontId::monospace(7.0 * z),
                    tone,
                );
            }
        } else {
            p.text(
                xy(x, 0),
                Align2::CENTER_CENTER,
                label,
                FontId::monospace(8.0 * z),
                tone,
            );
        }
        return;
    }
    let text = |point: Point, label: &str, color| {
        p.text(
            at(point),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(8.0 * z),
            color,
        );
    };
    let ports = c.local_ports();
    for port in &ports {
        let inside = Point::new(port.x.clamp(min.x, max.x), port.y.clamp(min.y, max.y));
        p.line_segment([at(*port), at(inside)], stroke);
    }
    let clock = ports[2];
    if c.kind == Kind::Register {
        text(Point::new(-24, 0), "D", color);
        text(Point::new(-5, 0), "Q", color);
        text(
            Point::new(-26, -10),
            if c.sequential.select_high { "C" } else { "/C" },
            color,
        );
        text(Point::new(-26, 10), "E", color);
        text(Point::new(-10, -15), "S", color);
        text(Point::new(-10, 15), "R", color);
    } else if let Kind::FlipFlop(ff) = c.kind {
        let labels: &[&str] = match ff {
            FlipFlop::D => &["D"],
            FlipFlop::T => &["T"],
            FlipFlop::Jk => &["J", "K"],
            FlipFlop::Sr => &["S", "R"],
        };
        for (i, label) in labels.iter().enumerate() {
            text(Point::new(min.x + 5, ports[3 + i].y), label, color);
        }
        text(Point::new(max.x - 6, 0), "Q", color);
        text(Point::new(max.x - 7, ports[1].y), "/Q", color);
        for (port, label) in ports[ports.len() - 2..].iter().zip(["S", "R"]) {
            let point = Point::new(port.x, if port.y < min.y { min.y + 5 } else { max.y - 5 });
            text(point, label, color);
            if c.sequential.negate_controls {
                p.circle_stroke(
                    at(Point::new(port.x, port.y.clamp(min.y, max.y))),
                    2.5 * z,
                    stroke,
                );
            }
        }
    }
    if matches!(c.sequential.trigger, Trigger::Rising | Trigger::Falling) {
        let points = if c.kind == Kind::Register {
            [
                Point::new(clock.x - 4, max.y),
                Point::new(clock.x, max.y - 5),
                Point::new(clock.x + 4, max.y),
            ]
        } else {
            [
                Point::new(min.x, clock.y - 4),
                Point::new(min.x + 5, clock.y),
                Point::new(min.x, clock.y + 4),
            ]
        };
        p.add(egui::Shape::line(
            points.into_iter().map(at).collect(),
            stroke,
        ));
        if c.sequential.trigger == Trigger::Falling {
            p.circle_stroke(at(clock), 2.5 * z, stroke);
        }
    } else {
        text(
            Point::new(clock.x, max.y - 4),
            if c.sequential.trigger == Trigger::High {
                "H"
            } else {
                "L"
            },
            color,
        );
    }
    let state = simulation.and_then(|s| s.storage.get(&c.id));
    let center = if c.kind == Kind::Register {
        Point::new(-14, 0)
    } else {
        Point::new((min.x + max.x) / 2, (min.y + max.y) / 2)
    };
    // Long bus values remain available in the inspector without enlarging terminal geometry.
    let label = state.map_or("0", |s| s.text.as_str());
    p.text(
        at(center),
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(if c.kind == Kind::Register {
            (24.0 / label.len().max(3) as f32).min(8.0) * z
        } else {
            12.0 * z
        }),
        state.map_or(color, |s| s.tone.color()),
    );
}

use crate::{canvas::Canvas, i18n::tr, tf};
use egui::{Align2, Color32, FontId, Painter, Rect, Shape, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Kind, Point, Ttl};

pub(crate) fn port_label(c: &Component, index: usize) -> String {
    let pin = c.ttl_pins()[index];
    let role = match pin {
        7 => "GND",
        14 => "Vcc",
        _ if c.ttl_output(index) => tr("Output"),
        _ => tr("Input"),
    };
    tf!("Pin {} · {}", pin, role)
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) {
    if matches!(c.kind, Kind::Ttl(_)) {
        ui.label(tr("Include Vcc/Gnd pins"));
        ui.checkbox(&mut c.ttl.power_pins, "").on_hover_text(tr(
            "Changing TTL facing or power pins keeps wire positions; reconnect moved ports.",
        ));
        ui.end_row();
        ui.label(tr("Show internal structure"));
        ui.checkbox(&mut c.ttl.internal, "");
        ui.end_row();
    }
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32) {
    let Kind::Ttl(kind) = c.kind else {
        return;
    };
    let at = |x, y| canvas.screen(c.position + c.facing.transform(Point::new(x, y)));
    let zoom = canvas.camera.zoom;
    let ink = if c.ttl.internal {
        color
    } else {
        Color32::from_rgb(235, 241, 248)
    };
    let fill = if c.ttl.internal {
        Color32::from_rgb(248, 250, 253)
    } else {
        Color32::from_rgb(48, 60, 77)
    };
    let stroke = Stroke::new(1.2, color);
    let line = |a: Point, b: Point| {
        p.line_segment([at(a.x, a.y), at(b.x, b.y)], stroke);
    };
    p.rect(
        Rect::from_two_pos(at(0, -23), at(140, 23)),
        3.0 * zoom,
        fill,
        stroke,
        StrokeKind::Inside,
    );
    let notch = (-3..=3)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::PI / 6.0;
            at(
                (6.0 * angle.cos()).round() as i32,
                (6.0 * angle.sin()).round() as i32,
            )
        })
        .collect();
    p.add(Shape::line(notch, Stroke::new(1.2, ink)));
    for pin in 1..=14 {
        let pos = Ttl::pin_position(pin);
        let sign = pos.y.signum();
        p.rect(
            Rect::from_two_pos(at(pos.x - 3, sign * 23), at(pos.x + 3, sign * 29)),
            0.0,
            Color32::from_rgb(160, 170, 185),
            stroke,
            StrokeKind::Inside,
        );
        p.text(
            at(pos.x, sign * 18),
            Align2::CENTER_CENTER,
            pin.to_string(),
            FontId::monospace(7.0 * zoom),
            ink,
        );
    }
    if !c.ttl.internal {
        p.text(
            at(70, 0),
            Align2::CENTER_CENTER,
            kind.name(),
            FontId::monospace(14.0 * zoom),
            ink,
        );
        return;
    }
    for &[a, b, o] in kind.gates() {
        let a = Ttl::pin_position(a);
        let b = Ttl::pin_position(b);
        let out = Ttl::pin_position(o);
        let cx = (a.x.min(b.x).min(out.x) + a.x.max(b.x).max(out.x)) / 2;
        let cy = if out.y > 0 { 8 } else { -8 };
        let direction = if out.x > a.x { 1 } else { -1 };
        let input_x = cx - direction * 7;
        for (pin, y) in if a == b {
            vec![(a, cy)]
        } else {
            vec![(a, cy - 3), (b, cy + 3)]
        } {
            line(pin, Point::new(pin.x, y));
            line(Point::new(pin.x, y), Point::new(input_x, y));
        }
        let inverted = matches!(
            kind,
            Ttl::Ic7400 | Ttl::Ic7402 | Ttl::Ic7404 | Ttl::Ic747266
        );
        let output_x = cx + direction * if inverted { 11 } else { 7 };
        line(Point::new(output_x, cy), Point::new(out.x, cy));
        line(Point::new(out.x, cy), out);
        p.rect(
            Rect::from_two_pos(at(cx - 7, cy - 6), at(cx + 7, cy + 6)),
            0.0,
            fill,
            stroke,
            StrokeKind::Inside,
        );
        if inverted {
            p.circle(at(cx + direction * 9, cy), 2.0 * zoom, fill, stroke);
        }
        let symbol = match kind {
            Ttl::Ic7400 | Ttl::Ic7408 => "&",
            Ttl::Ic7402 | Ttl::Ic7432 => "≥1",
            Ttl::Ic7404 => "1",
            _ => "=1",
        };
        p.text(
            at(cx, cy),
            Align2::CENTER_CENTER,
            symbol,
            FontId::monospace(7.0 * zoom),
            ink,
        );
    }
}

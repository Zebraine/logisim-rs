use crate::{canvas::Canvas, i18n::tr, tf};
use egui::{Align2, Color32, FontId, Painter, Stroke, Ui};
use logisim_core::{Component, Kind, Plexer, PlexerRole};
pub(crate) fn port_label(role: PlexerRole) -> String {
    match role {
        PlexerRole::Input(i) => tf!("Input {}", i),
        PlexerRole::Output(i) => tf!("Output {}", i),
        PlexerRole::Select => tr("Select").into(),
        PlexerRole::Enable => tr("Enable").into(),
        PlexerRole::Group => tr("Group signal").into(),
        PlexerRole::EnableOut => tr("Enable output").into(),
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    let Kind::Plexer(kind) = c.kind else {
        return false;
    };
    let p = &mut c.plexing;
    ui.label(tr("Select bits"));
    let r = ui.add(egui::DragValue::new(&mut p.select_bits).range(1..=5));
    let active = r.has_focus() || r.dragged();
    ui.end_row();
    if kind != Plexer::PriorityEncoder {
        ui.label(tr("Select location"));
        egui::ComboBox::from_id_salt("plexer_select_location")
            .selected_text(tr(if p.top_right {
                "Top / right"
            } else {
                "Bottom / left"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut p.top_right, false, tr("Bottom / left"));
                ui.selectable_value(&mut p.top_right, true, tr("Top / right"));
            });
        ui.end_row();
        ui.label(tr("Include enable"));
        ui.checkbox(&mut p.enable, "");
        ui.end_row();
        if kind != Plexer::Multiplexer {
            ui.label(tr("Three-state outputs"));
            ui.checkbox(&mut p.three_state, "");
            ui.end_row();
        }
    }
    for (key, label, value) in [
        ("plexer_disabled", "Disabled output", &mut p.disabled_zero),
        ("plexer_no_input", "No input state", &mut p.no_input_zero),
    ] {
        if label == "No input state" && kind != Plexer::PriorityEncoder {
            continue;
        }
        ui.label(tr(label));
        egui::ComboBox::from_id_salt(key)
            .selected_text(tr(if *value { "Zero output" } else { "Floating" }))
            .show_ui(ui, |ui| {
                ui.selectable_value(value, false, tr("Floating"));
                ui.selectable_value(value, true, tr("Zero output"));
            });
        ui.end_row();
    }
    active
}
pub(crate) fn draw(p: &Painter, canvas: &Canvas, c: &Component, color: Color32) {
    let Kind::Plexer(kind) = c.kind else {
        return;
    };
    let at = |point| canvas.screen(c.position + c.facing.transform(point));
    let body: Vec<_> = c.plexer_body().into_iter().map(at).collect();
    let center = body
        .iter()
        .fold(egui::Vec2::ZERO, |sum, p| sum + p.to_vec2())
        / body.len() as f32;
    p.add(egui::Shape::convex_polygon(
        body,
        Color32::from_rgb(244, 248, 255),
        Stroke::new(1.6, color),
    ));
    let label = match kind {
        Plexer::Multiplexer => "MUX",
        Plexer::Demultiplexer => "DMX",
        Plexer::Decoder => "DEC",
        Plexer::PriorityEncoder => "PRI",
    };
    p.text(
        center.to_pos2(),
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(10.0 * canvas.camera.zoom),
        color,
    );
    for terminal in c.plexer_terminals() {
        let label = match terminal.role {
            PlexerRole::Input(0) if kind != Plexer::Demultiplexer => "0",
            PlexerRole::Output(0) if matches!(kind, Plexer::Decoder | Plexer::Demultiplexer) => "0",
            PlexerRole::Select => "S",
            PlexerRole::Enable => "EN",
            PlexerRole::EnableOut => "EO",
            PlexerRole::Group => "GS",
            _ => continue,
        };
        let pos = at(terminal.offset);
        let inward = (center - pos.to_vec2()).normalized() * 9.0 * canvas.camera.zoom;
        p.text(
            pos + inward,
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(7.0 * canvas.camera.zoom),
            color,
        );
    }
}

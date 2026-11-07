use crate::{canvas::Canvas, i18n::tr, tf};
use egui::{Align2, Color32, FontId, Painter, Stroke, Ui};
use logisim_core::{Component, Facing, Kind, Plexer, PlexerRole};
pub(crate) fn port_label(c: &Component, role: PlexerRole) -> String {
    match (c.kind, role) {
        (Kind::Plexer(Plexer::DisplayDecoder), PlexerRole::Output(i)) => {
            return tf!("Segment {}", i);
        }
        (Kind::Plexer(Plexer::DisplayDecoder), PlexerRole::Enable) => {
            return tr("Enable (0 holds outputs)").into();
        }
        (Kind::Plexer(Plexer::BitSelector), PlexerRole::Input(0)) => {
            return tr("Data input").into();
        }
        (Kind::Plexer(Plexer::BitSelector), PlexerRole::Select) => return tr("Group index").into(),
        _ => {}
    }
    match role {
        PlexerRole::Input(i) => tf!("Input {}", i),
        PlexerRole::Output(i) => tf!("Output {}", i),
        PlexerRole::Select => tr("Select").into(),
        PlexerRole::Enable => tr("Enable").into(),
        PlexerRole::Group => tr("Group signal").into(),
        PlexerRole::EnableOut => tr("Enable output").into(),
        PlexerRole::LampTest => tr("Lamp test (LT, active low)").into(),
        PlexerRole::Blanking => tr("Blanking (BI, active low)").into(),
        PlexerRole::ZeroBlanking => tr("Zero blanking (RBI, active low)").into(),
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    let Kind::Plexer(kind) = c.kind else {
        return false;
    };
    let p = &mut c.plexing;
    if kind == Plexer::DisplayDecoder {
        ui.label(tr("Bus input"));
        ui.checkbox(&mut p.multibit, "").on_hover_text(tr(
            "Changing the input format keeps wire positions; reconnect data inputs.",
        ));
        ui.end_row();
        return false;
    }
    let mut active = false;
    if kind == Plexer::BitSelector {
        ui.label(tr("Output bits"));
        let r = ui.add(egui::DragValue::new(&mut p.group_bits).range(1..=32));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    ui.label(tr("Select bits"));
    if kind == Plexer::BitSelector {
        let groups = c.bits.clamp(1, 32).div_ceil(p.group_bits.clamp(1, 32));
        ui.label((8 - (groups - 1).leading_zeros() as u8).max(1).to_string());
    } else {
        let r = ui.add(egui::DragValue::new(&mut p.select_bits).range(1..=5));
        active |= r.has_focus() || r.dragged();
    }
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
        if kind == Plexer::BitSelector {
            return active;
        }
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
        Plexer::BitSelector => "SEL",
        Plexer::DisplayDecoder => "7SEG",
    };
    p.text(
        center.to_pos2(),
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(
            if kind == Plexer::DisplayDecoder {
                8.0
            } else {
                10.0
            } * canvas.camera.zoom,
        ),
        color,
    );
    for terminal in c.plexer_terminals() {
        if kind == Plexer::BitSelector {
            continue;
        }
        let numbered;
        let label = match terminal.role {
            PlexerRole::Input(0) if kind == Plexer::DisplayDecoder && c.plexing.multibit => "D",
            PlexerRole::Input(i) | PlexerRole::Output(i) if kind == Plexer::DisplayDecoder => {
                numbered = i.to_string();
                &numbered
            }
            PlexerRole::Input(0) if kind != Plexer::Demultiplexer => "0",
            PlexerRole::Output(0) if matches!(kind, Plexer::Decoder | Plexer::Demultiplexer) => "0",
            PlexerRole::Select => "S",
            PlexerRole::Enable => "EN",
            PlexerRole::EnableOut => "EO",
            PlexerRole::Group => "GS",
            PlexerRole::LampTest => "LT",
            PlexerRole::Blanking => "BI",
            PlexerRole::ZeroBlanking => "RBI",
            _ => continue,
        };
        let pos = at(terminal.offset);
        let (inward, size) = if kind == Plexer::DisplayDecoder {
            let along = match c.facing {
                Facing::East => egui::vec2(1.0, 0.0),
                Facing::West => egui::vec2(-1.0, 0.0),
                Facing::South => egui::vec2(0.0, 1.0),
                Facing::North => egui::vec2(0.0, -1.0),
            };
            let across = if matches!(c.facing, Facing::East | Facing::West) {
                egui::vec2(0.0, 1.0)
            } else {
                egui::vec2(1.0, 0.0)
            };
            let direction = match terminal.role {
                PlexerRole::Input(_) => along,
                PlexerRole::Output(_) => -along,
                PlexerRole::Enable => across,
                _ => -across,
            };
            (direction * 5.0, 5.0)
        } else {
            ((center - pos.to_vec2()).normalized() * 9.0, 7.0)
        };
        p.text(
            pos + inward * canvas.camera.zoom,
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(size * canvas.camera.zoom),
            color,
        );
    }
}

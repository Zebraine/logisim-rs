use crate::{
    canvas::Canvas,
    i18n::tr,
    simulation::{SimulationAction, SimulationView},
};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Id, Kind, Point, Trigger};

#[derive(Clone, Debug, Default)]
pub struct TtyView {
    pub lines: Vec<Vec<u16>>,
    pub row: usize,
    pub column: usize,
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    ui.label(tr("Character encoding"));
    egui::ComboBox::from_id_salt("terminal_encoding")
        .selected_text(tr(if c.bits == 7 {
            "ASCII (7-bit)"
        } else {
            "UTF-16 (16-bit)"
        }))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut c.bits, 7, tr("ASCII (7-bit)"));
            ui.selectable_value(&mut c.bits, 16, tr("UTF-16 (16-bit)"));
        });
    ui.end_row();
    let mut active = false;
    if c.kind == Kind::Keyboard {
        ui.label(tr("Buffer length"));
        let r = ui.add(egui::DragValue::new(&mut c.terminal.buffer_len).range(1..=256));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    } else {
        for (label, value, max) in [
            ("Display rows", &mut c.terminal.rows, 48),
            ("Display columns", &mut c.terminal.columns, 120),
        ] {
            ui.label(tr(label));
            let r = ui.add(egui::DragValue::new(value).range(1..=max));
            active |= r.has_focus() || r.dragged();
            ui.end_row();
        }
    }
    ui.label(tr("Trigger"));
    egui::ComboBox::from_id_salt("terminal_trigger")
        .selected_text(tr(c.terminal.trigger.name()))
        .show_ui(ui, |ui| {
            for trigger in [Trigger::Rising, Trigger::Falling] {
                ui.selectable_value(&mut c.terminal.trigger, trigger, tr(trigger.name()));
            }
        });
    ui.end_row();
    if c.kind == Kind::Tty {
        for (label, color) in [
            ("Text color", &mut c.io.on_color),
            ("Background", &mut c.io.background),
        ] {
            ui.label(tr(label));
            ui.color_edit_button_srgba_unmultiplied(color);
            ui.end_row();
        }
    }
    active
}
pub(crate) fn input(ui: &Ui, id: Id, actions: &mut Vec<SimulationAction>) -> bool {
    let mut keep_focus = true;
    ui.input_mut(|i| {
        i.events.retain(|event| {
            if !keep_focus {
                return true;
            }
            let mut text = None;
            match event {
                egui::Event::Text(s) | egui::Event::Paste(s) => text = Some(s.as_str()),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command && !modifiers.ctrl && !modifiers.alt => match key {
                    egui::Key::Enter => text = Some("\n"),
                    egui::Key::Backspace => text = Some("\u{8}"),
                    egui::Key::Escape | egui::Key::Tab => keep_focus = false,
                    _ => {}
                },
                _ => {}
            }
            if let Some(text) = text {
                // Native X11 character input exposes one low-16-bit code per key symbol.
                for value in text.chars().map(|ch| ch as u32 as u16) {
                    actions.push(SimulationAction::TypeCharacter { id, value });
                }
                return false;
            }
            true
        });
    });
    keep_focus
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    ink: Color32,
    view: Option<&SimulationView>,
) {
    let [min, max] = c.io_body();
    let body = Rect::from_two_pos(
        canvas.screen(c.position + min),
        canvas.screen(c.position + max),
    );
    let z = canvas.camera.zoom;
    let rgba = |a: [u8; 4]| Color32::from_rgba_unmultiplied(a[0], a[1], a[2], a[3]);
    let keyboard = c.kind == Kind::Keyboard;
    let focused = view.is_some_and(|v| v.keyboard_focus == Some(c.id));
    p.rect(
        body,
        4.0 * z,
        if keyboard {
            Color32::WHITE
        } else {
            rgba(c.io.background)
        },
        Stroke::new(
            if focused { 2.0 } else { 1.5 },
            if focused { crate::ACCENT } else { ink },
        ),
        StrokeKind::Inside,
    );
    let cp = p.with_clip_rect(body.shrink(2.0 * z));
    let font = FontId::monospace(12.0 * z);
    if keyboard {
        let buffer = view.and_then(|v| v.keyboards.get(&c.id));
        let text: String = buffer
            .into_iter()
            .flatten()
            .map(|v| match *v {
                8 => '⌫',
                10 | 13 => '↵',
                v => char::from_u32(u32::from(v)).unwrap_or('�'),
            })
            .collect();
        cp.text(
            body.left_center() + egui::vec2(5.0 * z, 0.0),
            Align2::LEFT_CENTER,
            text,
            font,
            ink,
        );
    } else {
        let screen = view.and_then(|v| v.ttys.get(&c.id));
        let at = |row: usize, col: usize| {
            body.left_top()
                + egui::vec2((4.0 + 7.0 * col as f32) * z, (3.0 + 15.0 * row as f32) * z)
        };
        if let Some(screen) = screen {
            for (row, line) in screen.lines.iter().enumerate() {
                cp.text(
                    at(row, 0),
                    Align2::LEFT_TOP,
                    String::from_utf16_lossy(line),
                    font.clone(),
                    rgba(c.io.on_color),
                );
            }
        }
        let (row, column) = screen.map_or((0, 0), |s| (s.row, s.column));
        let cursor = at(row, column);
        cp.line_segment(
            [cursor, cursor + egui::vec2(0.0, 12.0 * z)],
            Stroke::new(z, rgba(c.io.on_color)),
        );
    }
    let clock = canvas.screen(c.position);
    p.line_segment(
        [
            clock + egui::vec2(0.0, -4.0 * z),
            clock + egui::vec2(5.0 * z, 0.0),
        ],
        Stroke::new(z, ink),
    );
    p.line_segment(
        [
            clock + egui::vec2(5.0 * z, 0.0),
            clock + egui::vec2(0.0, 4.0 * z),
        ],
        Stroke::new(z, ink),
    );
    if c.terminal.trigger == Trigger::Falling {
        p.circle_stroke(
            canvas.screen(c.position + Point::new(-3, 0)),
            2.5 * z,
            Stroke::new(z, ink),
        );
    }
}

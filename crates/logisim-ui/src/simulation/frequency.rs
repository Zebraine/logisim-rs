//! Runtime frequency controls; changing them never edits the circuit document.
use super::{SimulationAction, SimulationUi};
use crate::dialog::{Dialog, Dismiss};
use crate::{TickHz, i18n::tr, tf};
use egui::{Context, Key, TextEdit, Ui};

pub const DEFAULT_TICK_HZ: u32 = 4;
pub const MAX_TICK_HZ: u32 = 32768;
const PRESETS: [u32; 16] = [
    1,
    2,
    4,
    8,
    16,
    32,
    64,
    128,
    256,
    512,
    1024,
    2048,
    4096,
    8192,
    16384,
    MAX_TICK_HZ,
];

// Frequency shorthand follows the binary tick presets: 1k = 1024 Hz.
pub(super) fn format_target(hz: u32) -> String {
    if hz < 1024 {
        hz.to_string()
    } else {
        let scaled = format!("{:.3}", f64::from(hz) / 1024.0);
        format!("{}k", scaled.trim_end_matches('0').trim_end_matches('.'))
    }
}

pub(super) fn format_actual(hz: f64) -> String {
    if hz < 1024.0 {
        format!("{hz:.1}")
    } else {
        format!("{:.1}k", hz / 1024.0)
    }
}

impl SimulationUi {
    pub(crate) fn set_tick_rate(&mut self, current: &mut TickHz, rate: TickHz) {
        if *current != rate {
            *current = rate;
            self.actions.push(SimulationAction::SetTickRate(rate.get()));
        }
    }

    pub(super) fn frequency_menu(&mut self, ui: &mut Ui, rate: &mut TickHz) {
        ui.menu_button(tr("Tick frequency"), |ui| {
            for hz in PRESETS {
                if ui
                    .selectable_label(rate.get() == hz, format!("{} Hz", format_target(hz)))
                    .on_hover_text(format!("{hz} Hz"))
                    .clicked()
                {
                    self.set_tick_rate(rate, TickHz::new(hz).expect("valid frequency"));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .selectable_label(!PRESETS.contains(&rate.get()), tr("Custom…"))
                .clicked()
            {
                self.tick_rate_draft = Some(rate.get().to_string());
                self.tick_rate_focus = true;
                ui.close();
            }
        });
    }
    pub(crate) fn frequency_dialog(&mut self, ctx: &Context, rate: &mut TickHz) {
        let Some(draft) = &mut self.tick_rate_draft else {
            return;
        };
        let mut apply = None;
        let dismiss = Dialog::prompt("custom-tick-frequency").show(
            ctx,
            tr("Custom tick frequency"),
            |dialog| {
                let value = dialog.body(|ui| {
                    ui.label(tr("Frequency (Hz)"));
                    let input = ui.add(
                        TextEdit::singleline(draft)
                            .id_salt("tick-frequency-value")
                            .char_limit(16),
                    );
                    if self.tick_rate_focus {
                        input.request_focus();
                        self.tick_rate_focus = false;
                    }
                    let value = draft
                        .trim()
                        .parse::<u32>()
                        .ok()
                        .filter(|hz| (1..=MAX_TICK_HZ).contains(hz));
                    if value.is_none() {
                        ui.colored_label(
                            crate::theme::palette(ui.ctx()).danger,
                            tf!("Enter a whole number from 1 to {}.", MAX_TICK_HZ),
                        );
                    }
                    if input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        apply = value;
                    }
                    value
                });
                dialog.footer("Cancel", |ui| {
                    if ui
                        .add_enabled(value.is_some(), egui::Button::new(tr("Apply")))
                        .clicked()
                    {
                        apply = value;
                    }
                });
            },
        );
        if let Some(hz) = apply {
            self.set_tick_rate(rate, TickHz::new(hz).expect("valid frequency"));
        }
        if apply.is_some() || dismiss != Dismiss::Open {
            self.tick_rate_draft = None;
        }
    }
}

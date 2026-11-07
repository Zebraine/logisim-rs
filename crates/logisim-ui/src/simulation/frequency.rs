//! Runtime frequency controls; changing them never edits the circuit document.
use super::{SimulationAction, SimulationUi};
use crate::{i18n::tr, tf};
use egui::{Color32, Context, Id, Key, Modal, TextEdit, Ui};

pub const DEFAULT_TICK_HZ: u32 = 4;
pub const MAX_TICK_HZ: u32 = 4096;
const PRESETS: [u32; 13] = [
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
    MAX_TICK_HZ,
];

impl SimulationUi {
    pub(super) fn frequency_menu(&mut self, ui: &mut Ui) {
        ui.menu_button(tr("Tick frequency"), |ui| {
            for hz in PRESETS {
                if ui
                    .selectable_label(self.view.tick_hz == hz, format!("{hz} Hz"))
                    .clicked()
                {
                    self.actions.push(SimulationAction::SetTickRate(hz));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .selectable_label(!PRESETS.contains(&self.view.tick_hz), tr("Custom…"))
                .clicked()
            {
                self.tick_rate_draft = Some(self.view.tick_hz.max(1).to_string());
                self.tick_rate_focus = true;
                ui.close();
            }
        });
    }
    pub(crate) fn frequency_dialog(&mut self, ctx: &Context) {
        let Some(draft) = &mut self.tick_rate_draft else {
            return;
        };
        let mut apply = None;
        let mut cancel = false;
        let modal = Modal::new(Id::unique("custom-tick-frequency")).show(ctx, |ui| {
            ui.set_width(300.0);
            ui.heading(tr("Custom tick frequency"));
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
                    Color32::DARK_RED,
                    tf!("Enter a whole number from 1 to {}.", MAX_TICK_HZ),
                );
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(value.is_some(), egui::Button::new(tr("Apply")))
                    .clicked()
                    || (input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                {
                    apply = value;
                }
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        if let Some(hz) = apply {
            self.actions.push(SimulationAction::SetTickRate(hz));
        }
        if apply.is_some() || cancel || modal.should_close() {
            self.tick_rate_draft = None;
        }
    }
}

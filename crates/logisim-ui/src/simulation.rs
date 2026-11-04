//! Presentation and commands only; the desktop host owns the simulation service.
use egui::{Color32, Key, Modifiers, Ui};
use logisim_core::{Component, Id};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalTone {
    Low,
    High,
    Bus,
    Unknown,
    Error,
    WidthMismatch,
}
impl SignalTone {
    pub(crate) fn color(self) -> Color32 {
        match self {
            Self::Low => Color32::from_rgb(21, 110, 65),
            Self::High => Color32::from_rgb(22, 180, 87),
            Self::Bus => crate::INK,
            Self::Unknown => Color32::from_rgb(80, 91, 210),
            Self::Error => Color32::from_rgb(210, 45, 58),
            Self::WidthMismatch => Color32::from_rgb(211, 115, 12),
        }
    }
}
#[derive(Clone, Debug)]
pub struct SignalView {
    pub text: String,
    pub binary: String,
    pub tone: SignalTone,
}
#[derive(Clone, Copy, Debug)]
pub struct InputValue {
    pub bits: u8,
    pub value: u32,
}
impl InputValue {
    pub fn text(self) -> String {
        if self.bits == 1 {
            self.value.to_string()
        } else {
            format!(
                "{:0width$X}",
                self.value,
                width = usize::from(self.bits.div_ceil(4))
            )
        }
    }
}
#[derive(Clone, Debug)]
pub struct SimulationIssue {
    pub message: String,
    pub targets: BTreeSet<Id>,
}
#[derive(Default)]
pub struct SimulationView {
    pub enabled: bool,
    pub pending: usize,
    pub steps: u64,
    pub limited: bool,
    pub wires: BTreeMap<Id, SignalView>,
    pub ports: BTreeMap<(Id, usize), SignalView>,
    pub inputs: BTreeMap<Id, InputValue>,
    pub issues: Vec<SimulationIssue>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationAction {
    ToggleEnabled,
    Step,
    Reset,
    ToggleInput { id: Id, bit: u8 },
    SetInput { id: Id, value: u32 },
}
#[derive(Default)]
pub(crate) struct SimulationUi {
    pub view: SimulationView,
    pub actions: Vec<SimulationAction>,
    pub focus_value: bool,
    draft: Option<(Id, u32, String)>,
}
impl SimulationUi {
    pub fn status(&self) -> String {
        if self.view.limited {
            return "Propagation limit — paused".into();
        }
        format!(
            "{} · {}",
            if self.view.enabled {
                "Running"
            } else {
                "Paused"
            },
            if self.view.pending == 0 {
                "settled".into()
            } else {
                format!("{} pending", self.view.pending)
            }
        )
    }
    pub fn shortcuts(&mut self, ui: &Ui, selected: &BTreeSet<Id>) {
        for (key, action) in [
            (Key::E, SimulationAction::ToggleEnabled),
            (Key::I, SimulationAction::Step),
            (Key::R, SimulationAction::Reset),
        ] {
            if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, key))
                && (action != SimulationAction::Step || !self.view.enabled)
            {
                self.actions.push(action);
            }
        }
        if selected.len() == 1
            && let Some(id) = selected.first()
            && self.view.inputs.contains_key(id)
        {
            for (key, value) in [(Key::Num0, 0), (Key::Num1, 1)] {
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                    self.actions
                        .push(SimulationAction::SetInput { id: *id, value });
                }
            }
        }
    }
    pub fn menu(&mut self, ui: &mut Ui) {
        let label = if self.view.enabled {
            "Pause · Ctrl+E"
        } else {
            "Enable Simulation · Ctrl+E"
        };
        for (label, action, enabled) in [
            (label, SimulationAction::ToggleEnabled, true),
            (
                "Step Simulation · Ctrl+I",
                SimulationAction::Step,
                !self.view.enabled && !self.view.limited,
            ),
            ("Reset Simulation · Ctrl+R", SimulationAction::Reset, true),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                self.actions.push(action);
                ui.close();
            }
        }
        ui.separator();
        ui.add_enabled(false, egui::Button::new("Tick Once — no clock components"));
    }
    pub fn toolbar(&mut self, ui: &mut Ui) {
        if ui
            .button(if self.view.enabled { "Pause" } else { "Run" })
            .on_hover_text("Enable / pause simulation · Ctrl+E")
            .clicked()
        {
            self.actions.push(SimulationAction::ToggleEnabled);
        }
        if ui
            .add_enabled(
                !self.view.enabled && !self.view.limited,
                egui::Button::new("Step"),
            )
            .on_hover_text("Propagate one batch · Ctrl+I")
            .clicked()
        {
            self.actions.push(SimulationAction::Step);
        }
        if ui
            .button("Reset")
            .on_hover_text("Reset all inputs to zero · Ctrl+R")
            .clicked()
        {
            self.actions.push(SimulationAction::Reset);
        }
    }
    pub fn overview(&self, ui: &mut Ui) {
        ui.label(self.status());
        ui.weak(format!("{} propagation steps", self.view.steps));
        if !self.view.enabled && self.view.pending > 0 {
            ui.weak("Inputs have pending changes. Step or Run to propagate.");
        }
        ui.weak("P: Poke · click a 1-bit input to toggle.");
        ui.weak("0 / 1: set selected input. Multi-bit values use hexadecimal.");
    }
    pub fn component(&mut self, ui: &mut Ui, c: &Component) {
        if let Some(signal) = self.view.ports.get(&(c.id, 0)) {
            ui.colored_label(signal.tone.color(), format!("Signal: {}", signal.text))
                .on_hover_text(format!("{} bit: {}", signal.binary.len(), signal.binary));
        }
        let Some(input) = self.view.inputs.get(&c.id).copied() else {
            return;
        };
        if input.bits == 1 {
            ui.horizontal(|ui| {
                ui.label("Input");
                for value in [0, 1] {
                    if ui
                        .selectable_label(input.value == value, value.to_string())
                        .clicked()
                    {
                        self.actions
                            .push(SimulationAction::SetInput { id: c.id, value });
                    }
                }
            });
            return;
        }
        if self.draft.as_ref().is_none_or(|(id, _, _)| *id != c.id) {
            self.draft = Some((c.id, input.value, input.text()));
        }
        let (_, previous, text) = self.draft.as_mut().unwrap();
        ui.label("Input (hex)");
        let response = ui.add(
            egui::TextEdit::singleline(text)
                .id_salt(("simulation_input", c.id))
                .desired_width(130.0)
                .char_limit(10),
        );
        if self.focus_value {
            response.request_focus();
            self.focus_value = false;
        }
        let value = parse_input(text, input.bits);
        let apply = ui
            .add_enabled(value.is_some(), egui::Button::new("Apply"))
            .on_hover_text("Apply the hexadecimal value · Enter")
            .clicked()
            || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
        if apply && let Some(value) = value {
            self.actions
                .push(SimulationAction::SetInput { id: c.id, value });
        }
        if value.is_none() {
            ui.colored_label(
                SignalTone::Error.color(),
                format!("Enter a hexadecimal value within {} bits.", input.bits),
            );
        }
        if *previous != input.value {
            if !response.has_focus() {
                *text = input.text();
            }
            *previous = input.value;
        }
        ui.horizontal_wrapped(|ui| {
            for bit in (0..input.bits).rev() {
                let high = input.value & (1 << bit) != 0;
                if ui
                    .selectable_label(high, if high { "1" } else { "0" })
                    .on_hover_text(format!("Toggle bit {bit}"))
                    .clicked()
                {
                    self.actions
                        .push(SimulationAction::ToggleInput { id: c.id, bit });
                }
            }
        });
    }
}
fn parse_input(text: &str, bits: u8) -> Option<u32> {
    let text = text.trim();
    let text = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    let value = u32::from_str_radix(text, 16).ok()?;
    (bits == 32 || value < (1u32 << bits.clamp(1, 31))).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_input_rejects_overflow_and_preserves_full_u32() {
        assert_eq!(parse_input("0xA", 4), Some(10));
        assert_eq!(parse_input("10", 4), None);
        assert_eq!(parse_input("FFFFFFFF", 32), Some(u32::MAX));
        assert_eq!(parse_input("100000000", 32), None);
        assert_eq!(parse_input("X", 4), None);
        assert_eq!(parse_input("-1", 4), None);
    }
}

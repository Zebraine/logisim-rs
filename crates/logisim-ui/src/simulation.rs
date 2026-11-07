//! Presentation and commands only; the desktop host owns the simulation service.
use crate::{i18n::tr, tf};
use egui::{Color32, Key, Modifiers, Ui};
use logisim_core::{Component, Id, Point};
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
    pub has_clocks: bool,
    pub ticks: u64,
    pub automatic_ticks: bool,
    pub tick_hz: u32,
    pub clocks: BTreeSet<Id>,
    pub buttons: BTreeMap<Id, bool>,
    pub joysticks: BTreeMap<Id, Point>,
    pub indicators: BTreeMap<Id, [bool; 8]>,
    pub rgb: BTreeMap<Id, [u8; 3]>,
    /// Row-major cells: None is an unknown/error signal.
    pub matrices: BTreeMap<Id, Vec<Option<bool>>>,
    pub storage: BTreeMap<Id, SignalView>,
    pub memories: BTreeMap<Id, logisim_core::MemoryContents>,
    pub wires: BTreeMap<Id, SignalView>,
    pub ports: BTreeMap<(Id, usize), SignalView>,
    pub inputs: BTreeMap<Id, InputValue>,
    pub issues: Vec<SimulationIssue>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationAction {
    PressButton {
        id: Id,
    },
    ReleaseButtons,
    SetJoystick {
        id: Id,
        offset: Point,
    },
    ReleaseJoysticks,
    ToggleEnabled,
    Step,
    Reset,
    Tick,
    ToggleTicks,
    SetTickRate(u32),
    PokeClock {
        id: Id,
    },
    ToggleStored {
        id: Id,
    },
    ToggleInput {
        id: Id,
        bit: u8,
    },
    SetInput {
        id: Id,
        value: u32,
    },
    FillMemory {
        id: Id,
        start: u32,
        end: u32,
        value: u32,
    },
}
#[derive(Default)]
pub(crate) struct SimulationUi {
    pub view: SimulationView,
    pub actions: Vec<SimulationAction>,
    pub focus_value: bool,
    draft: Option<(Id, u32, String)>,
    held_button: Option<Id>,
    held_joystick: Option<(Id, Point)>,
}
impl SimulationUi {
    pub fn press_button(&mut self, id: Id) {
        if self.held_button != Some(id) {
            self.release_momentary();
            self.held_button = Some(id);
            self.actions.push(SimulationAction::PressButton { id });
        }
    }
    pub fn press_joystick(&mut self, id: Id) {
        self.release_momentary();
        self.held_joystick = Some((id, Point::ZERO));
    }
    pub fn held_joystick(&self) -> Option<Id> {
        self.held_joystick.map(|(id, _)| id)
    }
    pub fn move_joystick(&mut self, offset: Point) {
        if let Some((id, current)) = &mut self.held_joystick {
            let offset = Point::new(offset.x.clamp(-14, 14), offset.y.clamp(-14, 14));
            if *current != offset {
                *current = offset;
                self.actions
                    .push(SimulationAction::SetJoystick { id: *id, offset });
            }
        }
    }
    pub fn release_momentary(&mut self) {
        if self.held_joystick.take().is_some() {
            self.actions.push(SimulationAction::ReleaseJoysticks);
        }
        if self.held_button.take().is_some() {
            self.actions.push(SimulationAction::ReleaseButtons);
        }
    }
    pub fn status(&self) -> String {
        if self.view.limited {
            return tr("Propagation limit — paused").into();
        }
        format!(
            "{} · {}",
            if self.view.enabled {
                tr("Running")
            } else {
                tr("Paused")
            },
            if self.view.pending == 0 {
                tr("settled").into()
            } else {
                tf!("{} pending", self.view.pending)
            }
        )
    }
    pub fn shortcuts(&mut self, ui: &Ui, selected: &BTreeSet<Id>) {
        for (key, action) in [
            (Key::E, SimulationAction::ToggleEnabled),
            (Key::I, SimulationAction::Step),
            (Key::R, SimulationAction::Reset),
            (Key::T, SimulationAction::Tick),
            (Key::K, SimulationAction::ToggleTicks),
        ] {
            if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, key))
                && (action != SimulationAction::Step || !self.view.enabled)
                && (action != SimulationAction::Tick || self.can_tick())
                && (action != SimulationAction::ToggleTicks || self.view.has_clocks)
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
            if ui
                .add_enabled(enabled, egui::Button::new(tr(label)))
                .clicked()
            {
                self.actions.push(action);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(self.can_tick(), egui::Button::new(tr("Tick once · Ctrl+T")))
            .clicked()
        {
            self.actions.push(SimulationAction::Tick);
            ui.close();
        }
        let mut automatic = self.view.automatic_ticks;
        if ui
            .add_enabled(
                self.view.has_clocks,
                egui::Checkbox::new(&mut automatic, tr("Automatic ticks · Ctrl+K")),
            )
            .changed()
        {
            self.actions.push(SimulationAction::ToggleTicks);
        }
        ui.menu_button(tr("Tick frequency"), |ui| {
            for hz in [1, 2, 4, 8, 16, 32, 64, 128] {
                if ui
                    .selectable_label(self.view.tick_hz == hz, tf!("{} ticks/s", hz))
                    .clicked()
                {
                    self.actions.push(SimulationAction::SetTickRate(hz));
                    ui.close();
                }
            }
        });
    }
    pub fn toolbar(&mut self, ui: &mut Ui) {
        if ui
            .button(if self.view.enabled {
                tr("Pause")
            } else {
                tr("Run")
            })
            .on_hover_text(tr("Enable / pause simulation · Ctrl+E"))
            .clicked()
        {
            self.actions.push(SimulationAction::ToggleEnabled);
        }
        if ui
            .add_enabled(
                !self.view.enabled && !self.view.limited,
                egui::Button::new(tr("Step")),
            )
            .on_hover_text(tr("Propagate one batch · Ctrl+I"))
            .clicked()
        {
            self.actions.push(SimulationAction::Step);
        }
        if ui
            .button(tr("Reset"))
            .on_hover_text(tr("Reset inputs, storage and clocks · Ctrl+R"))
            .clicked()
        {
            self.actions.push(SimulationAction::Reset);
        }
        if ui
            .add_enabled(self.can_tick(), egui::Button::new(tr("Tick")))
            .on_hover_text(tr(
                "Queue one clock tick · Ctrl+T; paused simulation uses Step or Run",
            ))
            .clicked()
        {
            self.actions.push(SimulationAction::Tick);
        }
        let mut automatic = self.view.automatic_ticks;
        if ui
            .add_enabled(
                self.view.has_clocks,
                egui::Checkbox::new(&mut automatic, tr("Auto ticks")),
            )
            .on_hover_text(tr("Automatic ticks · Ctrl+K"))
            .changed()
        {
            self.actions.push(SimulationAction::ToggleTicks);
        }
    }
    fn can_tick(&self) -> bool {
        self.view.has_clocks && self.view.pending == 0 && !self.view.limited
    }
    pub fn overview(&self, ui: &mut Ui) {
        ui.label(self.status());
        ui.weak(tf!("{} propagation steps", self.view.steps));
        if self.view.has_clocks {
            ui.weak(tf!(
                "{} clock ticks · {} ticks/s",
                self.view.ticks,
                self.view.tick_hz
            ));
        }
        if !self.view.enabled && self.view.pending > 0 {
            ui.weak(tr("Inputs have pending changes. Step or Run to propagate."));
        }
        ui.weak(tr("P: Poke · click a 1-bit input to toggle."));
        ui.weak(tr(
            "0 / 1: set selected input. Multi-bit values use hexadecimal.",
        ));
    }
    pub fn component(&mut self, ui: &mut Ui, c: &Component) {
        match c.kind {
            logisim_core::Kind::Switch => {
                ui.weak(tr("Poke: click to open or close the switch."));
            }
            logisim_core::Kind::DipSwitch => {
                ui.weak(tr("Poke: click each switch to toggle its output."));
            }
            logisim_core::Kind::Joystick => {
                ui.label(tr("Drag to move; release to center."));
                return;
            }
            logisim_core::Kind::Slider => {
                ui.weak(tr(
                    "Poke: drag the handle. The value is saved with the circuit.",
                ));
            }
            _ => {}
        }
        if c.kind == logisim_core::Kind::Button {
            ui.weak(tr(
                "Poke: hold the left mouse button to press; release anywhere.",
            ));
            if ui.button(tr("Hold to press")).is_pointer_button_down_on() {
                self.press_button(c.id);
            }
        }
        if let Some(state) = self.view.storage.get(&c.id) {
            ui.colored_label(state.tone.color(), tf!("Stored value: {}", state.text))
                .on_hover_text(&state.binary);
            if matches!(
                c.kind,
                logisim_core::Kind::FlipFlop(_) | logisim_core::Kind::Latch(_)
            ) && ui.button(tr("Toggle stored bit")).clicked()
            {
                self.actions
                    .push(SimulationAction::ToggleStored { id: c.id });
            }
        }
        if self.view.clocks.contains(&c.id) && ui.button(tr("Toggle clock output")).clicked() {
            self.actions.push(SimulationAction::PokeClock { id: c.id });
        }
        if let Some(signal) = self.view.ports.get(&(c.id, 0)) {
            ui.colored_label(signal.tone.color(), tf!("Signal: {}", signal.text))
                .on_hover_text(tf!("{} bit: {}", signal.binary.len(), signal.binary));
        }
        let Some(input) = self.view.inputs.get(&c.id).copied() else {
            return;
        };
        if input.bits == 1 {
            ui.horizontal(|ui| {
                ui.label(tr("Input"));
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
        ui.label(tr("Input (hex)"));
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
            .add_enabled(value.is_some(), egui::Button::new(tr("Apply")))
            .on_hover_text(tr("Apply the hexadecimal value · Enter"))
            .clicked()
            || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
        if apply && let Some(value) = value {
            self.actions
                .push(SimulationAction::SetInput { id: c.id, value });
        }
        if value.is_none() {
            ui.colored_label(
                SignalTone::Error.color(),
                tf!("Enter a hexadecimal value within {} bits.", input.bits),
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
                    .on_hover_text(tf!("Toggle bit {}", bit))
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

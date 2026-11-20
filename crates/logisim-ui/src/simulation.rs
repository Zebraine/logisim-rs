//! Presentation and commands only; the desktop host owns the simulation service.
mod frequency;
use crate::{i18n::tr, tf, theme};
use egui::{Key, Modifiers, Ui};
pub use frequency::{DEFAULT_TICK_HZ, MAX_TICK_HZ};
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
    pub(crate) fn bit(high: bool) -> Self {
        if high { Self::High } else { Self::Low }
    }

    pub(crate) fn word(bits: u8, value: u32) -> Self {
        if bits == 1 {
            Self::bit(value != 0)
        } else {
            Self::Bus
        }
    }
}
#[derive(Clone, Debug)]
pub struct SignalView {
    pub text: String,
    pub binary: String,
    pub tone: SignalTone,
}
impl SignalView {
    pub(crate) fn value_text(&self, ctx: &egui::Context, value: &str) -> egui::RichText {
        egui::RichText::new(value)
            .monospace()
            .color(theme::signal_text(ctx, self.tone))
    }

    pub(crate) fn pill(&self, ui: &mut Ui) {
        egui::Frame::NONE
            .fill(theme::SUNKEN)
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        dot.center(),
                        3.5,
                        theme::signal(ui.ctx(), self.tone),
                    );
                    ui.label(self.value_text(ui.ctx(), &self.text));
                    ui.separator();
                    ui.label(
                        egui::RichText::new(tf!("{} bit", self.binary.len())).color(theme::MUTED),
                    );
                });
            });
        if self.text != self.binary {
            ui.label(self.value_text(ui.ctx(), &self.binary));
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputValue {
    pub bits: u8,
    pub value: u32,
    pub unknown: u32,
    pub error: u32,
}
impl InputValue {
    pub fn bit_text(self, bit: u8) -> &'static str {
        let mask = 1u32 << bit;
        if self.error & mask != 0 {
            "E"
        } else if self.unknown & mask != 0 {
            "X"
        } else if self.value & mask != 0 {
            "1"
        } else {
            "0"
        }
    }
    pub fn text(self) -> String {
        if self.bits == 1 {
            self.bit_text(0).into()
        } else {
            (0..self.bits.div_ceil(4))
                .rev()
                .map(|n| {
                    let mask = 15u32 << (n * 4);
                    if self.error & mask != 0 {
                        'E'
                    } else if self.unknown & mask != 0 {
                        'X'
                    } else {
                        char::from_digit((self.value >> (n * 4)) & 15, 16)
                            .unwrap()
                            .to_ascii_uppercase()
                    }
                })
                .collect()
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
    pub plugin_error_count: usize,
    pub plugin_rasters: BTreeMap<Id, std::sync::Arc<logisim_core::plugin::api::raster::Image>>,
    pub plugin_graphics: BTreeMap<Id, Vec<logisim_core::plugin::api::Shape>>,
    pub plugin_errors: BTreeMap<Id, String>,
    pub enabled: bool,
    pub pending: usize,
    pub steps: u64,
    pub limited: bool,
    pub has_clocks: bool,
    pub ticks: u64,
    pub automatic_ticks: bool,
    pub tick_hz: u32,
    /// Host measurements over a wall-clock sampling interval, not target rates.
    pub frames_per_second: Option<f64>,
    pub actual_tick_hz: f64,
    pub clocks: BTreeSet<Id>,
    pub buttons: BTreeMap<Id, bool>,
    pub joysticks: BTreeMap<Id, Point>,
    pub keyboards: BTreeMap<Id, Vec<u16>>,
    pub ttys: BTreeMap<Id, crate::TtyView>,
    pub keyboard_focus: Option<Id>,
    pub indicators: BTreeMap<Id, [bool; 8]>,
    pub rgb: BTreeMap<Id, [u8; 3]>,
    /// Row-major cells: None is an unknown/error signal.
    pub matrices: BTreeMap<Id, Vec<Option<bool>>>,
    pub storage: BTreeMap<Id, SignalView>,
    pub memories: BTreeMap<Id, logisim_core::MemoryContents>,
    pub wires: BTreeMap<Id, std::sync::Arc<SignalView>>,
    pub ports: BTreeMap<(Id, usize), std::sync::Arc<SignalView>>,
    pub inputs: BTreeMap<Id, InputValue>,
    pub issues: Vec<SimulationIssue>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimulationAction {
    PluginInput {
        target: Box<crate::PluginInputTarget>,
        event: logisim_core::plugin::api::interaction::InputEvent,
    },
    PluginAction {
        id: Id,
        action: String,
        argument: String,
    },
    PressButton {
        id: Id,
    },
    ReleaseButtons,
    SetJoystick {
        id: Id,
        offset: Point,
    },
    ReleaseJoysticks,
    TypeCharacter {
        id: Id,
        value: u16,
    },
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
    SetInputUnknown {
        id: Id,
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
    pub plugin_keyboard: Option<crate::plugins::keyboard::Focus>,
    pub tick_rate_draft: Option<String>,
    tick_rate_focus: bool,
    pub plugin_arguments: BTreeMap<(Id, String), String>,
    pub view: SimulationView,
    pub actions: Vec<SimulationAction>,
    pub focus_value: bool,
    pub keyboard_focus: Option<Id>,
    draft: Option<(Id, InputValue, String)>,
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
            if self.view.plugin_error_count > 0 {
                tf!("{} plugin error(s)", self.view.plugin_error_count)
            } else if self.view.pending == 0 {
                tr("settled").into()
            } else {
                tf!("{} pending", self.view.pending)
            }
        )
    }
    pub fn input_shortcuts(&mut self, ui: &Ui, selected: &BTreeSet<Id>) {
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
    pub fn menu(&mut self, ui: &mut Ui, rate: &mut crate::TickHz) {
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
        self.frequency_menu(ui, rate);
    }
    pub fn toolbar(&mut self, ui: &mut Ui) {
        if ui
            .scope(|ui| {
                let colors = theme::colors(ui.ctx());
                let v = &mut ui.visuals_mut().widgets;
                v.inactive.weak_bg_fill = colors.base;
                v.hovered.weak_bg_fill = colors.hover;
                v.active.weak_bg_fill = colors.hover;
                ui.add(
                    egui::Button::new(
                        egui::RichText::new(tr(if self.view.enabled { "Pause" } else { "Run" }))
                            .color(theme::SURFACE)
                            .strong(),
                    )
                    .min_size(egui::vec2(60.0, theme::metrics(ui.ctx()).row)),
                )
            })
            .inner
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
        let automatic = self.view.automatic_ticks;
        let switch = ui
            .add_enabled_ui(self.view.has_clocks, |ui| {
                let text = ui.painter().layout_no_wrap(
                    tr("Auto ticks").into(),
                    egui::FontId::proportional(13.0),
                    theme::INK,
                );
                let (rect, r) = ui.allocate_exact_size(
                    egui::vec2(36.0 + text.size().x, theme::metrics(ui.ctx()).control),
                    egui::Sense::click(),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Switch,
                        ui.is_enabled(),
                        automatic,
                        tr("Auto ticks"),
                    )
                });
                let t = ui.ctx().animate_bool_with_time(r.id, automatic, 0.12);
                let rail = egui::Rect::from_center_size(
                    rect.left_center() + egui::vec2(15.0, 0.0),
                    egui::vec2(28.0, 16.0),
                );
                ui.painter().rect_filled(
                    rail,
                    8,
                    theme::BORDER_STRONG.lerp_to_gamma(theme::colors(ui.ctx()).base, t),
                );
                ui.painter().circle_filled(
                    rail.left_center() + egui::vec2(8.0 + t * 12.0, 0.0),
                    6.0,
                    theme::SURFACE,
                );
                ui.painter().galley(
                    rect.left_center() + egui::vec2(36.0, -text.size().y / 2.0),
                    text,
                    theme::INK,
                );
                theme::focus_ring(ui, &r);
                r
            })
            .inner;
        if switch
            .on_hover_text(tr("Automatic ticks · Ctrl+K"))
            .clicked()
        {
            self.actions.push(SimulationAction::ToggleTicks);
        }
    }

    pub fn telemetry(&self, ui: &mut Ui) {
        let fps = self
            .view
            .frames_per_second
            .map_or_else(|| "—".into(), |value| format!("{value:.1}"));
        let metrics = [
            (
                tf!("FPS: {}", fps),
                "Completed UI frames per second, measured over about one second. Idle redraws are included.",
            ),
            (
                tf!("Target: {} Hz", frequency::format_target(self.view.tick_hz)),
                "Configured automatic clock tick frequency.",
            ),
            (
                tf!(
                    "Actual: {} Hz",
                    frequency::format_actual(self.view.actual_tick_hz)
                ),
                "Successful clock ticks per second, measured over about one second. Includes manual ticks; not full clock cycles or propagation steps.",
            ),
        ].map(|(label,hint)|(ui.painter().layout_no_wrap(label,egui::FontId::monospace(12.0),theme::MUTED),hint));
        let width = metrics.iter().map(|(run, _)| run.size().x).sum::<f32>() + 32.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::hover());
        ui.painter().rect(
            rect,
            4,
            theme::SURFACE,
            egui::Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        let mut x = rect.left() + 8.0;
        for (index, (run, hint)) in metrics.into_iter().enumerate() {
            let at = egui::pos2(x, rect.center().y - run.size().y / 2.0);
            let area = egui::Rect::from_min_size(at, run.size());
            ui.interact(
                area,
                ui.make_persistent_id(("runtime-metric", index)),
                egui::Sense::hover(),
            )
            .on_hover_text(tr(hint));
            x += run.size().x + 8.0;
            ui.painter().galley(at, run, theme::MUTED);
        }
    }
    pub fn status_chip(&self, ui: &mut Ui) {
        let (name, tone) = if self.view.limited || self.view.plugin_error_count > 0 {
            (
                tr("Error").into(),
                theme::signal(ui.ctx(), SignalTone::Error),
            )
        } else if self.view.pending > 0 && ui.available_width() > 190.0 {
            (
                self.status(),
                if self.view.enabled {
                    theme::signal(ui.ctx(), SignalTone::High)
                } else {
                    theme::MUTED
                },
            )
        } else if self.view.enabled {
            (
                tr("Running").into(),
                theme::signal(ui.ctx(), SignalTone::High),
            )
        } else {
            (tr("Paused").into(), theme::MUTED)
        };
        let run = ui
            .painter()
            .layout_no_wrap(name, egui::FontId::proportional(13.0), theme::INK);
        let (rect, r) =
            ui.allocate_exact_size(egui::vec2(run.size().x + 29.0, 24.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 4, tone.gamma_multiply(0.08));
        ui.painter()
            .circle_filled(rect.left_center() + egui::vec2(9.0, 0.0), 3.0, tone);
        ui.painter().galley(
            rect.left_center() + egui::vec2(18.0, -run.size().y / 2.0),
            run,
            theme::INK,
        );
        r.on_hover_text(self.status());
    }
    pub(crate) fn can_tick(&self) -> bool {
        self.view.has_clocks && self.view.pending == 0 && !self.view.limited
    }
    pub fn overview(&self, ui: &mut Ui) {
        ui.label(self.status()).on_hover_ui(|ui| {
            ui.label(tr("P: Poke · click a 1-bit input to toggle."));
            ui.label(tr(
                "0 / 1: set selected input. Multi-bit values use hexadecimal.",
            ));
        });
        ui.weak(tf!("{} propagation steps", self.view.steps));
        if self.view.has_clocks {
            ui.weak(tf!(
                "{} clock ticks · {} ticks/s",
                self.view.ticks,
                frequency::format_target(self.view.tick_hz)
            ));
        }
        if !self.view.enabled && self.view.pending > 0 {
            ui.weak(tr("Inputs have pending changes. Step or Run to propagate."));
        }
    }
    pub fn has_component_controls(&self, c: &Component) -> bool {
        use logisim_core::Kind;
        self.view.ports.contains_key(&(c.id, 0))
            || self.view.storage.contains_key(&c.id)
            || self.view.inputs.contains_key(&c.id)
            || self.view.clocks.contains(&c.id)
            || matches!(
                c.kind,
                Kind::Keyboard
                    | Kind::Switch
                    | Kind::DipSwitch
                    | Kind::Joystick
                    | Kind::Slider
                    | Kind::Button
            )
    }
    pub fn component(&mut self, ui: &mut Ui, c: &Component) {
        if c.kind == logisim_core::Kind::Plugin {
            return;
        }
        match c.kind {
            logisim_core::Kind::Keyboard => {
                ui.weak(tr(
                    "Poke: click to type. Enter sends a newline; Esc stops typing.",
                ));
                return;
            }
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
            ui.colored_label(
                theme::signal_text(ui.ctx(), state.tone),
                tf!("Stored value: {}", state.text),
            )
            .on_hover_ui(|ui| state.pill(ui));
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
            ui.colored_label(
                theme::signal_text(ui.ctx(), signal.tone),
                tf!("Signal: {}", signal.text),
            )
            .on_hover_ui(|ui| signal.pill(ui));
        }
        let Some(input) = self.view.inputs.get(&c.id).copied() else {
            return;
        };
        if c.kind == logisim_core::Kind::Pin
            && c.pin.three_state
            && ui.button(tr("Set input to X")).clicked()
        {
            self.actions
                .push(SimulationAction::SetInputUnknown { id: c.id });
        }
        if input.bits == 1 {
            ui.horizontal(|ui| {
                ui.label(tr("Input"));
                for value in [0, 1] {
                    if ui
                        .selectable_label(
                            input.unknown == 0 && input.error == 0 && input.value == value,
                            value.to_string(),
                        )
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
            self.draft = Some((c.id, input, input.text()));
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
                theme::signal_text(ui.ctx(), SignalTone::Error),
                tf!("Enter a hexadecimal value within {} bits.", input.bits),
            );
        }
        if *previous != input {
            if !response.has_focus() {
                *text = input.text();
            }
            *previous = input;
        }
        ui.horizontal_wrapped(|ui| {
            for bit in (0..input.bits).rev() {
                let text = input.bit_text(bit);
                if ui
                    .selectable_label(text == "1", text)
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
mod tests;

//! Stable setting identities and their bindings, shared by search, controls and reset.
use super::*;
use crate::{Accent, Density};
use egui::Ui;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub(super) enum Category {
    #[default]
    General,
    Workspace,
    Appearance,
    Editor,
    Shortcuts,
    Simulation,
    Generation,
    Files,
    Advanced,
}
impl Category {
    pub const ALL: [Self; 9] = [
        Self::General,
        Self::Workspace,
        Self::Appearance,
        Self::Editor,
        Self::Shortcuts,
        Self::Simulation,
        Self::Generation,
        Self::Files,
        Self::Advanced,
    ];
    pub fn label(self) -> &'static str {
        tr(match self {
            Self::General => "General",
            Self::Workspace => "Workspace",
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Shortcuts => "Shortcuts",
            Self::Simulation => "Simulation",
            Self::Generation => "Circuit generation",
            Self::Files => "Files & Recovery",
            Self::Advanced => "Advanced",
        })
    }
}
pub(super) enum Control {
    Language,
    Scale,
    Density,
    Accent,
    SignalPalette,
    Toggle(fn(&mut Settings) -> &mut bool),
    Zoom,
    Mode,
    Frequency,
    RecentLimit,
    AutosaveMinutes,
    ProbeRadix,
    BitWidth(fn(&mut Settings) -> &mut u8),
    GateSize,
}
pub(super) struct Entry {
    pub key: &'static str,
    pub category: Category,
    pub label: &'static str,
    pub help: &'static str,
    pub control: Control,
}
macro_rules! registry {
    ($(($key:literal,$category:ident,$label:literal,$help:literal,$control:expr)),+ $(,)?)=>{
        pub(super) const REGISTRY:&[Entry]=&[$(Entry{key:$key,category:Category::$category,label:$label,help:$help,control:$control}),+];
    }
}
registry![
    (
        "general.language",
        General,
        "Language",
        "Choose the language used by the workbench.",
        Control::Language
    ),
    (
        "workspace.reopen_last",
        Workspace,
        "Reopen last project",
        "Open the most recent project on startup.",
        Control::Toggle(|s| &mut s.workspace.reopen_last)
    ),
    (
        "workspace.recent_limit",
        Workspace,
        "Recent project limit",
        "Number of files to keep in the recent list.",
        Control::RecentLimit
    ),
    (
        "workspace.restore_window",
        Workspace,
        "Restore window geometry",
        "Remember the window size, position and maximized state.",
        Control::Toggle(|s| &mut s.workspace.restore_window)
    ),
    (
        "workspace.restore_panels",
        Workspace,
        "Restore inspector layout",
        "Remember both dividers and the selected catalogue tab.",
        Control::Toggle(|s| &mut s.workspace.restore_panels)
    ),
    (
        "appearance.ui_scale",
        Appearance,
        "Interface scale",
        "Resize text and controls throughout the app.",
        Control::Scale
    ),
    (
        "appearance.density",
        Appearance,
        "Density",
        "Adjust spacing and control sizes.",
        Control::Density
    ),
    (
        "appearance.accent",
        Appearance,
        "Accent",
        "Choose the color of controls and selection outlines.",
        Control::Accent
    ),
    (
        "appearance.reduce_motion",
        Appearance,
        "Reduce motion",
        "Turn off interface animations.",
        Control::Toggle(|s| &mut s.reduce_motion)
    ),
    (
        "appearance.show_telemetry",
        Appearance,
        "Show telemetry",
        "Show FPS and clock rates in the toolbar.",
        Control::Toggle(|s| &mut s.show_telemetry)
    ),
    (
        "appearance.signal_palette",
        Appearance,
        "Signal palette",
        "Choose colors for simulation signals.",
        Control::SignalPalette
    ),
    (
        "appearance.show_wire_values",
        Appearance,
        "Show wire values",
        "Display value badges on buses.",
        Control::Toggle(|s| &mut s.show_wire_values)
    ),
    (
        "appearance.show_origin_marker",
        Appearance,
        "Show origin marker",
        "Mark the canvas origin with a cross.",
        Control::Toggle(|s| &mut s.show_origin_marker)
    ),
    (
        "editor.show_grid",
        Editor,
        "Show grid",
        "Display the dot grid on the canvas.",
        Control::Toggle(|s| &mut s.show_grid)
    ),
    (
        "editor.default_zoom",
        Editor,
        "Initial canvas zoom",
        "Set the zoom when opening a circuit or entering a subcircuit.",
        Control::Zoom
    ),
    (
        "editor.highlight_net",
        Editor,
        "Highlight connected net",
        "Highlight every wire connected to the selected wire.",
        Control::Toggle(|s| &mut s.highlight_net)
    ),
    (
        "editor.show_connection_markers",
        Editor,
        "Show connection markers",
        "Mark junctions and dangling wire ends.",
        Control::Toggle(|s| &mut s.show_connection_markers)
    ),
    (
        "editor.invert_wheel_zoom",
        Editor,
        "Invert wheel zoom",
        "Reverse the mouse wheel direction for canvas zoom.",
        Control::Toggle(|s| &mut s.invert_wheel_zoom)
    ),
    (
        "editor.default_probe_radix",
        Editor,
        "Default probe radix",
        "Set the number format for new probes.",
        Control::ProbeRadix
    ),
    (
        "editor.default_pin_bits",
        Editor,
        "Default pin width",
        "Set the bit width for new pins.",
        Control::BitWidth(|s| &mut s.default_pin_bits)
    ),
    (
        "editor.default_gate_size",
        Editor,
        "Default gate size",
        "New logic and parity gates use this size. Buffer size is fixed. NOT supports Narrow and Wide. For NOT, Medium uses Narrow.",
        Control::GateSize
    ),
    (
        "editor.default_tunnel_width",
        Editor,
        "Default tunnel width",
        "Set the bit width for new tunnels.",
        Control::BitWidth(|s| &mut s.default_tunnel_width)
    ),
    (
        "editor.remember_last_properties",
        Editor,
        "Remember last component properties",
        "Reuse each built-in component’s placement properties for this session.",
        Control::Toggle(|s| &mut s.remember_last_properties)
    ),
    (
        "simulation.mode",
        Simulation,
        "Simulation mode",
        "Choose how the simulator propagates signals.",
        Control::Mode
    ),
    (
        "simulation.tick_hz",
        Simulation,
        "Tick frequency",
        "Set the automatic clock rate in Hz.",
        Control::Frequency
    ),
    (
        "generation.truth_table",
        Generation,
        "Enable truth table generation",
        "Show truth tables in circuit generation.",
        Control::Toggle(|s| &mut s.truth_table)
    ),
    (
        "files.autosave",
        Files,
        "Autosave",
        "Save a recovery copy of unsaved changes.",
        Control::Toggle(|s| &mut s.files.autosave)
    ),
    (
        "files.autosave_minutes",
        Files,
        "Interval (minutes)",
        "Enable Autosave to change the interval.",
        Control::AutosaveMinutes
    ),
];
impl Entry {
    pub fn matches(&self, query: &str) -> bool {
        [self.key, tr(self.label), tr(self.help)]
            .iter()
            .any(|s| s.to_lowercase().contains(query))
    }
    pub fn visible(&self, category: Category, query: &str) -> bool {
        if query.is_empty() {
            self.category == category
        } else {
            self.matches(query)
        }
    }
    pub fn copy(&self, target: &mut Settings, mut source: Settings) {
        match self.control {
            Control::Language => target.language = source.language,
            Control::SignalPalette => target.signal_palette = source.signal_palette,
            Control::Density => target.density = source.density,
            Control::Accent => target.accent = source.accent,
            Control::Scale => target.ui_scale = source.ui_scale,
            Control::Toggle(field) => *field(target) = *field(&mut source),
            Control::ProbeRadix => target.default_probe_radix = source.default_probe_radix,
            Control::BitWidth(field) => *field(target) = *field(&mut source),
            Control::GateSize => target.default_gate_size = source.default_gate_size,
            Control::Zoom => target.default_zoom = source.default_zoom,
            Control::Mode => target.simulation_mode = source.simulation_mode,
            Control::AutosaveMinutes => {
                target.files.autosave_minutes = source.files.autosave_minutes
            }
            Control::RecentLimit => target.workspace.recent_limit = source.workspace.recent_limit,
            Control::Frequency => target.tick_hz = source.tick_hz,
        }
    }
    pub fn changed(&self, current: Settings, baseline: Settings) -> bool {
        let mut one = baseline;
        self.copy(&mut one, current);
        baseline != one
    }
    pub fn show(&self, ui: &mut Ui, values: &mut Settings, draft: &mut Option<u32>) {
        match self.control {
            Control::Language => choice(ui, &mut values.language, Language::ALL, |v| {
                v.name().to_owned()
            }),
            Control::Density => choice(ui, &mut values.density, Density::ALL, |v| {
                v.label().to_owned()
            }),
            Control::Accent => {
                swatches(ui, Accent::ALL.map(|a| (a.colors().base, a.label())));
                choice(ui, &mut values.accent, Accent::ALL, |v| {
                    v.label().to_owned()
                })
            }
            Control::SignalPalette => {
                palette_preview(ui, values.signal_palette);
                choice(ui, &mut values.signal_palette, SignalPalette::ALL, |v| {
                    v.name().to_owned()
                })
            }
            Control::Scale => choice(ui, &mut values.ui_scale, UiScale::ALL, |v| {
                format!("{}%", v.percent())
            }),
            Control::Zoom => choice(
                ui,
                &mut values.default_zoom,
                [DefaultZoom::Fit, DefaultZoom::Actual],
                |v| match v {
                    DefaultZoom::Fit => tr("Fit to canvas").to_owned(),
                    DefaultZoom::Actual => "100%".into(),
                },
            ),
            Control::Mode => choice(ui, &mut values.simulation_mode, SimulationMode::ALL, |v| {
                v.name().to_owned()
            }),
            Control::Toggle(field) => {
                let current = field(values);
                let label = tr(if *current { "On" } else { "Off" });
                let response = ui.checkbox(current, label);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::CheckBox,
                        ui.is_enabled(),
                        *current,
                        tr(self.label),
                    )
                });
                theme::focus_ring(ui, &response);
            }
            Control::ProbeRadix => choice(
                ui,
                &mut values.default_probe_radix,
                logisim_core::Radix::ALL,
                |v| tr(v.name()).to_owned(),
            ),
            Control::GateSize => {
                use logisim_core::GateSize::{Medium, Narrow, Wide};
                choice(
                    ui,
                    &mut values.default_gate_size,
                    [Narrow, Medium, Wide],
                    |v| tr(v.name()).to_owned(),
                )
            }
            Control::BitWidth(field) => {
                let r = ui.add(
                    egui::DragValue::new(field(values))
                        .range(1..=32)
                        .update_while_editing(false),
                );
                theme::focus_ring(ui, &r);
            }
            Control::AutosaveMinutes => {
                let r = ui.add_enabled(
                    values.files.autosave,
                    egui::DragValue::new(&mut values.files.autosave_minutes)
                        .range(1..=30)
                        .update_while_editing(false),
                );
                theme::focus_ring(ui, &r);
            }
            Control::RecentLimit => {
                let r = ui.add(
                    egui::DragValue::new(&mut values.workspace.recent_limit)
                        .range(1..=20)
                        .update_while_editing(false),
                );
                theme::focus_ring(ui, &r);
            }
            Control::Frequency => {
                let mut hz = draft.unwrap_or(values.tick_hz.get());
                let r = ui.add(
                    egui::DragValue::new(&mut hz)
                        .range(1..=crate::MAX_TICK_HZ)
                        .suffix(" Hz")
                        .update_while_editing(false),
                );
                if r.changed() || draft.is_some() {
                    if r.dragged() {
                        *draft = Some(hz);
                    } else {
                        values.tick_hz = TickHz::new(hz).expect("bounded frequency control");
                        *draft = None;
                    }
                }
                theme::focus_ring(ui, &r);
            }
        }
    }
}
fn palette_preview(ui: &mut Ui, palette: SignalPalette) {
    use crate::SignalTone::*;
    let tones = [
        (Low, "Low signal"),
        (High, "High signal"),
        (Bus, "Bus signal"),
        (Unknown, "Unknown signal"),
        (Error, "Signal error"),
        (WidthMismatch, "Width mismatch"),
    ];
    swatches(ui, tones.map(|(tone, key)| (palette.color(tone), tr(key))));
}

fn swatches<const N: usize>(ui: &mut Ui, swatches: [(egui::Color32, &str); N]) {
    let cell = egui::vec2(14., theme::metrics(ui.ctx()).control);
    let (area, _) = ui.allocate_exact_size(
        egui::vec2(cell.x * swatches.len() as f32, cell.y),
        egui::Sense::hover(),
    );
    for (i, (color, label)) in swatches.into_iter().enumerate() {
        let rect = egui::Rect::from_min_size(area.min + egui::vec2(i as f32 * cell.x, 0.), cell);
        ui.painter().circle_filled(rect.center(), 4.5, color);
        ui.interact(
            rect,
            ui.make_persistent_id(("swatch", i)),
            egui::Sense::hover(),
        )
        .on_hover_text(label);
    }
}

fn choice<T: Copy + PartialEq, const N: usize>(
    ui: &mut Ui,
    current: &mut T,
    options: [T; N],
    label: impl Fn(T) -> String,
) {
    let r = egui::ComboBox::from_id_salt("choice")
        .width(ui.available_width().clamp(88.0, 156.0))
        .selected_text(label(*current))
        .show_ui(ui, |ui| {
            for value in options {
                ui.selectable_value(current, value, label(value));
            }
        })
        .response;
    theme::focus_ring(ui, &r);
}

//! Application preferences, independent of documents, cameras and runtime snapshots.
use super::SignalPalette;
use crate::{Accent, DEFAULT_TICK_HZ, Density, Language, MAX_TICK_HZ, SimulationMode};
use logisim_core::Radix;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiScale {
    P80,
    P90,
    #[default]
    P100,
    P110,
    P125,
    P150,
}

impl UiScale {
    pub const ALL: [Self; 6] = [
        Self::P80,
        Self::P90,
        Self::P100,
        Self::P110,
        Self::P125,
        Self::P150,
    ];
    pub const fn percent(self) -> u16 {
        match self {
            Self::P80 => 80,
            Self::P90 => 90,
            Self::P100 => 100,
            Self::P110 => 110,
            Self::P125 => 125,
            Self::P150 => 150,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DefaultZoom {
    #[default]
    Fit,
    Actual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickHz(u32);
impl TickHz {
    pub fn new(hz: u32) -> Option<Self> {
        (1..=MAX_TICK_HZ).contains(&hz).then_some(Self(hz))
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl Default for TickHz {
    fn default() -> Self {
        Self(DEFAULT_TICK_HZ)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspaceSettings {
    pub reopen_last: bool,
    pub recent_limit: usize,
    pub restore_window: bool,
    pub restore_panels: bool,
}
impl WorkspaceSettings {
    pub fn bounded_recent_limit(self) -> usize {
        self.recent_limit.clamp(1, 20)
    }
}
impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            reopen_last: false,
            recent_limit: 10,
            restore_window: true,
            restore_panels: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileSettings {
    pub autosave: bool,
    pub autosave_minutes: u8,
}
impl FileSettings {
    pub fn valid_interval(self) -> bool {
        (1..=30).contains(&self.autosave_minutes)
    }
}
impl Default for FileSettings {
    fn default() -> Self {
        Self {
            autosave: true,
            autosave_minutes: 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub files: FileSettings,
    pub workspace: WorkspaceSettings,
    pub language: Language,
    pub density: Density,
    pub accent: Accent,
    pub ui_scale: UiScale,
    pub reduce_motion: bool,
    pub show_telemetry: bool,
    pub signal_palette: SignalPalette,
    pub show_wire_values: bool,
    pub show_origin_marker: bool,
    pub show_grid: bool,
    pub default_zoom: DefaultZoom,
    pub highlight_net: bool,
    pub show_connection_markers: bool,
    pub invert_wheel_zoom: bool,
    pub default_probe_radix: Radix,
    pub default_pin_bits: u8,
    pub simulation_mode: SimulationMode,
    pub tick_hz: TickHz,
    pub truth_table: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            files: FileSettings::default(),
            workspace: WorkspaceSettings::default(),
            language: Language::default(),
            density: Density::default(),
            accent: Accent::default(),
            ui_scale: UiScale::default(),
            reduce_motion: false,
            show_telemetry: true,
            signal_palette: SignalPalette::default(),
            show_wire_values: true,
            show_origin_marker: true,
            show_grid: true,
            default_zoom: DefaultZoom::default(),
            highlight_net: true,
            show_connection_markers: true,
            invert_wheel_zoom: false,
            default_probe_radix: Radix::Binary,
            default_pin_bits: 1,
            simulation_mode: SimulationMode::default(),
            tick_hz: TickHz::default(),
            truth_table: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EditorDisplay {
    pub show_wire_values: bool,
    pub highlight_net: bool,
    pub show_connection_markers: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct PlacementDefaults {
    pub probe_radix: Radix,
    pub pin_bits: u8,
}

impl Settings {
    pub(crate) fn placement_defaults(self) -> PlacementDefaults {
        PlacementDefaults {
            probe_radix: self.default_probe_radix,
            pin_bits: self.default_pin_bits.clamp(1, 32),
        }
    }

    pub(crate) fn editor_display(self) -> EditorDisplay {
        EditorDisplay {
            show_wire_values: self.show_wire_values,
            highlight_net: self.highlight_net,
            show_connection_markers: self.show_connection_markers,
        }
    }

    /// Apply interface density and motion without touching the canvas camera.
    pub fn apply_appearance(&self, ctx: &egui::Context) {
        crate::theme::apply_chrome(ctx, self.density, self.accent);
        self.signal_palette.apply(ctx);
        ctx.set_zoom_factor(f32::from(self.ui_scale.percent()) / 100.0);
        let animation = if self.reduce_motion {
            0.0
        } else {
            crate::theme::ANIMATION_SECONDS
        };
        if ctx.global_style().animation_time != animation {
            ctx.global_style_mut(|style| style.animation_time = animation);
        }
    }
    /// Replay only local edits when Retry loads a configuration repaired on disk.
    pub fn with_changes(mut self, before: Self, after: Self) -> Self {
        for entry in super::registry::REGISTRY {
            if entry.changed(after, before) {
                entry.copy(&mut self, after);
            }
        }
        self
    }
}

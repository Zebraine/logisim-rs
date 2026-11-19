//! Application preferences, independent of documents, cameras and runtime snapshots.
use crate::{DEFAULT_TICK_HZ, Language, MAX_TICK_HZ, SimulationMode};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub language: Language,
    pub ui_scale: UiScale,
    pub reduce_motion: bool,
    pub show_telemetry: bool,
    pub show_grid: bool,
    pub default_zoom: DefaultZoom,
    pub simulation_mode: SimulationMode,
    pub tick_hz: TickHz,
    pub truth_table: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::default(),
            ui_scale: UiScale::default(),
            reduce_motion: false,
            show_telemetry: true,
            show_grid: true,
            default_zoom: DefaultZoom::default(),
            simulation_mode: SimulationMode::default(),
            tick_hz: TickHz::default(),
            truth_table: false,
        }
    }
}

impl Settings {
    /// Apply interface density and motion without touching the canvas camera.
    pub fn apply_appearance(&self, ctx: &egui::Context) {
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

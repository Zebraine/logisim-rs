//! Application preferences, independent of documents, cameras and runtime snapshots.
use super::SignalPalette;
use crate::{Accent, Density, Language, SimulationMode};
use logisim_core::{GateSize, Radix};

pub use logisim_preferences::model::{
    DefaultZoom, FileSettings, FontPreset, TickHz, UiScale, WasmEnginePreference, WorkspaceSettings,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub wasm_engine: WasmEnginePreference,
    pub shortcuts: crate::shortcuts::Shortcuts,
    pub files: FileSettings,
    pub workspace: WorkspaceSettings,
    pub language: Language,
    pub density: Density,
    pub theme: crate::ThemeMode,
    pub dark_artwork_contrast: bool,
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
    pub default_gate_size: GateSize,
    pub default_font_family: FontPreset,
    pub default_font_bold: bool,
    pub default_font_italic: bool,
    pub default_font_size: Option<u8>,
    pub default_gate_inputs: u8,
    pub default_tunnel_width: u8,
    pub remember_last_properties: bool,
    pub simulation_mode: SimulationMode,
    pub tick_hz: TickHz,
    pub truth_table: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            wasm_engine: Default::default(),
            shortcuts: Default::default(),
            files: FileSettings::default(),
            workspace: WorkspaceSettings::default(),
            language: Language::default(),
            density: Density::default(),
            theme: crate::ThemeMode::default(),
            dark_artwork_contrast: true,
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
            default_gate_size: GateSize::Narrow,
            default_font_family: FontPreset::Dialog,
            default_font_bold: false,
            default_font_italic: false,
            default_font_size: None,
            default_gate_inputs: 2,
            default_tunnel_width: 1,
            remember_last_properties: false,
            simulation_mode: SimulationMode::default(),
            tick_hz: TickHz::default(),
            truth_table: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EditorDisplay {
    pub dark_artwork_contrast: bool,
    pub show_wire_values: bool,
    pub highlight_net: bool,
    pub show_connection_markers: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct PlacementDefaults {
    pub probe_radix: Radix,
    pub pin_bits: u8,
    pub gate_size: GateSize,
    pub font_family: FontPreset,
    pub font_bold: bool,
    pub font_italic: bool,
    pub font_size: Option<u8>,
    pub gate_inputs: u8,
    pub tunnel_bits: u8,
}

impl Settings {
    pub const DEFAULT_FONT_SIZE_RANGE: std::ops::RangeInclusive<u8> =
        logisim_preferences::model::FONT_SIZES;
    pub const DEFAULT_GATE_INPUTS_RANGE: std::ops::RangeInclusive<u8> =
        logisim_preferences::model::GATE_INPUTS;

    pub(crate) fn placement_defaults(self) -> PlacementDefaults {
        PlacementDefaults {
            probe_radix: self.default_probe_radix,
            pin_bits: self.default_pin_bits.clamp(1, 32),
            gate_size: self.default_gate_size,
            font_family: self.default_font_family,
            font_bold: self.default_font_bold,
            font_italic: self.default_font_italic,
            font_size: self.default_font_size.map(|size| size.clamp(8, 32)),
            gate_inputs: self.default_gate_inputs.clamp(2, 32),
            tunnel_bits: self.default_tunnel_width.clamp(1, 32),
        }
    }

    pub(crate) fn editor_display(self) -> EditorDisplay {
        EditorDisplay {
            dark_artwork_contrast: self.dark_artwork_contrast,
            show_wire_values: self.show_wire_values,
            highlight_net: self.highlight_net,
            show_connection_markers: self.show_connection_markers,
        }
    }

    /// Apply interface density and motion without touching the canvas camera.
    pub fn apply_appearance(&self, ctx: &egui::Context) {
        crate::theme::apply_chrome(ctx, self.theme, self.density, self.accent);
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
    pub fn with_changes(
        mut self,
        before: Self,
        after: Self,
    ) -> Result<Self, crate::shortcuts::Conflict> {
        if before.shortcuts != after.shortcuts {
            self.shortcuts = self
                .shortcuts
                .with_changes(before.shortcuts, after.shortcuts)?;
        }
        for entry in super::registry::REGISTRY {
            entry.merge(&mut self, before, after);
        }
        Ok(self)
    }
}

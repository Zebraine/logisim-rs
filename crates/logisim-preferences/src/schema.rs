//! Schema shared by GUI preferences and headless settings transactions.
use crate::{
    host::{WindowState, WorkspaceLayout},
    model::*,
    shortcuts::Shortcuts,
};
use logisim_core::{GateSize, Radix};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    deserialize = "R: Deserialize<'de>, W: Deserialize<'de>, P: Deserialize<'de>, F: Deserialize<'de>"
))]
pub struct File<R = Vec<String>, W = WindowState, P = WorkspaceLayout, F = PluginFields> {
    pub version: u32,
    #[serde(default)]
    pub shortcuts: Shortcuts,
    #[serde(default)]
    pub files: FileSettings,
    #[serde(default, deserialize_with = "present")]
    pub workspace: Option<WorkspaceSettings>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub recent: Option<R>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub window: Option<W>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub panels: Option<P>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<F>,
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub editor: Editor,
    #[serde(default)]
    pub simulation: Simulation,
    #[serde(default)]
    pub generation: Generation,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub language: LanguageCode,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum LanguageCode {
    #[serde(rename = "en")]
    #[default]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub density: DensityCode,
    pub theme: ThemeCode,
    pub dark_artwork_contrast: bool,
    pub accent: AccentCode,
    pub ui_scale: f64,
    pub reduce_motion: bool,
    pub show_telemetry: bool,
    pub signal_palette: SignalPaletteCode,
    pub show_wire_values: bool,
    pub show_origin_marker: bool,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DensityCode {
    #[default]
    Comfortable,
    Compact,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeCode {
    #[default]
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccentCode {
    #[default]
    Indigo,
    Blue,
    Teal,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalPaletteCode {
    #[default]
    Default,
    HighContrast,
    ColorblindSafe,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            density: DensityCode::default(),
            theme: ThemeCode::default(),
            dark_artwork_contrast: true,
            accent: AccentCode::default(),
            ui_scale: 1.0,
            reduce_motion: false,
            show_telemetry: true,
            signal_palette: SignalPaletteCode::default(),
            show_wire_values: true,
            show_origin_marker: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Editor {
    pub show_grid: bool,
    #[serde(with = "ZoomCode")]
    pub default_zoom: DefaultZoom,
    pub highlight_net: bool,
    pub show_connection_markers: bool,
    pub invert_wheel_zoom: bool,
    #[serde(with = "RadixCode")]
    pub default_probe_radix: Radix,
    pub default_pin_bits: u8,
    #[serde(with = "GateSizeCode")]
    pub default_gate_size: GateSize,
    pub default_font_family: FontPreset,
    pub default_font_bold: bool,
    pub default_font_italic: bool,
    pub default_font_size: Option<u8>,
    pub default_gate_inputs: u8,
    pub default_tunnel_width: u8,
    pub remember_last_properties: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "GateSize", rename_all = "lowercase")]
enum GateSizeCode {
    Narrow,
    Medium,
    Wide,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            show_grid: true,
            default_zoom: DefaultZoom::Fit,
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
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Radix", rename_all = "lowercase")]
enum RadixCode {
    Binary,
    Octal,
    Signed,
    Unsigned,
    Hex,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "DefaultZoom")]
enum ZoomCode {
    #[serde(rename = "fit")]
    Fit,
    #[serde(rename = "100%")]
    Actual,
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Simulation {
    pub mode: ModeCode,
    pub tick_hz: u32,
}
impl Default for Simulation {
    fn default() -> Self {
        Self {
            mode: ModeCode::default(),
            tick_hz: TickHz::default().get(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeCode {
    #[default]
    Stable,
    Event,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Generation {
    pub truth_table: bool,
}

// An omitted section receives defaults. An explicitly null section is invalid.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PluginFields {
    pub fallback_directory: serde_json::Value,
}
impl<R, W, P, F> File<R, W, P, F> {
    pub fn validate(&self) -> Result<(UiScale, TickHz), &'static str> {
        if !matches!(self.version, 1..=CONFIG_VERSION) {
            return Err("Unsupported configuration version.");
        }
        if self.version == 1 && self.workspace.is_some() {
            return Err("Unknown or invalid settings.");
        }
        let workspace = self.workspace.unwrap_or_default();
        if !self.files.valid_interval() {
            return Err("Invalid autosave interval.");
        }
        if !(1..=20).contains(&workspace.recent_limit) {
            return Err("Recent project limit must be between 1 and 20.");
        }
        if !(1..=32).contains(&self.editor.default_pin_bits)
            || !(1..=32).contains(&self.editor.default_tunnel_width)
        {
            return Err("Pin and tunnel widths must be between 1 and 32.");
        }
        if self
            .editor
            .default_font_size
            .is_some_and(|size| !FONT_SIZES.contains(&size))
        {
            return Err("Default label size must be Auto or between 8 and 32.");
        }
        if !GATE_INPUTS.contains(&self.editor.default_gate_inputs) {
            return Err("Default gate inputs must be between 2 and 32.");
        }
        let ui_scale = UiScale::ALL
            .into_iter()
            .find(|s| f64::from(s.percent()) / 100.0 == self.appearance.ui_scale)
            .ok_or("Invalid interface scale.")?;
        let tick_hz = TickHz::new(self.simulation.tick_hz).ok_or("Invalid tick frequency.")?;
        Ok((ui_scale, tick_hz))
    }
}
impl File {
    pub fn validate_local(&self) -> Result<(), &'static str> {
        self.validate()?;
        if self.version == 1
            && (self.recent.is_some() || self.window.is_some() || self.panels.is_some())
        {
            return Err("Unknown or invalid settings.");
        }
        crate::host::validate(
            self.window.unwrap_or_default(),
            self.panels.unwrap_or_default(),
            self.recent.as_deref().unwrap_or(&[]),
        )
    }
    pub fn portable(mut self) -> Self {
        self.version = CONFIG_VERSION;
        self.workspace = Some(self.workspace.unwrap_or_default());
        self.recent = None;
        self.window = None;
        self.panels = None;
        self.plugins = None;
        self
    }
}

//! Versioned disk representation. The UI owns the validated preference types.
use logisim_core::{GateSize, Radix};
use logisim_ui::{
    Accent, DefaultZoom, Density, FontPreset, Language, Settings, SignalPalette, SimulationMode,
    ThemeMode, TickHz, UiScale, WorkspaceSettings,
};
use serde::{Deserialize, Serialize};

use crate::window::WindowState;
use logisim_ui::{CONFIG_VERSION, WorkspaceLayout};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    deserialize = "R: Deserialize<'de>, W: Deserialize<'de>, P: Deserialize<'de>, F: Deserialize<'de>"
))]
struct File<R = Vec<String>, W = WindowState, P = WorkspaceLayout, F = PluginSettings> {
    version: u32,
    #[serde(default)]
    shortcuts: logisim_ui::shortcuts::Shortcuts,
    #[serde(default)]
    files: logisim_ui::FileSettings,
    #[serde(default, deserialize_with = "present")]
    workspace: Option<WorkspaceSettings>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    recent: Option<R>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    window: Option<W>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    panels: Option<P>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plugins: Option<F>,
    #[serde(default)]
    general: General,
    #[serde(default)]
    appearance: Appearance,
    #[serde(default)]
    editor: Editor,
    #[serde(default)]
    simulation: Simulation,
    #[serde(default)]
    generation: Generation,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct General {
    #[serde(with = "LanguageCode")]
    language: Language,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Language")]
enum LanguageCode {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Appearance {
    #[serde(with = "DensityCode")]
    density: Density,
    #[serde(with = "ThemeCode")]
    theme: ThemeMode,
    dark_artwork_contrast: bool,
    #[serde(with = "AccentCode")]
    accent: Accent,
    ui_scale: f64,
    reduce_motion: bool,
    show_telemetry: bool,
    #[serde(with = "SignalPaletteCode")]
    signal_palette: SignalPalette,
    show_wire_values: bool,
    show_origin_marker: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Density", rename_all = "lowercase")]
enum DensityCode {
    Comfortable,
    Compact,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ThemeMode", rename_all = "lowercase")]
enum ThemeCode {
    Light,
    Dark,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Accent", rename_all = "lowercase")]
enum AccentCode {
    Indigo,
    Blue,
    Teal,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "SignalPalette", rename_all = "kebab-case")]
enum SignalPaletteCode {
    Default,
    HighContrast,
    ColorblindSafe,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            density: Density::default(),
            theme: ThemeMode::default(),
            dark_artwork_contrast: true,
            accent: Accent::default(),
            ui_scale: 1.0,
            reduce_motion: false,
            show_telemetry: true,
            signal_palette: SignalPalette::default(),
            show_wire_values: true,
            show_origin_marker: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Editor {
    show_grid: bool,
    #[serde(with = "ZoomCode")]
    default_zoom: DefaultZoom,
    highlight_net: bool,
    show_connection_markers: bool,
    invert_wheel_zoom: bool,
    #[serde(with = "RadixCode")]
    default_probe_radix: Radix,
    default_pin_bits: u8,
    #[serde(with = "GateSizeCode")]
    default_gate_size: GateSize,
    default_font_family: FontPreset,
    default_font_bold: bool,
    default_font_italic: bool,
    default_font_size: Option<u8>,
    default_gate_inputs: u8,
    default_tunnel_width: u8,
    remember_last_properties: bool,
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
        let settings = Settings::default();
        Self {
            show_grid: settings.show_grid,
            default_zoom: settings.default_zoom,
            highlight_net: settings.highlight_net,
            show_connection_markers: settings.show_connection_markers,
            invert_wheel_zoom: settings.invert_wheel_zoom,
            default_probe_radix: settings.default_probe_radix,
            default_pin_bits: settings.default_pin_bits,
            default_gate_size: settings.default_gate_size,
            default_font_family: settings.default_font_family,
            default_font_bold: settings.default_font_bold,
            default_font_italic: settings.default_font_italic,
            default_font_size: settings.default_font_size,
            default_gate_inputs: settings.default_gate_inputs,
            default_tunnel_width: settings.default_tunnel_width,
            remember_last_properties: settings.remember_last_properties,
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
struct Simulation {
    #[serde(with = "ModeCode")]
    mode: SimulationMode,
    tick_hz: u32,
}
impl Default for Simulation {
    fn default() -> Self {
        Self {
            mode: SimulationMode::default(),
            tick_hz: TickHz::default().get(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "SimulationMode", rename_all = "lowercase")]
enum ModeCode {
    Stable,
    Event,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Generation {
    truth_table: bool,
}

// An omitted section receives defaults. An explicitly null section is invalid.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct PluginSettings {
    #[serde(deserialize_with = "fallback_directory")]
    pub fallback_directory: Option<String>,
}

fn fallback_directory<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    let Some(text) = value.as_str() else {
        return Ok(None);
    };
    let path = std::path::Path::new(text);
    Ok(match super::canonical_plugin_directory(path) {
        Ok(path) => path.into_os_string().into_string().ok(),
        // A configured directory can be temporarily inaccessible (for example a network mount).
        Err(e) if path.is_absolute() && e.kind() == std::io::ErrorKind::PermissionDenied => {
            Some(text.to_owned())
        }
        Err(_) => None,
    })
}

#[derive(Default)]
pub(super) struct Config {
    pub plugins: PluginSettings,
    pub settings: Settings,
    pub recent: Vec<String>,
    pub window: WindowState,
    pub panels: WorkspaceLayout,
}

pub(super) fn decode(bytes: &[u8]) -> Result<Config, &'static str> {
    let file: File = serde_json::from_slice(bytes).map_err(|_| "Unknown or invalid settings.")?;
    let settings = validated(&file)?;
    if file.version == 1
        && (file.recent.is_some() || file.panels.is_some() || file.window.is_some())
    {
        return Err("Unknown or invalid settings.");
    }
    let recent = file.recent.unwrap_or_default();
    let panels = file.panels.unwrap_or_default();
    let window = file.window.unwrap_or_default();
    if !window.valid()
        || !(230.0..=380.0).contains(&panels.sidebar_width)
        || !(0.25..=0.75).contains(&panels.split)
        || recent.len() > 20
        || recent
            .iter()
            .enumerate()
            .any(|(i, p)| !super::recent::valid(p) || recent[..i].contains(p))
    {
        return Err("Unknown or invalid settings.");
    }
    Ok(Config {
        plugins: file.plugins.unwrap_or_default(),
        settings,
        recent,
        panels,
        window: window.bounded(None),
    })
}

pub(super) fn decode_settings(bytes: &[u8]) -> Result<Settings, String> {
    use serde::de::IgnoredAny;
    if bytes.len() > logisim_file::MAX_BYTES {
        return Err("File exceeds the 8 MiB limit.".into());
    }
    // Host history and geometry can occur in a local config copied by the user.
    // Deserialize only the portable sections, retaining serde's strict field checks.
    let file: File<IgnoredAny, IgnoredAny, IgnoredAny, IgnoredAny> = serde_json::from_slice(bytes)
        .map_err(|error| match error.classify() {
            serde_json::error::Category::Data => logisim_ui::tf!("Invalid preference: {}", error),
            _ => logisim_ui::tf!("Invalid settings JSON: {}", error),
        })?;
    validated(&file).map_err(str::to_owned)
}

fn validated<R, W, P, F>(file: &File<R, W, P, F>) -> Result<Settings, &'static str> {
    if !matches!(file.version, 1..=CONFIG_VERSION) {
        return Err("Unsupported configuration version.");
    }
    if file.version == 1 && file.workspace.is_some() {
        return Err("Unknown or invalid settings.");
    }
    let workspace = file.workspace.unwrap_or_default();
    if !file.files.valid_interval() {
        return Err("Invalid autosave interval.");
    }
    if !(1..=20).contains(&workspace.recent_limit) {
        return Err("Recent project limit must be between 1 and 20.");
    }
    if !(1..=32).contains(&file.editor.default_pin_bits)
        || !(1..=32).contains(&file.editor.default_tunnel_width)
    {
        return Err("Pin and tunnel widths must be between 1 and 32.");
    }
    if file
        .editor
        .default_font_size
        .is_some_and(|size| !Settings::DEFAULT_FONT_SIZE_RANGE.contains(&size))
    {
        return Err("Default label size must be Auto or between 8 and 32.");
    }
    if !Settings::DEFAULT_GATE_INPUTS_RANGE.contains(&file.editor.default_gate_inputs) {
        return Err("Default gate inputs must be between 2 and 32.");
    }
    let ui_scale = UiScale::ALL
        .into_iter()
        .find(|s| f64::from(s.percent()) / 100.0 == file.appearance.ui_scale)
        .ok_or("Invalid interface scale.")?;
    let tick_hz = TickHz::new(file.simulation.tick_hz).ok_or("Invalid tick frequency.")?;
    Ok(Settings {
        shortcuts: file.shortcuts,
        files: file.files,
        workspace,
        language: file.general.language,
        density: file.appearance.density,
        theme: file.appearance.theme,
        dark_artwork_contrast: file.appearance.dark_artwork_contrast,
        accent: file.appearance.accent,
        ui_scale,
        reduce_motion: file.appearance.reduce_motion,
        show_telemetry: file.appearance.show_telemetry,
        signal_palette: file.appearance.signal_palette,
        show_wire_values: file.appearance.show_wire_values,
        show_origin_marker: file.appearance.show_origin_marker,
        show_grid: file.editor.show_grid,
        default_zoom: file.editor.default_zoom,
        highlight_net: file.editor.highlight_net,
        show_connection_markers: file.editor.show_connection_markers,
        invert_wheel_zoom: file.editor.invert_wheel_zoom,
        default_probe_radix: file.editor.default_probe_radix,
        default_pin_bits: file.editor.default_pin_bits,
        default_gate_size: file.editor.default_gate_size,
        default_font_family: file.editor.default_font_family,
        default_font_bold: file.editor.default_font_bold,
        default_font_italic: file.editor.default_font_italic,
        default_font_size: file.editor.default_font_size,
        default_gate_inputs: file.editor.default_gate_inputs,
        default_tunnel_width: file.editor.default_tunnel_width,
        remember_last_properties: file.editor.remember_last_properties,
        simulation_mode: file.simulation.mode,
        tick_hz,
        truth_table: file.generation.truth_table,
    })
}
pub(super) fn encode(config: &Config) -> Vec<u8> {
    bytes(&File {
        plugins: Some(config.plugins.clone()),
        panels: Some(config.panels),
        window: Some(config.window),
        recent: Some(config.recent.clone()),
        ..portable(config.settings)
    })
}

pub(super) fn encode_settings(settings: Settings) -> Vec<u8> {
    bytes(&portable(settings))
}

fn portable(s: Settings) -> File {
    File {
        plugins: None,
        recent: None,
        window: None,
        panels: None,
        workspace: Some(s.workspace),
        version: CONFIG_VERSION,
        shortcuts: s.shortcuts,
        files: s.files,
        general: General {
            language: s.language,
        },
        appearance: Appearance {
            density: s.density,
            theme: s.theme,
            dark_artwork_contrast: s.dark_artwork_contrast,
            accent: s.accent,
            ui_scale: f64::from(s.ui_scale.percent()) / 100.0,
            reduce_motion: s.reduce_motion,
            show_telemetry: s.show_telemetry,
            signal_palette: s.signal_palette,
            show_wire_values: s.show_wire_values,
            show_origin_marker: s.show_origin_marker,
        },
        editor: Editor {
            show_grid: s.show_grid,
            default_zoom: s.default_zoom,
            highlight_net: s.highlight_net,
            show_connection_markers: s.show_connection_markers,
            invert_wheel_zoom: s.invert_wheel_zoom,
            default_probe_radix: s.default_probe_radix,
            default_pin_bits: s.default_pin_bits,
            default_gate_size: s.default_gate_size,
            default_font_family: s.default_font_family,
            default_font_bold: s.default_font_bold,
            default_font_italic: s.default_font_italic,
            default_font_size: s.default_font_size,
            default_gate_inputs: s.default_gate_inputs,
            default_tunnel_width: s.default_tunnel_width,
            remember_last_properties: s.remember_last_properties,
        },
        simulation: Simulation {
            mode: s.simulation_mode,
            tick_hz: s.tick_hz.get(),
        },
        generation: Generation {
            truth_table: s.truth_table,
        },
    }
}

fn bytes(file: &File) -> Vec<u8> {
    let mut bytes =
        serde_json::to_vec_pretty(file).expect("typed preferences have a JSON representation");
    bytes.push(b'\n');
    bytes
}

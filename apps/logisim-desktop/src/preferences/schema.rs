//! Versioned disk representation. The UI owns the validated preference types.
use logisim_core::Radix;
use logisim_ui::{
    DefaultZoom, Language, Settings, SignalPalette, SimulationMode, TickHz, UiScale,
    WorkspaceSettings,
};
use serde::{Deserialize, Serialize};

use crate::window::WindowState;
use logisim_ui::{CONFIG_VERSION, WorkspaceLayout};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    #[serde(default, deserialize_with = "present")]
    workspace: Option<WorkspaceSettings>,
    #[serde(default, deserialize_with = "present")]
    recent: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    window: Option<WindowState>,
    #[serde(default, deserialize_with = "present")]
    panels: Option<WorkspaceLayout>,
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
    ui_scale: f64,
    reduce_motion: bool,
    show_telemetry: bool,
    #[serde(with = "SignalPaletteCode")]
    signal_palette: SignalPalette,
    show_wire_values: bool,
    show_origin_marker: bool,
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

#[derive(Default)]
pub(super) struct Config {
    pub settings: Settings,
    pub recent: Vec<String>,
    pub window: WindowState,
    pub panels: WorkspaceLayout,
}

pub(super) fn decode(bytes: &[u8]) -> Result<Config, &'static str> {
    let file: File = serde_json::from_slice(bytes).map_err(|_| "Unknown or invalid settings.")?;
    if !matches!(file.version, 1..=CONFIG_VERSION) {
        return Err("Unsupported configuration version.");
    }
    if file.version == 1
        && (file.recent.is_some()
            || file.workspace.is_some()
            || file.panels.is_some()
            || file.window.is_some())
    {
        return Err("Unknown or invalid settings.");
    }
    let workspace = file.workspace.unwrap_or_default();
    let recent = file.recent.unwrap_or_default();
    let panels = file.panels.unwrap_or_default();
    let window = file.window.unwrap_or_default();
    if !(1..=32).contains(&file.editor.default_pin_bits)
        || !window.valid()
        || !(230.0..=380.0).contains(&panels.sidebar_width)
        || !(0.25..=0.75).contains(&panels.split)
        || !(1..=20).contains(&workspace.recent_limit)
        || recent.len() > workspace.recent_limit
        || recent
            .iter()
            .enumerate()
            .any(|(i, p)| !super::recent::valid(p) || recent[..i].contains(p))
    {
        return Err("Unknown or invalid settings.");
    }
    let ui_scale = UiScale::ALL
        .into_iter()
        .find(|s| f64::from(s.percent()) / 100.0 == file.appearance.ui_scale)
        .ok_or("Invalid interface scale.")?;
    let tick_hz = TickHz::new(file.simulation.tick_hz).ok_or("Invalid tick frequency.")?;
    Ok(Config {
        panels,
        window: window.bounded(None),
        recent,
        settings: Settings {
            workspace,
            language: file.general.language,
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
            simulation_mode: file.simulation.mode,
            tick_hz,
            truth_table: file.generation.truth_table,
        },
    })
}
pub(super) fn encode(config: &Config) -> Vec<u8> {
    let s = config.settings;
    let file = File {
        panels: Some(config.panels),
        window: Some(config.window),
        workspace: Some(s.workspace),
        recent: Some(config.recent.clone()),
        version: CONFIG_VERSION,
        general: General {
            language: s.language,
        },
        appearance: Appearance {
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
        },
        simulation: Simulation {
            mode: s.simulation_mode,
            tick_hz: s.tick_hz.get(),
        },
        generation: Generation {
            truth_table: s.truth_table,
        },
    };
    let mut bytes =
        serde_json::to_vec_pretty(&file).expect("typed preferences have a JSON representation");
    bytes.push(b'\n');
    bytes
}

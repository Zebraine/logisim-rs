//! Versioned disk representation. The UI owns the validated preference types.
use logisim_ui::{Accent, Density, Language, Settings, SignalPalette, SimulationMode, ThemeMode};
use serde::{Deserialize, Serialize};

use crate::window::WindowGeometry as _;
use crate::window::WindowState;
use logisim_preferences::schema::{
    self as wire, AccentCode, Appearance, DensityCode, Editor, General, Generation, LanguageCode,
    ModeCode, SignalPaletteCode, Simulation, ThemeCode,
};
use logisim_ui::{CONFIG_VERSION, WorkspaceLayout};
type File<R = Vec<String>, W = WindowState, P = WorkspaceLayout, F = PluginSettings> =
    wire::File<R, W, P, F>;

macro_rules! bridge {
    ($read:ident,$write:ident,$ui:ident,$disk:ident; $($v:ident),+) => {
        fn $read(value:$disk)->$ui {match value {$($disk::$v=>$ui::$v),+}}
        fn $write(value:$ui)->$disk {match value {$($ui::$v=>$disk::$v),+}}
    }
}
bridge!(language_from_disk,language_to_disk,Language,LanguageCode; English,SimplifiedChinese);
bridge!(theme_from_disk,theme_to_disk,ThemeMode,ThemeCode; Light,Dark);
bridge!(accent_from_disk,accent_to_disk,Accent,AccentCode; Indigo,Blue,Teal);
bridge!(density_from_disk,density_to_disk,Density,DensityCode; Comfortable,Compact);
bridge!(signal_palette_from_disk,signal_palette_to_disk,SignalPalette,SignalPaletteCode; Default,HighContrast,ColorblindSafe);
bridge!(mode_from_disk,mode_to_disk,SimulationMode,ModeCode; Stable,Event);
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
    logisim_preferences::host::validate(window, panels, &recent)?;
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
    let (ui_scale, tick_hz) = file.validate()?;
    let workspace = file.workspace.unwrap_or_default();
    Ok(Settings {
        shortcuts: file.shortcuts,
        files: file.files,
        workspace,
        language: language_from_disk(file.general.language),
        density: density_from_disk(file.appearance.density),
        theme: theme_from_disk(file.appearance.theme),
        dark_artwork_contrast: file.appearance.dark_artwork_contrast,
        accent: accent_from_disk(file.appearance.accent),
        ui_scale,
        reduce_motion: file.appearance.reduce_motion,
        show_telemetry: file.appearance.show_telemetry,
        signal_palette: signal_palette_from_disk(file.appearance.signal_palette),
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
        simulation_mode: mode_from_disk(file.simulation.mode),
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
            language: language_to_disk(s.language),
        },
        appearance: Appearance {
            density: density_to_disk(s.density),
            theme: theme_to_disk(s.theme),
            dark_artwork_contrast: s.dark_artwork_contrast,
            accent: accent_to_disk(s.accent),
            ui_scale: f64::from(s.ui_scale.percent()) / 100.0,
            reduce_motion: s.reduce_motion,
            show_telemetry: s.show_telemetry,
            signal_palette: signal_palette_to_disk(s.signal_palette),
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
            mode: mode_to_disk(s.simulation_mode),
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

//! Version-one disk representation. The UI owns the validated preference types.
use logisim_ui::{DefaultZoom, Language, Settings, SimulationMode, TickHz, UiScale};
use serde::{Deserialize, Serialize};

pub(super) const VERSION: u32 = 1;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
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
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            reduce_motion: false,
            show_telemetry: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Editor {
    show_grid: bool,
    #[serde(with = "ZoomCode")]
    default_zoom: DefaultZoom,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            show_grid: true,
            default_zoom: DefaultZoom::Fit,
        }
    }
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

pub(super) fn decode(bytes: &[u8]) -> Result<Settings, &'static str> {
    let file: File = serde_json::from_slice(bytes).map_err(|_| "Unknown or invalid settings.")?;
    if file.version != VERSION {
        return Err("Unsupported configuration version.");
    }
    let ui_scale = UiScale::ALL
        .into_iter()
        .find(|s| f64::from(s.percent()) / 100.0 == file.appearance.ui_scale)
        .ok_or("Invalid interface scale.")?;
    let tick_hz = TickHz::new(file.simulation.tick_hz).ok_or("Invalid tick frequency.")?;
    Ok(Settings {
        language: file.general.language,
        ui_scale,
        reduce_motion: file.appearance.reduce_motion,
        show_telemetry: file.appearance.show_telemetry,
        show_grid: file.editor.show_grid,
        default_zoom: file.editor.default_zoom,
        simulation_mode: file.simulation.mode,
        tick_hz,
        truth_table: file.generation.truth_table,
    })
}
pub(super) fn encode(s: Settings) -> Vec<u8> {
    let file = File {
        version: VERSION,
        general: General {
            language: s.language,
        },
        appearance: Appearance {
            ui_scale: f64::from(s.ui_scale.percent()) / 100.0,
            reduce_motion: s.reduce_motion,
            show_telemetry: s.show_telemetry,
        },
        editor: Editor {
            show_grid: s.show_grid,
            default_zoom: s.default_zoom,
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

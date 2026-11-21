pub const DEFAULT_TICK_HZ: u32 = 4;
pub const MAX_TICK_HZ: u32 = 32768;
pub const CONFIG_VERSION: u32 = 17;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WasmEnginePreference {
    #[default]
    Wasmi,
    Wasmtime,
}
pub const FONT_SIZES: std::ops::RangeInclusive<u8> = 8..=32;
pub const GATE_INPUTS: std::ops::RangeInclusive<u8> = 2..=32;
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontPreset {
    #[default]
    Dialog,
    Monospace,
    Serif,
}

impl FontPreset {
    pub fn family(self) -> &'static str {
        match self {
            Self::Dialog => "Dialog",
            Self::Monospace => "Monospace",
            Self::Serif => "Serif",
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
    pub backup_before_save: bool,
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
            backup_before_save: false,
        }
    }
}

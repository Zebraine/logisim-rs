//! Optional keyboard interaction v1, independent of simulation ABI and saved tools.
//!
//! `logisim_interaction_version() -> u32` returns 1; `logisim_interaction() -> u64`
//! returns bounded JSON metadata using the usual length/offset convention. Missing or
//! unknown versions provide no keyboard capture; malformed recognized metadata fails.
//! A poke click focuses one instance. The host sends [`InputEvent`] as JSON in the
//! declared existing Action's argument. Keys are logical, layout-aware keys (letters
//! are case independent); text/IME input and OS repeat are not key transitions.
//! Escape/Tab, window/modal/context changes release capture. ReleaseAll clears held
//! input even when a corresponding key-up cannot arrive. Command shortcuts stay with
//! the application. Events are ordered, instance-bound runtime input, never document
//! edits. Plugins must handle duplicate transitions and ReleaseAll idempotently.
use crate::{MAX_MESSAGE, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const VERSION: u32 = 1;

/// Stable wire names, deliberately independent of GUI and operating-system types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Space,
    Enter,
}
impl Key {
    pub const ALL: [Self; 32] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::W,
        Self::X,
        Self::Y,
        Self::Z,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::Space,
        Self::Enter,
    ];
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 32] = [
            "A",
            "B",
            "C",
            "D",
            "E",
            "F",
            "G",
            "H",
            "I",
            "J",
            "K",
            "L",
            "M",
            "N",
            "O",
            "P",
            "Q",
            "R",
            "S",
            "T",
            "U",
            "V",
            "W",
            "X",
            "Y",
            "Z",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Space",
            "Enter",
        ];
        NAMES[Self::ALL.iter().position(|k| *k == self).unwrap()]
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputEvent {
    Key { key: Key, pressed: bool },
    ReleaseAll {},
}
impl InputEvent {
    pub fn argument(&self) -> String {
        serde_json::to_string(self).expect("serializable input event")
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub keyboards: Vec<Keyboard>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyboard {
    pub tool: String,
    pub action: String,
    pub keys: Vec<Key>,
}
impl Interaction {
    pub fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        if self.keyboards.len() > 64
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_MESSAGE
        {
            return Err("Plugin interaction exceeds limits.".into());
        }
        let mut seen = BTreeSet::new();
        for keyboard in &self.keyboards {
            let action = manifest
                .tools
                .iter()
                .find(|t| t.id == keyboard.tool)
                .and_then(|t| t.actions.iter().find(|a| a.id == keyboard.action));
            if !seen.insert(&keyboard.tool)
                || action.is_none_or(|a| a.argument.is_none())
                || keyboard.keys.is_empty()
                || keyboard.keys.len() > Key::ALL.len()
                || keyboard.keys.iter().collect::<BTreeSet<_>>().len() != keyboard.keys.len()
            {
                return Err("Invalid plugin keyboard interaction.".into());
            }
        }
        Ok(())
    }
}

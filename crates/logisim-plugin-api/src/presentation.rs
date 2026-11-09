//! Optional UI extension, independent of simulation ABI and saved Tool snapshots.
//!
//! `logisim_ui_version() -> u32` returns 1. `logisim_ui() -> u64` returns packed
//! length/offset JSON [`Presentation`], using the same bounded response convention.
//! Missing/unknown extensions keep the base UI; malformed recognized extensions fail.
//! Translations are plugin-owned display text only, never IDs, properties or state.
//! Memory imports are host-parsed `v2.0 raw` images. After full validation the host
//! invokes the existing action once, with `argument_prefix` followed by exactly
//! 2^address_bits lower-case hexadecimal words separated by spaces. Missing words
//! become zero. File paths never enter the guest or the circuit document.
use crate::{MAX_MESSAGE, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const VERSION: u32 = 1;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
    pub imports: Vec<MemoryImport>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryImport {
    pub tool: String,
    pub action: String,
    pub label: String,
    pub address_bits: u8,
    pub data_bits: u8,
    pub argument_prefix: String,
}
/// Loaded UI metadata; not serialized into the circuit's component definitions.
#[derive(Clone, Debug)]
pub struct Library {
    pub manifest: Manifest,
    pub presentation: Presentation,
}
impl From<Manifest> for Library {
    fn from(manifest: Manifest) -> Self {
        Self {
            manifest,
            presentation: Presentation::default(),
        }
    }
}
impl Presentation {
    pub fn text<'a>(&'a self, language: &str, text: &'a str) -> &'a str {
        self.translations
            .get(language)
            .and_then(|m| m.get(text))
            .map_or(text, String::as_str)
    }
    pub fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        if self.translations.len() > 16
            || self.imports.len() > 64
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_MESSAGE
        {
            return Err("Plugin UI metadata exceeds limits.".into());
        }
        for (language, entries) in &self.translations {
            if language.is_empty()
                || language.len() > 32
                || !language
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || entries.len() > 512
                || entries.iter().any(|(a, b)| {
                    a.is_empty()
                        || a.len() > 1024
                        || b.is_empty()
                        || b.len() > 1024
                        || a.chars().chain(b.chars()).any(char::is_control)
                })
            {
                return Err("Invalid plugin translations.".into());
            }
        }
        for import in &self.imports {
            let action = manifest
                .tools
                .iter()
                .find(|t| t.id == import.tool)
                .and_then(|t| t.actions.iter().find(|a| a.id == import.action));
            if action.is_none_or(|a| a.argument.is_none())
                || !(2..=8).contains(&import.address_bits)
                || !(1..=32).contains(&import.data_bits)
                || import.label.is_empty()
                || import.label.len() > 256
                || import.label.chars().any(char::is_control)
                || import.argument_prefix.len() > 64
                || import.argument_prefix.chars().any(char::is_control)
            {
                return Err("Invalid plugin memory import.".into());
            }
            if (1usize << import.address_bits) * (usize::from(import.data_bits.div_ceil(4)) + 1)
                + import.argument_prefix.len()
                > 1024
            {
                return Err("Plugin memory import exceeds action argument limit.".into());
            }
        }
        Ok(())
    }
}

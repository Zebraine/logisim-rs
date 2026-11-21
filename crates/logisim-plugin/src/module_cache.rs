//! Opt-in storage for Wasmtime compilations. The host supplies its private cache
//! directory, independently of document paths. The OS user and administrator are trusted.
use crate::{Plugin, Result, WasmEngine};
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;

/// A miss was compiled and published. A bypass ran the ordinary compiler or could not
/// persist its result. A rebuild replaced an unusable record using the original Wasm.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    LegacyWasmi,
    Hit,
    Miss,
    Rebuilt(String),
    Bypassed(String),
}

impl Outcome {
    pub fn warning(&self) -> Option<&str> {
        match self {
            Self::Rebuilt(reason) | Self::Bypassed(reason) => Some(reason),
            Self::LegacyWasmi | Self::Hit | Self::Miss => None,
        }
    }
}

/// Compile or open using Wasmtime, then run the usual manifest and ABI admission.
/// Project loaders must still call `Plugin::check_dependency` with their saved definitions.
pub fn load(bytes: &[u8], directory: &Path) -> Result<(Plugin, Outcome)> {
    let declaration = crate::Declaration::read(bytes)?;
    if declaration.select(WasmEngine::Wasmtime)? == WasmEngine::Wasmi {
        return Ok((Plugin::load(bytes)?, Outcome::LegacyWasmi));
    }
    #[cfg(target_os = "linux")]
    let reason = match linux::prepare(directory, bytes) {
        Ok(cache) => return cache.load(bytes, declaration),
        Err(error) => error.to_string(),
    };
    #[cfg(not(target_os = "linux"))]
    let reason = format!(
        "Private Wasmtime cache is supported only on Linux ({})",
        directory.display()
    );
    Ok((
        Plugin::load_with_engine(bytes, WasmEngine::Wasmtime)?,
        Outcome::Bypassed(reason),
    ))
}

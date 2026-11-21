//! Bound saves keep their codec. Explicit legacy exports verify what the XML reader can recover.
use crate::{Document, Error, Import, Result, native};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectFormat {
    Circ,
    Lrs,
}
impl ProjectFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Circ => "circ",
            Self::Lrs => "lrs",
        }
    }
    pub fn from_extension(path: &Path) -> Option<Self> {
        [Self::Circ, Self::Lrs].into_iter().find(|f| {
            path.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(f.extension()))
        })
    }
    /// Older callers could open XML projects without a .circ suffix.
    pub fn binding(path: &Path) -> Self {
        Self::from_extension(path).unwrap_or(Self::Circ)
    }
    pub fn parse(self, bytes: &[u8]) -> Result<Import> {
        match self {
            Self::Lrs => Ok(Import {
                document: native::parse_native(bytes)?,
                partial: false,
                warnings: vec![],
            }),
            Self::Circ => crate::parse(
                std::str::from_utf8(bytes).map_err(|e| Error(format!("Invalid UTF-8: {e}")))?,
            ),
        }
    }
    pub fn serialize(self, p: &Document) -> Result<Vec<u8>> {
        match self {
            Self::Circ => crate::serialize(p).map(String::into_bytes),
            Self::Lrs => native::serialize_native(p),
        }
    }
    pub fn export(self, p: &Document) -> Result<Vec<u8>> {
        let bytes = self.serialize(p)?;
        if self == Self::Circ {
            let imported = self.parse(&bytes)?;
            if !imported.warnings.is_empty() {
                return Err(Error(format!(
                    "Legacy export would lose data: {}",
                    imported.warnings.join("\n")
                )));
            }
            native::verify_export(p, &imported.document)?;
        }
        Ok(bytes)
    }
}

fn relative(path: &Path, base: &Path) -> Option<PathBuf> {
    let path: Vec<_> = path.components().collect();
    let base: Vec<_> = base.components().collect();
    if path.first() != base.first()
        || path
            .iter()
            .chain(&base)
            .any(|p| matches!(p, Component::ParentDir))
    {
        return None;
    }
    let shared = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in shared..base.len() {
        result.push("..");
    }
    for part in &path[shared..] {
        result.push(part.as_os_str());
    }
    Some(result)
}
/// Preserve dependencies when importing a library or saving into another directory.
pub fn rebase_plugin_sources(project: &mut Document, from: &Path, to: &Path) {
    for source in &mut project.plugin_sources {
        if !source.path.is_empty() {
            let full = from.join(&source.path);
            source.path = relative(&full, to)
                .unwrap_or_else(|| full.clone())
                .to_string_lossy()
                .into_owned();
            if source.fallback.is_empty() {
                source.fallback = full.to_string_lossy().into_owned();
            }
        }
        if !source.fallback.is_empty() {
            source.fallback = from.join(&source.fallback).to_string_lossy().into_owned();
        }
    }
}

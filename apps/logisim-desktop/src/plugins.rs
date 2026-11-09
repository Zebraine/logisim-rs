//! Filesystem policy for document plugin dependencies; no component-specific knowledge.
use logisim_core::{
    Project,
    plugin::{PluginSource, api},
};
use logisim_plugin::{Plugin, Registry};
use logisim_ui::i18n::tr;
#[cfg(test)]
mod tests;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub(crate) struct Row {
    pub source: PluginSource,
    pub location: String,
    pub error: Option<String>,
    pub loaded: Option<Plugin>,
    pub found: Option<PathBuf>,
    components: Vec<api::Component>,
}
pub(crate) struct Resolution {
    pub rows: Vec<Row>,
    pub base: PathBuf,
    pub warnings: Vec<String>,
}
pub(crate) fn base(path: Option<&Path>) -> PathBuf {
    path.and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
fn absolute(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
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
pub(crate) fn located(plugin: &str, version: &str, path: &Path, base: &Path) -> PluginSource {
    let full = absolute(path, base);
    PluginSource {
        plugin: plugin.into(),
        version: version.into(),
        path: full.to_string_lossy().into_owned(),
        fallback: full.to_string_lossy().into_owned(),
    }
}
/// Keep in-memory references absolute so Undo and library imports have no hidden base.
pub(crate) fn absolutize(project: &mut Project, base: &Path) {
    for source in &mut project.plugin_sources {
        if Path::new(&source.path).is_absolute()
            && !source.fallback.is_empty()
            && Path::new(&source.fallback).is_relative()
        {
            std::mem::swap(&mut source.path, &mut source.fallback);
        }
        for value in [&mut source.path, &mut source.fallback] {
            if !value.is_empty() {
                let full = absolute(Path::new(value), base);
                *value = full
                    .canonicalize()
                    .unwrap_or(full)
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }
}
/// Preserve dependencies when importing a library or saving into another directory.
pub(crate) fn rebase(project: &mut Project, from: &Path, to: &Path) {
    for source in &mut project.plugin_sources {
        if !source.path.is_empty() {
            let full = absolute(Path::new(&source.path), from);
            source.path = relative(&full, to)
                .unwrap_or_else(|| full.clone())
                .to_string_lossy()
                .into_owned();
            if source.fallback.is_empty() {
                source.fallback = full.to_string_lossy().into_owned();
            }
        }
        if !source.fallback.is_empty() {
            source.fallback = absolute(Path::new(&source.fallback), from)
                .to_string_lossy()
                .into_owned();
        }
    }
}
impl Resolution {
    pub fn new(project: &Project, base: PathBuf, warnings: Vec<String>) -> Self {
        let mut rows = BTreeMap::new();
        for source in &project.plugin_sources {
            rows.insert(
                (source.plugin.clone(), source.version.clone()),
                Row {
                    source: source.clone(),
                    location: String::new(),
                    error: None,
                    loaded: None,
                    found: None,
                    components: vec![],
                },
            );
        }
        for c in project
            .circuits
            .iter()
            .flat_map(|d| &d.circuit.components)
            .filter_map(|c| c.plugin.as_ref())
        {
            rows.entry((c.plugin.clone(), c.version.clone()))
                .or_insert_with(|| Row {
                    source: PluginSource {
                        plugin: c.plugin.clone(),
                        version: c.version.clone(),
                        path: String::new(),
                        fallback: String::new(),
                    },
                    location: String::new(),
                    error: None,
                    loaded: None,
                    found: None,
                    components: vec![],
                })
                .components
                .push(c.clone());
        }
        let mut result = Self {
            rows: rows.into_values().collect(),
            base,
            warnings,
        };
        for i in 0..result.rows.len() {
            result.check(i);
        }
        result
    }
    pub fn missing(&self) -> bool {
        self.rows.iter().any(|r| r.loaded.is_none())
    }
    pub fn check(&mut self, index: usize) {
        let row = &mut self.rows[index];
        row.loaded = None;
        row.found = None;
        let mut candidates = if row.location.is_empty() {
            vec![row.source.path.as_str(), row.source.fallback.as_str()]
        } else {
            vec![row.location.as_str()]
        };
        candidates.retain(|s| !s.is_empty());
        candidates.sort_by_key(|p| Path::new(p).is_absolute());
        let mut errors = Vec::new();
        for candidate in candidates {
            let path = absolute(Path::new(candidate), &self.base);
            match load(
                &path,
                &row.source.plugin,
                &row.source.version,
                &row.components,
            ) {
                Ok(plugin) => {
                    row.loaded = Some(plugin);
                    row.found = Some(path.canonicalize().unwrap_or(path));
                    row.error = None;
                    return;
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        row.error = Some(if errors.is_empty() {
            tr("No plugin path is recorded.").into()
        } else {
            errors.join("\n")
        });
    }
    pub fn registry(&self) -> Registry {
        let mut registry = Registry::default();
        for row in &self.rows {
            if let Some(plugin) = &row.loaded {
                registry
                    .insert(plugin.clone())
                    .expect("unique dependency identities");
            } else {
                registry.unavailable(
                    row.source.plugin.clone(),
                    row.source.version.clone(),
                    row.error.clone().unwrap_or_default(),
                );
            }
        }
        registry
    }
    pub fn sources(&self, old: &[PluginSource]) -> Vec<PluginSource> {
        let mut sources = old.to_vec();
        for row in &self.rows {
            if !row.location.is_empty() && row.loaded.is_some() {
                let updated = located(
                    &row.source.plugin,
                    &row.source.version,
                    row.found.as_ref().unwrap(),
                    &self.base,
                );
                if let Some(source) = sources
                    .iter_mut()
                    .find(|s| s.plugin == updated.plugin && s.version == updated.version)
                {
                    *source = updated;
                } else {
                    sources.push(updated);
                }
            }
        }
        sources
    }
}
fn load(
    path: &Path,
    id: &str,
    version: &str,
    components: &[api::Component],
) -> Result<Plugin, String> {
    let bytes = crate::storage::read_file(path, tr("Choose a regular .wasm file."))?;
    let plugin = Plugin::load(&bytes)?;
    if plugin.manifest().id != id || plugin.manifest().version != version {
        return Err(tr("Plugin identity or version does not match the circuit.").into());
    }
    for c in components {
        plugin.check_component(c)?;
    }
    Ok(plugin)
}

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
    pub from_directory: bool,
    components: Vec<api::Component>,
}
pub(crate) struct Resolution {
    pub rows: Vec<Row>,
    pub base: PathBuf,
    fallback_directory: Option<PathBuf>,
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
impl Row {
    fn candidates(&self, base: &Path, directory: Option<&Path>) -> Vec<(PathBuf, bool)> {
        // Never mask a bad location explicitly entered in the repair dialog.
        if !self.location.is_empty() {
            return vec![(absolute(Path::new(&self.location), base), false)];
        }
        let mut recorded = [
            Path::new(&self.source.path),
            Path::new(&self.source.fallback),
        ];
        recorded.sort_by_key(|p| p.is_absolute());
        let mut paths: Vec<_> = recorded
            .into_iter()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| (absolute(p, base), false))
            .collect();
        paths.dedup();
        if let Some(dir) = directory {
            let name = [&self.source.path, &self.source.fallback]
                .into_iter()
                .filter_map(|p| Path::new(p).file_name())
                .find(|name| {
                    Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"))
                });
            if let Some(name) = name {
                let path = dir.join(name);
                if !paths.iter().any(|(recorded, _)| *recorded == path) {
                    paths.push((path, true));
                }
            }
        }
        paths
    }
}

impl Resolution {
    pub fn new(
        project: &Project,
        base: PathBuf,
        fallback_directory: Option<&Path>,
        warnings: Vec<String>,
    ) -> Self {
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
                    from_directory: false,
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
                    from_directory: false,
                    components: vec![],
                })
                .components
                .push(c.clone());
        }
        let mut result = Self {
            rows: rows.into_values().collect(),
            base,
            fallback_directory: fallback_directory.map(Path::to_owned),
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
        row.from_directory = false;
        let candidates = row.candidates(&self.base, self.fallback_directory.as_deref());
        let mut errors = Vec::new();
        for (path, from_directory) in candidates {
            match load(
                &path,
                &row.source.plugin,
                &row.source.version,
                &row.components,
            ) {
                Ok(plugin) => {
                    row.from_directory = from_directory;
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
            if (row.from_directory || !row.location.is_empty()) && row.loaded.is_some() {
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
    Plugin::load_dependency(&bytes, id, version, components).map_err(|e| tr(&e).to_owned())
}

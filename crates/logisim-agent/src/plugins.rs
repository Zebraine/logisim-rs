//! Project-scoped module resolution and manifest projections for the headless transports.
use crate::{Error, Result, io};
use logisim_core::{
    Project,
    plugin::{PluginSource, api},
};
use logisim_plugin::{MAX_MODULE, Plugin, Registry};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(crate) fn unavailable(message: impl ToString) -> Error {
    Error::new("plugin_unavailable", message)
}
pub(crate) fn no_source() -> Error {
    unavailable("No plugin path is recorded.")
}

pub(crate) fn resolve<'a>(
    source: &PluginSource,
    base: &Path,
    components: impl Iterator<Item = &'a api::Component> + Clone,
) -> Result<(Plugin, PathBuf)> {
    let mut paths: Vec<_> = [&source.path, &source.fallback]
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(Path::new)
        .collect();
    paths.sort_by_key(|p| p.is_absolute());
    paths.dedup();
    let mut errors = Vec::new();
    for path in paths {
        let path = base.join(path);
        let loaded = io::read_limited(&path, MAX_MODULE)
            .map_err(|e| e.message)
            .and_then(|bytes| {
                Plugin::load_dependency(&bytes, &source.plugin, &source.version, components.clone())
            });
        match loaded {
            Ok(plugin) => return Ok((plugin, path)),
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    Err(if errors.is_empty() {
        no_source()
    } else {
        unavailable(errors.join("\n"))
    })
}

pub(crate) fn tool_info(tool: &api::Tool) -> Value {
    json!({"id":tool.id,"name":tool.name,"ports":tool.ports,"actions":tool.actions})
}

pub(crate) fn wire_schema(tool: &api::Tool) -> Value {
    Value::Object(tool.properties.iter().map(|p| {
        let mut schema = match &p.kind {
            api::PropertyKind::Boolean => json!({"type":"string","enum":["false","true"]}),
            api::PropertyKind::Integer {min,max} => json!({"type":"string","pattern":"^[+-]?[0-9]+$","maxLength":1024,"x-minimum":min,"x-maximum":max}),
            api::PropertyKind::Choice {values} => json!({"type":"string","enum":values}),
        };
        schema["default"] = json!(p.default);
        (p.id.clone(),schema)
    }).collect())
}

fn tool_metadata(tool: &api::Tool) -> Value {
    let mut metadata = tool_info(tool);
    let mut properties = Map::new();
    for p in &tool.properties {
        let mut descriptor = match &p.kind {
            api::PropertyKind::Boolean => json!({"type":"boolean"}),
            api::PropertyKind::Integer { min, max } => {
                json!({"type":"integer","minimum":min,"maximum":max})
            }
            api::PropertyKind::Choice { values } => json!({"type":"choice","values":values}),
        };
        descriptor["label"] = json!(p.label);
        descriptor["default"] = json!(p.default);
        properties.insert(p.id.clone(), descriptor);
    }
    metadata["properties"] = properties.into();
    metadata["property_schema"] = wire_schema(tool);
    metadata
}
pub(crate) fn catalog(path: &Path) -> Result<Value> {
    let bytes = io::read_limited(path, MAX_MODULE)?;
    let plugin = Plugin::load(&bytes).map_err(unavailable)?;
    let manifest = plugin.manifest();
    Ok(
        json!({"id":manifest.id,"version":manifest.version,"name":manifest.name,
        "tools":manifest.tools.iter().map(tool_metadata).collect::<Vec<_>>()}),
    )
}

#[derive(Default)]
struct Dependency<'a> {
    source: Option<&'a PluginSource>,
    components: Vec<&'a api::Component>,
}

pub(crate) fn load_dependencies(project: &Project, base: &Path) -> (Registry, Vec<Value>) {
    let mut dependencies = BTreeMap::new();
    for source in &project.plugin_sources {
        dependencies.insert(
            (source.plugin.as_str(), source.version.as_str()),
            Dependency {
                source: Some(source),
                components: Vec::new(),
            },
        );
    }
    for component in project
        .circuits
        .iter()
        .flat_map(|d| &d.circuit.components)
        .filter_map(|c| c.plugin.as_ref())
    {
        dependencies
            .entry((&component.plugin, &component.version))
            .or_default()
            .components
            .push(component);
    }
    let mut registry = Registry::default();
    let mut reports = Vec::new();
    for ((id, version), dep) in dependencies {
        match dep
            .source
            .ok_or_else(no_source)
            .and_then(|source| resolve(source, base, dep.components.iter().copied()))
        {
            Ok((plugin, path)) => {
                registry
                    .insert(plugin)
                    .expect("one entry per plugin identity");
                reports.push(json!({"id":id,"version":version,"status":"loaded","path":path}));
            }
            Err(error) => {
                let error = error.message;
                registry.unavailable(id.into(), version.into(), error.clone());
                reports.push(json!({"id":id,"version":version,"status":"missing","error":error}));
            }
        }
    }
    (registry, reports)
}

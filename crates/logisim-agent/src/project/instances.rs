//! Plugin edits bind to the module named by this project and only change document data.
use super::{
    edit, patch,
    properties::DIRECTION,
    schema::{self, Rule},
};
use crate::{Error, Result, paths::Root, plugins};
use logisim_core::{
    Component, Id, Project,
    plugin::{PluginSource, api},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, path::Path};

fn recorded<'a>(project: &'a Project, plugin: &str, version: &str) -> Option<&'a PluginSource> {
    project
        .plugin_sources
        .iter()
        .find(|s| s.plugin == plugin && s.version == version)
}

fn read_manifest(project: &Project, source: &PluginSource, base: &Path) -> Result<api::Manifest> {
    let instances = project
        .circuits
        .iter()
        .flat_map(|d| &d.circuit.components)
        .filter_map(|c| c.plugin.as_ref())
        .filter(|c| c.plugin == source.plugin && c.version == source.version);
    plugins::resolve(source, base, instances).map(|(p, _)| p.manifest().clone())
}
fn tool<'a>(manifest: &'a api::Manifest, id: &str) -> Result<&'a api::Tool> {
    manifest.tools.iter().find(|t| t.id == id).ok_or_else(|| {
        plugins::unavailable(format!("Unknown tool '{id}' in plugin '{}'.", manifest.id))
    })
}

/// One projection or patch transaction. Snapshots cache missing modules as well as successes.
pub(super) struct Manifests<'a> {
    base: &'a Path,
    entries: BTreeMap<(String, String), std::result::Result<api::Manifest, String>>,
}
impl<'a> Manifests<'a> {
    pub(super) fn new(project_path: &'a Path) -> Self {
        Self {
            base: project_path.parent().expect("absolute project file"),
            entries: BTreeMap::new(),
        }
    }

    fn manifest(&mut self, project: &Project, source: &PluginSource) -> Result<&api::Manifest> {
        let entry = self
            .entries
            .entry((source.plugin.clone(), source.version.clone()))
            .or_insert_with(|| read_manifest(project, source, self.base).map_err(|e| e.message));
        entry.as_ref().map_err(plugins::unavailable)
    }

    pub(super) fn describe(
        &mut self,
        project: &Project,
        component: &api::Component,
        data: &mut Value,
    ) {
        let source = recorded(project, &component.plugin, &component.version);
        data["source"] = json!(source);
        let tool = source
            .ok_or_else(plugins::no_source)
            .and_then(|s| tool(self.manifest(project, s)?, &component.tool.id));
        match tool {
            Ok(tool) => {
                data["property_schema"] = plugins::wire_schema(tool);
                data["patchable_properties"] =
                    tool.properties.iter().map(|p| json!(p.id)).collect();
                data["property_types"] = Value::Object(
                    tool.properties
                        .iter()
                        .map(|p| (p.id.clone(), json!("string")))
                        .collect(),
                );
                data["tool"] = plugins::tool_info(tool);
            }
            Err(e) => data["plugin_error"] = json!(e),
        }
    }
}

const DOCUMENT_FIELDS: [(&str, Rule); 4] = [
    ("label", Rule::Label),
    ("facing", DIRECTION),
    ("font", Rule::Font),
    ("color", Rule::Rgba { opaque: true }),
];

#[derive(Debug, Deserialize, Serialize)]
pub struct NewPlugin {
    pub(super) circuit: Id,
    position: [i32; 2],
    tool: String,
    #[serde(
        default,
        deserialize_with = "crate::supplied",
        skip_serializing_if = "Option::is_none"
    )]
    plugin: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::supplied",
        skip_serializing_if = "Option::is_none"
    )]
    version: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::supplied",
        skip_serializing_if = "Option::is_none"
    )]
    source: Option<PluginSource>,
    #[serde(flatten)]
    fields: InstanceFields,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PluginUpdate {
    pub(super) circuit: Option<Id>,
    pub(super) id: Id,
    #[serde(flatten)]
    fields: InstanceFields,
}

#[derive(Debug, Deserialize, Serialize)]
struct InstanceFields {
    #[serde(default)]
    properties: api::Properties,
    // The shared document schema limits this map to the four named fields.
    #[serde(flatten)]
    document: Map<String, Value>,
}

impl InstanceFields {
    fn apply(&self, component: &mut Component) -> Result<()> {
        let instance = component
            .plugin
            .as_mut()
            .expect("constructed plugin instance");
        instance.properties.extend(self.properties.clone());
        instance
            .tool
            .validate_properties(&instance.properties)
            .map_err(|e| Error::new("invalid_property", e))?;
        for (key, value) in &self.document {
            let (_, rule) = DOCUMENT_FIELDS
                .iter()
                .find(|(k, _)| k == key)
                .ok_or_else(|| Error::invalid(format!("Unknown plugin edit field: {key}")))?;
            rule.validate(key, value)?;
            edit::apply_document_field(component, key, value)?;
        }
        Ok(())
    }
}

impl NewPlugin {
    pub(super) fn build(
        &self,
        project: &mut Project,
        manifests: &mut Manifests,
        root: Option<&Root>,
    ) -> Result<Component> {
        let at = patch::point(self.position)?;
        let source = if let Some(input) = &self.source {
            input.validate().map_err(Error::invalid)?;
            if self.plugin.as_ref().is_some_and(|id| id != &input.plugin)
                || self.version.as_ref().is_some_and(|v| v != &input.version)
            {
                return Err(Error::invalid("The plugin and version must match source."));
            }
            if let Some(root) = root {
                for name in [&input.path, &input.fallback]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                {
                    root.check_candidate(&manifests.base.join(name))?;
                }
            }
            if let Some(existing) = recorded(project, &input.plugin, &input.version) {
                if existing != input {
                    // Keep the recorded source for every instance. Accept a second location
                    // only when its requested tool agrees with the module used on reopen.
                    let explicit = read_manifest(project, input, manifests.base)?;
                    if tool(&explicit, &self.tool)?
                        != tool(manifests.manifest(project, existing)?, &self.tool)?
                    {
                        return Err(plugins::unavailable(
                            "The supplied and recorded tool definitions differ.",
                        ));
                    }
                }
                existing
            } else {
                project.plugin_sources.push(input.clone());
                input
            }
        } else {
            let (Some(id), Some(version)) = (&self.plugin, &self.version) else {
                return Err(Error::invalid("Provide source or both plugin and version."));
            };
            recorded(project, id, version).ok_or_else(plugins::no_source)?
        };
        let manifest = manifests.manifest(project, source)?;
        let tool = tool(manifest, &self.tool)?;
        let definition = api::Component {
            plugin: manifest.id.clone(),
            version: manifest.version.clone(),
            properties: tool
                .properties
                .iter()
                .map(|p| (p.id.clone(), p.default.clone()))
                .collect(),
            tool: tool.clone(),
        };
        let mut c = Component::from_plugin(definition, at);
        self.fields.apply(&mut c)?;
        Ok(c)
    }
}

impl PluginUpdate {
    pub(super) fn build(
        &self,
        old: &Component,
        project: &Project,
        manifests: &mut Manifests,
    ) -> Result<Component> {
        let definition = old
            .plugin
            .as_ref()
            .ok_or_else(|| Error::invalid("update_plugin requires a plugin component."))?;
        let source = recorded(project, &definition.plugin, &definition.version)
            .ok_or_else(plugins::no_source)?;
        // Resolution validates saved tool definitions and properties against this module.
        manifests.manifest(project, source)?;
        let mut c = Component::from_plugin(definition.clone(), old.position);
        c.id = old.id;
        c.font.clone_from(&old.font);
        c.label.clone_from(&old.label);
        c.color = old.color;
        c.facing = old.facing;
        self.fields.apply(&mut c)?;
        Ok(c)
    }
}

impl patch::ProjectOp {
    pub fn plugin_document_schema() -> Value {
        schema::object(DOCUMENT_FIELDS)
    }
}

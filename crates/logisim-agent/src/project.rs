//! Project sessions and projections of their circuit-scoped item identities.
mod builtins;
pub(crate) mod catalog;
mod edit;
mod ids;
mod instances;
mod memory;
pub(crate) mod patch;
mod pla;
mod properties;
pub(crate) mod reference;
mod save;
mod schema;
use crate::{Error, Result, digest, io};
use logisim_core::{AgentIdKind, Component, Interface, Kind, Point, Project};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) struct Session {
    pub project: Project,
    pub path: PathBuf,
    sha256: String,
    warnings: Vec<String>,
    partial: bool,
    issued: ids::Issued,
    pub simulation: Option<crate::simulation::Runtime>,
}

impl Session {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize().map_err(io::file_error)?;
        let bytes = io::read(&path)?;
        let xml = std::str::from_utf8(&bytes).map_err(|e| Error::new("invalid_project", e))?;
        let mut imported =
            logisim_file::parse(xml).map_err(|e| Error::new("invalid_project", e))?;
        let issued = ids::Issued::open(&mut imported.document)?;
        Ok(Self {
            issued,
            partial: imported.partial,
            project: imported.document,
            path,
            sha256: digest::sha256(&bytes),
            warnings: imported.warnings,
            simulation: None,
        })
    }
    fn check_editable(&self, expected: Option<&str>) -> Result<()> {
        if self.partial {
            let mut e = Error::new(
                "partial_project",
                "This import reported omissions. Patching and saving are disabled.",
            );
            e.details["warnings"] = json!(self.warnings);
            return Err(e);
        }
        if let Some(hash) = expected {
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(Error::invalid("Expected 64 hexadecimal SHA-256 digits."));
            }
            if !self.sha256.eq_ignore_ascii_case(hash) {
                return Err(Error::new(
                    "conflict",
                    "Expected SHA-256 differs from the session source.",
                ));
            }
        }
        Ok(())
    }
    pub fn summary(&self, id: &str) -> Value {
        json!({"session_id":id,"path":self.path,"source_sha256":self.sha256,"schema":1,"main":self.project.main,"partial":self.partial,"warnings":self.warnings})
    }
    pub fn snapshot(&self, id: &str) -> Result<Value> {
        let mut result = self.summary(id);
        let mut circuits = Vec::with_capacity(self.project.circuits.len());
        let mut manifests = instances::Manifests::new(&self.path);
        for def in &self.project.circuits {
            let ids = &def.circuit.agent_ids;
            let components: Vec<_> = def
                .circuit
                .components
                .iter()
                .map(|c| {
                    let mut data = component(c)?;
                    data["agent_id"] = AgentIdKind::Component.encode(ids.items[&c.id]).into();
                    if let Some(plugin) = &c.plugin {
                        manifests.describe(&self.project, plugin, &mut data);
                    }
                    Ok(data)
                })
                .collect::<Result<_>>()?;
            let wires: Vec<_> = def
                .circuit
                .wires
                .iter()
                .map(|wire| {
                    json!({
                        "id": wire.id, "agent_id":AgentIdKind::Wire.encode(ids.items[&wire.id]), "from": point(wire.a), "to": point(wire.b)
                    })
                })
                .collect();
            circuits.push(json!({
                "id": def.id, "agent_id":AgentIdKind::Circuit.encode(ids.circuit.expect("assigned by the session")), "name": def.name, "main": def.id == self.project.main,
                "components": components, "wires": wires, "interface_ports": interface_ports(&def.interface())
            }));
        }
        result["circuits"] = circuits.into();
        result["plugin_sources"] = json!(self.project.plugin_sources);
        Ok(result)
    }
}

// Custom appearance ports, for example, bind pins by coordinates in .circ, even though
// they have independent IDs in memory. Never commit a result that the reader would omit.
fn saveable(project: &Project) -> Result<String> {
    let xml = logisim_file::serialize(project).map_err(|e| Error::new("invalid_project", e))?;
    let reopened = logisim_file::parse(&xml).map_err(|e| Error::new("invalid_project", e))?;
    if !reopened.warnings.is_empty() {
        let mut error = Error::new(
            "invalid_project",
            "Edited project would reopen with import warnings.",
        );
        error.details["warnings"] = json!(reopened.warnings);
        return Err(error);
    }
    Ok(xml)
}

fn point(p: Point) -> [i32; 2] {
    [p.x, p.y]
}
fn interface_ports(interface: &Interface) -> Value {
    interface.ports.iter().map(|port| {
        json!({"pin":port.pin, "label":port.label, "bits":port.bits, "output":port.output, "offset":point(port.offset)})
    }).collect()
}
fn component(c: &Component) -> Result<Value> {
    let mut data = json!({"id":c.id,"kind":c.kind.name(),"position":point(c.position),"facing":c.facing.name(),"bits":c.bits});
    if !matches!(c.kind, Kind::Joystick | Kind::Keyboard | Kind::Tty) {
        data["label"] = json!(c.label);
        data["font"] = json!({"family":c.font.family,"size":c.font.size,"bold":c.font.bold,"italic":c.font.italic});
        data["color"] = json!([c.color[0], c.color[1], c.color[2], c.color_alpha]);
    }
    let ports: Vec<_> = c
        .ports()
        .into_iter()
        .enumerate()
        .map(|(index, position)| {
            json!({
                "index": index, "position": point(position), "bits": c.port_width(index)
            })
        })
        .collect();
    data["ports"] = ports.into();
    properties::describe(c, &mut data);
    if c.kind == Kind::Rom {
        data["contents"] = memory::snapshot(c)?;
    }
    if let Some(plugin) = &c.plugin {
        data["plugin"] = json!({"id":plugin.plugin,"version":plugin.version,"tool":plugin.tool.id,"name":plugin.tool.name,"properties":plugin.properties});
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::{Gate, Kind, Parity, ParityOutput, PinPull, Radix};

    fn component(c: &Component) -> Value {
        super::component(c).unwrap()
    }

    #[test]
    fn plugin_catalog_entry_requires_a_module_for_dynamic_fields() {
        let kind = Kind::Plugin;
        let catalog = catalog::catalog();
        let snapshot = component(&Component::new(kind, Point::ZERO));
        assert_eq!(snapshot["patchable_properties"], json!([]));
        assert_eq!(snapshot["property_types"], json!({}));
        let entry = catalog["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|k| k["name"] == kind.name())
            .unwrap();
        assert_eq!(entry["addable"], false);
        assert_eq!(entry["editable"], true);
        assert_eq!(entry["requires_plugin_source"], true);
        assert_eq!(entry["properties"], json!({}));
        assert_eq!(snapshot["property_schema"], entry["properties"]);
        assert_eq!(
            edit::require_supported(kind).unwrap_err().code,
            "unsupported_component"
        );
    }

    #[test]
    fn snapshot_reads_the_component_model_and_identifies_writable_fields() {
        let mut pin = Component::new(Kind::Pin, Point::new(30, 70));
        pin.pin.three_state = true;
        pin.pin.pull = PinPull::Down;
        pin.bits = 8;
        let p = component(&pin);
        assert_eq!(p["three_state"], true);
        assert_eq!(p["pull"], "down");
        assert_eq!(
            p["ports"][0],
            json!({"index":0,"bits":8,"position":[30,70]})
        );
        let mut gate = Component::new(Kind::Gate(Gate::Xor), Point::ZERO);
        gate.gate_options.negated = 2;
        gate.gate_options.xor_odd = true;
        assert_eq!(component(&gate)["xor_odd"], true);
        assert_eq!(component(&gate)["negation_mask"], 2);
        let mut parity = Component::new(Kind::Parity(Parity::Odd), Point::ZERO);
        parity.parity.output = ParityOutput::FloatingOne;
        assert_eq!(component(&parity)["output"], "floating/1");
        let mut probe = Component::new(Kind::Probe, Point::ZERO);
        probe.radix = Radix::Hex;
        assert_eq!(component(&probe)["radix"], "hex");
        assert!(component(&probe)["ports"][0]["bits"].is_null());
        let splitter = Component::new(Kind::Splitter, Point::ZERO);
        assert_eq!(component(&splitter)["mapping"], json!([0, 1, 2, 3]));
        assert_eq!(
            component(&Component::new(Kind::Plugin, Point::ZERO))["patchable_properties"],
            json!([])
        );
    }
}

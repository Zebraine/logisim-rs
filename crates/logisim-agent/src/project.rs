//! Read-only projection of the imported document. IDs belong to this parsed project.
use crate::{Error, Result, digest, io};
use logisim_core::{Component, Kind, Point, Project};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) struct Session {
    pub project: Project,
    pub path: PathBuf,
    sha256: String,
    warnings: Vec<String>,
    pub simulation: Option<Value>,
}

impl Session {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize().map_err(io::file_error)?;
        let bytes = io::read(&path)?;
        let xml = std::str::from_utf8(&bytes).map_err(|e| Error::new("invalid_project", e))?;
        let imported = logisim_file::parse(xml).map_err(|e| Error::new("invalid_project", e))?;
        Ok(Self {
            project: imported.document,
            path,
            sha256: digest::sha256(&bytes),
            warnings: imported.warnings,
            simulation: None,
        })
    }
    pub fn summary(&self, id: &str) -> Value {
        json!({"session_id":id,"path":self.path,"source_sha256":self.sha256,"schema":1,"main":self.project.main,"partial":!self.warnings.is_empty(),"warnings":self.warnings})
    }
    pub fn snapshot(&self, id: &str) -> Value {
        let mut result = self.summary(id);
        let circuits:Vec<_>=self.project.circuits.iter().map(|def|{
            let components:Vec<_>=def.circuit.components.iter().map(component).collect();
            let wires:Vec<_>=def.circuit.wires.iter().map(|w|json!({"id":w.id,"from":point(w.a),"to":point(w.b)})).collect();
            let ports:Vec<_>=def.interface().ports.iter().map(|p|json!({"pin":p.pin,"label":p.label,"bits":p.bits,"output":p.output,"offset":point(p.offset)})).collect();
            json!({"id":def.id,"name":def.name,"main":def.id==self.project.main,"components":components,"wires":wires,"interface_ports":ports})
        }).collect();
        result["circuits"] = circuits.into();
        result["plugin_sources"] = json!(self.project.plugin_sources);
        result
    }
}

fn point(p: Point) -> [i32; 2] {
    [p.x, p.y]
}
fn component(c: &Component) -> Value {
    let mut data = json!({"id":c.id,"kind":c.kind.name(),"position":point(c.position),"facing":c.facing.name(),"bits":c.bits,
        "label":c.label,"font":{"family":c.font.family,"size":c.font.size,"bold":c.font.bold,"italic":c.font.italic},
        "color":[c.color[0],c.color[1],c.color[2],c.color_alpha]});
    match c.kind {
        Kind::Pin => data["output"] = c.output.into(),
        Kind::Constant => data["constant"] = c.constant.into(),
        Kind::Gate(_) | Kind::Parity(_) => data["inputs"] = c.input_count().into(),
        Kind::Subcircuit(id) => data["circuit_id"] = id.into(),
        _ => {}
    }
    if let Some(plugin) = &c.plugin {
        data["plugin"] = json!({"id":plugin.plugin,"version":plugin.version,"tool":plugin.tool.id,"name":plugin.tool.name,"properties":plugin.properties});
    }
    data
}
